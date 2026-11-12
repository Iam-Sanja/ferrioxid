//! Central CLI messages. English is the default developer-facing language.
//! German is an explicit legacy locale override, not inferred from app content.

pub fn line(key: &str) -> &'static str {
    line_for(&locale(), key)
}

fn line_for(locale: &str, key: &str) -> &'static str {
    match (locale, key) {
        ("de", "project_created") => "Projekt erzeugt:",
        ("de", "model_created") => "Modell erzeugt:",
        ("de", "dev_started") => "Entwicklungsserver für",
        ("de", "dev_stop_hint") => "Beenden mit Strg+C.",
        ("de", "dev_stopped") => "Entwicklungsserver beendet.",
        ("de", "dev_test_stopped") => "Entwicklungsserver nach Testfrist beendet.",
        ("de", "admin_assets_missing") => "Der Admin wird mit dem Rust-Binary ausgeliefert.",
        ("de", "user_created") => "Benutzer angelegt:",
        ("de", "flag_active") => "Aktiv: ja. Ein inaktives Konto erhält keine Sitzung.",
        ("de", "flag_staff_on") => "Staff: ja. Dieses Konto darf die Verwaltung öffnen.",
        ("de", "flag_staff_off") => {
            "Staff: nein. Ohne Staff bleibt die Verwaltung geschlossen."
        }
        ("de", "flag_super_on") => "Superuser: ja. Das ersetzt Staff nicht.",
        ("de", "flag_super_off") => "Superuser: nein.",
        ("de", "super_without_staff") => {
            "Hinweis: --superuser ohne --staff. Die Modellrechte gelten, der Admin-Zugang bleibt zu."
        }
        ("en", "project_created") => "Created project:",
        ("en", "model_created") => "Created model:",
        ("en", "dev_started") => "Development server for",
        ("en", "dev_stop_hint") => "Stop with Ctrl+C.",
        ("en", "dev_stopped") => "Development server stopped.",
        ("en", "dev_test_stopped") => "Development server stopped after the test limit.",
        ("de", "dev_no_migrate") => {
            "ferrioxid dev wendet keine Migrationen an und schreibt das Schema nicht."
        }
        ("en", "dev_no_migrate") => {
            "ferrioxid dev does not apply migrations and does not write the schema."
        }
        ("de", "port_busy") => {
            "Der Port ist belegt. Beenden Sie den anderen Prozess oder ändern Sie server.port. Es wird nichts migriert."
        }
        ("en", "port_busy") => {
            "The port is already in use. Stop the other process or change server.port. Nothing is migrated."
        }
        ("en", "admin_assets_missing") => "The admin ships inside the Rust binary.",
        ("en", "user_created") => "Created user:",
        ("en", "flag_active") => "Active: yes. An inactive account cannot sign in.",
        ("en", "flag_staff_on") => "Staff: yes. This account can open the admin.",
        ("en", "flag_staff_off") => "Staff: no. Without staff the admin stays closed.",
        ("en", "flag_super_on") => "Superuser: yes. This does not replace staff.",
        ("en", "flag_super_off") => "Superuser: no.",
        ("en", "super_without_staff") => {
            "Note: --superuser without --staff. Model rights apply, but the admin stays closed."
        }
        _ => "missing message",
    }
}

pub fn locale() -> String {
    if let Ok(value) = std::env::var("FERRIOXID_LOCALE") {
        if let Some(locale) = supported(&value) {
            return locale;
        }
    }
    // CLI messages must not inherit the application's UI language.
    // A project with default_locale = "de" still gets English CLI output.
    "en".to_owned()
}

fn supported(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "de" => Some("de".to_owned()),
        "en" => Some("en".to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::line_for;

    #[test]
    fn central_lines_exist_in_german_and_english() {
        assert_eq!(line_for("en", "model_created"), "Created model:");
        assert_eq!(line_for("de", "model_created"), "Modell erzeugt:");
        assert_eq!(line_for("fr", "model_created"), "missing message");
        assert!(line_for("de", "super_without_staff").contains("--superuser"));
        assert!(line_for("en", "flag_staff_off").contains("Without staff"));
        assert!(!line_for("de", "flag_super_on").contains("is_staff = true"));
    }
}
