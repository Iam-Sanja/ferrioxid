//! Gemeinsame Feldregeln für Formulare. Modellmakros behalten ihre eigenen Meldungen.

pub fn text_code(
    value: &str,
    required: bool,
    min_length: Option<usize>,
    max_length: Option<usize>,
    email: bool,
    url: bool,
) -> Option<&'static str> {
    if value.is_empty() {
        return required.then_some("validation.required");
    }
    let length = value.chars().count();
    if min_length.is_some_and(|min| length < min) {
        return Some("validation.min_length");
    }
    if max_length.is_some_and(|max| length > max) {
        return Some("validation.max_length");
    }
    if email && !is_email(value) {
        return Some("validation.email");
    }
    if url && !is_http_url(value) {
        return Some("validation.url");
    }
    None
}

pub fn parse_i64(
    value: &str,
    min_value: Option<i64>,
    max_value: Option<i64>,
) -> Result<i64, &'static str> {
    if value.is_empty() {
        return Err("validation.required");
    }
    let parsed: i64 = value.parse().map_err(|_| "validation.invalid_type")?;
    if min_value.is_some_and(|min| parsed < min) {
        return Err("validation.min_value");
    }
    if max_value.is_some_and(|max| parsed > max) {
        return Err("validation.max_value");
    }
    Ok(parsed)
}

pub fn parse_bool(value: &str) -> Result<bool, &'static str> {
    match value {
        "true" | "1" | "on" => Ok(true),
        "false" | "0" | "off" => Ok(false),
        "" => Err("validation.required"),
        _ => Err("validation.invalid_type"),
    }
}

pub fn parse_uuid(value: &str) -> Result<uuid::Uuid, &'static str> {
    if value.is_empty() {
        return Err("validation.required");
    }
    uuid::Uuid::parse_str(value).map_err(|_| "validation.uuid")
}

pub fn is_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && domain.len() <= 253
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !value.contains("..")
        && !value.contains(' ')
        && local
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '_' | '+' | '-'))
        && domain
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '-'))
}

pub fn is_http_url(value: &str) -> bool {
    if value.len() > 2048 {
        return false;
    }
    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"));
    let Some(rest) = rest else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty() && !host.contains(' ') && host.contains('.')
}

#[cfg(test)]
mod tests {
    use super::{is_email, parse_i64, text_code};

    #[test]
    fn text_rules_use_stable_codes() {
        assert_eq!(
            text_code("", true, None, None, false, false),
            Some("validation.required")
        );
        assert_eq!(
            text_code("ab", true, Some(3), None, false, false),
            Some("validation.min_length")
        );
        assert_eq!(
            text_code("abcd", true, None, Some(3), false, false),
            Some("validation.max_length")
        );
        assert_eq!(
            text_code("a@b", true, None, None, true, false),
            Some("validation.email")
        );
        assert!(is_email("ada@example.com"));
        assert_eq!(
            parse_i64("nope", None, None),
            Err("validation.invalid_type")
        );
        assert_eq!(parse_i64("1", Some(2), None), Err("validation.min_value"));
    }
}
