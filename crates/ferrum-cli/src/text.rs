//! Zentrale CLI-Texte. Befehlsnamen und skriptbare Migrationszeilen bleiben Englisch beziehungsweise stabil.

use ferrum_migrations::discover_project;

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
        ("de", "admin_assets_missing") => "Admin-Assets fehlen. Starte npm run build.",
        ("en", "project_created") => "Created project:",
        ("en", "model_created") => "Created model:",
        ("en", "dev_started") => "Development server for",
        ("en", "dev_stop_hint") => "Stop with Ctrl+C.",
        ("en", "dev_stopped") => "Development server stopped.",
        ("en", "dev_test_stopped") => "Development server stopped after the test limit.",
        ("en", "admin_assets_missing") => "Admin assets are missing. Starting npm run build.",
        _ => "missing message",
    }
}

pub fn locale() -> String {
    if let Ok(value) = std::env::var("FERRUM_LOCALE") {
        if let Some(locale) = supported(&value) {
            return locale;
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Ok(project) = discover_project(&cwd) {
            if let Some(locale) = supported(&project.default_locale) {
                return locale;
            }
        }
    }
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
    }
}
