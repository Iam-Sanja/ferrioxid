//! Disposable PostgreSQL databases for tests.
//!
//! `create` only accepts an administrator URL whose database name is one of
//! the documented test databases. It creates `fexu_test_<32 hex>` and
//! `drop_database` drops that name only. It does not remove tables from the
//! administrator database and it does not accept an arbitrary catalog.

use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};

use crate::connect;

pub struct DisposablePostgres {
    url: String,
    db_name: String,
    admin_prefix: String,
}

impl DisposablePostgres {
    pub async fn create(admin_url: &str) -> Result<Self, String> {
        let (prefix, admin_name) = split_database_url(admin_url)?;
        if !approved_admin_database(&admin_name) {
            return Err(format!(
                "Refusing to create a test database from '{admin_name}'. Use fexu, fexu_migrations_down_test, postgres, template1, or a database whose name contains fexu_test."
            ));
        }
        let db_name = format!("fexu_test_{}", uuid::Uuid::new_v4().simple());
        if !is_disposable_name(&db_name) {
            return Err("The generated test database name is not disposable.".into());
        }
        let admin = admin_connection(&prefix, &admin_name).await?;
        admin
            .execute(Statement::from_string(
                DatabaseBackend::Postgres,
                format!("CREATE DATABASE \"{db_name}\""),
            ))
            .await
            .map_err(|error| {
                format!(
                    "Could not create isolated test database {db_name}: {}",
                    redact(&error.to_string())
                )
            })?;
        admin
            .close()
            .await
            .map_err(|error| redact(&error.to_string()))?;
        Ok(Self {
            url: format!("{prefix}/{db_name}"),
            db_name,
            admin_prefix: prefix,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn name(&self) -> &str {
        &self.db_name
    }

    pub async fn drop_database(self) -> Result<(), String> {
        if !is_disposable_name(&self.db_name) {
            return Err(format!(
                "Refusing to drop '{}' because this helper did not create it.",
                self.db_name
            ));
        }
        let admin = admin_connection(&self.admin_prefix, "postgres").await?;
        let _ = admin
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1 AND pid <> pg_backend_pid()",
                [sea_orm::Value::String(Some(Box::new(self.db_name.clone())))],
            ))
            .await;
        admin
            .execute(Statement::from_string(
                DatabaseBackend::Postgres,
                format!("DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)", self.db_name),
            ))
            .await
            .map_err(|error| {
                format!(
                    "Could not drop isolated test database {}: {}",
                    self.db_name,
                    redact(&error.to_string())
                )
            })?;
        admin
            .close()
            .await
            .map_err(|error| redact(&error.to_string()))?;
        Ok(())
    }
}

fn approved_admin_database(name: &str) -> bool {
    matches!(
        name,
        "fexu" | "fexu_migrations_down_test" | "postgres" | "template1"
    ) || (name.contains("fexu_test") && is_safe_database_name(name))
}

fn is_disposable_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("fexu_test_") else {
        return false;
    };
    rest.len() == 32 && rest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_safe_database_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_alphabetic() || char == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
        && name.len() <= 63
}

fn split_database_url(url: &str) -> Result<(String, String), String> {
    if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
        return Err("The isolated PostgreSQL helper accepts only a postgres URL.".into());
    }
    let without_query = url.split('?').next().unwrap_or(url);
    let (prefix, name) = without_query
        .rsplit_once('/')
        .ok_or_else(|| "The PostgreSQL URL has no database name.".to_owned())?;
    if name.is_empty() || name.contains('@') || !is_safe_database_name(name) {
        return Err("The PostgreSQL URL has no usable database name.".into());
    }
    Ok((prefix.to_owned(), name.to_owned()))
}

async fn admin_connection(prefix: &str, fallback_db: &str) -> Result<DatabaseConnection, String> {
    let mut candidates = vec!["template1".to_owned(), "postgres".to_owned()];
    if is_safe_database_name(fallback_db)
        && fallback_db != "template1"
        && fallback_db != "postgres"
        && !candidates.iter().any(|existing| existing == fallback_db)
    {
        candidates.push(fallback_db.to_owned());
    }
    let mut errors = Vec::new();
    for database in candidates {
        match connect(&format!("{prefix}/{database}")).await {
            Ok(connection) => return Ok(connection),
            Err(error) => errors.push(redact(&error.to_string())),
        }
    }
    Err(format!(
        "Could not open a PostgreSQL admin connection. {}",
        errors.join(" | ")
    ))
}

fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(scheme) = rest.find("://") {
        out.push_str(&rest[..scheme + 3]);
        rest = &rest[scheme + 3..];
        if let Some(at) = rest.find('@') {
            let userinfo = &rest[..at];
            if !userinfo.contains('/') {
                out.push_str("redacted");
                rest = &rest[at..];
                continue;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::{approved_admin_database, is_disposable_name, redact};

    #[test]
    fn disposable_names_and_admin_targets_are_closed() {
        assert!(is_disposable_name(
            "fexu_test_0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_disposable_name("fexu_test"));
        assert!(!is_disposable_name("fexu"));
        assert!(!is_disposable_name("fexu_test_short"));
        assert!(approved_admin_database("fexu"));
        assert!(approved_admin_database("fexu_test"));
        assert!(!approved_admin_database("shop"));
        assert!(!approved_admin_database("fexu_test;drop"));
        let secret = redact("postgres://fexu:fexu_dev_only@127.0.0.1:5432/fexu");
        assert!(!secret.contains("fexu_dev_only"));
        assert!(secret.contains("redacted@"));
    }

    #[tokio::test]
    async fn create_refuses_an_unapproved_database_before_connecting() {
        let error =
            match super::DisposablePostgres::create("postgres://fexu:secret@127.0.0.1:5432/shop")
                .await
            {
                Ok(_) => panic!("an unapproved database was accepted"),
                Err(error) => error,
            };
        assert!(error.contains("shop"));
        assert!(error.contains("Refusing"));
        assert!(!error.contains("secret"));
    }
}
