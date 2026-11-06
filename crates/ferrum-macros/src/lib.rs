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
    let mut sql_columns = Vec::new();
    let mut names = Vec::new();
    let mut dto_fields = Vec::new();
    let mut values = Vec::new();
    let mut checks = Vec::new();
    let mut update_fields = Vec::new();
    let mut update_bindings = Vec::new();
    let mut update_values = Vec::new();
    let mut update_assignments = Vec::new();
    let mut seen_id = false;
    let mut parameter = 1usize;
    for field in fields {
        let ident = field.ident.unwrap();
        let field_name = ident.to_string();
        if !valid_ident(&field_name) {
            return Err(syn::Error::new_spanned(ident, "invalid column name"));
        }
        let (type_name, nullable, value_ty) = field_type(&field.ty)?;
        let sql_ty = match type_name.as_str() {
            "Uuid" => "UUID",
            "String" => "TEXT",
            "i64" => "BIGINT",
            "bool" => "BOOLEAN",
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
        let mut column_sql = if nullable {
            format!("\"{field_name}\" {sql_ty}")
        } else {
            format!("\"{field_name}\" {sql_ty} NOT NULL")
        };
        if primary {
            column_sql.push_str(" PRIMARY KEY");
        }
        if let Some(default) = default_bool {
            column_sql.push_str(if default {
                " DEFAULT TRUE"
            } else {
                " DEFAULT FALSE"
            });
        }
        sql_columns.push(column_sql);
        names.push(field_name.clone());
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
        let (binding, apply_token, value_token) = update_binding(PatchRules {
            ident: &ident,
            field_name: &field_name,
            type_name: &type_name,
            nullable,
            required,
            min_length,
            max_length,
            min,
        });
        update_bindings.push(binding);
        update_values.push(apply_token);
        update_values.push(value_token);
        let cast = sql_cast(&type_name);
        let flag = parameter;
        let value_slot = parameter + 1;
        update_assignments.push(format!(
            "\"{field_name}\" = CASE WHEN ${flag}::boolean THEN ${value_slot}::{cast} ELSE \"{field_name}\" END"
        ));
        parameter += 2;
    }
    if !seen_id {
        return Err(syn::Error::new_spanned(
            &name,
            "missing id: Uuid with #[field(primary_key, auto)]",
        ));
    }
    let create_dto = format_ident!("{}Create", name);
    let update_dto = format_ident!("{}Update", name);
    let id_slot = if update_assignments.is_empty() {
        1
    } else {
        parameter
    };
    let update_sql = if update_assignments.is_empty() {
        format!("UPDATE \"{table}\" SET \"id\" = \"id\" WHERE \"id\" = $1 RETURNING to_jsonb(\"{table}\") AS record")
    } else {
        format!(
            "UPDATE \"{table}\" SET {} WHERE \"id\" = ${id_slot} RETURNING to_jsonb(\"{table}\") AS record",
            update_assignments.join(", ")
        )
    };
    let delete_sql = format!("DELETE FROM \"{table}\" WHERE \"id\" = $1");
    let descriptors = format!(
        "CREATE TABLE IF NOT EXISTS \"{table}\" ({})",
        sql_columns.join(", ")
    );
    let columns = names
        .iter()
        .map(|n| format!("\"{n}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = (1..=names.len())
        .map(|i| format!("${i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let insert_sql = format!("INSERT INTO \"{table}\" ({columns}) VALUES ({placeholders}) RETURNING to_jsonb(\"{table}\") AS record");
    let list_sql =
        format!("SELECT to_jsonb(t) AS record FROM \"{table}\" t ORDER BY \"id\" LIMIT 101");
    let get_sql = format!("SELECT to_jsonb(t) AS record FROM \"{table}\" t WHERE \"id\" = $1");
    Ok(quote! {
        #[derive(::ferrum::serde::Deserialize)]
        #[serde(crate = "::ferrum::serde", deny_unknown_fields)]
        pub struct #create_dto { #(#dto_fields,)* }

        #[derive(::ferrum::serde::Deserialize)]
        #[serde(crate = "::ferrum::serde", deny_unknown_fields)]
        pub struct #update_dto { #(#update_fields,)* }

        impl ::ferrum::core::FerrumModel for #name {
            fn descriptor() -> &'static ::ferrum::core::ModelDescriptor {
                static DESCRIPTOR: ::ferrum::core::ModelDescriptor = ::ferrum::core::ModelDescriptor {
                    name: stringify!(#name), table: #table, api_path: #api, allow_anonymous: #anonymous,
                };
                &DESCRIPTOR
            }
            fn schema_sql() -> &'static str { #descriptors }
        }

        #[::ferrum::async_trait::async_trait]
        impl ::ferrum::db::ModelRepository for #name {
            type Create = #create_dto;
            type Update = #update_dto;

            async fn list(db: &::ferrum::sea_orm::DatabaseConnection)
                -> Result<::ferrum::db::ListPage<Self>, ::ferrum::core::FerrumError>
            {
                use ::ferrum::sea_orm::ConnectionTrait;
                let statement = ::ferrum::sea_orm::Statement::from_string(
                    ::ferrum::sea_orm::DbBackend::Postgres, #list_sql);
                let rows = db.query_all(statement).await
                    .map_err(::ferrum::core::FerrumError::database)?;
                let truncated = rows.len() > 100;
                let data = rows.into_iter().take(100).map(|row| {
                    let record: ::ferrum::serde_json::Value = row.try_get("", "record")
                        .map_err(::ferrum::core::FerrumError::database)?;
                    ::ferrum::serde_json::from_value::<Self>(record)
                        .map_err(::ferrum::core::FerrumError::database)
                }).collect::<Result<Vec<_>, _>>()?;
                Ok(::ferrum::db::ListPage { data, limit: 100, truncated })
            }

            async fn get(db: &::ferrum::sea_orm::DatabaseConnection, id: ::ferrum::uuid::Uuid)
                -> Result<Option<Self>, ::ferrum::core::FerrumError>
            {
                use ::ferrum::sea_orm::ConnectionTrait;
                let statement = ::ferrum::sea_orm::Statement::from_sql_and_values(
                    ::ferrum::sea_orm::DbBackend::Postgres, #get_sql,
                    vec![::ferrum::sea_orm::Value::from(id)],
                );
                let row = db.query_one(statement).await
                    .map_err(::ferrum::core::FerrumError::database)?;
                row.map(|row| {
                    let record: ::ferrum::serde_json::Value = row.try_get("", "record")
                        .map_err(::ferrum::core::FerrumError::database)?;
                    ::ferrum::serde_json::from_value::<Self>(record)
                        .map_err(::ferrum::core::FerrumError::database)
                }).transpose()
            }

            async fn create(db: &::ferrum::sea_orm::DatabaseConnection, payload: Self::Create)
                -> Result<Self, ::ferrum::core::FerrumError>
            {
                use ::ferrum::sea_orm::ConnectionTrait;
                #(#checks)*
                let values: Vec<::ferrum::sea_orm::Value> = vec![#(#values),*];
                let statement = ::ferrum::sea_orm::Statement::from_sql_and_values(
                    ::ferrum::sea_orm::DbBackend::Postgres, #insert_sql, values,
                );
                let row = db.query_one(statement).await.map_err(::ferrum::core::FerrumError::database)?
                    .ok_or_else(|| ::ferrum::core::FerrumError::Validation("no row returned".into()))?;
                let record: ::ferrum::serde_json::Value = row.try_get("", "record")
                    .map_err(::ferrum::core::FerrumError::database)?;
                ::ferrum::serde_json::from_value::<Self>(record)
                    .map_err(::ferrum::core::FerrumError::database)
            }

            async fn update(
                db: &::ferrum::sea_orm::DatabaseConnection,
                id: ::ferrum::uuid::Uuid,
                payload: Self::Update,
            ) -> Result<Self, ::ferrum::core::FerrumError> {
                use ::ferrum::sea_orm::ConnectionTrait;
                #(#update_bindings)*
                let values: Vec<::ferrum::sea_orm::Value> = vec![#(#update_values,)* ::ferrum::sea_orm::Value::from(id)];
                let statement = ::ferrum::sea_orm::Statement::from_sql_and_values(
                    ::ferrum::sea_orm::DbBackend::Postgres, #update_sql, values,
                );
                let row = db.query_one(statement).await.map_err(::ferrum::core::FerrumError::database)?
                    .ok_or(::ferrum::core::FerrumError::NotFound)?;
                let record: ::ferrum::serde_json::Value = row.try_get("", "record")
                    .map_err(::ferrum::core::FerrumError::database)?;
                ::ferrum::serde_json::from_value::<Self>(record)
                    .map_err(::ferrum::core::FerrumError::database)
            }

            async fn delete(
                db: &::ferrum::sea_orm::DatabaseConnection,
                id: ::ferrum::uuid::Uuid,
            ) -> Result<::ferrum::db::DeleteOutcome, ::ferrum::core::FerrumError> {
                use ::ferrum::sea_orm::ConnectionTrait;
                let statement = ::ferrum::sea_orm::Statement::from_sql_and_values(
                    ::ferrum::sea_orm::DbBackend::Postgres, #delete_sql,
                    vec![::ferrum::sea_orm::Value::from(id)],
                );
                let result = db.execute(statement).await.map_err(::ferrum::core::FerrumError::database)?;
                if result.rows_affected() == 0 {
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

fn sql_cast(type_name: &str) -> &'static str {
    match type_name {
        "String" => "text",
        "i64" => "bigint",
        "bool" => "boolean",
        "Uuid" => "uuid",
        _ => "text",
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

fn update_binding(
    rules: PatchRules<'_>,
) -> (
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
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
    let apply_ident = format_ident!("{ident}_apply");
    let value_ident = format_ident!("{ident}_value");
    let null_sql = null_value(type_name);
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
        quote! { ::ferrum::db::PatchField::Null => (true, #null_sql), }
    } else {
        quote! {
            ::ferrum::db::PatchField::Null => {
                return Err(::ferrum::core::FerrumError::Validation(format!("{} cannot be null", #field_name)));
            }
        }
    };
    let binding = quote! {
        let (#apply_ident, #value_ident) = match payload.#ident {
            ::ferrum::db::PatchField::Missing => (false, #null_sql),
            #null_arm
            ::ferrum::db::PatchField::Value(value) => {
                #(#value_checks)*
                (true, ::ferrum::sea_orm::Value::from(value))
            }
        };
    };
    (
        binding,
        quote! { ::ferrum::sea_orm::Value::from(#apply_ident) },
        quote! { #value_ident },
    )
}
