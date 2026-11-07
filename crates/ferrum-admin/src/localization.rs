//! Feste Kataloge für die HTML-Oberfläche. Locale-Werte werden nie zu Dateipfaden.

use serde_json::{Map, Value};

pub fn normalize(requested: &str, default_locale: &str, fallback_locale: &str) -> String {
    match requested {
        "de" | "en" => requested.to_owned(),
        _ if default_locale == "de" || default_locale == "en" => default_locale.to_owned(),
        _ if fallback_locale == "de" || fallback_locale == "en" => fallback_locale.to_owned(),
        _ => "en".to_owned(),
    }
}

pub fn catalog(locale: &str) -> Value {
    let mut map = Map::new();
    for (key, german, english) in STRINGS {
        map.insert(
            (*key).to_owned(),
            Value::String(if locale == "de" {
                (*german).to_owned()
            } else {
                (*english).to_owned()
            }),
        );
    }
    Value::Object(map)
}

const STRINGS: &[(&str, &str, &str)] = &[
    ("login", "Anmelden", "Sign in"),
    ("email", "E-Mail", "Email"),
    ("password", "Passwort", "Password"),
    (
        "invalid_credentials",
        "E-Mail oder Passwort ist ungültig.",
        "Invalid email or password.",
    ),
    (
        "rate_limited",
        "Zu viele Versuche. Bitte später erneut versuchen.",
        "Too many attempts. Try again later.",
    ),
    (
        "csrf",
        "Die Sicherheitsprüfung ist fehlgeschlagen.",
        "The security check failed.",
    ),
    ("dashboard", "Übersicht", "Dashboard"),
    ("welcome", "Willkommen", "Welcome"),
    ("models", "Modelle", "Models"),
    ("users", "Benutzer", "Users"),
    ("roles", "Rollen", "Roles"),
    ("permissions", "Berechtigungen", "Permissions"),
    ("directory", "Verzeichnis", "Directory"),
    ("logout", "Abmelden", "Sign out"),
    ("search", "Suchen", "Search"),
    ("filter", "Filtern", "Filter"),
    ("save", "Speichern", "Save"),
    ("cancel", "Abbrechen", "Cancel"),
    ("create", "Anlegen", "Create"),
    ("edit", "Bearbeiten", "Edit"),
    ("delete", "Löschen", "Delete"),
    ("back", "Zurück", "Back"),
    ("detail", "Details", "Details"),
    ("empty", "Keine Einträge.", "No records."),
    (
        "no_models",
        "Keine Modelle für dieses Konto.",
        "No models are available for this account.",
    ),
    (
        "forbidden",
        "Diese Aktion ist nicht erlaubt.",
        "This action is not allowed.",
    ),
    (
        "not_found",
        "Der Eintrag wurde nicht gefunden.",
        "The record was not found.",
    ),
    (
        "staff_required",
        "Die Verwaltung verlangt einen Staff-Zugang.",
        "Administration requires staff access.",
    ),
    (
        "confirm_delete",
        "Dieser Eintrag wird dauerhaft gelöscht.",
        "This record will be deleted permanently.",
    ),
    (
        "notice_created",
        "Der Eintrag wurde angelegt.",
        "The record was created.",
    ),
    (
        "notice_saved",
        "Die Änderungen wurden gespeichert.",
        "The changes were saved.",
    ),
    (
        "notice_deleted",
        "Der Eintrag wurde gelöscht.",
        "The record was deleted.",
    ),
    (
        "notice_updated",
        "Die Änderung wurde gespeichert.",
        "The change was saved.",
    ),
    ("yes", "Ja", "Yes"),
    ("no", "Nein", "No"),
    ("active", "Aktiv", "Active"),
    ("inactive", "Inaktiv", "Inactive"),
    ("staff", "Staff", "Staff"),
    ("superuser", "Superuser", "Superuser"),
    ("actions", "Aktionen", "Actions"),
    ("required", "Pflichtfeld", "Required"),
    ("optional", "Optional", "Optional"),
    ("readonly", "Nur lesen", "Read only"),
    ("none", "Keine", "None"),
    ("total", "Gesamt", "Total"),
    ("previous", "Zurück", "Previous"),
    ("next", "Weiter", "Next"),
    ("page", "Seite", "Page"),
    ("language", "Sprache", "Language"),
    ("theme", "Darstellung", "Theme"),
    ("light", "Hell", "Light"),
    ("dark", "Dunkel", "Dark"),
    ("system", "System", "System"),
    ("apply", "Übernehmen", "Apply"),
    ("assign", "Zuweisen", "Assign"),
    ("revoke", "Entziehen", "Revoke"),
    ("grant", "Gewähren", "Grant"),
    ("role", "Rolle", "Role"),
    ("add_role", "Rolle anlegen", "Create role"),
    ("permission_code", "Berechtigungscode", "Permission code"),
    ("deactivate", "Deaktivieren", "Deactivate"),
    ("activate", "Aktivieren", "Activate"),
    ("remove_staff", "Staff entziehen", "Remove staff"),
    ("make_staff", "Staff geben", "Grant staff"),
    ("blocked", "Gesperrt", "Blocked"),
    ("open", "Öffnen", "Open"),
    ("new_record", "Neuer Eintrag", "New record"),
    ("results", "Treffer", "Results"),
];

pub fn notice(locale: &str, code: &str) -> Option<String> {
    let catalog = catalog(locale);
    let key = match code {
        "created" => "notice_created",
        "saved" => "notice_saved",
        "deleted" => "notice_deleted",
        "updated" => "notice_updated",
        _ => return None,
    };
    catalog
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

pub fn validation_text(locale: &str, detail: &str) -> String {
    let german = locale == "de";
    if detail.contains("must not be blank") {
        return if german {
            "Pflichtfeld.".into()
        } else {
            "Required.".into()
        };
    }
    if detail.contains("must have at least") {
        return if german {
            "Der Wert ist zu kurz.".into()
        } else {
            "The value is too short.".into()
        };
    }
    if detail.contains("may have at most") {
        return if german {
            "Der Wert ist zu lang.".into()
        } else {
            "The value is too long.".into()
        };
    }
    if detail.contains("not writable") {
        return if german {
            "Dieses Feld kann nicht gesetzt werden.".into()
        } else {
            "This field cannot be set.".into()
        };
    }
    if detail == "Invalid email or password" {
        return catalog(locale)["invalid_credentials"]
            .as_str()
            .unwrap_or(detail)
            .to_owned();
    }
    detail.to_owned()
}
