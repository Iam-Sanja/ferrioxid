//! Sitzungsgebundene Hinweise. Die Tabelle entsteht nur durch `create_flash_table`.

use fexu_core::FexuError;
use fexu_db::driver::sea_query::{
    Alias, Expr, Order, PostgresQueryBuilder, Query, SqliteQueryBuilder,
};
use fexu_db::driver::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait, Value,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashLevel {
    Success,
    Info,
    Error,
}

impl FlashLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Info => "info",
            Self::Error => "error",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "success" => Some(Self::Success),
            "info" => Some(Self::Info),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashMessage {
    pub level: FlashLevel,
    pub text: String,
}

const TABLE: &str = "_fexu_flashes";

pub async fn create_flash_table(db: &impl ConnectionTrait) -> Result<(), FexuError> {
    let sql = match db.get_database_backend() {
        DatabaseBackend::Postgres => {
            "CREATE TABLE IF NOT EXISTS _fexu_flashes (
                id text PRIMARY KEY,
                session_id text NOT NULL,
                level text NOT NULL,
                body text NOT NULL,
                created_at bigint NOT NULL
            )"
        }
        DatabaseBackend::Sqlite => {
            "CREATE TABLE IF NOT EXISTS _fexu_flashes (
                id text PRIMARY KEY,
                session_id text NOT NULL,
                level text NOT NULL,
                body text NOT NULL,
                created_at integer NOT NULL
            )"
        }
        DatabaseBackend::MySql => {
            return Err(FexuError::Validation("MySQL is not supported".into()))
        }
    };
    db.execute(Statement::from_string(
        db.get_database_backend(),
        sql.to_owned(),
    ))
    .await
    .map_err(FexuError::database)?;
    Ok(())
}

pub async fn push_flash(
    db: &DatabaseConnection,
    session_id: Uuid,
    level: FlashLevel,
    text: &str,
) -> Result<(), FexuError> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 240 || text.chars().any(|char| char.is_control()) {
        return Err(FexuError::Validation(
            "flash text is empty, too long, or contains a control character".into(),
        ));
    }
    let mut insert = Query::insert();
    insert
        .into_table(Alias::new(TABLE))
        .columns([
            Alias::new("id"),
            Alias::new("session_id"),
            Alias::new("level"),
            Alias::new("body"),
            Alias::new("created_at"),
        ])
        .values_panic([
            Value::String(Some(Box::new(Uuid::new_v4().to_string()))).into(),
            Value::String(Some(Box::new(session_id.to_string()))).into(),
            Value::String(Some(Box::new(level.as_str().to_owned()))).into(),
            Value::String(Some(Box::new(text.to_owned()))).into(),
            Value::BigInt(Some(fexu_core::utc_now().timestamp())).into(),
        ]);
    db.execute(statement(db, &insert)?)
        .await
        .map_err(FexuError::database)?;
    Ok(())
}

/// Liest die Hinweise der Sitzung und löscht sie in derselben Transaktion.
pub async fn take_flashes(
    db: &DatabaseConnection,
    session_id: Uuid,
) -> Result<Vec<FlashMessage>, FexuError> {
    let tx = db.begin().await.map_err(FexuError::database)?;
    let mut select = Query::select();
    select
        .columns([Alias::new("level"), Alias::new("body")])
        .from(Alias::new(TABLE))
        .and_where(
            Expr::col(Alias::new("session_id"))
                .eq(Value::String(Some(Box::new(session_id.to_string())))),
        )
        .order_by(Alias::new("created_at"), Order::Asc);
    let rows = tx
        .query_all(statement(&tx, &select)?)
        .await
        .map_err(FexuError::database)?;
    let mut messages = Vec::new();
    for row in rows {
        let level = row
            .try_get::<String>("", "level")
            .map_err(FexuError::database)?;
        let text = row
            .try_get::<String>("", "body")
            .map_err(FexuError::database)?;
        let Some(level) = FlashLevel::parse(&level) else {
            continue;
        };
        messages.push(FlashMessage { level, text });
    }
    let mut delete = Query::delete();
    delete.from_table(Alias::new(TABLE)).and_where(
        Expr::col(Alias::new("session_id"))
            .eq(Value::String(Some(Box::new(session_id.to_string())))),
    );
    tx.execute(statement(&tx, &delete)?)
        .await
        .map_err(FexuError::database)?;
    tx.commit().await.map_err(FexuError::database)?;
    Ok(messages)
}

pub fn render_flashes(messages: &[FlashMessage]) -> String {
    let mut html = String::from("<ul class=\"flashes\">");
    for message in messages {
        html.push_str(&format!(
            "<li data-level=\"{}\">{}</li>",
            message.level.as_str(),
            escape(&message.text)
        ));
    }
    html.push_str("</ul>");
    html
}

fn escape(value: &str) -> String {
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

fn statement(
    db: &impl ConnectionTrait,
    query: &impl fexu_db::driver::sea_query::QueryStatementWriter,
) -> Result<Statement, FexuError> {
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => {
            return Err(FexuError::Validation("MySQL is not supported".into()))
        }
    };
    Ok(Statement::from_sql_and_values(backend, sql, values.0))
}

#[cfg(test)]
mod tests {
    use super::{create_flash_table, push_flash, render_flashes, take_flashes, FlashLevel};
    use uuid::Uuid;

    async fn exercise(db: &fexu_db::driver::DatabaseConnection) {
        let session = Uuid::new_v4();
        let other = Uuid::new_v4();
        assert!(push_flash(db, session, FlashLevel::Success, "saved")
            .await
            .is_err());
        create_flash_table(db).await.unwrap();
        push_flash(db, session, FlashLevel::Success, "saved <b>")
            .await
            .unwrap();
        push_flash(db, other, FlashLevel::Error, "other")
            .await
            .unwrap();
        assert!(push_flash(db, session, FlashLevel::Info, "bad\nline")
            .await
            .is_err());
        let first = take_flashes(db, session).await.unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].level, FlashLevel::Success);
        let html = render_flashes(&first);
        assert!(html.contains("&lt;b&gt;"));
        assert!(!html.contains("<b>"));
        assert!(take_flashes(db, session).await.unwrap().is_empty());
        let remaining = take_flashes(db, other).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(take_flashes(db, other).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn sqlite_flash_is_consumed_once_per_session() {
        let db = fexu_db::connect("sqlite::memory:").await.unwrap();
        exercise(&db).await;
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL, CREATEDB, and an approved PostgreSQL test database"]
    async fn postgres_flash_is_consumed_once_per_session() {
        let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let disposable = fexu_db::DisposablePostgres::create(&admin)
            .await
            .expect("isolated postgres");
        let db = fexu_db::connect(disposable.url()).await.expect("postgres");
        exercise(&db).await;
        db.close().await.expect("close");
        disposable.drop_database().await.expect("drop");
    }
}
