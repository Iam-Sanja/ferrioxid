//! Shared contracts, independent of HTTP and database implementations.

pub mod choice;
pub mod decimal;
mod messages;
mod problem;
pub mod temporal;
mod validation;

pub use choice::{choice_is_known, choice_label, ChoiceValue, FerrioxidChoice};
pub use decimal::{
    decimal_sql_literal, decode_decimal, encode_decimal, format_decimal, format_decimal_text,
    parse_decimal, prepare_decimal, validate_decimal_meta, DecimalError,
};
pub use messages::{localize_detail, message_for, message_text, FieldOverride};
pub use problem::problem_json;
pub use rust_decimal::Decimal;
pub use temporal::{
    hierarchy_bound, parse_date, parse_datetime_local, parse_datetime_utc, parse_time, DateBound,
};
pub use validation::{is_email, is_http_url, parse_bool, parse_i64, parse_uuid, text_code};

#[derive(Debug, Clone, Copy)]
pub struct ModelDescriptor {
    pub name: &'static str,
    pub table: &'static str,
    pub api_path: &'static str,
    /// Explicit opt-in for unauthenticated access (demo only).
    pub allow_anonymous: bool,
    /// `None` verweigert den Zugriff, sofern `allow_anonymous` nicht gesetzt ist.
    /// `Some("authenticated")` verlangt eine aktive Sitzung.
    /// Jeder andere Wert ist der Ressourcenname für `{name}.read|create|update|delete`.
    pub permission: Option<&'static str>,
    /// Objektregeln liefern einen `QueryScope`. Ohne gebundene Policy bleiben die Routen gesperrt.
    pub requires_object_policy: bool,
}

/// Vergleichswert eines Query-Scopes. Er stammt aus der Policy, nicht aus der HTTP-Anfrage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeValue {
    Text(String),
    Uuid(uuid::Uuid),
    BigInt(i64),
    Bool(bool),
}

/// Datenbankfilter einer Objektregel. `None` liefert keine Zeile. `All` nur, wenn die Policy das ausdrücklich sagt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryScope {
    All,
    None,
    Equals {
        column: &'static str,
        value: ScopeValue,
    },
}

/// Benannter Formularabschnitt. Die Feldreihenfolge bleibt die Struct-Reihenfolge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminFieldset {
    pub name: &'static str,
    pub description: &'static str,
    pub optional: bool,
}

/// Admin-Feld. Die Werte stammen aus demselben Modell wie das Schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminFieldMeta {
    pub name: &'static str,
    pub label: &'static str,
    pub list_display: bool,
    pub list_link: bool,
    pub searchable: bool,
    pub sortable: bool,
    pub filterable: bool,
    pub readonly: bool,
    pub hidden: bool,
    /// Werte dieses Feldes dürfen weder in Entwürfen noch in der Historie stehen.
    pub sensitive: bool,
    /// Leer, wenn das Modell keine Fieldsets deklariert.
    pub fieldset: &'static str,
}

/// Has-Many-Abschnitt. `form` speichert Eltern und Kinder in einer Transaktion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminInline {
    pub name: &'static str,
    pub child_table: &'static str,
    pub column: &'static str,
    /// Leer: die Oberfläche verwendet das Label des Kindmodells.
    pub label: &'static str,
    /// Leer: alle schreibbaren Felder außer dem Eltern-Fremdschlüssel.
    pub fields: &'static [&'static str],
    pub readonly_fields: &'static [&'static str],
    pub min_count: u16,
    /// `0` ohne `form` bedeutet: das gemeinsame Formular ist aus.
    pub max_count: u16,
    pub extra: u16,
    pub can_add: bool,
    pub can_change: bool,
    pub can_delete: bool,
    pub ordering: Option<&'static str>,
    pub form: bool,
}

/// Admin-Freigabe eines Modells. `None` bei `admin_meta` bedeutet: nicht im Admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminModelMeta {
    pub key: &'static str,
    pub label: &'static str,
    /// `None`: nur ein aktiver Superuser. Sonst `{permission}.read|create|update|delete`.
    pub permission: Option<&'static str>,
    pub fields: &'static [AdminFieldMeta],
    /// `Some("-name")` sortiert absteigend. `None` lässt die Liste bei `id` aufsteigend.
    pub ordering: Option<&'static str>,
    /// HTML-Seitengröße. Der Server deckelt sie bei 100.
    pub per_page: u16,
    pub fieldsets: &'static [AdminFieldset],
    pub inlines: &'static [AdminInline],
    /// `Some(field)` schaltet die UTC-Datumsnavigation der Liste ein. Nur `Date` oder `DateTime`.
    pub date_hierarchy: Option<&'static str>,
}

/// Backend, für den Ferrioxid DDL und Abfragen erzeugt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
}

/// Spaltentyp, unabhängig vom SQL-Dialekt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    Uuid,
    Text,
    BigInt,
    Boolean,
    /// Kalendertag ohne Uhrzeit und ohne Zeitzone. Kanonisch `YYYY-MM-DD`.
    Date,
    /// Tageszeit ohne Datum. Kanonisch `HH:MM:SS`.
    Time,
    /// Absoluter Zeitpunkt in UTC. Kanonisch RFC 3339 mit `Z`.
    DateTime,
    /// Exakter Dezimalwert. Speicherformat: 16-Byte-Koeffizient, siehe `decimal`.
    Decimal,
    /// Enum-Wert mit festem Speichertext. Zulässige Werte stehen in `ColumnSpec::choices`.
    Choice,
    /// JSON-Dokument. PostgreSQL speichert `jsonb`, SQLite kanonischen Text.
    /// Pfadoperatoren sind kein portabler Vertrag.
    Json,
}

/// Aktion für `ON DELETE`. `ON UPDATE` bleibt `NO ACTION`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferentialAction {
    Restrict,
    Cascade,
    SetNull,
}

/// Metadaten einer Modellspalte. Das Makro erzeugt sie, die Datenbankschicht rendert sie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnSpec {
    pub name: &'static str,
    pub kind: ColumnKind,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_bool: Option<bool>,
    pub unique: bool,
    pub indexed: bool,
    pub foreign_table: Option<&'static str>,
    pub foreign_column: Option<&'static str>,
    pub on_delete: Option<ReferentialAction>,
    /// `0`, wenn die Spalte kein `Decimal` ist. Sonst `1..=28`.
    pub precision: u8,
    /// Nachkommastellen. Nur für `Decimal` gesetzt und immer `<= precision`.
    pub scale: u8,
    /// Leer, außer bei `Choice`.
    pub choices: &'static [ChoiceValue],
}

/// Zusammengesetzte Unique-Constraints und Indizes. Leere Listen sind der Normalfall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableConstraints {
    pub unique_groups: &'static [&'static [&'static str]],
    pub index_groups: &'static [&'static [&'static str]],
}

impl TableConstraints {
    pub const EMPTY: Self = Self {
        unique_groups: &[],
        index_groups: &[],
    };
}

impl Default for TableConstraints {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// Server-side numeric limits declared on a model field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericBound {
    pub column: &'static str,
    pub min: Option<i64>,
    pub max: Option<i64>,
}

/// Write and validation rules emitted by the model derive.
///
/// The same field attributes that build `ColumnSpec` and `NumericBound` fill this
/// contract. Hand-written models inherit the empty default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldContract {
    pub column: &'static str,
    pub in_create: bool,
    pub in_update: bool,
    pub create_required: bool,
    pub min_length: Option<u32>,
    pub max_length: Option<u32>,
    /// `"slug"` or `"email"` when the field declares `format`.
    pub format: Option<&'static str>,
}

pub trait FerrioxidModel: Send + Sync + 'static {
    fn descriptor() -> &'static ModelDescriptor;
    fn columns() -> &'static [ColumnSpec];
    fn constraints() -> &'static TableConstraints {
        &TableConstraints::EMPTY
    }
    fn schema_sql(kind: DatabaseKind) -> String;
    fn admin_meta() -> Option<&'static AdminModelMeta> {
        None
    }
    /// Limits from `#[field(min = ..., max = ...)]`. Empty when the model has none.
    fn numeric_bounds() -> &'static [NumericBound] {
        &[]
    }

    /// Create, update and text rules from the same derive. Empty for hand-written models.
    fn field_contracts() -> &'static [FieldContract] {
        &[]
    }
}

pub fn unique_index_name(table: &str, columns: &[&str]) -> String {
    object_name("uq", table, columns)
}

pub fn secondary_index_name(table: &str, columns: &[&str]) -> String {
    object_name("ix", table, columns)
}

pub fn foreign_key_name(
    table: &str,
    column: &str,
    referenced_table: &str,
    referenced_column: &str,
) -> String {
    format!("fk_{table}_{column}_{referenced_table}_{referenced_column}")
}

/// Kompaktes JSON. Objektschlüssel folgen der `serde_json`-Map, hier alphabetisch, weil `preserve_order` nicht aktiv ist.
///
/// Ein JSON-`null` ist ein Dokument, kein SQL-`NULL`. Leerer Text und ungültiges JSON
/// werden abgelehnt. PostgreSQL-`jsonb`-Operatoren werden nicht erzeugt.
pub fn canonical_json_text(text: &str) -> Result<String, &'static str> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|_| "validation.json")?;
    serde_json::to_string(&value).map_err(|_| "validation.json")
}

fn object_name(prefix: &str, table: &str, columns: &[&str]) -> String {
    let mut name = format!("{prefix}_{table}");
    for column in columns {
        name.push('_');
        name.push_str(column);
    }
    name
}

#[derive(Debug, thiserror::Error)]
pub enum FerrioxidError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("permission denied")]
    Forbidden,
    #[error("{0}")]
    Denied(String),
    #[error("authentication required")]
    Unauthorized(String),
    #[error("resource not found")]
    NotFound,
    #[error("database operation failed")]
    Database(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl FerrioxidError {
    pub fn database<E: std::error::Error + Send + Sync + 'static>(error: E) -> Self {
        Self::Database(Box::new(error))
    }
}

/// Reads `FERRIOXID_<NAME>` and the legacy `FERRUM_<NAME>` alias.
///
/// One set variable is used. Equal values are used. Different values are an
/// error and neither value is returned. `DATABASE_URL` and `BIND_ADDRESS` are
/// not aliases, so a second name cannot select another database or address.
pub fn aliased_env(name: &str) -> Result<Option<String>, String> {
    let Some(rest) = name.strip_prefix("FERRIOXID_") else {
        return Err(format!(
            "{name} is not a FERRIOXID_ setting. DATABASE_URL and BIND_ADDRESS have no legacy alias."
        ));
    };
    if rest.is_empty()
        || !rest
            .chars()
            .all(|char| char.is_ascii_uppercase() || char.is_ascii_digit() || char == '_')
    {
        return Err(format!("{name} is not a FERRIOXID_ setting."));
    }
    let legacy = format!("FERRUM_{rest}");
    match (std::env::var(name), std::env::var(&legacy)) {
        (Ok(current), Ok(previous)) if current != previous => Err(format!(
            "{name} and {legacy} are both set to different values. Ferrioxid refuses to choose either value."
        )),
        (Ok(value), _) | (Err(_), Ok(value)) => Ok(Some(value)),
        (Err(_), Err(_)) => Ok(None),
    }
}

/// Joins `Display` and `source` for server logs.
///
/// The result can contain SQL text from a database driver. Do not write it to
/// an HTTP response. Values that follow `password`, `secret`, `token`,
/// `csrf_token` or a Ferrioxid session cookie name are replaced.
pub fn diagnostic_chain(error: &dyn std::error::Error) -> String {
    let mut parts = Vec::new();
    let mut current = Some(error);
    let mut depth = 0;
    while let Some(item) = current {
        if depth == 8 {
            break;
        }
        parts.push(redact_secrets(&item.to_string()));
        current = item.source();
        depth += 1;
    }
    parts.join(" | ")
}

fn redact_secrets(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;
    while cursor < text.len() {
        if let Some((key_len, value_end)) = secret_at(&lower, cursor) {
            result.push_str(&text[cursor..cursor + key_len]);
            result.push_str("[redacted]");
            cursor = value_end;
            continue;
        }
        let next = text[cursor..].chars().next().unwrap_or('\u{FFFD}');
        result.push(next);
        cursor += next.len_utf8();
    }
    result
}

fn secret_at(lower: &str, index: usize) -> Option<(usize, usize)> {
    const KEYS: [&str; 6] = [
        "password=",
        "secret=",
        "token=",
        "csrf_token=",
        "ferrioxid_session=",
        "ferrioxid_anonymous=",
    ];
    let rest = lower.get(index..)?;
    let key = KEYS.iter().copied().find(|key| rest.starts_with(key))?;
    let mut end = index + key.len();
    let bytes = lower.as_bytes();
    while end < bytes.len() && !matches!(bytes[end], b' ' | b'\n' | b'\r' | b'\t' | b'&' | b';') {
        end += 1;
    }
    Some((key.len(), end))
}

/// Modelle mit den beiden verwalteten Zeitstempeln.
pub trait HasTimestamps {
    fn created_at(&self) -> chrono::DateTime<chrono::Utc>;
    fn updated_at(&self) -> chrono::DateTime<chrono::Utc>;
}

thread_local! {
    static FIXED_CLOCK: std::cell::Cell<Option<chrono::DateTime<chrono::Utc>>> =
        const { std::cell::Cell::new(None) };
}

/// UTC-Zeitpunkt für verwaltete Felder. Tests können ihn über [`set_test_clock`] festlegen.
pub fn utc_now() -> chrono::DateTime<chrono::Utc> {
    FIXED_CLOCK.with(|slot| slot.get().unwrap_or_else(chrono::Utc::now))
}

/// Setzt die Uhr für den aktuellen Thread. `None` verwendet wieder die Systemzeit.
pub fn set_test_clock(now: Option<chrono::DateTime<chrono::Utc>>) {
    FIXED_CLOCK.with(|slot| slot.set(now));
}

pub fn check_slug(field: &str, value: &str) -> Result<(), FerrioxidError> {
    let ok = !value.is_empty()
        && value.len() <= 80
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .chars()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '-');
    if ok {
        Ok(())
    } else {
        Err(FerrioxidError::Validation(format!(
            "field:{field}: validation.slug"
        )))
    }
}

pub fn check_email(field: &str, value: &str) -> Result<(), FerrioxidError> {
    let Some((local, domain)) = value.split_once('@') else {
        return Err(FerrioxidError::Validation(format!(
            "field:{field}: validation.email"
        )));
    };
    let labels = domain.split('.').collect::<Vec<_>>();
    let ok = !local.is_empty()
        && local.len() <= 64
        && !local
            .chars()
            .any(|char| char.is_whitespace() || char.is_control())
        && labels.len() >= 2
        && labels
            .iter()
            .all(|label| !label.is_empty() && label.len() <= 63)
        && domain.len() <= 255
        && !domain
            .chars()
            .any(|char| char.is_whitespace() || char.is_control() || char == '@');
    if ok {
        Ok(())
    } else {
        Err(FerrioxidError::Validation(format!(
            "field:{field}: validation.email"
        )))
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::{diagnostic_chain, FerrioxidError};

    #[test]
    fn database_display_hides_the_source_and_the_chain_redacts_secrets() {
        let error = FerrioxidError::database(std::io::Error::other(
            "password=hunter2 csrf_token=abc123 near users",
        ));
        assert_eq!(error.to_string(), "database operation failed");
        assert!(!error.to_string().contains("hunter2"));
        let chain = diagnostic_chain(&error);
        assert!(chain.contains("database operation failed"));
        assert!(chain.contains("password=[redacted]"));
        assert!(chain.contains("csrf_token=[redacted]"));
        assert!(!chain.contains("hunter2"));
        assert!(!chain.contains("abc123"));
    }
}

#[cfg(test)]
mod json_tests {
    use super::canonical_json_text;

    #[test]
    fn canonical_json_keeps_documents_distinct_from_empty_text() {
        assert_eq!(
            canonical_json_text(" { \"b\": 1, \"a\": null } ").unwrap(),
            "{\"a\":null,\"b\":1}"
        );
        assert_eq!(canonical_json_text("null").unwrap(), "null");
        assert!(canonical_json_text("").is_err());
        assert!(canonical_json_text("{").is_err());
    }
}

#[cfg(test)]
mod alias_tests {
    use super::aliased_env;

    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn conflicting_aliases_fail_without_returning_either_value() {
        let _guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        unsafe {
            std::env::set_var("FERRIOXID_ALIAS_TEST", "one");
            std::env::set_var("FERRUM_ALIAS_TEST", "two");
        }
        let error = aliased_env("FERRIOXID_ALIAS_TEST").unwrap_err();
        assert!(error.contains("FERRIOXID_ALIAS_TEST"));
        assert!(error.contains("FERRUM_ALIAS_TEST"));
        assert!(!error.contains("one"));
        assert!(!error.contains("two"));
        unsafe {
            std::env::set_var("FERRUM_ALIAS_TEST", "one");
        }
        assert_eq!(
            aliased_env("FERRIOXID_ALIAS_TEST").unwrap().as_deref(),
            Some("one")
        );
        unsafe {
            std::env::remove_var("FERRIOXID_ALIAS_TEST");
        }
        assert_eq!(
            aliased_env("FERRIOXID_ALIAS_TEST").unwrap().as_deref(),
            Some("one")
        );
        unsafe {
            std::env::remove_var("FERRUM_ALIAS_TEST");
        }
        assert_eq!(aliased_env("FERRIOXID_ALIAS_TEST").unwrap(), None);
        assert!(aliased_env("DATABASE_URL").is_err());
    }
}
