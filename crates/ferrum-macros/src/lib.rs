//! Derive a typed repository and SQL schema from a Rust struct.
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Data, DeriveInput, Fields, GenericArgument, LitBool, LitInt, LitStr,
    PathArguments, Type,
};

#[proc_macro_derive(FerrumModel, attributes(ferrum, field, admin))]
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
    let mut permission: Option<String> = None;
    let mut object_policy = false;
    let mut unique_groups = Vec::new();
    let mut index_groups = Vec::new();
    let mut has_many = Vec::new();
    let mut admin_enabled = false;
    let mut admin_label: Option<String> = None;
    let mut admin_permission: Option<String> = None;
    let mut admin_ordering: Option<String> = None;
    let mut admin_per_page: Option<u16> = None;
    let mut admin_fieldsets: Vec<(String, String, bool)> = Vec::new();
    let mut admin_inlines: Vec<String> = Vec::new();
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
            } else if meta.path.is_ident("permissions") {
                permission = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("object_policy") {
                object_policy = meta.value()?.parse::<LitBool>()?.value;
            } else if meta.path.is_ident("unique_together") {
                unique_groups.push(
                    split_names(&meta.value()?.parse::<LitStr>()?.value(), "unique_together")
                        .map_err(|error| meta.error(error))?,
                );
            } else if meta.path.is_ident("index_together") {
                index_groups.push(
                    split_names(&meta.value()?.parse::<LitStr>()?.value(), "index_together")
                        .map_err(|error| meta.error(error))?,
                );
            } else if meta.path.is_ident("admin") {
                admin_enabled = meta.value()?.parse::<LitBool>()?.value;
            } else if meta.path.is_ident("admin_label") {
                admin_label = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("admin_permission") {
                admin_permission = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("admin_ordering") {
                admin_ordering = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("admin_per_page") {
                let value = meta.value()?.parse::<LitInt>()?.base10_parse::<u16>()?;
                if !(1..=100).contains(&value) {
                    return Err(meta.error("admin_per_page must be from 1 to 100"));
                }
                admin_per_page = Some(value);
            } else if meta.path.is_ident("admin_fieldset") {
                admin_fieldsets.push(
                    parse_fieldset(&meta.value()?.parse::<LitStr>()?.value())
                        .map_err(|error| meta.error(error))?,
                );
            } else if meta.path.is_ident("admin_inline") {
                admin_inlines.push(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("has_many") {
                has_many.push(
                    parse_has_many(&meta.value()?.parse::<LitStr>()?.value())
                        .map_err(|error| meta.error(error))?,
                );
            } else {
                return Err(meta.error("unknown ferrum setting"));
            }
            Ok(())
        })?;
    }
    let table = table
        .ok_or_else(|| syn::Error::new_spanned(&name, "missing #[ferrum(table = \"...\")]"))?;
    let api = api.unwrap_or_else(|| format!("/{table}"));
    if anonymous && (permission.is_some() || object_policy) {
        return Err(syn::Error::new_spanned(
            &name,
            "allow_anonymous cannot be combined with permissions or object_policy",
        ));
    }
    if object_policy && permission.is_none() {
        return Err(syn::Error::new_spanned(
            &name,
            "object_policy requires permissions",
        ));
    }
    if let Some(value) = &permission {
        if value != "authenticated" && !valid_ident(value) {
            return Err(syn::Error::new_spanned(
                &name,
                "permissions must be \"authenticated\" or a SQL identifier such as products",
            ));
        }
    }
    if !admin_enabled
        && (admin_label.is_some()
            || admin_permission.is_some()
            || admin_ordering.is_some()
            || admin_per_page.is_some()
            || !admin_fieldsets.is_empty())
    {
        return Err(syn::Error::new_spanned(
            &name,
            "admin settings require admin = true",
        ));
    }
    if let Some(value) = &admin_permission {
        if value != "authenticated" && !valid_ident(value) {
            return Err(syn::Error::new_spanned(
                &name,
                "admin_permission must be \"authenticated\" or a SQL identifier",
            ));
        }
    }
    if let Some(value) = &admin_label {
        if value.trim().is_empty() || value.chars().count() > 80 {
            return Err(syn::Error::new_spanned(
                &name,
                "admin_label must contain 1 to 80 characters",
            ));
        }
    }
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
    let mut field_names = Vec::new();
    let mut relation_methods = Vec::new();
    let mut admin_fields = Vec::new();
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
        let mut unique = false;
        let mut indexed = false;
        let mut foreign_key: Option<String> = None;
        let mut on_delete: Option<String> = None;
        let mut belongs_to: Option<String> = None;
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
                } else if meta.path.is_ident("unique") {
                    unique = true;
                } else if meta.path.is_ident("index") {
                    indexed = true;
                } else if meta.path.is_ident("foreign_key") {
                    foreign_key = Some(meta.value()?.parse::<LitStr>()?.value());
                } else if meta.path.is_ident("on_delete") {
                    on_delete = Some(meta.value()?.parse::<LitStr>()?.value());
                } else if meta.path.is_ident("belongs_to") {
                    belongs_to = Some(meta.value()?.parse::<LitStr>()?.value());
                } else {
                    return Err(meta.error("unsupported field attribute"));
                }
                Ok(())
            })?;
        }
        let mut admin_list = false;
        let mut admin_search = false;
        let mut admin_sort = false;
        let mut admin_filter = false;
        let mut admin_readonly = false;
        let mut admin_hidden = false;
        let mut admin_link = false;
        let mut admin_fieldset: Option<String> = None;
        let mut admin_sensitive = false;
        let mut admin_field_label: Option<String> = None;
        let mut saw_admin_attr = false;
        for attr in &field.attrs {
            if !attr.path().is_ident("admin") {
                continue;
            }
            saw_admin_attr = true;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("list_display") {
                    admin_list = true;
                } else if meta.path.is_ident("searchable") {
                    admin_search = true;
                } else if meta.path.is_ident("sortable") {
                    admin_sort = true;
                } else if meta.path.is_ident("filterable") {
                    admin_filter = true;
                } else if meta.path.is_ident("readonly") {
                    admin_readonly = true;
                } else if meta.path.is_ident("hidden") {
                    admin_hidden = true;
                } else if meta.path.is_ident("list_link") {
                    admin_link = true;
                } else if meta.path.is_ident("fieldset") {
                    admin_fieldset = Some(meta.value()?.parse::<LitStr>()?.value());
                } else if meta.path.is_ident("sensitive") {
                    admin_sensitive = true;
                } else if meta.path.is_ident("label") {
                    admin_field_label = Some(meta.value()?.parse::<LitStr>()?.value());
                } else {
                    return Err(meta.error(
                        "unknown admin attribute; expected list_display, list_link, searchable, sortable, filterable, readonly, hidden, sensitive, fieldset or label",
                    ));
                }
                Ok(())
            })?;
        }
        if saw_admin_attr && !admin_enabled {
            return Err(syn::Error::new_spanned(
                &ident,
                "field admin attributes require #[ferrum(admin = true)]",
            ));
        }
        if admin_enabled {
            if field_name == "id" && admin_hidden {
                return Err(syn::Error::new_spanned(&ident, "id cannot be hidden"));
            }
            if admin_hidden
                && (admin_list || admin_link || admin_search || admin_sort || admin_filter)
            {
                return Err(syn::Error::new_spanned(
                    &ident,
                    "hidden admin fields cannot be displayed, searched, sorted or filtered",
                ));
            }
            if admin_link && !admin_list && field_name != "id" {
                return Err(syn::Error::new_spanned(
                    &ident,
                    "list_link requires list_display",
                ));
            }
            if let Some(section) = &admin_fieldset {
                if section.trim().is_empty()
                    || section.chars().count() > 80
                    || !admin_fieldsets.iter().any(|(name, _, _)| name == section)
                {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        "fieldset must name a declared admin_fieldset of 1 to 80 characters",
                    ));
                }
            }
            if admin_search && type_name != "String" {
                return Err(syn::Error::new_spanned(
                    &ident,
                    "searchable requires String",
                ));
            }
            if (admin_sensitive || sensitive_admin_name(&field_name))
                && (!admin_hidden || admin_list || admin_search || admin_sort || admin_filter)
            {
                return Err(syn::Error::new_spanned(
                    &ident,
                    "password, token, secret and sensitive fields must be #[admin(hidden)] and cannot be listed",
                ));
            }
            if let Some(label) = &admin_field_label {
                if label.trim().is_empty() || label.chars().count() > 80 {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        "admin label must contain 1 to 80 characters",
                    ));
                }
            }
            admin_fields.push(AdminFieldBits {
                name: field_name.clone(),
                label: admin_field_label.unwrap_or_else(|| field_label(&field_name)),
                list_display: admin_list || field_name == "id",
                list_link: admin_link,
                searchable: admin_search,
                sortable: admin_sort,
                filterable: admin_filter,
                readonly: admin_readonly || primary || readonly,
                hidden: admin_hidden,
                sensitive: admin_sensitive || sensitive_admin_name(&field_name),
                fieldset: admin_fieldset.unwrap_or_default(),
            });
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
        field_names.push(field_name.clone());
        if unique && primary {
            return Err(syn::Error::new_spanned(
                &ident,
                "primary key columns are already unique",
            ));
        }
        if indexed && (primary || unique) {
            return Err(syn::Error::new_spanned(
                &ident,
                "index is redundant on a primary key or unique column",
            ));
        }
        if on_delete.is_some() && foreign_key.is_none() {
            return Err(syn::Error::new_spanned(
                &ident,
                "on_delete requires foreign_key",
            ));
        }
        if belongs_to.is_some() && foreign_key.is_none() {
            return Err(syn::Error::new_spanned(
                &ident,
                "belongs_to requires foreign_key",
            ));
        }
        let (foreign_table, foreign_column, on_delete_token) = if let Some(reference) = &foreign_key
        {
            let (foreign_table, foreign_column) = reference.split_once('.').ok_or_else(|| {
                syn::Error::new_spanned(&ident, "foreign_key must look like table.column")
            })?;
            if !valid_ident(foreign_table) || !valid_ident(foreign_column) {
                return Err(syn::Error::new_spanned(
                    &ident,
                    "foreign_key names must be SQL identifiers",
                ));
            }
            let action = on_delete
                .as_deref()
                .ok_or_else(|| syn::Error::new_spanned(&ident, "foreign_key requires on_delete"))?;
            if action == "set_null" && !nullable {
                return Err(syn::Error::new_spanned(
                    &ident,
                    "on_delete = \"set_null\" requires a nullable column",
                ));
            }
            let token = match action {
                "restrict" => {
                    quote! { ::core::option::Option::Some(::ferrum::core::ReferentialAction::Restrict) }
                }
                "cascade" => {
                    quote! { ::core::option::Option::Some(::ferrum::core::ReferentialAction::Cascade) }
                }
                "set_null" => {
                    quote! { ::core::option::Option::Some(::ferrum::core::ReferentialAction::SetNull) }
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        "on_delete must be restrict, cascade or set_null",
                    ))
                }
            };
            (
                Some(foreign_table.to_owned()),
                Some(foreign_column.to_owned()),
                token,
            )
        } else {
            (None, None, quote! { ::core::option::Option::None })
        };
        if let Some(path) = &belongs_to {
            let ty: syn::Path =
                syn::parse_str(path).map_err(|error| syn::Error::new_spanned(&ident, error))?;
            let method_name = belongs_method(&field_name)
                .map_err(|error| syn::Error::new_spanned(&ident, error))?;
            let method = syn::Ident::new(&method_name, ident.span());
            let field_ident = ident.clone();
            relation_methods.push(if nullable {
                quote! {
                    pub async fn #method(&self, db: &::ferrum::sea_orm::DatabaseConnection) -> ::core::result::Result<::core::option::Option<#ty>, ::ferrum::core::FerrumError> {
                        let ::core::option::Option::Some(id) = self.#field_ident else {
                            return ::core::result::Result::Ok(::core::option::Option::None);
                        };
                        match <#ty as ::ferrum::db::ModelRepository>::get(db, id).await? {
                            ::core::option::Option::Some(value) => ::core::result::Result::Ok(::core::option::Option::Some(value)),
                            ::core::option::Option::None => ::core::result::Result::Err(::ferrum::core::FerrumError::NotFound),
                        }
                    }
                }
            } else {
                quote! {
                    pub async fn #method(&self, db: &::ferrum::sea_orm::DatabaseConnection) -> ::core::result::Result<#ty, ::ferrum::core::FerrumError> {
                        match <#ty as ::ferrum::db::ModelRepository>::get(db, self.#field_ident).await? {
                            ::core::option::Option::Some(value) => ::core::result::Result::Ok(value),
                            ::core::option::Option::None => ::core::result::Result::Err(::ferrum::core::FerrumError::NotFound),
                        }
                    }
                }
            });
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
        let foreign_table_token = match &foreign_table {
            Some(value) => quote! { ::core::option::Option::Some(#value) },
            None => quote! { ::core::option::Option::None },
        };
        let foreign_column_token = match &foreign_column {
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
                unique: #unique,
                indexed: #indexed,
                foreign_table: #foreign_table_token,
                foreign_column: #foreign_column_token,
                on_delete: #on_delete_token,
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
    validate_groups(&name, &field_names, &unique_groups, "unique_together")?;
    validate_groups(&name, &field_names, &index_groups, "index_together")?;
    for group in &index_groups {
        if unique_groups.iter().any(|unique| same_names(unique, group)) {
            return Err(syn::Error::new_spanned(
                &name,
                "index_together repeats a unique_together column list",
            ));
        }
    }
    for (method, type_path, column) in &has_many {
        if field_names.iter().any(|field| field == method) {
            return Err(syn::Error::new_spanned(
                &name,
                format!("has_many method {method} collides with a field"),
            ));
        }
        let ty: syn::Path =
            syn::parse_str(type_path).map_err(|error| syn::Error::new_spanned(&name, error))?;
        let method_ident = syn::Ident::new(method, name.span());
        relation_methods.push(quote! {
            pub async fn #method_ident(&self, db: &::ferrum::sea_orm::DatabaseConnection) -> ::core::result::Result<::std::vec::Vec<#ty>, ::ferrum::core::FerrumError> {
                ::ferrum::db::find_by_column::<#ty>(db, #column, ::ferrum::sea_orm::Value::from(self.id)).await
            }
        });
    }
    if admin_enabled {
        let mut seen_sections = Vec::new();
        for (section, _, _) in &admin_fieldsets {
            if seen_sections
                .iter()
                .any(|existing: &String| existing == section)
            {
                return Err(syn::Error::new_spanned(
                    &name,
                    "admin_fieldset names must be unique",
                ));
            }
            seen_sections.push(section.clone());
        }
        if !admin_fieldsets.is_empty() {
            for field in &admin_fields {
                if !field.hidden && field.fieldset.is_empty() {
                    return Err(syn::Error::new_spanned(
                        &name,
                        format!("field {} needs a fieldset", field.name),
                    ));
                }
            }
            for (section, _, _) in &admin_fieldsets {
                if !admin_fields
                    .iter()
                    .any(|field| !field.hidden && &field.fieldset == section)
                {
                    return Err(syn::Error::new_spanned(
                        &name,
                        format!("fieldset {section} has no fields"),
                    ));
                }
            }
        } else if admin_fields.iter().any(|field| !field.fieldset.is_empty()) {
            return Err(syn::Error::new_spanned(
                &name,
                "fieldset requires admin_fieldset on the model",
            ));
        }
        if let Some(ordering) = &admin_ordering {
            let column = ordering.strip_prefix('-').unwrap_or(ordering);
            let allowed = admin_fields.iter().any(|field| {
                field.name == column && !field.hidden && (field.sortable || field.name == "id")
            });
            if !valid_ident(column) || !allowed {
                return Err(syn::Error::new_spanned(
                    &name,
                    "admin_ordering must be a sortable field or id, with an optional leading -",
                ));
            }
        }
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
    let unique_lists = groups_tokens(&unique_groups);
    let index_lists = groups_tokens(&index_groups);
    let permission_token = match &permission {
        Some(value) => quote! { ::core::option::Option::Some(#value) },
        None => quote! { ::core::option::Option::None },
    };
    let resolved_inlines = if admin_enabled {
        resolve_inlines(&name, &admin_inlines, &has_many)?
    } else if admin_inlines.is_empty() {
        Vec::new()
    } else {
        return Err(syn::Error::new_spanned(
            &name,
            "admin_inline requires admin = true",
        ));
    };
    let admin_meta_fn = if admin_enabled {
        let resolved = admin_permission.clone().or(permission.clone());
        let permission_meta = match &resolved {
            Some(value) => quote! { ::core::option::Option::Some(#value) },
            None => quote! { ::core::option::Option::None },
        };
        let label_value = admin_label.clone().unwrap_or_else(|| field_label(&table));
        let field_tokens = admin_fields.iter().map(|field| {
            let name = &field.name;
            let label = &field.label;
            let list_display = field.list_display;
            let list_link = field.list_link;
            let searchable = field.searchable;
            let sortable = field.sortable;
            let filterable = field.filterable;
            let readonly = field.readonly;
            let hidden = field.hidden;
            let sensitive = field.sensitive;
            let fieldset = &field.fieldset;
            quote! {
                ::ferrum::core::AdminFieldMeta {
                    name: #name,
                    label: #label,
                    list_display: #list_display,
                    list_link: #list_link,
                    searchable: #searchable,
                    sortable: #sortable,
                    filterable: #filterable,
                    readonly: #readonly,
                    hidden: #hidden,
                    sensitive: #sensitive,
                    fieldset: #fieldset,
                }
            }
        });
        let ordering_meta = match &admin_ordering {
            Some(value) => quote! { ::core::option::Option::Some(#value) },
            None => quote! { ::core::option::Option::None },
        };
        let per_page = admin_per_page.unwrap_or(25);
        let inline_tokens = resolved_inlines.iter().map(|(method, child, column)| {
            quote! {
                ::ferrum::core::AdminInline {
                    name: #method,
                    child_table: #child,
                    column: #column,
                }
            }
        });
        let fieldset_tokens = admin_fieldsets
            .iter()
            .map(|(section, description, optional)| {
                quote! {
                    ::ferrum::core::AdminFieldset {
                        name: #section,
                        description: #description,
                        optional: #optional,
                    }
                }
            });
        quote! {
            fn admin_meta() -> ::core::option::Option<&'static ::ferrum::core::AdminModelMeta> {
                static FIELDS: &[::ferrum::core::AdminFieldMeta] = &[#(#field_tokens),*];
                static FIELDSETS: &[::ferrum::core::AdminFieldset] = &[#(#fieldset_tokens),*];
                static INLINES: &[::ferrum::core::AdminInline] = &[#(#inline_tokens),*];
                static META: ::ferrum::core::AdminModelMeta = ::ferrum::core::AdminModelMeta {
                    key: #table,
                    label: #label_value,
                    permission: #permission_meta,
                    fields: FIELDS,
                    ordering: #ordering_meta,
                    per_page: #per_page,
                    fieldsets: FIELDSETS,
                    inlines: INLINES,
                };
                ::core::option::Option::Some(&META)
            }
        }
    } else {
        quote! {}
    };
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
                    name: stringify!(#name),
                    table: #table,
                    api_path: #api,
                    allow_anonymous: #anonymous,
                    permission: #permission_token,
                    requires_object_policy: #object_policy,
                };
                &DESCRIPTOR
            }
            fn columns() -> &'static [::ferrum::core::ColumnSpec] { #columns_ident }
            fn constraints() -> &'static ::ferrum::core::TableConstraints {
                static CONSTRAINTS: ::ferrum::core::TableConstraints = ::ferrum::core::TableConstraints {
                    unique_groups: &[#(#unique_lists),*],
                    index_groups: &[#(#index_lists),*],
                };
                &CONSTRAINTS
            }
            #admin_meta_fn
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
                Self::list_scoped(db, &::ferrum::core::QueryScope::All).await
            }

            async fn list_scoped(
                db: &::ferrum::sea_orm::DatabaseConnection,
                scope: &::ferrum::core::QueryScope,
            ) -> Result<::ferrum::db::ListPage<Self>, ::ferrum::core::FerrumError> {
                let page = ::ferrum::db::list_records(db, #table, #columns_ident, scope).await?;
                let data = page.rows.into_iter().map(|record| {
                    ::ferrum::serde_json::from_value::<Self>(record)
                        .map_err(::ferrum::core::FerrumError::database)
                }).collect::<Result<Vec<_>, _>>()?;
                Ok(::ferrum::db::ListPage { data, limit: page.limit, truncated: page.truncated })
            }

            async fn get(db: &::ferrum::sea_orm::DatabaseConnection, id: ::ferrum::uuid::Uuid)
                -> Result<Option<Self>, ::ferrum::core::FerrumError>
            {
                Self::get_scoped(db, id, &::ferrum::core::QueryScope::All).await
            }

            async fn get_scoped(
                db: &::ferrum::sea_orm::DatabaseConnection,
                id: ::ferrum::uuid::Uuid,
                scope: &::ferrum::core::QueryScope,
            ) -> Result<Option<Self>, ::ferrum::core::FerrumError> {
                Self::get_in(db, id, scope).await
            }

            async fn create(db: &::ferrum::sea_orm::DatabaseConnection, payload: Self::Create)
                -> Result<Self, ::ferrum::core::FerrumError>
            {
                Self::create_in(db, payload).await
            }

            async fn update(
                db: &::ferrum::sea_orm::DatabaseConnection,
                id: ::ferrum::uuid::Uuid,
                payload: Self::Update,
            ) -> Result<Self, ::ferrum::core::FerrumError> {
                Self::update_scoped(db, id, payload, &::ferrum::core::QueryScope::All).await
            }

            async fn update_scoped(
                db: &::ferrum::sea_orm::DatabaseConnection,
                id: ::ferrum::uuid::Uuid,
                payload: Self::Update,
                scope: &::ferrum::core::QueryScope,
            ) -> Result<Self, ::ferrum::core::FerrumError> {
                Self::update_in(db, id, payload, scope).await
            }

            async fn delete(
                db: &::ferrum::sea_orm::DatabaseConnection,
                id: ::ferrum::uuid::Uuid,
            ) -> Result<::ferrum::db::DeleteOutcome, ::ferrum::core::FerrumError> {
                Self::delete_scoped(db, id, &::ferrum::core::QueryScope::All).await
            }

            async fn delete_scoped(
                db: &::ferrum::sea_orm::DatabaseConnection,
                id: ::ferrum::uuid::Uuid,
                scope: &::ferrum::core::QueryScope,
            ) -> Result<::ferrum::db::DeleteOutcome, ::ferrum::core::FerrumError> {
                Self::delete_in(db, id, scope).await
            }

            async fn get_tx(
                db: &::ferrum::sea_orm::DatabaseTransaction,
                id: ::ferrum::uuid::Uuid,
                scope: &::ferrum::core::QueryScope,
            ) -> Result<Option<Self>, ::ferrum::core::FerrumError> {
                Self::get_in(db, id, scope).await
            }

            async fn create_tx(
                db: &::ferrum::sea_orm::DatabaseTransaction,
                payload: Self::Create,
            ) -> Result<Self, ::ferrum::core::FerrumError> {
                Self::create_in(db, payload).await
            }

            async fn update_tx(
                db: &::ferrum::sea_orm::DatabaseTransaction,
                id: ::ferrum::uuid::Uuid,
                payload: Self::Update,
                scope: &::ferrum::core::QueryScope,
            ) -> Result<Self, ::ferrum::core::FerrumError> {
                Self::update_in(db, id, payload, scope).await
            }

            async fn delete_tx(
                db: &::ferrum::sea_orm::DatabaseTransaction,
                id: ::ferrum::uuid::Uuid,
                scope: &::ferrum::core::QueryScope,
            ) -> Result<::ferrum::db::DeleteOutcome, ::ferrum::core::FerrumError> {
                Self::delete_in(db, id, scope).await
            }
        }

        impl #name {
            pub async fn get_in<C>(
                db: &C,
                id: ::ferrum::uuid::Uuid,
                scope: &::ferrum::core::QueryScope,
            ) -> Result<Option<Self>, ::ferrum::core::FerrumError>
            where
                C: ::ferrum::sea_orm::ConnectionTrait,
            {
                ::ferrum::db::get_record(db, #table, #columns_ident, id, scope).await?
                    .map(|record| ::ferrum::serde_json::from_value::<Self>(record)
                        .map_err(::ferrum::core::FerrumError::database))
                    .transpose()
            }

            pub async fn create_in<C>(
                db: &C,
                payload: #create_dto,
            ) -> Result<Self, ::ferrum::core::FerrumError>
            where
                C: ::ferrum::sea_orm::ConnectionTrait,
            {
                #(#checks)*
                let values: Vec<::ferrum::sea_orm::Value> = vec![#(#values),*];
                let record = ::ferrum::db::insert_record(db, #table, #columns_ident, values).await?;
                ::ferrum::serde_json::from_value::<Self>(record)
                    .map_err(::ferrum::core::FerrumError::database)
            }

            pub async fn update_in<C>(
                db: &C,
                id: ::ferrum::uuid::Uuid,
                payload: #update_dto,
                scope: &::ferrum::core::QueryScope,
            ) -> Result<Self, ::ferrum::core::FerrumError>
            where
                C: ::ferrum::sea_orm::ConnectionTrait,
            {
                let mut changes: Vec<(&str, ::ferrum::sea_orm::Value)> = Vec::new();
                #(#update_bindings)*
                let record = ::ferrum::db::update_record(db, #table, #columns_ident, id, changes, scope).await?;
                ::ferrum::serde_json::from_value::<Self>(record)
                    .map_err(::ferrum::core::FerrumError::database)
            }

            pub async fn delete_in<C>(
                db: &C,
                id: ::ferrum::uuid::Uuid,
                scope: &::ferrum::core::QueryScope,
            ) -> Result<::ferrum::db::DeleteOutcome, ::ferrum::core::FerrumError>
            where
                C: ::ferrum::sea_orm::ConnectionTrait,
            {
                let affected = ::ferrum::db::delete_record(db, #table, id, scope, #columns_ident).await?;
                if affected == 0 {
                    Ok(::ferrum::db::DeleteOutcome::Missing)
                } else {
                    Ok(::ferrum::db::DeleteOutcome::Deleted)
                }
            }

            #(#relation_methods)*
        }
    })
}

fn groups_tokens(groups: &[Vec<String>]) -> Vec<proc_macro2::TokenStream> {
    groups
        .iter()
        .map(|group| {
            let columns = group.iter().map(|column| quote! { #column });
            quote! { &[#(#columns),*] }
        })
        .collect()
}

fn split_names(value: &str, label: &str) -> Result<Vec<String>, String> {
    let names = value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if names.len() < 2 {
        return Err(format!("{label} needs at least two column names"));
    }
    if names.iter().any(|name| !valid_ident(name)) {
        return Err(format!("{label} contains an invalid column name"));
    }
    Ok(names)
}

fn resolve_inlines(
    name: &syn::Ident,
    declared: &[String],
    has_many: &[(String, String, String)],
) -> syn::Result<Vec<(String, String, String)>> {
    let mut resolved = Vec::new();
    for value in declared {
        let (method, child, column) =
            parse_admin_inline(value).map_err(|error| syn::Error::new_spanned(name, error))?;
        let matched = has_many
            .iter()
            .any(|(existing, _, has_column)| existing == &method && has_column == &column);
        if !matched {
            return Err(syn::Error::new_spanned(
                name,
                format!("admin_inline {method} must match has_many {method}=Type.{column}"),
            ));
        }
        if resolved.iter().any(|(existing, _, _)| existing == &method) {
            return Err(syn::Error::new_spanned(
                name,
                format!("admin_inline {method} is declared twice"),
            ));
        }
        resolved.push((method, child, column));
    }
    Ok(resolved)
}

fn parse_admin_inline(value: &str) -> Result<(String, String, String), String> {
    let (method, rest) = value
        .split_once('=')
        .ok_or("admin_inline must look like products=products.category_id")?;
    let (table, column) = rest
        .rsplit_once('.')
        .ok_or("admin_inline must name a table and a column")?;
    if !valid_ident(method) || !valid_ident(table) || !valid_ident(column) {
        return Err("admin_inline contains an invalid name".to_owned());
    }
    Ok((method.to_owned(), table.to_owned(), column.to_owned()))
}

fn parse_has_many(value: &str) -> Result<(String, String, String), String> {
    let (method, rest) = value
        .split_once('=')
        .ok_or("has_many must look like products=Product.category_id")?;
    let (type_path, column) = rest
        .rsplit_once('.')
        .ok_or("has_many must name a type and a column")?;
    if !valid_ident(method) || !valid_ident(column) || type_path.is_empty() {
        return Err("has_many contains an invalid name".to_owned());
    }
    if type_path
        .split("::")
        .any(|segment| segment.is_empty() || !valid_ident(segment))
    {
        return Err("has_many type path is invalid".to_owned());
    }
    Ok((method.to_owned(), type_path.to_owned(), column.to_owned()))
}

fn belongs_method(field_name: &str) -> Result<String, String> {
    let method = field_name
        .strip_suffix("_id")
        .filter(|name| valid_ident(name))
        .ok_or("belongs_to needs a column name ending in _id")?;
    Ok(method.to_owned())
}

fn validate_groups(
    name: &syn::Ident,
    fields: &[String],
    groups: &[Vec<String>],
    label: &str,
) -> syn::Result<()> {
    for group in groups {
        for column in group {
            if !fields.iter().any(|field| field == column) {
                return Err(syn::Error::new_spanned(
                    name,
                    format!("{label} names unknown column {column}"),
                ));
            }
        }
        if fields.iter().any(|field| field == "id") && group.iter().any(|column| column == "id") {
            return Err(syn::Error::new_spanned(
                name,
                format!("{label} cannot include the primary key"),
            ));
        }
    }
    Ok(())
}

fn same_names(left: &[String], right: &[String]) -> bool {
    let mut left = left.to_vec();
    let mut right = right.to_vec();
    left.sort();
    right.sort();
    left == right
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

struct AdminFieldBits {
    name: String,
    label: String,
    list_display: bool,
    list_link: bool,
    searchable: bool,
    sortable: bool,
    filterable: bool,
    readonly: bool,
    hidden: bool,
    sensitive: bool,
    fieldset: String,
}

fn parse_fieldset(value: &str) -> Result<(String, String, bool), String> {
    let mut parts = value.splitn(3, '|');
    let name = parts.next().unwrap_or("").trim();
    let description = parts.next().unwrap_or("").trim();
    let flag = parts.next().unwrap_or("").trim();
    if name.is_empty() || name.chars().count() > 80 || description.chars().count() > 160 {
        return Err("admin_fieldset needs a name of 1 to 80 characters".into());
    }
    let optional = match flag {
        "" => false,
        "optional" => true,
        _ => return Err("admin_fieldset flag must be empty or optional".into()),
    };
    Ok((name.to_owned(), description.to_owned(), optional))
}

fn field_label(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => name.to_owned(),
    }
}

fn sensitive_admin_name(name: &str) -> bool {
    matches!(
        name,
        "password"
            | "password_hash"
            | "token"
            | "token_hash"
            | "secret"
            | "csrf_token"
            | "csrf_token_hash"
    )
}
