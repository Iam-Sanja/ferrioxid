use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, LitStr};

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = input.ident;
    let Data::Enum(data) = input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "FexuChoice only supports enums",
        ));
    };
    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            name,
            "a choice enum needs at least one variant",
        ));
    }
    let mut values = Vec::new();
    let mut stored_arms = Vec::new();
    let mut parse_arms = Vec::new();
    let mut seen = Vec::new();
    for variant in data.variants {
        if !variant.fields.is_empty() {
            return Err(syn::Error::new_spanned(
                variant.ident,
                "choice variants cannot hold data",
            ));
        }
        let mut stored: Option<String> = None;
        let mut label_de: Option<String> = None;
        let mut label_en: Option<String> = None;
        for attr in &variant.attrs {
            if !attr.path().is_ident("choice") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("value") {
                    stored = Some(meta.value()?.parse::<LitStr>()?.value());
                } else if meta.path.is_ident("de") {
                    label_de = Some(meta.value()?.parse::<LitStr>()?.value());
                } else if meta.path.is_ident("en") {
                    label_en = Some(meta.value()?.parse::<LitStr>()?.value());
                } else {
                    return Err(meta.error("supported choice attributes are value, de and en"));
                }
                Ok(())
            })?;
        }
        let ident = variant.ident;
        let stored = stored.unwrap_or_else(|| {
            let raw = ident.to_string();
            let mut snake = String::new();
            for (index, character) in raw.chars().enumerate() {
                if character.is_uppercase() {
                    if index > 0 {
                        snake.push('_');
                    }
                    snake.push(character.to_ascii_lowercase());
                } else {
                    snake.push(character);
                }
            }
            snake
        });
        if !is_choice_token(&stored) || seen.contains(&stored) {
            return Err(syn::Error::new_spanned(
                &ident,
                "choice values must be unique lowercase identifiers of at most 64 characters",
            ));
        }
        seen.push(stored.clone());
        let de = label_de.unwrap_or_else(|| stored.clone());
        let en = label_en.unwrap_or_else(|| stored.clone());
        values.push(quote! {
            ::fexu::core::ChoiceValue {
                value: #stored,
                label_de: #de,
                label_en: #en,
            }
        });
        stored_arms.push(quote! { Self::#ident => #stored });
        parse_arms.push(quote! { #stored => ::core::option::Option::Some(Self::#ident) });
    }
    Ok(quote! {
        impl ::fexu::core::FexuChoice for #name {
            const VALUES: &'static [::fexu::core::ChoiceValue] = &[#(#values),*];

            fn stored(self) -> &'static str {
                match self {
                    #(#stored_arms,)*
                }
            }

            fn from_stored(value: &str) -> ::core::option::Option<Self> {
                match value {
                    #(#parse_arms,)*
                    _ => ::core::option::Option::None,
                }
            }
        }

        impl ::fexu::serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
            where
                S: ::fexu::serde::Serializer,
            {
                serializer.serialize_str(::fexu::core::FexuChoice::stored(*self))
            }
        }

        impl<'de> ::fexu::serde::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
            where
                D: ::fexu::serde::Deserializer<'de>,
            {
                let value = <String as ::fexu::serde::Deserialize>::deserialize(deserializer)?;
                ::fexu::core::FexuChoice::from_stored(&value).ok_or_else(|| {
                    <D::Error as ::fexu::serde::de::Error>::custom(format!(
                        "unknown choice {value}"
                    ))
                })
            }
        }
    })
}

fn is_choice_token(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(character) if character.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && value.len() <= 64
}
