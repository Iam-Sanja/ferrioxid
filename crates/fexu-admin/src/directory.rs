//! Verzeichnisänderungen und ihr Audit teilen sich eine Transaktion.

use crate::audit::{self, Event};
use fexu_auth::{AuthError, AuthService, PublicUser};
use fexu_core::FexuError;
use fexu_db::conclude;
use fexu_db::driver::DatabaseTransaction;
use uuid::Uuid;

pub async fn create_user(
    auth: &AuthService,
    actor: Uuid,
    email: &str,
    password: &str,
    active: bool,
    staff: bool,
    roles: &[String],
) -> Result<PublicUser, FexuError> {
    fexu_auth::validate_new_password(password).map_err(auth_error)?;
    let hash = fexu_auth::hash_password(password).map_err(auth_error)?;
    let tx = auth.begin().await.map_err(auth_error)?;
    let result = async {
        auth.require_directory_actor(&tx, actor)
            .await
            .map_err(auth_error)?;
        let created = auth
            .insert_directory_user_on(&tx, email, &hash, active, staff)
            .await
            .map_err(auth_error)?;
        for role in roles {
            auth.assign_role_on(&tx, &created.email, role)
                .await
                .map_err(auth_error)?;
        }
        let record = created.id.to_string();
        let summary = if roles.is_empty() {
            created.email.clone()
        } else {
            format!("{}\n{}", created.email, roles.join(", "))
        };
        Ok((created, event(actor, record, "user_create", &summary)))
    }
    .await;
    finish(tx, result).await
}

pub async fn update_user(
    auth: &AuthService,
    actor: Uuid,
    id: Uuid,
    email: &str,
    active: bool,
    staff: bool,
) -> Result<PublicUser, FexuError> {
    let tx = auth.begin().await.map_err(auth_error)?;
    let result = async {
        auth.require_directory_actor(&tx, actor)
            .await
            .map_err(auth_error)?;
        let updated = auth
            .update_profile_on(&tx, actor, id, email, active, staff)
            .await
            .map_err(auth_error)?;
        Ok((
            updated.clone(),
            event(actor, id.to_string(), "user_update", &updated.email),
        ))
    }
    .await;
    finish(tx, result).await
}

pub async fn reset_password(
    auth: &AuthService,
    actor: Uuid,
    id: Uuid,
    password: &str,
) -> Result<(), FexuError> {
    fexu_auth::validate_new_password(password).map_err(auth_error)?;
    let hash = fexu_auth::hash_password(password).map_err(auth_error)?;
    let tx = auth.begin().await.map_err(auth_error)?;
    let result = apply_password(auth, &tx, actor, id, &hash)
        .await
        .map(|()| ((), event(actor, id.to_string(), "password_reset", "")));
    finish(tx, result).await
}

pub async fn apply_password(
    auth: &AuthService,
    tx: &DatabaseTransaction,
    actor: Uuid,
    id: Uuid,
    password_hash: &str,
) -> Result<(), FexuError> {
    auth.require_directory_actor(tx, actor)
        .await
        .map_err(auth_error)?;
    auth.reset_password_hash_on(tx, id, password_hash)
        .await
        .map_err(auth_error)
}

pub async fn set_active(
    auth: &AuthService,
    actor: Uuid,
    id: Uuid,
    active: bool,
) -> Result<(), FexuError> {
    if actor == id && !active {
        return Err(FexuError::Forbidden);
    }
    let tx = auth.begin().await.map_err(auth_error)?;
    let result = async {
        auth.require_directory_actor(&tx, actor)
            .await
            .map_err(auth_error)?;
        auth.set_active_on(&tx, id, active)
            .await
            .map_err(auth_error)?;
        Ok((
            (),
            event(
                actor,
                id.to_string(),
                "user_update",
                if active { "active" } else { "inactive" },
            ),
        ))
    }
    .await;
    finish(tx, result).await
}

pub async fn set_staff(
    auth: &AuthService,
    actor: Uuid,
    id: Uuid,
    staff: bool,
) -> Result<(), FexuError> {
    let tx = auth.begin().await.map_err(auth_error)?;
    let result = async {
        auth.require_directory_actor(&tx, actor)
            .await
            .map_err(auth_error)?;
        auth.set_staff_on(&tx, actor, id, staff)
            .await
            .map_err(auth_error)?;
        Ok((
            (),
            event(
                actor,
                id.to_string(),
                "user_update",
                if staff { "staff" } else { "not-staff" },
            ),
        ))
    }
    .await;
    finish(tx, result).await
}

pub async fn assign_role(
    auth: &AuthService,
    actor: Uuid,
    email: &str,
    role: &str,
    grant: bool,
) -> Result<(), FexuError> {
    let tx = auth.begin().await.map_err(auth_error)?;
    let result = async {
        auth.require_directory_actor(&tx, actor)
            .await
            .map_err(auth_error)?;
        let user_id = if grant {
            auth.assign_role_on(&tx, email, role).await
        } else {
            auth.revoke_role_on(&tx, email, role).await
        }
        .map_err(auth_error)?;
        Ok((
            (),
            event(
                actor,
                user_id.to_string(),
                if grant { "role_assign" } else { "role_revoke" },
                role,
            ),
        ))
    }
    .await;
    finish(tx, result).await
}

pub async fn grant_permission(
    auth: &AuthService,
    actor: Uuid,
    role: &str,
    code: &str,
    grant: bool,
) -> Result<(), FexuError> {
    if code.contains("superuser") {
        return Err(FexuError::Forbidden);
    }
    let tx = auth.begin().await.map_err(auth_error)?;
    let result = async {
        auth.require_directory_actor(&tx, actor)
            .await
            .map_err(auth_error)?;
        if grant {
            auth.grant_permission_on(&tx, role, code).await
        } else {
            auth.revoke_permission_on(&tx, role, code).await
        }
        .map_err(auth_error)?;
        Ok((
            (),
            event(
                actor,
                role.to_owned(),
                if grant {
                    "permission_grant"
                } else {
                    "permission_revoke"
                },
                code,
            ),
        ))
    }
    .await;
    finish(tx, result).await
}

fn event(actor: Uuid, record: String, action: &str, summary: &str) -> Event {
    Event {
        actor_id: actor,
        model_key: "_fexu_users".to_owned(),
        record_id: clip(record, 80),
        action: action.to_owned(),
        outcome: "ok".to_owned(),
        summary: clip(summary.to_owned(), 1500),
        operation_id: Some(Uuid::new_v4()),
    }
}

fn clip(value: String, max: usize) -> String {
    value.chars().take(max).collect()
}

async fn finish<T>(
    tx: DatabaseTransaction,
    result: Result<(T, Event), FexuError>,
) -> Result<T, FexuError> {
    let result = match result {
        Ok((value, event)) => audit::insert(&tx, &event).await.map(|()| value),
        Err(error) => Err(error),
    };
    conclude(tx, result, FexuError::database).await
}

fn auth_error(error: AuthError) -> FexuError {
    match error {
        AuthError::Forbidden => FexuError::Forbidden,
        AuthError::Validation(text) => FexuError::Validation(text),
        _ => FexuError::database(std::io::Error::other("directory write failed")),
    }
}

#[cfg(test)]
mod tests {
    use super::{assign_role, grant_permission, reset_password, set_active};
    use crate::audit::{self, fail_next_insert};
    use fexu_auth::{AuthConfig, AuthService};
    use fexu_core::FexuError;
    use fexu_db::driver::{ConnectionTrait, DatabaseBackend, Statement, Value as DbValue};

    fn ephemeral_password() -> String {
        uuid::Uuid::new_v4().simple().to_string()
    }

    async fn service() -> AuthService {
        let db = fexu_db::connect("sqlite::memory:").await.expect("sqlite");
        fexu_auth::apply_migrations(&db).await.expect("migrate");
        AuthService::new(db, AuthConfig::development())
    }

    #[tokio::test]
    async fn password_audit_commits_and_rolls_back_together() {
        let auth = service().await;
        let current = ephemeral_password();
        let replacement = ephemeral_password();
        let actor = auth
            .create_user("root@example.com", &current, true, true)
            .await
            .unwrap();
        let target = auth
            .create_user("person@example.com", &current, true, false)
            .await
            .unwrap();
        let old = auth
            .login("person@example.com", &current, "peer")
            .await
            .unwrap();
        fail_next_insert();
        let failed = reset_password(&auth, actor.id, target.id, &replacement).await;
        assert!(matches!(failed, Err(FexuError::Database(_))));
        auth.authenticate_token(&old.token)
            .await
            .expect("session remains");
        auth.login("person@example.com", &current, "peer")
            .await
            .expect("old password");
        assert!(auth
            .login("person@example.com", &replacement, "peer")
            .await
            .is_err());
        let (events, _) = audit::page(auth.database(), None, 0).await.unwrap();
        assert!(events.is_empty());

        reset_password(&auth, actor.id, target.id, &replacement)
            .await
            .unwrap();
        assert!(auth.authenticate_token(&old.token).await.is_err());
        auth.login("person@example.com", &replacement, "peer")
            .await
            .unwrap();
        let (events, _) = audit::page(auth.database(), None, 0).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "password_reset");
        assert!(events[0].operation_id.is_some());
        assert!(!events[0].summary.contains(&replacement));
        assert!(!events[0].summary.contains('$'));
    }

    #[tokio::test]
    async fn deactivation_and_roles_share_the_audit_transaction() {
        let auth = service().await;
        let password = ephemeral_password();
        let actor = auth
            .create_user("root@example.com", &password, true, true)
            .await
            .unwrap();
        let target = auth
            .create_user("person@example.com", &password, true, false)
            .await
            .unwrap();
        let session = auth
            .login("person@example.com", &password, "peer")
            .await
            .unwrap();
        fail_next_insert();
        assert!(set_active(&auth, actor.id, target.id, false).await.is_err());
        auth.authenticate_token(&session.token)
            .await
            .expect("session remains");
        set_active(&auth, actor.id, target.id, false).await.unwrap();
        assert!(auth.authenticate_token(&session.token).await.is_err());

        auth.ensure_role("clerk").await.unwrap();
        fail_next_insert();
        assert!(
            assign_role(&auth, actor.id, "person@example.com", "clerk", true)
                .await
                .is_err()
        );
        let names = auth.list_user_role_names().await.unwrap();
        assert!(!names.iter().any(|(_, name)| name == "clerk"));
        assign_role(&auth, actor.id, "person@example.com", "clerk", true)
            .await
            .unwrap();
        grant_permission(&auth, actor.id, "clerk", "orders.read", true)
            .await
            .unwrap();
        let codes = auth.list_role_permissions("clerk").await.unwrap();
        assert_eq!(codes, vec!["orders.read".to_owned()]);
        assert!(
            grant_permission(&auth, actor.id, "clerk", "superuser", true)
                .await
                .is_err()
        );
        assert!(matches!(
            set_active(&auth, actor.id, actor.id, false).await,
            Err(FexuError::Forbidden)
        ));
        assert!(matches!(
            reset_password(&auth, actor.id, actor.id, &password).await,
            Err(FexuError::Forbidden)
        ));
    }

    #[tokio::test]
    async fn a_revoked_superuser_cannot_finish_the_open_transaction() {
        let auth = service().await;
        let password = ephemeral_password();
        let replacement = ephemeral_password();
        let actor = auth
            .create_user("root@example.com", &password, true, true)
            .await
            .unwrap();
        let target = auth
            .create_user("person@example.com", &password, true, false)
            .await
            .unwrap();
        let hash = fexu_auth::hash_password(&replacement).unwrap();
        let tx = auth.begin().await.unwrap();
        tx.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE \"_fexu_users\" SET \"is_superuser\" = 0 WHERE \"id\" = ?",
            [DbValue::from(actor.id.to_string())],
        ))
        .await
        .unwrap();
        let refused = super::apply_password(&auth, &tx, actor.id, target.id, &hash).await;
        assert!(matches!(refused, Err(FexuError::Forbidden)));
        tx.rollback().await.unwrap();
        auth.login("person@example.com", &password, "peer")
            .await
            .unwrap();
        auth.login("root@example.com", &password, "peer")
            .await
            .unwrap();
    }
}
