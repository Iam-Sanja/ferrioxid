//! Änderungshistorie. Einträge entstehen in derselben Transaktion wie die Änderung.

use fexu_auth::ADMIN_AUDIT;
use fexu_core::{AdminFieldMeta, AdminModelMeta, FexuError};
use sea_orm::sea_query::{Alias, PostgresQueryBuilder, Query, SqliteQueryBuilder};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, Value as DbValue};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const PAGE_SIZE: u64 = 25;

#[derive(Debug, Clone)]
pub struct Event {
    pub actor_id: Uuid,
    pub model_key: String,
    pub record_id: String,
    pub action: String,
    pub outcome: String,
    pub summary: String,
    /// Gemeinsamer Vorgang mehrerer Zeilen. `None` bei älteren Einzeleinträgen.
    pub operation_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StoredEvent {
    pub id: Uuid,
    pub created_at: i64,
    pub actor_id: Uuid,
    pub model_key: String,
    pub record_id: String,
    pub action: String,
    pub outcome: String,
    pub summary: String,
    pub operation_id: Option<Uuid>,
}

pub async fn insert(db: &impl ConnectionTrait, event: &Event) -> Result<(), FexuError> {
    if FAIL_NEXT.with(|flag| flag.replace(false)) {
        return Err(FexuError::database(std::io::Error::other(
            "audit write failed",
        )));
    }
    let backend = db.get_database_backend();
    let mut query = Query::insert();
    let values = [
        stored_uuid(backend, Uuid::new_v4()).into(),
        DbValue::from(unix_now()).into(),
        stored_uuid(backend, event.actor_id).into(),
        DbValue::from(clip(&event.model_key, 80)).into(),
        DbValue::from(clip(&event.record_id, 80)).into(),
        DbValue::from(clip(&event.action, 40)).into(),
        DbValue::from(clip(&event.outcome, 16)).into(),
        DbValue::from(clip(&event.summary, 1500)).into(),
        match event.operation_id {
            Some(id) => stored_uuid(backend, id).into(),
            None => stored_uuid_null(backend).into(),
        },
    ];
    query
        .into_table(Alias::new(ADMIN_AUDIT))
        .columns([
            Alias::new("id"),
            Alias::new("created_at"),
            Alias::new("actor_id"),
            Alias::new("model_key"),
            Alias::new("record_id"),
            Alias::new("action"),
            Alias::new("outcome"),
            Alias::new("summary"),
            Alias::new("operation_id"),
        ])
        .values_panic(values);
    execute(db, backend, &query).await
}

pub async fn page(
    db: &impl ConnectionTrait,
    actor: Option<Uuid>,
    offset: u64,
) -> Result<(Vec<StoredEvent>, bool), FexuError> {
    matching(db, actor, None, None, offset).await
}

pub async fn matching(
    db: &impl ConnectionTrait,
    actor: Option<Uuid>,
    model_key: Option<&str>,
    record_id: Option<&str>,
    offset: u64,
) -> Result<(Vec<StoredEvent>, bool), FexuError> {
    let backend = db.get_database_backend();
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("created_at"),
            Alias::new("actor_id"),
            Alias::new("model_key"),
            Alias::new("record_id"),
            Alias::new("action"),
            Alias::new("outcome"),
            Alias::new("summary"),
            Alias::new("operation_id"),
        ])
        .from(Alias::new(ADMIN_AUDIT))
        .order_by(Alias::new("created_at"), sea_orm::sea_query::Order::Desc)
        .order_by(Alias::new("id"), sea_orm::sea_query::Order::Desc)
        .limit(PAGE_SIZE + 1)
        .offset(offset);
    if let Some(actor) = actor {
        query.and_where(
            sea_orm::sea_query::Expr::col(Alias::new("actor_id")).eq(stored_uuid(backend, actor)),
        );
    }
    if let Some(model_key) = model_key {
        query.and_where(sea_orm::sea_query::Expr::col(Alias::new("model_key")).eq(model_key));
    }
    if let Some(record_id) = record_id {
        query.and_where(sea_orm::sea_query::Expr::col(Alias::new("record_id")).eq(record_id));
    }
    let (sql, values) = build(backend, &query)?;
    let rows = db
        .query_all(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map_err(FexuError::database)?;
    let mut events = Vec::new();
    for row in rows {
        events.push(StoredEvent {
            id: read_uuid(&row, "id")?,
            created_at: row
                .try_get::<i64>("", "created_at")
                .map_err(FexuError::database)?,
            actor_id: read_uuid(&row, "actor_id")?,
            model_key: row
                .try_get::<String>("", "model_key")
                .map_err(FexuError::database)?,
            record_id: row
                .try_get::<String>("", "record_id")
                .map_err(FexuError::database)?,
            action: row
                .try_get::<String>("", "action")
                .map_err(FexuError::database)?,
            outcome: row
                .try_get::<String>("", "outcome")
                .map_err(FexuError::database)?,
            summary: row
                .try_get::<String>("", "summary")
                .map_err(FexuError::database)?,
            operation_id: optional_uuid(&row, "operation_id")?,
        });
    }
    let has_more = events.len() > PAGE_SIZE as usize;
    events.truncate(PAGE_SIZE as usize);
    Ok((events, has_more))
}

pub fn change_summary(
    meta: &AdminModelMeta,
    before: Option<&Value>,
    after: Option<&Value>,
) -> String {
    let mut lines = Vec::new();
    for field in meta.fields {
        if concealed(field) {
            continue;
        }
        let previous = field_text(before, field.name);
        let next = field_text(after, field.name);
        if previous == next {
            continue;
        }
        let line = match (previous, next) {
            (None, Some(value)) => format!("{}: {}", field.label, quote_text(&value)),
            (Some(value), None) => format!("{}: {}", field.label, quote_text(&value)),
            (Some(left), Some(right)) => format!(
                "{}: {} → {}",
                field.label,
                quote_text(&left),
                quote_text(&right)
            ),
            (None, None) => continue,
        };
        lines.push(line);
        if lines.len() == 12 {
            break;
        }
    }
    clip(&lines.join("\n"), 1500)
}

pub fn concealed(field: &AdminFieldMeta) -> bool {
    field.hidden
        || field.sensitive
        || crate::registry::sensitive(field.name)
        || field.name.ends_with("_hash")
        || field.name.ends_with("_secret")
}

fn field_text(record: Option<&Value>, name: &str) -> Option<String> {
    let value = record?.get(name)?;
    match value {
        Value::Null => None,
        Value::String(text) if text.is_empty() => None,
        Value::String(text) => Some(clip(text, 80)),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn quote_text(value: &str) -> String {
    format!("\"{}\"", value.replace(['\n', '\r'], " "))
}

fn clip(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

pub fn format_unix_utc(seconds: i64) -> Option<(String, String)> {
    let stamp = chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, 0)?;
    Some((
        stamp.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        stamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    ))
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn optional_uuid(row: &sea_orm::QueryResult, column: &str) -> Result<Option<Uuid>, FexuError> {
    if let Ok(value) = row.try_get::<Option<String>>("", column) {
        return match value {
            Some(text) if !text.is_empty() => Uuid::parse_str(&text)
                .map(Some)
                .map_err(|_| FexuError::database(std::io::Error::other("operation id is invalid"))),
            _ => Ok(None),
        };
    }
    if let Ok(value) = row.try_get::<Option<Uuid>>("", column) {
        return Ok(value);
    }
    Ok(None)
}

thread_local! {
    static FAIL_NEXT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn fail_next_insert() {
    FAIL_NEXT.with(|flag| flag.set(true));
}

fn stored_uuid(backend: DatabaseBackend, id: Uuid) -> DbValue {
    match backend {
        DatabaseBackend::Postgres => DbValue::Uuid(Some(Box::new(id))),
        _ => DbValue::String(Some(Box::new(id.to_string()))),
    }
}

fn stored_uuid_null(backend: DatabaseBackend) -> DbValue {
    match backend {
        DatabaseBackend::Postgres => DbValue::Uuid(None),
        _ => DbValue::String(None),
    }
}

fn read_uuid(row: &sea_orm::QueryResult, column: &str) -> Result<Uuid, FexuError> {
    if let Ok(value) = row.try_get::<Uuid>("", column) {
        return Ok(value);
    }
    let text = row
        .try_get::<String>("", column)
        .map_err(FexuError::database)?;
    Uuid::parse_str(&text).map_err(|_| FexuError::Validation("stored uuid is invalid".into()))
}

async fn execute(
    db: &impl ConnectionTrait,
    backend: DatabaseBackend,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<(), FexuError> {
    let (sql, values) = build(backend, query)?;
    db.execute(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map(|_| ())
        .map_err(FexuError::database)
}

fn build(
    backend: DatabaseBackend,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<(String, Vec<DbValue>), FexuError> {
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => {
            return Err(FexuError::Validation(
                "MySQL wird nicht unterstützt.".into(),
            ))
        }
    };
    Ok((sql, values.0))
}

#[cfg(test)]
mod tests {
    use super::change_summary;
    use fexu_core::{AdminFieldMeta, AdminModelMeta};

    fn field(name: &'static str, sensitive: bool) -> AdminFieldMeta {
        AdminFieldMeta {
            name,
            label: name,
            list_display: false,
            list_link: false,
            searchable: false,
            sortable: false,
            filterable: false,
            readonly: false,
            hidden: sensitive,
            sensitive,
            fieldset: "",
        }
    }

    #[test]
    fn summary_skips_sensitive_values() {
        let fields: &'static [AdminFieldMeta] = Box::leak(Box::new([
            field("name", false),
            field("password", true),
            field("password_hash", true),
        ]));
        let meta = AdminModelMeta {
            key: "products",
            label: "Produkte",
            permission: None,
            fields,
            ordering: None,
            per_page: 25,
            fieldsets: &[],
            inlines: &[],
            date_hierarchy: None,
        };
        let before =
            serde_json::json!({"name": "Apfel", "password": "secret", "password_hash": "hash"});
        let after =
            serde_json::json!({"name": "Birne", "password": "next", "password_hash": "other"});
        let text = change_summary(&meta, Some(&before), Some(&after));
        assert!(text.contains("Apfel"));
        assert!(text.contains("Birne"));
        assert!(!text.contains("secret"));
        assert!(!text.contains("hash"));
        assert!(!text.contains("next"));
    }

    #[test]
    fn summary_quotes_date_and_utc_instants() {
        let fields: &'static [AdminFieldMeta] = Box::leak(Box::new([field("created_at", false)]));
        let meta = AdminModelMeta {
            key: "events",
            label: "Ereignisse",
            permission: None,
            fields,
            ordering: None,
            per_page: 25,
            fieldsets: &[],
            inlines: &[],
            date_hierarchy: Some("created_at"),
        };
        let before = serde_json::json!({"created_at": "2026-11-06T23:59:59Z"});
        let after = serde_json::json!({"created_at": "2026-11-07T00:00:00Z"});
        let text = change_summary(&meta, Some(&before), Some(&after));
        assert!(text.contains("2026-11-06T23:59:59Z"));
        assert!(text.contains("2026-11-07T00:00:00Z"));
        assert!(text.contains("→"));
    }

    #[test]
    fn unix_seconds_render_as_utc_and_reject_an_unreadable_instant() {
        let (label, iso) = super::format_unix_utc(0).expect("epoch");
        assert_eq!(label, "1970-01-01 00:00:00 UTC");
        assert_eq!(iso, "1970-01-01T00:00:00Z");
        assert!(super::format_unix_utc(i64::MAX).is_none());
    }

    #[tokio::test]
    async fn matching_uses_the_record_id_not_a_similar_name() {
        let db = fexu_db::connect("sqlite::memory:").await.unwrap();
        fexu_auth::apply_migrations(&db).await.unwrap();
        record_id_match(&db).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "creates and drops one fexu_test_* PostgreSQL database"]
    async fn postgres_matching_uses_the_record_id_not_a_similar_name() {
        let admin = std::env::var("FEXU_POSTGRES_ADMIN")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| std::env::var("DATABASE_URL").ok())
            .expect("FEXU_POSTGRES_ADMIN or DATABASE_URL must name an approved PostgreSQL admin database");
        let disposable = fexu_db::DisposablePostgres::create(&admin)
            .await
            .expect("disposable database");
        let db = fexu_db::connect(disposable.url()).await.expect("connect");
        let matched = async {
            fexu_auth::apply_migrations(&db)
                .await
                .map_err(|error| format!("{error:?}"))?;
            record_id_match(&db).await
        }
        .await;
        db.close().await.ok();
        disposable
            .drop_database()
            .await
            .expect("drop test database");
        matched.expect("record id match on the disposable database");
    }

    async fn record_id_match(db: &sea_orm::DatabaseConnection) -> Result<(), String> {
        let actor = uuid::Uuid::nil();
        let apple = "11111111-1111-1111-1111-111111111111";
        let other = "22222222-2222-2222-2222-222222222222";
        for (record, summary) in [(apple, "Name: \"Apfel\""), (other, "Name: \"Apfelbaum\"")] {
            super::insert(
                db,
                &super::Event {
                    actor_id: actor,
                    model_key: "products".into(),
                    record_id: record.into(),
                    action: "update".into(),
                    outcome: "ok".into(),
                    summary: summary.into(),
                    operation_id: None,
                },
            )
            .await
            .map_err(|error| format!("{error:?}"))?;
        }
        let (rows, has_more) = super::matching(db, None, Some("products"), Some(apple), 0)
            .await
            .map_err(|error| format!("{error:?}"))?;
        if has_more || rows.len() != 1 || rows[0].record_id != apple {
            return Err(format!("expected one row for {apple}, got {rows:?}"));
        }
        if !rows[0].summary.contains("Apfel") || rows[0].summary.contains("Apfelbaum") {
            return Err(rows[0].summary.clone());
        }
        let (foreign, _) = super::matching(
            db,
            Some(uuid::Uuid::from_u128(9)),
            Some("products"),
            Some(apple),
            0,
        )
        .await
        .map_err(|error| format!("{error:?}"))?;
        if !foreign.is_empty() {
            return Err("another actor can see the record history".into());
        }
        Ok(())
    }
}
