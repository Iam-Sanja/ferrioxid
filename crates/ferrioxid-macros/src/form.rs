//! `#[derive(FerrioxidForm)]` builds `from_form` from an allowlisted model.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, GenericArgument, Ident, LitStr, PathArguments, Type};

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let model = model_path(&input)?;
    let name = input.ident;
    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(named) => named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    name,
                    "FerrioxidForm needs named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "FerrioxidForm only supports structs",
            ))
        }
    };
    let mut idents = Vec::new();
    let mut columns = Vec::new();
    let mut readers = Vec::new();
    for field in fields {
        let ident = field
            .ident
            .clone()
            .ok_or_else(|| syn::Error::new_spanned(&name, "FerrioxidForm needs named fields"))?;
        let mut column = ident.to_string();
        for attr in &field.attrs {
            if !attr.path().is_ident("form") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("column") {
                    column = meta.value()?.parse::<LitStr>()?.value();
                    Ok(())
                } else {
                    Err(meta.error("unsupported form field attribute"))
                }
            })?;
        }
        if !is_field_name(&column) {
            return Err(syn::Error::new_spanned(
                &ident,
                "form column name is not an identifier",
            ));
        }
        let reader = read_field(&ident, &column, &field.ty)?;
        idents.push(ident);
        columns.push(column);
        readers.push(reader);
    }
    if columns.is_empty() {
        return Err(syn::Error::new_spanned(
            &name,
            "FerrioxidForm needs at least one field",
        ));
    }
    Ok(quote! {
        impl #name {
            pub fn from_submit(
                mode: ::ferrioxid::forms::SubmitMode,
                values: &::ferrioxid::forms::FormValues,
            ) -> ::core::result::Result<Self, ::ferrioxid::forms::FormErrors> {
                let allow: &[&str] = &[#(#columns),*];
                let admin = <#model as ::ferrioxid::core::FerrioxidModel>::admin_meta()
                    .map(|meta| meta.fields)
                    .unwrap_or(&[]);
                let mut fields = match ::ferrioxid::forms::model_fields(
                    <#model as ::ferrioxid::core::FerrioxidModel>::columns(),
                    admin,
                    allow,
                ) {
                    ::core::result::Result::Ok(fields) => fields,
                    ::core::result::Result::Err(error) => {
                        let mut errors = ::ferrioxid::forms::FormErrors::default();
                        errors.push("", error.code);
                        return ::core::result::Result::Err(errors);
                    }
                };
                ::ferrioxid::forms::apply_bounds(
                    &mut fields,
                    <#model as ::ferrioxid::core::FerrioxidModel>::numeric_bounds(),
                );
                let bound = ::ferrioxid::forms::bind_model(mode, &fields, values, &[])?;
                ::core::result::Result::Ok(Self {
                    #(#readers,)*
                })
            }
        }

        impl ::ferrioxid::forms::FerrioxidForm for #name {
            fn from_form(
                values: &::ferrioxid::forms::FormValues,
            ) -> ::core::result::Result<Self, ::ferrioxid::forms::FormErrors> {
                Self::from_submit(::ferrioxid::forms::SubmitMode::Create, values)
            }
        }
    })
}

fn model_path(input: &DeriveInput) -> syn::Result<syn::Path> {
    let mut found = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("form") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("model") {
                found = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("FerrioxidForm requires #[form(model = Type)]"))
            }
        })?;
    }
    found.ok_or_else(|| {
        syn::Error::new_spanned(&input.ident, "FerrioxidForm requires #[form(model = Type)]")
    })
}

fn read_field(ident: &Ident, column: &str, ty: &Type) -> syn::Result<TokenStream> {
    let kind = match type_kind(ty) {
        Some(kind) => kind,
        None => {
            return Err(syn::Error::new_spanned(
                ty,
                "FerrioxidForm fields must be String, i64, bool, or Option of those",
            ))
        }
    };
    let value = format_ident!("value");
    let body = match kind {
        FieldKind::String => quote! {
            match #value {
                ::ferrioxid::forms::BoundValue::Text(text) => text.clone(),
                _ => return ::core::result::Result::Err(invalid(#column)),
            }
        },
        FieldKind::Integer => quote! {
            match #value {
                ::ferrioxid::forms::BoundValue::Integer(number) => *number,
                _ => return ::core::result::Result::Err(invalid(#column)),
            }
        },
        FieldKind::Boolean => quote! {
            match #value {
                ::ferrioxid::forms::BoundValue::Boolean(flag) => *flag,
                _ => return ::core::result::Result::Err(invalid(#column)),
            }
        },
        FieldKind::OptionalString => quote! {
            match #value {
                ::ferrioxid::forms::BoundValue::Text(text) => ::core::option::Option::Some(text.clone()),
                ::ferrioxid::forms::BoundValue::Null | ::ferrioxid::forms::BoundValue::Unchanged => ::core::option::Option::None,
                _ => return ::core::result::Result::Err(invalid(#column)),
            }
        },
        FieldKind::OptionalInteger => quote! {
            match #value {
                ::ferrioxid::forms::BoundValue::Integer(number) => ::core::option::Option::Some(*number),
                ::ferrioxid::forms::BoundValue::Null | ::ferrioxid::forms::BoundValue::Unchanged => ::core::option::Option::None,
                _ => return ::core::result::Result::Err(invalid(#column)),
            }
        },
        FieldKind::OptionalBoolean => quote! {
            match #value {
                ::ferrioxid::forms::BoundValue::Boolean(flag) => ::core::option::Option::Some(*flag),
                ::ferrioxid::forms::BoundValue::Null | ::ferrioxid::forms::BoundValue::Unchanged => ::core::option::Option::None,
                _ => return ::core::result::Result::Err(invalid(#column)),
            }
        },
    };
    Ok(quote! {
        #ident: {
            fn invalid(name: &str) -> ::ferrioxid::forms::FormErrors {
                let mut errors = ::ferrioxid::forms::FormErrors::default();
                errors.push(name, "validation.invalid_type");
                errors
            }
            let #value = bound.iter().find(|field| field.name == #column).map(|field| &field.value);
            let Some(#value) = #value else {
                return ::core::result::Result::Err(invalid(#column));
            };
            #body
        }
    })
}

enum FieldKind {
    String,
    Integer,
    Boolean,
    OptionalString,
    OptionalInteger,
    OptionalBoolean,
}

fn type_kind(ty: &Type) -> Option<FieldKind> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    match segment.ident.to_string().as_str() {
        "String" => Some(FieldKind::String),
        "i64" => Some(FieldKind::Integer),
        "bool" => Some(FieldKind::Boolean),
        "Option" => {
            let PathArguments::AngleBracketed(args) = &segment.arguments else {
                return None;
            };
            let inner = args.args.first()?;
            let GenericArgument::Type(inner) = inner else {
                return None;
            };
            match type_kind(inner)? {
                FieldKind::String => Some(FieldKind::OptionalString),
                FieldKind::Integer => Some(FieldKind::OptionalInteger),
                FieldKind::Boolean => Some(FieldKind::OptionalBoolean),
                _ => None,
            }
        }
        _ => None,
    }
}

fn is_field_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_alphabetic() || char == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
        && name.len() <= 64
}
