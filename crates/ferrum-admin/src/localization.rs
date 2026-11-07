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
    (
        "save_continue",
        "Speichern und weiter bearbeiten",
        "Save and continue editing",
    ),
    (
        "save_add",
        "Speichern und weiteren Eintrag erstellen",
        "Save and add another",
    ),
    ("pick_more", "Weiter suchen", "Search further"),
    ("pick_heading", "Eintrag wählen", "Choose a record"),
    (
        "pick_empty",
        "Kein passender Eintrag.",
        "No matching record.",
    ),
    ("pick_back", "Zurück zum Formular", "Back to the form"),
    (
        "invalid_relation",
        "Wählen Sie einen sichtbaren Eintrag.",
        "Choose a visible record.",
    ),
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
    ("no_staff", "Kein Staff", "Not staff"),
    ("superuser", "Superuser", "Superuser"),
    ("no_superuser", "Kein Superuser", "Not a superuser"),
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
    ("skip", "Zum Inhalt", "Skip to content"),
    ("navigation", "Navigation", "Navigation"),
    ("breadcrumb", "Pfad", "Breadcrumb"),
    ("reset", "Zurücksetzen", "Reset"),
    ("error", "Fehler", "Error"),
    ("sort_asc", "aufsteigend", "ascending"),
    ("sort_desc", "absteigend", "descending"),
    (
        "hint_select",
        "Wählen Sie einen vorhandenen Eintrag.",
        "Choose an existing record.",
    ),
    (
        "hint_flags",
        "Staff öffnet die Verwaltung. Superuser ersetzt Staff nicht.",
        "Staff opens administration. Superuser does not replace staff.",
    ),
    (
        "hint_role",
        "Nur vorhandene Rollen. Eine neue Rolle entsteht auf der Rollenseite.",
        "Existing roles only. Create a new role on the roles page.",
    ),
    (
        "hint_role_name",
        "Buchstaben, Ziffern, Punkt oder Unterstrich.",
        "Letters, digits, dot, or underscore.",
    ),
    (
        "hint_permission",
        "Buchstaben, Ziffern, Punkt oder Unterstrich. Beispiel: notes.read",
        "Letters, digits, dot, or underscore. Example: notes.read",
    ),
    (
        "confirm_check",
        "Ich bestätige diese Änderung.",
        "I confirm this change.",
    ),
    ("assign_heading", "Rolle zuweisen", "Assign role"),
    (
        "no_roles",
        "Es gibt noch keine Rolle.",
        "There is no role yet.",
    ),
    ("existing_roles", "Vorhandene Rollen", "Existing roles"),
    (
        "unknown_role",
        "Diese Rolle gibt es nicht. Legen Sie sie zuerst an.",
        "That role does not exist. Create it first.",
    ),
    ("choose", "Bitte wählen", "Choose"),
    ("add_user", "Benutzer anlegen", "Add user"),
    (
        "password_confirm",
        "Passwort wiederholen",
        "Confirm password",
    ),
    (
        "password_rules",
        "12 bis 128 Zeichen. Das Passwort wird nicht angezeigt.",
        "12 to 128 characters. The password is not shown.",
    ),
    ("password_heading", "Passwort setzen", "Set password"),
    (
        "password_mismatch",
        "Die Passwörter stimmen nicht überein.",
        "The passwords do not match.",
    ),
    (
        "confirm_required",
        "Diese Aktion braucht eine Bestätigung.",
        "This action requires confirmation.",
    ),
    (
        "confirm_invalid",
        "Die Bestätigung ist ungültig.",
        "The confirmation is invalid.",
    ),
    ("missing_relation", "Nicht vorhanden", "Missing"),
    ("hidden_relation", "Nicht sichtbar", "Not visible"),
    ("all", "Alle", "All"),
    ("view", "Anzeigen", "View"),
    ("manage_roles", "Rollen", "Roles"),
    (
        "email_taken",
        "Diese E-Mail ist bereits vergeben.",
        "That email is already registered.",
    ),
    (
        "password_invalid",
        "Das Passwort muss 12 bis 128 Zeichen haben.",
        "The password must contain 12 to 128 characters.",
    ),
    ("no_permissions", "Keine Berechtigungen.", "No permissions."),
    (
        "user_saved",
        "Der Benutzer wurde gespeichert.",
        "The user was saved.",
    ),
    ("confirm", "Bestätigen", "Confirm"),
    ("history", "Historie", "History"),
    ("bulk_page", "Gesamte Seite", "Entire page"),
    ("bulk_action", "Aktion", "Action"),
    ("bulk_run", "Ausführen", "Run"),
    ("select", "Auswählen", "Select"),
    ("bulk_confirm", "Auswahl bestätigen", "Confirm selection"),
    ("bulk_count", "Betroffene Datensätze", "Affected records"),
    (
        "bulk_empty",
        "Keine Datensätze ausgewählt.",
        "No records selected.",
    ),
    (
        "bulk_denied",
        "Die Auswahl enthält einen nicht erlaubten Datensatz. Es wurde nichts geändert.",
        "The selection contains a record that is not allowed. Nothing was changed.",
    ),
    (
        "bulk_failed",
        "Die Aktion wurde zurückgerollt. Es wurde nichts geändert.",
        "The action was rolled back. Nothing was changed.",
    ),
    ("inline_add", "Hinzufügen", "Add"),
    (
        "inline_empty",
        "Keine verknüpften Einträge.",
        "No related records.",
    ),
    ("inline_open", "Öffnen", "Open"),
    ("audit_empty", "Keine Einträge.", "No entries."),
    ("when", "Zeitpunkt", "When"),
    ("actor", "Akteur", "Actor"),
    ("outcome", "Ergebnis", "Outcome"),
    ("details", "Details", "Details"),
    ("record", "Datensatz", "Record"),
    ("succeeded", "Erfolgreich", "Succeeded"),
    ("denied", "Verweigert", "Denied"),
];

pub fn notice(locale: &str, code: &str) -> Option<String> {
    let catalog = catalog(locale);
    let key = match code {
        "created" => "notice_created",
        "saved" => "notice_saved",
        "deleted" => "notice_deleted",
        "updated" => "notice_updated",
        "changed" => "notice_updated",
        _ => return None,
    };
    catalog
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

pub fn bulk_notice(locale: &str, count: u32) -> String {
    if locale == "de" {
        format!("{count} Datensätze wurden geändert.")
    } else {
        format!("{count} records were changed.")
    }
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
