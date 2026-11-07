//! Auth-Tabellen als Ferrum-Snapshot. Die Historie liegt getrennt von der Anwendung.

use crate::error::AuthError;
use ferrum_migrations::{
    apply_directory_with_history, plan_change, seal, ColumnSnapshot, ForeignKeySnapshot,
    IndexSnapshot, LogicalType, MigrationFile, OnDelete, SchemaSnapshot, TableSnapshot,
    UniqueConstraintSnapshot, AUTH_HISTORY_TABLE, FORMAT_VERSION,
};
use sea_orm::DatabaseConnection;
use std::path::PathBuf;

pub const USERS: &str = "_ferrum_users";
pub const SESSIONS: &str = "_ferrum_sessions";
pub const ROLES: &str = "_ferrum_roles";
pub const PERMISSIONS: &str = "_ferrum_permissions";
pub const USER_ROLES: &str = "_ferrum_user_roles";
pub const ROLE_PERMISSIONS: &str = "_ferrum_role_permissions";
pub const ADMIN_DRAFTS: &str = "_ferrum_admin_drafts";
pub const ADMIN_AUDIT: &str = "_ferrum_admin_audit";

pub fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

pub fn schema() -> Result<SchemaSnapshot, AuthError> {
    let snapshot = SchemaSnapshot {
        format_version: FORMAT_VERSION,
        tables: vec![
            users(),
            roles(),
            permissions(),
            sessions(),
            user_roles(),
            role_permissions(),
        ],
    };
    snapshot
        .validate()
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    Ok(snapshot)
}

pub fn migration_file() -> Result<MigrationFile, AuthError> {
    let snapshot = schema()?;
    let plan = plan_change(&SchemaSnapshot::empty(), &snapshot)
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    if let Some(reason) = &plan.refusal {
        return Err(AuthError::Validation(reason.clone()));
    }
    seal(
        "0001_auth_schema".to_owned(),
        "auth_schema".to_owned(),
        SchemaSnapshot::empty(),
        snapshot,
        &plan,
    )
    .map_err(|error| AuthError::Validation(error.to_string()))
}

#[cfg(test)]
pub fn admin_migration_file() -> Result<MigrationFile, AuthError> {
    let from = schema()?;
    let mut to = from.clone();
    to.tables.push(admin_drafts());
    to.tables.push(admin_audit());
    to.validate()
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    let plan = plan_change(&from, &to).map_err(|error| AuthError::Validation(error.to_string()))?;
    if let Some(reason) = &plan.refusal {
        return Err(AuthError::Validation(reason.clone()));
    }
    seal(
        "0002_admin_journal".to_owned(),
        "admin_journal".to_owned(),
        from,
        to,
        &plan,
    )
    .map_err(|error| AuthError::Validation(error.to_string()))
}

pub async fn apply_migrations(db: &DatabaseConnection) -> Result<Vec<String>, AuthError> {
    let report = apply_directory_with_history(db, &migrations_dir(), false, AUTH_HISTORY_TABLE)
        .await
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    Ok(report.applied)
}

#[cfg(test)]
fn admin_drafts() -> TableSnapshot {
    table(
        ADMIN_DRAFTS,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("session_id", LogicalType::Uuid, false, false, None),
            column("user_id", LogicalType::Uuid, false, false, None),
            column("model_key", LogicalType::String, false, false, None),
            column("record_id", LogicalType::String, false, true, None),
            column("purpose", LogicalType::String, false, false, None),
            column("payload", LogicalType::String, false, false, None),
            column("created_at", LogicalType::I64, false, false, None),
            column("expires_at", LogicalType::I64, false, false, None),
        ],
        Vec::new(),
        vec![index(ADMIN_DRAFTS, &["session_id"])],
        vec![foreign_key(ADMIN_DRAFTS, "session_id", SESSIONS)],
    )
}

#[cfg(test)]
fn admin_audit() -> TableSnapshot {
    table(
        ADMIN_AUDIT,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("created_at", LogicalType::I64, false, false, None),
            column("actor_id", LogicalType::Uuid, false, false, None),
            column("model_key", LogicalType::String, false, false, None),
            column("record_id", LogicalType::String, false, false, None),
            column("action", LogicalType::String, false, false, None),
            column("outcome", LogicalType::String, false, false, None),
            column("summary", LogicalType::String, false, false, None),
        ],
        Vec::new(),
        vec![index(ADMIN_AUDIT, &["created_at"])],
        Vec::new(),
    )
}

#[cfg(test)]
fn index(table_name: &str, columns: &[&str]) -> IndexSnapshot {
    let mut name = format!("ix_{table_name}");
    let mut owned = Vec::new();
    for column in columns {
        name.push('_');
        name.push_str(column);
        owned.push((*column).to_owned());
    }
    IndexSnapshot {
        name,
        columns: owned,
    }
}

fn users() -> TableSnapshot {
    table(
        USERS,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("email", LogicalType::String, false, false, None),
            column("password_hash", LogicalType::String, false, false, None),
            column("is_active", LogicalType::Bool, false, false, Some(true)),
            column("is_staff", LogicalType::Bool, false, false, Some(false)),
            column("is_superuser", LogicalType::Bool, false, false, Some(false)),
            column("created_at", LogicalType::I64, false, false, None),
            column("updated_at", LogicalType::I64, false, false, None),
            column("last_login", LogicalType::I64, false, true, None),
        ],
        vec![unique(USERS, &["email"])],
        Vec::new(),
        Vec::new(),
    )
}

fn roles() -> TableSnapshot {
    table(
        ROLES,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("name", LogicalType::String, false, false, None),
        ],
        vec![unique(ROLES, &["name"])],
        Vec::new(),
        Vec::new(),
    )
}

fn permissions() -> TableSnapshot {
    table(
        PERMISSIONS,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("code", LogicalType::String, false, false, None),
        ],
        vec![unique(PERMISSIONS, &["code"])],
        Vec::new(),
        Vec::new(),
    )
}

fn sessions() -> TableSnapshot {
    table(
        SESSIONS,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("user_id", LogicalType::Uuid, false, false, None),
            column("token_hash", LogicalType::String, false, false, None),
            column("csrf_token_hash", LogicalType::String, false, false, None),
            column("created_at", LogicalType::I64, false, false, None),
            column("expires_at", LogicalType::I64, false, false, None),
            column("last_seen_at", LogicalType::I64, false, false, None),
            column("revoked_at", LogicalType::I64, false, true, None),
        ],
        vec![unique(SESSIONS, &["token_hash"])],
        vec![IndexSnapshot {
            name: format!("ix_{SESSIONS}_user_id"),
            columns: vec!["user_id".to_owned()],
        }],
        vec![foreign_key(SESSIONS, "user_id", USERS)],
    )
}

fn user_roles() -> TableSnapshot {
    table(
        USER_ROLES,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("user_id", LogicalType::Uuid, false, false, None),
            column("role_id", LogicalType::Uuid, false, false, None),
        ],
        vec![unique(USER_ROLES, &["user_id", "role_id"])],
        Vec::new(),
        vec![
            foreign_key(USER_ROLES, "user_id", USERS),
            foreign_key(USER_ROLES, "role_id", ROLES),
        ],
    )
}

fn role_permissions() -> TableSnapshot {
    table(
        ROLE_PERMISSIONS,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("role_id", LogicalType::Uuid, false, false, None),
            column("permission_id", LogicalType::Uuid, false, false, None),
        ],
        vec![unique(ROLE_PERMISSIONS, &["role_id", "permission_id"])],
        Vec::new(),
        vec![
            foreign_key(ROLE_PERMISSIONS, "role_id", ROLES),
            foreign_key(ROLE_PERMISSIONS, "permission_id", PERMISSIONS),
        ],
    )
}

fn table(
    name: &str,
    columns: Vec<ColumnSnapshot>,
    unique_constraints: Vec<UniqueConstraintSnapshot>,
    indexes: Vec<IndexSnapshot>,
    foreign_keys: Vec<ForeignKeySnapshot>,
) -> TableSnapshot {
    TableSnapshot {
        name: name.to_owned(),
        columns,
        unique_constraints,
        indexes,
        foreign_keys,
    }
}

fn column(
    name: &str,
    logical_type: LogicalType,
    primary_key: bool,
    nullable: bool,
    default: Option<bool>,
) -> ColumnSnapshot {
    ColumnSnapshot {
        name: name.to_owned(),
        logical_type,
        primary_key,
        nullable,
        default,
    }
}

fn unique(table_name: &str, columns: &[&str]) -> UniqueConstraintSnapshot {
    let mut name = format!("uq_{table_name}");
    let mut owned = Vec::new();
    for column in columns {
        name.push('_');
        name.push_str(column);
        owned.push((*column).to_owned());
    }
    UniqueConstraintSnapshot {
        name,
        columns: owned,
    }
}

fn foreign_key(table_name: &str, column_name: &str, referenced: &str) -> ForeignKeySnapshot {
    ForeignKeySnapshot {
        name: format!("fk_{table_name}_{column_name}_{referenced}_id"),
        columns: vec![column_name.to_owned()],
        referenced_table: referenced.to_owned(),
        referenced_columns: vec!["id".to_owned()],
        on_delete: OnDelete::Cascade,
        on_update: "no_action".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{admin_migration_file, migration_file, migrations_dir};

    #[test]
    fn sealed_migration_matches_the_schema() {
        let file = migration_file().expect("auth schema can be sealed");
        let path = migrations_dir().join("0001_auth_schema.json");
        if !path.exists() {
            ferrum_migrations::save_migration(&migrations_dir(), &file).expect("write migration");
        }
        let loaded = ferrum_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0001_auth_schema");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(loaded.to_snapshot.tables.len() >= 6);
    }

    #[test]
    fn admin_journal_migration_is_sealed() {
        let file = admin_migration_file().expect("admin journal can be sealed");
        let path = migrations_dir().join("0002_admin_journal.json");
        if !path.exists() {
            ferrum_migrations::save_migration(&migrations_dir(), &file).expect("write migration");
        }
        let loaded = ferrum_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0002_admin_journal");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(loaded.operations.len() >= 2);
    }
}
