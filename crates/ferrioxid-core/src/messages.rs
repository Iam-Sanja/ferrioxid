//! Anzeigetexte für Validierung. Codes bleiben stabil; nur diese Schicht übersetzt.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldOverride {
    pub field: &'static str,
    pub code: &'static str,
    pub de: &'static str,
    pub en: &'static str,
}

pub fn message_text(locale: &str, code: &str) -> &'static str {
    let german = locale == "de";
    match (german, code) {
        (true, "validation.required") => "Dieses Feld ist erforderlich.",
        (false, "validation.required") => "This field is required.",
        (true, "validation.min_length") => "Der Text ist zu kurz.",
        (false, "validation.min_length") => "The text is too short.",
        (true, "validation.max_length") => "Der Text ist zu lang.",
        (false, "validation.max_length") => "The text is too long.",
        (true, "validation.email") => "Die E-Mail-Adresse ist ungültig.",
        (false, "validation.email") => "The email address is invalid.",
        (true, "validation.url") => "Die Adresse muss mit http:// oder https:// beginnen.",
        (false, "validation.url") => "The address must start with http:// or https://.",
        (true, "validation.invalid_type") => "Der Wert hat den falschen Typ.",
        (false, "validation.invalid_type") => "The value has the wrong type.",
        (true, "validation.min_value") => "Der Wert ist zu klein.",
        (false, "validation.min_value") => "The value is too small.",
        (true, "validation.max_value") => "Der Wert ist zu groß.",
        (false, "validation.max_value") => "The value is too large.",
        (true, "validation.uuid") => "Die Kennung ist ungültig.",
        (false, "validation.uuid") => "The identifier is invalid.",
        (true, "validation.unknown_field") => "Dieses Feld ist nicht erlaubt.",
        (false, "validation.unknown_field") => "This field is not allowed.",
        (true, "validation.readonly") => "Dieses Feld kann nicht geändert werden.",
        (false, "validation.readonly") => "This field cannot be changed.",
        (true, "validation.duplicate") => "Dieses Feld wurde mehrfach gesendet.",
        (false, "validation.duplicate") => "This field was submitted more than once.",
        (true, "validation.upload_name") => "Der Dateiname ist ungültig.",
        (false, "validation.upload_name") => "The file name is invalid.",
        (true, "validation.upload_size") => "Die Datei ist zu groß.",
        (false, "validation.upload_size") => "The file is too large.",
        (true, "validation.upload_type") => "Dieser Dateityp ist nicht erlaubt.",
        (false, "validation.upload_type") => "This file type is not allowed.",
        (true, "validation.upload") => "Die Datei konnte nicht gespeichert werden.",
        (false, "validation.upload") => "The file could not be stored.",
        (true, "validation.formset_size") => "Die Anzahl der Formulare ist ungültig.",
        (false, "validation.formset_size") => "The number of forms is invalid.",
        (true, "form.formset") => "Das Formularset ist ungültig.",
        (false, "form.formset") => "The formset is invalid.",
        (true, "validation.mismatch") => {
            "Name und E-Mail dürfen nicht denselben lokalen Teil haben."
        }
        (false, "validation.mismatch") => "The name and email must not share the same local part.",
        (true, "validation.choice") => "Wählen Sie einen der vorgegebenen Werte.",
        (false, "validation.choice") => "Choose one of the listed values.",
        (true, "validation.postal") => "Die Postleitzahl muss fünf Ziffern haben.",
        (false, "validation.postal") => "The postal code must contain five digits.",
        (true, "validation.slug") => "Der Kurzname ist ungültig.",
        (false, "validation.slug") => "The slug is invalid.",
        (true, "validation.null") => "Dieses Feld darf nicht leer sein.",
        (false, "validation.null") => "This field must not be empty.",
        (true, "validation.date") => "Das Datum ist ungültig.",
        (false, "validation.date") => "The date is invalid.",
        (true, "validation.time") => "Die Uhrzeit ist ungültig.",
        (false, "validation.time") => "The time is invalid.",
        (true, "validation.datetime") => "Der Zeitpunkt ist ungültig.",
        (false, "validation.datetime") => "The timestamp is invalid.",
        (true, "validation.datetime_offset") => "Der Zeitpunkt braucht ein Offset.",
        (false, "validation.datetime_offset") => "The timestamp needs an offset.",
        (true, "form.csrf") => "Die Sicherheitsprüfung ist fehlgeschlagen.",
        (false, "form.csrf") => "The security check failed.",
        (true, "form.thanks") => "Danke. Die Nachricht wurde geprüft und nicht versendet.",
        (false, "form.thanks") => "Thank you. The message was checked and was not sent.",
        (true, "request.rejected") => "Die Anfrage wurde abgelehnt.",
        (false, "request.rejected") => "The request was rejected.",
        _ => "missing message",
    }
}

pub fn message_for(
    locale: &str,
    field: &str,
    code: &str,
    params: &[(&str, &str)],
    overrides: &[FieldOverride],
) -> String {
    let german = locale == "de";
    let template = overrides
        .iter()
        .find(|item| item.field == field && item.code == code)
        .map(|item| if german { item.de } else { item.en })
        .unwrap_or_else(|| message_text(locale, code));
    apply_params(template, params)
}

/// Übersetzt einen Modell- oder Formularfehler. Unbekannte Texte bleiben erhalten und werden escaped.
/// SQL, Passwortfelder und Stacktraces werden nicht angezeigt.
pub fn localize_detail(locale: &str, detail: &str) -> String {
    if sensitive(detail) {
        return message_text(locale, "request.rejected").to_owned();
    }
    let message = field_message(detail);
    if let Some(code) = known_code(message) {
        let mut params = Vec::new();
        if let Some((key, value)) = limit_param(message) {
            params.push((key, value));
        }
        let borrowed: Vec<(&str, &str)> = params
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        return message_for(locale, "", code, &borrowed, &[]);
    }
    message.to_owned()
}

fn known_code(message: &str) -> Option<&'static str> {
    let head = message.split(':').next().unwrap_or(message);
    let code = match head {
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
        "validation.readonly" => "validation.readonly",
        "validation.duplicate" => "validation.duplicate",
        "validation.upload_name" => "validation.upload_name",
        "validation.upload_size" => "validation.upload_size",
        "validation.upload_type" => "validation.upload_type",
        "validation.upload" => "validation.upload",
        "validation.formset_size" => "validation.formset_size",
        "validation.date" => "validation.date",
        "validation.time" => "validation.time",
        "validation.datetime" => "validation.datetime",
        "validation.datetime_offset" => "validation.datetime_offset",
        _ if message.contains("must not be blank") || message.contains("cannot be null") => {
            "validation.required"
        }
        _ if message.contains("must have at least") => "validation.min_length",
        _ if message.contains("may have at most") => "validation.max_length",
        _ if message.contains("must be >=") => "validation.min_value",
        _ if message.contains("must be <=") => "validation.max_value",
        _ if message.contains("must be an email") => "validation.email",
        _ if message.contains("lowercase slug") => "validation.slug",
        _ => return None,
    };
    Some(code)
}

fn field_message(detail: &str) -> &str {
    let Some(rest) = detail.strip_prefix("field:") else {
        return detail;
    };
    rest.split_once(':')
        .map(|(_, message)| message.trim())
        .unwrap_or(detail)
}

fn limit_param(message: &str) -> Option<(&'static str, String)> {
    let (head, raw) = message.split_once(':')?;
    if !raw.chars().all(|char| char.is_ascii_digit()) || raw.is_empty() || raw.len() > 8 {
        return None;
    }
    let key = match head {
        "validation.min_length" | "validation.min_value" => "min",
        "validation.max_length" | "validation.max_value" => "max",
        _ => return None,
    };
    Some((key, raw.to_owned()))
}

fn sensitive(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    lower.len() > 240
        || lower.contains("password_hash")
        || lower.contains("stack backtrace")
        || lower.contains("select ")
        || lower.contains("insert ")
        || lower.contains("update ")
        || lower.contains("delete from")
}

fn apply_params(template: &str, params: &[(&str, &str)]) -> String {
    let mut output = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let Some(end) = rest[start + 1..].find('}') else {
            output.push_str(&rest[start..]);
            return output;
        };
        let key = &rest[start + 1..start + 1 + end];
        if valid_key(key) {
            if let Some((_, value)) = params.iter().find(|(name, _)| *name == key) {
                output.push_str(&escape_text(value));
            }
        }
        rest = &rest[start + 1 + end + 1..];
    }
    output.push_str(rest);
    output
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 16
        && key
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '_')
}

fn escape_text(value: &str) -> String {
    if value.chars().count() > 32
        || !value
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, ' ' | '.' | '_' | '-'))
    {
        return String::new();
    }
    escape_html(value)
}

fn escape_html(value: &str) -> String {
    let mut output = String::new();
    for char in value.chars() {
        match char {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(char),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{localize_detail, message_for, message_text, FieldOverride};

    #[test]
    fn german_and_english_catalogs_cover_the_same_rules() {
        for code in [
            "validation.required",
            "validation.invalid_type",
            "validation.min_length",
            "validation.max_length",
            "validation.min_value",
            "validation.max_value",
            "validation.choice",
            "validation.postal",
        ] {
            assert_ne!(message_text("de", code), message_text("en", code));
            assert_ne!(message_text("de", code), "missing message");
            assert_eq!(message_text("fr", code), message_text("en", code));
        }
    }

    #[test]
    fn missing_codes_and_overrides_are_deterministic() {
        assert_eq!(message_text("de", "validation.missing"), "missing message");
        let overrides = [FieldOverride {
            field: "postal_code",
            code: "validation.postal",
            de: "Genau fünf Ziffern.",
            en: "Exactly five digits.",
        }];
        assert_eq!(
            message_for("de", "postal_code", "validation.postal", &[], &overrides),
            "Genau fünf Ziffern."
        );
        assert_eq!(
            message_for("en", "city", "validation.postal", &[], &overrides),
            "The postal code must contain five digits."
        );
        assert_eq!(
            message_for(
                "de",
                "title",
                "validation.min_length",
                &[("min", "3")],
                &[FieldOverride {
                    field: "title",
                    code: "validation.min_length",
                    de: "Mindestens {min} Zeichen.",
                    en: "At least {min} characters.",
                }]
            ),
            "Mindestens 3 Zeichen."
        );
    }

    #[test]
    fn parameter_values_cannot_inject_markup_or_further_placeholders() {
        let overrides = [FieldOverride {
            field: "title",
            code: "validation.min_length",
            de: "Grenze {min}.",
            en: "Limit {min}.",
        }];
        assert_eq!(
            message_for(
                "en",
                "title",
                "validation.min_length",
                &[("min", "<script>{max}</script>")],
                &overrides
            ),
            "Limit ."
        );
        assert_eq!(
            message_for(
                "de",
                "title",
                "validation.min_length",
                &[("min", "3{max}")],
                &overrides
            ),
            "Grenze ."
        );
    }

    #[test]
    fn model_details_localize_without_sql() {
        assert_eq!(
            localize_detail("de", "field:title: validation.min_length:3"),
            "Der Text ist zu kurz."
        );
        assert_eq!(
            localize_detail("en", "title must not be blank"),
            "This field is required."
        );
        assert_eq!(
            localize_detail("de", "SELECT password_hash FROM users"),
            "Die Anfrage wurde abgelehnt."
        );
        assert!(!localize_detail("en", "SELECT password_hash FROM users").contains("SELECT"));
    }
}
