//! Derive a typed repository and SQL schema from a Rust struct.
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Data, DeriveInput, Fields, GenericArgument, LitBool, LitInt, LitStr,
    PathArguments, Type,
};

#[proc_macro_derive(FerrumModel, attributes(ferrum, field))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    match expand(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = input.ident;
    let mut table: Option<String> = None;
    let mut api: Option<String> = None;
    let mut anonymous = false;
    for attr in &input.attrs {
        if !attr.path().is_ident("ferrum") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("table") {
                table = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("api") {
                api = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("allow_anonymous") {
                anonymous = meta.value()?.parse::<LitBool>()?.value;
            } else {
                return Err(meta.error("unknown ferrum setting"));
            }
            Ok(())
        })?;
    }
    let table = table
        .ok_or_else(|| syn::Error::new_spanned(&name, "missing #[ferrum(table = \"...\")]"))?;
    let api = api.unwrap_or_else(|| format!("/{table}"));
    if !valid_ident(&table)
        || !api.starts_with('/')
        || api == "/"
        || api.contains('{')
        || api.contains('*')
        || api.contains('?')
        || api.split('/').skip(1).any(|p| !valid_ident(p))
    {
        return Err(syn::Error::new_spanned(
            &name,
            "table must be a SQL identifier; api must be a static path like /products",
        ));
    }
    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(named) => named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &name,
                    "FerrumModel needs named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &name,
                "FerrumModel only supports structs",
            ))
        }
    };
    let mut column_specs = Vec::new();
    let mut dto_fields = Vec::new();
    let mut values = Vec::new();
    let mut checks = Vec::new();
    let mut update_fields = Vec::new();
    let mut update_bindings = Vec::new();
    let mut seen_id = false;
    for field in fields {
        let ident = field.ident.unwrap();
        let field_name = ident.to_string();
        if !valid_ident(&field_name) {
            return Err(syn::Error::new_spanned(ident, "invalid column name"));
        }
        let (type_name, nullable, value_ty) = field_type(&field.ty)?;
        let column_kind = match type_name.as_str() {
            "Uuid" => quote! { ::ferrum::core::ColumnKind::Uuid },
            "String" => quote! { ::ferrum::core::ColumnKind::Text },
            "i64" => quote! { ::ferrum::core::ColumnKind::BigInt },
            "bool" => quote! { ::ferrum::core::ColumnKind::Boolean },
            _ => {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "supported types are Uuid, String, i64, bool and Option of those except a nullable id",
                ))
            }
        };
        let mut primary = false;
        let mut auto = false;
        let mut required = false;
        let mut readonly = false;
        let mut min_length: Option<usize> = None;
        let mut max_length: Option<usize> = None;
        let mut min: Option<i64> = None;
        let mut default_bool: Option<bool> = None;
        for attr in &field.attrs {
            if !attr.path().is_ident("field") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("primary_key") {
                    primary = true;
                } else if meta.path.is_ident("auto") {
                    auto = true;
                } else if meta.path.is_ident("required") {
                    required = true;
                } else if meta.path.is_ident("readonly") {
                    readonly = true;
                } else if meta.path.is_ident("min_length") {
                    min_length = Some(meta.value()?.parse::<LitInt>()?.base10_parse()?);
                } else if meta.path.is_ident("max_length") {
                    max_length = Some(meta.value()?.parse::<LitInt>()?.base10_parse()?);
                } else if meta.path.is_ident("min") {
                    min = Some(meta.value()?.parse::<LitInt>()?.base10_parse()?);
                } else if meta.path.is_ident("default") {
                    default_bool = Some(meta.value()?.parse::<LitBool>()?.value);
                } else {
                    return Err(meta.error("unsupported field attribute"));
                }
                Ok(())
            })?;
        }
        if primary || auto {
            if field_name != "id" || type_name != "Uuid" || !primary || !auto || seen_id {
                return Err(syn::Error::new_spanned(
                    &ident,
                    "v0.1 requires #[field(primary_key, auto)] on id: Uuid",
                ));
            }
            if nullable {
                return Err(syn::Error::new_spanned(&ident, "id cannot be nullable"));
            }
            seen_id = true;
        } else if field_name == "id" {
            return Err(syn::Error::new_spanned(
                &ident,
                "id must use #[field(primary_key, auto)]",
            ));
        }
        if (min_length.is_some() || max_length.is_some()) && type_name != "String" {
            return Err(syn::Error::new_spanned(
                &ident,
                "length constraints require String",
            ));
        }
        if min.is_some() && type_name != "i64" {
            return Err(syn::Error::new_spanned(&ident, "min requires i64"));
        }
        if required && nullable {
            return Err(syn::Error::new_spanned(
                &ident,
                "required cannot be combined with a nullable field",
            ));
        }
        if required && type_name != "String" {
            return Err(syn::Error::new_spanned(
                &ident,
                "required is supported only for String",
            ));
        }
        if default_bool.is_some() && nullable {
            return Err(syn::Error::new_spanned(
                &ident,
                "default cannot be combined with a nullable field",
            ));
        }
        if default_bool.is_some() && type_name != "bool" {
            return Err(syn::Error::new_spanned(
                &ident,
                "default currently supports only bool",
            ));
        }
        if let (Some(lo), Some(hi)) = (min_length, max_length) {
            if lo > hi {
                return Err(syn::Error::new_spanned(
                    &ident,
                    "min_length must not exceed max_length",
                ));
            }
        }
        let default_token = match default_bool {
            Some(value) => quote! { ::core::option::Option::Some(#value) },
            None => quote! { ::core::option::Option::None },
        };
        column_specs.push(quote! {
            ::ferrum::core::ColumnSpec {
                name: #field_name,
                kind: #column_kind,
                nullable: #nullable,
                primary_key: #primary,
                default_bool: #default_token,
            }
        });
        let null_sql = null_value(&type_name);
        if primary {
            values.push(quote! { ::ferrum::sea_orm::Value::from(::ferrum::uuid::Uuid::new_v4()) });
            continue;
        }
        if let Some(default) = default_bool {
            dto_fields.push(quote! {
                #[serde(default)]
                pub #ident: ::core::option::Option<#value_ty>
            });
            values.push(
                quote! { ::ferrum::sea_orm::Value::from(payload.#ident.unwrap_or(#default)) },
            );
        } else if nullable {
            dto_fields.push(quote! {
                #[serde(default)]
                pub #ident: ::core::option::Option<#value_ty>
            });
            values.push(quote! {
                match payload.#ident {
                    ::core::option::Option::Some(value) => ::ferrum::sea_orm::Value::from(value),
                    ::core::option::Option::None => #null_sql,
                }
            });
        } else {
            dto_fields.push(quote! { pub #ident: #value_ty });
            values.push(quote! { ::ferrum::sea_orm::Value::from(payload.#ident) });
        }
        if type_name == "String" {
            push_string_checks(
                &mut checks,
                &ident,
                &field_name,
                nullable,
                required,
                min_length,
                max_length,
            );
        }
        if let Some(minimum) = min {
            if nullable {
                checks.push(quote! { if let ::core::option::Option::Some(value) = payload.#ident {
                    if value < #minimum {
                        return Err(::ferrum::core::FerrumError::Validation(format!("{} must be >= {}", #field_name, #minimum)));
                    }
                } });
            } else {
                checks.push(quote! { if payload.#ident < #minimum {
                    return Err(::ferrum::core::FerrumError::Validation(format!("{} must be >= {}", #field_name, #minimum)));
                } });
            }
        }
        if readonly {
            continue;
        }
        update_fields.push(quote! {
            #[serde(default)]
            pub #ident: ::ferrum::db::PatchField<#value_ty>
        });
        update_bindings.push(update_binding(PatchRules {
            ident: &ident,
            field_name: &field_name,
            type_name: &type_name,
            nullable,
            required,
            min_length,
            max_length,
            min,
        }));
    }
    if !seen_id {
        return Err(syn::Error::new_spanned(
            &name,
            "missing id: Uuid with #[field(primary_key, auto)]",
        ));
    }
    let create_dto = format_ident!("{}Create", name);
    let update_dto = format_ident!("{}Update", name);
    let columns_ident = format_ident!("__FERRUM_COLUMNS_{}", name.to_string().to_uppercase());
    Ok(quote! {
        #[derive(::ferrum::serde::Deserialize)]
        #[serde(crate = "::ferrum::serde", deny_unknown_fields)]
        pub struct #create_dto { #(#dto_fields,)* }

        #[derive(::ferrum::serde::Deserialize)]
        #[serde(crate = "::ferrum::serde", deny_unknown_fields)]
        pub struct #update_dto { #(#update_fields,)* }

        const #columns_ident: &[::ferrum::core::ColumnSpec] = &[#(#column_specs),*];

        impl ::ferrum::core::FerrumModel for #name {
            fn descriptor() -> &'static ::ferrum::core::ModelDescriptor {
                static DESCRIPTOR: ::ferrum::core::ModelDescriptor = ::ferrum::core::ModelDescriptor {
                    name: stringify!(#name), table: #table, api_path: #api, allow_anonymous: #anonymous,
                };
                &DESCRIPTOR
            }
            fn columns() -> &'static [::ferrum::core::ColumnSpec] { #columns_ident }
            fn schema_sql(kind: ::ferrum::core::DatabaseKind) -> ::std::string::String {
                ::ferrum::db::render_schema(kind, #table, #columns_ident)
            }
        }

        #[::ferrum::async_trait::async_trait]
        impl ::ferrum::db::ModelRepository for #name {
            type Create = #create_dto;
            type Update = #update_dto;

            async fn list(db: &::ferrum::sea_orm::DatabaseConnection)
                -> Result<::ferrum::db::ListPage<Self>, ::ferrum::core::FerrumError>
            {
                let page = ::ferrum::db::list_records(db, #table, #columns_ident).await?;
                let data = page.rows.into_iter().map(|record| {
                    ::ferrum::serde_json::from_value::<Self>(record)
                        .map_err(::ferrum::core::FerrumError::database)
                }).collect::<Result<Vec<_>, _>>()?;
                Ok(::ferrum::db::ListPage { data, limit: page.limit, truncated: page.truncated })
            }

            async fn get(db: &::ferrum::sea_orm::DatabaseConnection, id: ::ferrum::uuid::Uuid)
                -> Result<Option<Self>, ::ferrum::core::FerrumError>
            {
                ::ferrum::db::get_record(db, #table, #columns_ident, id).await?
                    .map(|record| ::ferrum::serde_json::from_value::<Self>(record)
                        .map_err(::ferrum::core::FerrumError::database))
                    .transpose()
            }

            async fn create(db: &::ferrum::sea_orm::DatabaseConnection, payload: Self::Create)
                -> Result<Self, ::ferrum::core::FerrumError>
            {
                #(#checks)*
                let values: Vec<::ferrum::sea_orm::Value> = vec![#(#values),*];
                let record = ::ferrum::db::insert_record(db, #table, #columns_ident, values).await?;
                ::ferrum::serde_json::from_value::<Self>(record)
                    .map_err(::ferrum::core::FerrumError::database)
            }

            async fn update(
                db: &::ferrum::sea_orm::DatabaseConnection,
                id: ::ferrum::uuid::Uuid,
                payload: Self::Update,
            ) -> Result<Self, ::ferrum::core::FerrumError> {
                let mut changes: Vec<(&str, ::ferrum::sea_orm::Value)> = Vec::new();
                #(#update_bindings)*
                let record = ::ferrum::db::update_record(db, #table, #columns_ident, id, changes).await?;
                ::ferrum::serde_json::from_value::<Self>(record)
                    .map_err(::ferrum::core::FerrumError::database)
            }

            async fn delete(
                db: &::ferrum::sea_orm::DatabaseConnection,
                id: ::ferrum::uuid::Uuid,
            ) -> Result<::ferrum::db::DeleteOutcome, ::ferrum::core::FerrumError> {
                let affected = ::ferrum::db::delete_record(db, #table, id).await?;
                if affected == 0 {
                    Ok(::ferrum::db::DeleteOutcome::Missing)
                } else {
                    Ok(::ferrum::db::DeleteOutcome::Deleted)
                }
            }
        }
    })
}

fn valid_ident(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn field_type(ty: &Type) -> syn::Result<(String, bool, Type)> {
    let Type::Path(path) = ty else {
        return Err(syn::Error::new_spanned(ty, "unsupported field type"));
    };
    let segment = path.path.segments.last().unwrap();
    if segment.ident == "Option" {
        let PathArguments::AngleBracketed(args) = &segment.arguments else {
            return Err(syn::Error::new_spanned(
                ty,
                "Option needs a single type argument",
            ));
        };
        let mut inner = args.args.iter();
        let Some(GenericArgument::Type(inner_ty)) = inner.next() else {
            return Err(syn::Error::new_spanned(
                ty,
                "Option needs a single type argument",
            ));
        };
        if inner.next().is_some() {
            return Err(syn::Error::new_spanned(
                ty,
                "Option needs a single type argument",
            ));
        }
        let Type::Path(inner_path) = inner_ty else {
            return Err(syn::Error::new_spanned(
                inner_ty,
                "unsupported nullable field type",
            ));
        };
        let name = inner_path.path.segments.last().unwrap().ident.to_string();
        Ok((name, true, inner_ty.clone()))
    } else {
        Ok((segment.ident.to_string(), false, ty.clone()))
    }
}

fn null_value(type_name: &str) -> proc_macro2::TokenStream {
    match type_name {
        "String" => quote! { ::ferrum::sea_orm::Value::String(None) },
        "i64" => quote! { ::ferrum::sea_orm::Value::BigInt(None) },
        "bool" => quote! { ::ferrum::sea_orm::Value::Bool(None) },
        "Uuid" => quote! { ::ferrum::sea_orm::Value::Uuid(None) },
        _ => quote! { ::ferrum::sea_orm::Value::String(None) },
    }
}

fn push_string_checks(
    checks: &mut Vec<proc_macro2::TokenStream>,
    ident: &syn::Ident,
    field_name: &str,
    nullable: bool,
    required: bool,
    min_length: Option<usize>,
    max_length: Option<usize>,
) {
    if nullable {
        if let Some(lo) = min_length {
            checks.push(quote! { if let ::core::option::Option::Some(value) = payload.#ident.as_ref() {
                if value.trim().chars().count() < #lo {
                    return Err(::ferrum::core::FerrumError::Validation(format!("{} must have at least {} characters", #field_name, #lo)));
                }
            } });
        }
        if let Some(hi) = max_length {
            checks.push(quote! { if let ::core::option::Option::Some(value) = payload.#ident.as_ref() {
                if value.trim().chars().count() > #hi {
                    return Err(::ferrum::core::FerrumError::Validation(format!("{} may have at most {} characters", #field_name, #hi)));
                }
            } });
        }
        return;
    }
    if required {
        checks.push(quote! { if payload.#ident.trim().is_empty() {
            return Err(::ferrum::core::FerrumError::Validation(format!("{} must not be blank", #field_name)));
        } });
    }
    if let Some(lo) = min_length {
        checks.push(quote! { if payload.#ident.trim().chars().count() < #lo {
            return Err(::ferrum::core::FerrumError::Validation(format!("{} must have at least {} characters", #field_name, #lo)));
        } });
    }
    if let Some(hi) = max_length {
        checks.push(quote! { if payload.#ident.trim().chars().count() > #hi {
            return Err(::ferrum::core::FerrumError::Validation(format!("{} may have at most {} characters", #field_name, #hi)));
        } });
    }
}

struct PatchRules<'a> {
    ident: &'a syn::Ident,
    field_name: &'a str,
    type_name: &'a str,
    nullable: bool,
    required: bool,
    min_length: Option<usize>,
    max_length: Option<usize>,
    min: Option<i64>,
}

fn update_binding(rules: PatchRules<'_>) -> proc_macro2::TokenStream {
    let PatchRules {
        ident,
        field_name,
        type_name,
        nullable,
        required,
        min_length,
        max_length,
        min,
    } = rules;
    let mut value_checks = Vec::new();
    if type_name == "String" {
        if required {
            value_checks.push(quote! { if value.trim().is_empty() {
                return Err(::ferrum::core::FerrumError::Validation(format!("{} must not be blank", #field_name)));
            } });
        }
        if let Some(lo) = min_length {
            value_checks.push(quote! { if value.trim().chars().count() < #lo {
                return Err(::ferrum::core::FerrumError::Validation(format!("{} must have at least {} characters", #field_name, #lo)));
            } });
        }
        if let Some(hi) = max_length {
            value_checks.push(quote! { if value.trim().chars().count() > #hi {
                return Err(::ferrum::core::FerrumError::Validation(format!("{} may have at most {} characters", #field_name, #hi)));
            } });
        }
    }
    if let Some(minimum) = min {
        value_checks.push(quote! { if value < #minimum {
            return Err(::ferrum::core::FerrumError::Validation(format!("{} must be >= {}", #field_name, #minimum)));
        } });
    }
    let null_arm = if nullable {
        let null_sql = null_value(type_name);
        quote! { ::ferrum::db::PatchField::Null => { changes.push((#field_name, #null_sql)); } }
    } else {
        quote! {
            ::ferrum::db::PatchField::Null => {
                return Err(::ferrum::core::FerrumError::Validation(format!("{} cannot be null", #field_name)));
            }
        }
    };
    quote! {
        match payload.#ident {
            ::ferrum::db::PatchField::Missing => {}
            #null_arm
            ::ferrum::db::PatchField::Value(value) => {
                #(#value_checks)*
                changes.push((#field_name, ::ferrum::sea_orm::Value::from(value)));
            }
        };
    }
}
