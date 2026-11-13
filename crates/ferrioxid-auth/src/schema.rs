//! Auth-Tabellen als Ferrioxid-Snapshot. Die Historie liegt getrennt von der Anwendung.

use crate::error::AuthError;
use ferrioxid_migrations::{
    plan_change, seal, ColumnSnapshot, ForeignKeySnapshot, IndexSnapshot, LogicalType,
    MigrationFile, OnDelete, SchemaSnapshot, TableSnapshot, UniqueConstraintSnapshot,
    AUTH_HISTORY_TABLE,
};
use sea_orm::DatabaseConnection;
use std::path::PathBuf;

pub const USERS: &str = "_ferrioxid_users";
pub const SESSIONS: &str = "_ferrioxid_sessions";
pub const ANONYMOUS_SESSIONS: &str = "_ferrioxid_anonymous_sessions";
pub const ROLES: &str = "_ferrioxid_roles";
pub const PERMISSIONS: &str = "_ferrioxid_permissions";
pub const USER_ROLES: &str = "_ferrioxid_user_roles";
pub const ROLE_PERMISSIONS: &str = "_ferrioxid_role_permissions";
pub const ADMIN_DRAFTS: &str = "_ferrioxid_admin_drafts";
pub const ADMIN_AUDIT: &str = "_ferrioxid_admin_audit";
pub const PASSWORD_RESETS: &str = "_ferrioxid_password_resets";
pub const PASSWORD_RESET_EXCHANGES: &str = "_ferrioxid_password_reset_exchanges";
pub const MAIL_OUTBOX: &str = "_ferrioxid_mail_outbox";
pub const MFA_TOTP: &str = "_ferrioxid_mfa_totp";
pub const RECOVERY_CODES: &str = "_ferrioxid_recovery_codes";
pub const LOGIN_CHALLENGES: &str = "_ferrioxid_login_challenges";

pub fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

const EMBEDDED_MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_auth_schema",
        include_str!("../migrations/0001_auth_schema.json"),
    ),
    (
        "0002_admin_journal",
        include_str!("../migrations/0002_admin_journal.json"),
    ),
    (
        "0003_audit_operation",
        include_str!("../migrations/0003_audit_operation.json"),
    ),
    (
        "0004_password_reset",
        include_str!("../migrations/0004_password_reset.json"),
    ),
    (
        "0005_anonymous_sessions",
        include_str!("../migrations/0005_anonymous_sessions.json"),
    ),
    (
        "0006_account_security",
        include_str!("../migrations/0006_account_security.json"),
    ),
];

const EXPECTED_MIGRATIONS: &[&str] = &[
    "0001_auth_schema",
    "0002_admin_journal",
    "0003_audit_operation",
    "0004_password_reset",
    "0005_anonymous_sessions",
    "0006_account_security",
];

/// Auth migrations compiled into the binary. Runtime does not open `CARGO_MANIFEST_DIR`.
pub fn embedded_migrations() -> Result<Vec<ferrioxid_migrations::MigrationFile>, AuthError> {
    ferrioxid_migrations::require_embedded_migrations(
        "ferrioxid-auth",
        EXPECTED_MIGRATIONS,
        EMBEDDED_MIGRATIONS,
    )
    .map_err(|error| AuthError::Validation(error.to_string()))
}

pub fn schema() -> Result<SchemaSnapshot, AuthError> {
    let snapshot = SchemaSnapshot::from_tables(vec![
        users(),
        roles(),
        permissions(),
        sessions(),
        user_roles(),
        role_permissions(),
    ]);
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

/// Hängt `operation_id` an die bestehende Audit-Tabelle. `0002` bleibt versiegelt.
#[allow(dead_code)]
pub fn operation_migration_file() -> Result<MigrationFile, AuthError> {
    let previous =
        ferrioxid_migrations::load_migration(&migrations_dir().join("0002_admin_journal.json"))
            .map_err(|error| AuthError::Validation(error.to_string()))?;
    let mut to = previous.to_snapshot.clone();
    let table = to
        .tables
        .iter_mut()
        .find(|table| table.name == ADMIN_AUDIT)
        .ok_or_else(|| AuthError::Validation("audit table is missing from 0002".into()))?;
    if table
        .columns
        .iter()
        .any(|column| column.name == "operation_id")
    {
        return Err(AuthError::Validation(
            "operation_id is already in the sealed journal".into(),
        ));
    }
    table.columns.push(ColumnSnapshot {
        name: "operation_id".to_owned(),
        logical_type: LogicalType::String,
        primary_key: false,
        nullable: true,
        default: None,
        precision: None,
        scale: None,
        choices: Vec::new(),
    });
    to.validate()
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    let plan = plan_change(&previous.to_snapshot, &to)
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    if let Some(reason) = &plan.refusal {
        return Err(AuthError::Validation(reason.clone()));
    }
    seal(
        "0003_audit_operation".to_owned(),
        "audit_operation".to_owned(),
        previous.to_snapshot,
        to,
        &plan,
    )
    .map_err(|error| AuthError::Validation(error.to_string()))
}

/// Reset-Token, Austausch-Cookie und Versandprotokoll. `0001` bis `0003` bleiben versiegelt.
#[cfg(test)]
fn password_reset_migration_file() -> Result<MigrationFile, AuthError> {
    let previous =
        ferrioxid_migrations::load_migration(&migrations_dir().join("0003_audit_operation.json"))
            .map_err(|error| AuthError::Validation(error.to_string()))?;
    let mut to = previous.to_snapshot.clone();
    to.tables.push(password_resets());
    to.tables.push(password_reset_exchanges());
    to.tables.push(mail_outbox());
    to.validate()
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    let plan = plan_change(&previous.to_snapshot, &to)
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    if let Some(reason) = &plan.refusal {
        return Err(AuthError::Validation(reason.clone()));
    }
    seal(
        "0004_password_reset".to_owned(),
        "password_reset".to_owned(),
        previous.to_snapshot,
        to,
        &plan,
    )
    .map_err(|error| AuthError::Validation(error.to_string()))
}

/// Anonyme serverseitige Sitzungen. `0001` bis `0004` bleiben versiegelt.
#[cfg(test)]
fn anonymous_migration_file() -> Result<MigrationFile, AuthError> {
    let previous =
        ferrioxid_migrations::load_migration(&migrations_dir().join("0004_password_reset.json"))
            .map_err(|error| AuthError::Validation(error.to_string()))?;
    let mut to = previous.to_snapshot.clone();
    to.tables.push(anonymous_sessions());
    to.validate()
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    let plan = plan_change(&previous.to_snapshot, &to)
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    if let Some(reason) = &plan.refusal {
        return Err(AuthError::Validation(reason.clone()));
    }
    seal(
        "0005_anonymous_sessions".to_owned(),
        "anonymous_sessions".to_owned(),
        previous.to_snapshot,
        to,
        &plan,
    )
    .map_err(|error| AuthError::Validation(error.to_string()))
}

#[cfg(test)]
fn ensure_account_security_migration() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let path = migrations_dir().join("0006_account_security.json");
        if path.exists() {
            return;
        }
        let file =
            account_security_migration_file().expect("account security schema can be sealed");
        ferrioxid_migrations::save_migration(&migrations_dir(), &file).expect("write migration");
    });
}

#[cfg(test)]
fn account_security_migration_file() -> Result<MigrationFile, AuthError> {
    let previous = ferrioxid_migrations::load_migration(
        &migrations_dir().join("0005_anonymous_sessions.json"),
    )
    .map_err(|error| AuthError::Validation(error.to_string()))?;
    let mut to = previous.to_snapshot.clone();
    to.tables.push(mfa_totp());
    to.tables.push(recovery_codes());
    to.tables.push(login_challenges());
    to.validate()
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    let plan = plan_change(&previous.to_snapshot, &to)
        .map_err(|error| AuthError::Validation(error.to_string()))?;
    if let Some(reason) = &plan.refusal {
        return Err(AuthError::Validation(reason.clone()));
    }
    seal(
        "0006_account_security".to_owned(),
        "account_security".to_owned(),
        previous.to_snapshot,
        to,
        &plan,
    )
    .map_err(|error| AuthError::Validation(error.to_string()))
}

#[cfg(test)]
fn mfa_totp() -> TableSnapshot {
    table(
        MFA_TOTP,
        vec![
            column("user_id", LogicalType::Uuid, true, false, None),
            column("nonce", LogicalType::String, false, false, None),
            column("ciphertext", LogicalType::String, false, false, None),
            column("confirmed_at", LogicalType::I64, false, true, None),
            column("created_at", LogicalType::I64, false, false, None),
            column("last_used_step", LogicalType::I64, false, true, None),
        ],
        Vec::new(),
        Vec::new(),
        vec![foreign_key(MFA_TOTP, "user_id", USERS)],
    )
}

#[cfg(test)]
fn recovery_codes() -> TableSnapshot {
    table(
        RECOVERY_CODES,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("user_id", LogicalType::Uuid, false, false, None),
            column("code_hash", LogicalType::String, false, false, None),
            column("used_at", LogicalType::I64, false, true, None),
            column("created_at", LogicalType::I64, false, false, None),
        ],
        vec![unique(RECOVERY_CODES, &["code_hash"])],
        Vec::new(),
        vec![foreign_key(RECOVERY_CODES, "user_id", USERS)],
    )
}

#[cfg(test)]
fn login_challenges() -> TableSnapshot {
    table(
        LOGIN_CHALLENGES,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("user_id", LogicalType::Uuid, false, false, None),
            column("token_hash", LogicalType::String, false, false, None),
            column("expires_at", LogicalType::I64, false, false, None),
            column("consumed_at", LogicalType::I64, false, true, None),
            column("attempts", LogicalType::I64, false, false, None),
            column("created_at", LogicalType::I64, false, false, None),
        ],
        vec![unique(LOGIN_CHALLENGES, &["token_hash"])],
        Vec::new(),
        vec![foreign_key(LOGIN_CHALLENGES, "user_id", USERS)],
    )
}

#[cfg(test)]
fn anonymous_sessions() -> TableSnapshot {
    table(
        ANONYMOUS_SESSIONS,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("token_hash", LogicalType::String, false, false, None),
            column("csrf_token_hash", LogicalType::String, false, false, None),
            column("state", LogicalType::String, false, true, None),
            column("created_at", LogicalType::I64, false, false, None),
            column("expires_at", LogicalType::I64, false, false, None),
            column("last_seen_at", LogicalType::I64, false, false, None),
            column("revoked_at", LogicalType::I64, false, true, None),
        ],
        vec![unique(ANONYMOUS_SESSIONS, &["token_hash"])],
        Vec::new(),
        Vec::new(),
    )
}

#[cfg(test)]
fn password_resets() -> TableSnapshot {
    table(
        PASSWORD_RESETS,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("user_id", LogicalType::Uuid, false, false, None),
            column("token_hash", LogicalType::String, false, false, None),
            column("created_at", LogicalType::I64, false, false, None),
            column("expires_at", LogicalType::I64, false, false, None),
            column("used_at", LogicalType::I64, false, true, None),
        ],
        vec![unique(PASSWORD_RESETS, &["token_hash"])],
        vec![IndexSnapshot {
            name: format!("ix_{PASSWORD_RESETS}_user_id"),
            columns: vec!["user_id".to_owned()],
        }],
        vec![foreign_key(PASSWORD_RESETS, "user_id", USERS)],
    )
}

#[cfg(test)]
fn password_reset_exchanges() -> TableSnapshot {
    table(
        PASSWORD_RESET_EXCHANGES,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("reset_id", LogicalType::Uuid, false, false, None),
            column("secret_hash", LogicalType::String, false, false, None),
            column("created_at", LogicalType::I64, false, false, None),
            column("expires_at", LogicalType::I64, false, false, None),
        ],
        vec![unique(PASSWORD_RESET_EXCHANGES, &["secret_hash"])],
        vec![IndexSnapshot {
            name: format!("ix_{PASSWORD_RESET_EXCHANGES}_reset_id"),
            columns: vec!["reset_id".to_owned()],
        }],
        vec![foreign_key(
            PASSWORD_RESET_EXCHANGES,
            "reset_id",
            PASSWORD_RESETS,
        )],
    )
}

#[cfg(test)]
fn mail_outbox() -> TableSnapshot {
    table(
        MAIL_OUTBOX,
        vec![
            column("id", LogicalType::Uuid, true, false, None),
            column("kind", LogicalType::String, false, false, None),
            column("recipient", LogicalType::String, false, false, None),
            column("reset_id", LogicalType::Uuid, false, true, None),
            column("created_at", LogicalType::I64, false, false, None),
            column("sent_at", LogicalType::I64, false, true, None),
            column("failed_at", LogicalType::I64, false, true, None),
            column("attempts", LogicalType::I64, false, false, None),
            column("error_code", LogicalType::String, false, true, None),
        ],
        Vec::new(),
        vec![IndexSnapshot {
            name: format!("ix_{MAIL_OUTBOX}_created_at"),
            columns: vec!["created_at".to_owned()],
        }],
        Vec::new(),
    )
}

pub async fn apply_migrations(db: &DatabaseConnection) -> Result<Vec<String>, AuthError> {
    let files = embedded_migrations()?;
    let report =
        ferrioxid_migrations::apply_files_with_history(db, &files, false, AUTH_HISTORY_TABLE)
            .await
            .map_err(|error| AuthError::Validation(error.to_string()))?;
    Ok(report.applied)
}

/// Read-only auth migration status. It does not create tables or apply migrations.
pub async fn migration_status(
    db: &DatabaseConnection,
) -> Result<ferrioxid_migrations::StatusReport, AuthError> {
    let files = embedded_migrations()?;
    ferrioxid_migrations::status_of_files(db, &files, AUTH_HISTORY_TABLE, None)
        .await
        .map_err(|error| AuthError::Validation(error.to_string()))
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
        precision: None,
        scale: None,
        choices: Vec::new(),
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
            ferrioxid_migrations::save_migration(&migrations_dir(), &file)
                .expect("write migration");
        }
        let loaded = ferrioxid_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0001_auth_schema");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(loaded.to_snapshot.tables.len() >= 6);
    }

    #[test]
    fn admin_journal_migration_is_sealed() {
        let file = admin_migration_file().expect("admin journal can be sealed");
        let path = migrations_dir().join("0002_admin_journal.json");
        if !path.exists() {
            ferrioxid_migrations::save_migration(&migrations_dir(), &file)
                .expect("write migration");
        }
        let loaded = ferrioxid_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0002_admin_journal");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(loaded.operations.len() >= 2);
    }

    #[test]
    fn audit_operation_migration_is_sealed() {
        let file = super::operation_migration_file().expect("operation migration");
        let path = migrations_dir().join("0003_audit_operation.json");
        if !path.exists() {
            ferrioxid_migrations::save_migration(&migrations_dir(), &file)
                .expect("write migration");
        }
        let loaded = ferrioxid_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0003_audit_operation");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(loaded
            .operations
            .iter()
            .any(|operation| format!("{operation:?}").contains("operation_id")));
    }

    #[test]
    fn password_reset_migration_is_sealed() {
        let file = super::password_reset_migration_file().expect("password reset migration");
        let path = migrations_dir().join("0004_password_reset.json");
        if !path.exists() {
            ferrioxid_migrations::save_migration(&migrations_dir(), &file)
                .expect("write migration");
        }
        let loaded = ferrioxid_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0004_password_reset");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(loaded
            .operations
            .iter()
            .any(|operation| format!("{operation:?}").contains("_ferrioxid_password_resets")));
    }

    #[test]
    fn anonymous_session_migration_is_sealed() {
        let file = super::anonymous_migration_file().expect("anonymous session migration");
        let path = migrations_dir().join("0005_anonymous_sessions.json");
        if !path.exists() {
            ferrioxid_migrations::save_migration(&migrations_dir(), &file)
                .expect("write migration");
        }
        let loaded = ferrioxid_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0005_anonymous_sessions");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(loaded
            .operations
            .iter()
            .any(|operation| format!("{operation:?}").contains("_ferrioxid_anonymous_sessions")));
    }

    #[test]
    fn account_security_migration_is_sealed() {
        super::ensure_account_security_migration();
        let file = super::account_security_migration_file().expect("account security migration");
        let path = migrations_dir().join("0006_account_security.json");
        let loaded = ferrioxid_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0006_account_security");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(loaded
            .operations
            .iter()
            .any(|operation| format!("{operation:?}").contains("_ferrioxid_mfa_totp")));
    }

    #[tokio::test]
    async fn embedded_migrations_run_without_the_source_directory() {
        let files = super::embedded_migrations().expect("embedded auth migrations");
        assert_eq!(files.len(), 6);
        let db = ferrioxid_db::connect("sqlite::memory:").await.unwrap();
        let applied = super::apply_migrations(&db).await.unwrap();
        assert_eq!(applied.len(), 6);
        let again = super::apply_migrations(&db).await.unwrap();
        assert!(again.is_empty());
        let status = super::migration_status(&db).await.unwrap();
        assert!(status.conflicts.is_empty());
        assert!(status.lines.iter().all(|line| line.state == "angewendet"));
        let auth = crate::AuthService::new(db, crate::AuthConfig::development());
        let password = ephemeral_password();
        auth.create_user("embedded@example.com", &password, false, false)
            .await
            .unwrap();
        auth.login("embedded@example.com", &password, "127.0.0.1")
            .await
            .unwrap();
    }

    fn ephemeral_password() -> String {
        let mut bytes = [0u8; 16];
        getrandom::getrandom(&mut bytes).expect("test entropy");
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
