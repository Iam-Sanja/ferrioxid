//! Stabiler Fehlervertrag. `code` ist sprachunabhängig, `detail` bleibt der bisherige Fallback.

pub fn problem_json(status: u16, title: &str, detail: &str) -> serde_json::Value {
    let (code, field, safe_detail) = classify(detail);
    let mut body = serde_json::json!({
        "type": "about:blank",
        "code": code,
        "title": title,
        "status": status,
        "detail": safe_detail,
        "params": coded_params(detail)
    });
    if let Some(name) = field {
        body["field"] = serde_json::Value::String(name);
    }
    body
}

fn classify(detail: &str) -> (&'static str, Option<String>, String) {
    let (field, message) = split_field(detail);
    if looks_sensitive(message) {
        return ("request.rejected", None, "Request rejected".to_owned());
    }
    let code = match message.split(':').next().unwrap_or(message) {
        "validation.required" => "validation.required",
        "validation.min_length" => "validation.min_length",
        "validation.max_length" => "validation.max_length",
        "validation.min_value" => "validation.min_value",
        "validation.max_value" => "validation.max_value",
        "validation.email" => "validation.email",
        "validation.slug" => "validation.slug",
        "validation.null" => "validation.null",
        "validation.choice" => "validation.choice",
        "validation.postal" => "validation.postal",
        "validation.invalid_type" => "validation.invalid_type",
        "validation.uuid" => "validation.uuid",
        "validation.unknown_field" => "validation.unknown_field",
        "validation.date" => "validation.date",
        "validation.time" => "validation.time",
        "validation.datetime_offset" => "validation.datetime_offset",
        "validation.json" => "validation.json",
        "validation.datetime" => "validation.datetime",
        _ => legacy_code(message, field.is_some()),
    };
    (code, field, detail.to_owned())
}

fn legacy_code(message: &str, has_field: bool) -> &'static str {
    match message {
        "Field is required" | "active is required" | "email and role are required" => {
            "validation.required"
        }
        _ if message.contains("validation.datetime_offset") => "validation.datetime_offset",
        _ if message.contains("validation.datetime") => "validation.datetime",
        _ if message.contains("validation.date") => "validation.date",
        _ if message.contains("validation.time") => "validation.time",
        "Password must contain 12 to 128 characters" => "validation.password",
        "Invalid email or password" => "auth.invalid_credentials",
        "Authentication required" => "auth.required",
        "Access denied" => "auth.forbidden",
        "csrf token is missing or invalid" => "auth.csrf",
        "origin is not allowed" => "auth.origin",
        "authentication is not configured" => "auth.not_configured",
        "Too many login attempts" => "auth.rate_limited",
        "request rejected" => "auth.rejected",
        "Resource not found" | "user was not found" => "resource.not_found",
        "Database operation failed" => "database.failed",
        "Internal server error" => "server.error",
        "object policy is not bound; call register_policy"
        | "object policy is not bound; call add_policy" => "policy.unbound",
        "create policy rejected the record" => "policy.create_rejected",
        "sort field is not allowed" => "list.sort",
        "filter field is not allowed" => "list.filter",
        "search is not available" => "list.search",
        "you cannot deactivate your own account" => "directory.self_deactivate",
        "you cannot remove your own staff access" => "directory.self_staff",
        "superuser changes are blocked" => "directory.superuser_blocked",
        "relation options require a registered admin model without an object policy" => {
            "list.relation"
        }
        _ if has_field => "validation.field",
        _ => "request.failed",
    }
}

fn coded_params(detail: &str) -> serde_json::Value {
    let (_, message) = split_field(detail);
    let Some((head, raw)) = message.split_once(':') else {
        return serde_json::json!({});
    };
    if raw.is_empty()
        || raw.len() > 8
        || !raw.chars().all(|char| char.is_ascii_digit())
        || head.contains(':')
    {
        return serde_json::json!({});
    }
    let key = match head {
        "validation.min_length" | "validation.min_value" => "min",
        "validation.max_length" | "validation.max_value" => "max",
        _ => return serde_json::json!({}),
    };
    serde_json::json!({ key: raw })
}

fn split_field(detail: &str) -> (Option<String>, &str) {
    let Some(rest) = detail.strip_prefix("field:") else {
        return (None, detail);
    };
    let Some((name, message)) = rest.split_once(':') else {
        return (None, detail);
    };
    if !valid_field(name) {
        return (None, detail);
    }
    (Some(name.to_owned()), message.trim())
}

fn valid_field(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn looks_sensitive(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    lower.len() > 240
        || lower.contains("password_hash")
        || lower.contains("select ")
        || lower.contains("insert ")
        || lower.contains("update ")
        || lower.contains("delete from")
        || lower.contains("stack backtrace")
}

#[cfg(test)]
mod tests {
    use super::problem_json;

    #[test]
    fn credential_failures_share_code_and_detail() {
        let unknown = problem_json(401, "Unauthorized", "Invalid email or password");
        let wrong = problem_json(401, "Unauthorized", "Invalid email or password");
        assert_eq!(unknown, wrong);
        assert_eq!(unknown["code"], "auth.invalid_credentials");
        assert_eq!(unknown["detail"], "Invalid email or password");
        assert!(unknown.get("field").is_none());
    }

    #[test]
    fn coded_model_errors_keep_the_detail_and_a_safe_param() {
        let body = problem_json(
            422,
            "Validation failed",
            "field:title: validation.min_length:3",
        );
        assert_eq!(body["code"], "validation.min_length");
        assert_eq!(body["field"], "title");
        assert_eq!(body["detail"], "field:title: validation.min_length:3");
        assert_eq!(body["params"]["min"], "3");
    }

    #[test]
    fn field_errors_keep_the_legacy_detail() {
        let body = problem_json(422, "Validation failed", "field:name: Field is required");
        assert_eq!(body["code"], "validation.required");
        assert_eq!(body["field"], "name");
        assert_eq!(body["detail"], "field:name: Field is required");
    }

    #[test]
    fn sql_is_not_returned() {
        let body = problem_json(
            500,
            "Internal server error",
            "SELECT password_hash FROM _ferrioxid_users",
        );
        assert_eq!(body["code"], "request.rejected");
        assert_eq!(body["detail"], "Request rejected");
    }
}
