//! Request extractors that answer with the Fexu problem document.

use axum::extract::{FromRequest, FromRequestParts, Path, Request};
use axum::http::request::Parts;
use axum::Json;
use fexu_core::FexuError;
use serde::de::DeserializeOwned;

use crate::listing::RawList;
use crate::ApiError;

pub(crate) struct ListQuery(pub RawList);

pub(crate) struct ApiJson<T>(pub T);

pub(crate) struct RecordId(pub uuid::Uuid);

impl<S> FromRequestParts<S> for ListQuery
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or("");
        parse_query(query).map(Self).map_err(ApiError)
    }
}

pub(crate) fn parse_query(query: &str) -> Result<RawList, FexuError> {
    let mut raw = RawList::default();
    if query.is_empty() {
        return Ok(raw);
    }
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = decode_query(key)
            .map_err(|_| FexuError::Validation("validation.invalid_type".into()))?;
        let value = decode_query(value)
            .map_err(|_| FexuError::Validation("validation.invalid_type".into()))?;
        match key.as_str() {
            "limit" => {
                raw.limit = Some(value.parse::<u64>().map_err(|_| {
                    FexuError::Validation("page size must be between 1 and 100".into())
                })?);
            }
            "cursor" => raw.cursor = Some(value),
            "sort" => {
                if raw.sort.is_some() {
                    return Err(FexuError::Validation("sort field is not allowed".into()));
                }
                raw.sort = Some(value);
            }
            "filter" => raw.filter.push(value),
            _ => {}
        }
    }
    Ok(raw)
}

fn decode_query(value: &str) -> Result<String, ()> {
    let mut out = Vec::new();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => out.push(b' '),
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(());
                }
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).map_err(|_| ())?;
                let byte = u8::from_str_radix(hex, 16).map_err(|_| ())?;
                out.push(byte);
                index += 2;
            }
            byte => out.push(byte),
        }
        index += 1;
    }
    String::from_utf8(out).map_err(|_| ())
}

impl<S> FromRequestParts<S> for RecordId
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Path::<uuid::Uuid>::from_request_parts(parts, state).await {
            Ok(Path(id)) => Ok(Self(id)),
            Err(_) => Err(ApiError(FexuError::Validation("validation.uuid".into()))),
        }
    }
}

impl<S, T> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(request, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => {
                let text = rejection.body_text();
                let code = if text.contains("unknown field") {
                    "validation.unknown_field"
                } else {
                    "validation.invalid_type"
                };
                Err(ApiError(FexuError::Validation(code.into())))
            }
        }
    }
}
