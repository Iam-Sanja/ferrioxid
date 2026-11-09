//! SQL für Auth-Tabellen über SeaQuery. Keine zweite ORM-Schicht.

use crate::error::AuthError;
use crate::schema::{
    ADMIN_AUDIT, ANONYMOUS_SESSIONS, MAIL_OUTBOX, PASSWORD_RESETS, PASSWORD_RESET_EXCHANGES,
    PERMISSIONS, ROLES, ROLE_PERMISSIONS, SESSIONS, USERS, USER_ROLES,
};
use sea_orm::sea_query::{Alias, Expr, Order, PostgresQueryBuilder, Query, SqliteQueryBuilder};
use sea_orm::{ConnectionTrait, DatabaseBackend, QueryResult, Statement, Value};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub is_active: bool,
    pub is_staff: bool,
    pub is_superuser: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_login: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AnonymousRow {
    pub id: Uuid,
    pub token_hash: String,
    pub csrf_token_hash: String,
    pub state: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_seen_at: i64,
    pub revoked_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub csrf_token_hash: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_seen_at: i64,
    pub revoked_at: Option<i64>,
}

pub async fn insert_user(db: &impl ConnectionTrait, user: &UserRow) -> Result<(), AuthError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(USERS))
        .columns([
            Alias::new("id"),
            Alias::new("email"),
            Alias::new("password_hash"),
            Alias::new("is_active"),
            Alias::new("is_staff"),
            Alias::new("is_superuser"),
            Alias::new("created_at"),
            Alias::new("updated_at"),
            Alias::new("last_login"),
        ])
        .values_panic([
            text(&user.id.to_string()),
            text(&user.email),
            text(&user.password_hash),
            flag(user.is_active),
            flag(user.is_staff),
            flag(user.is_superuser),
            number(user.created_at),
            number(user.updated_at),
            opt_number(user.last_login),
        ]);
    execute(db, &query).await
}

pub async fn find_user_by_email(
    db: &impl ConnectionTrait,
    email: &str,
) -> Result<Option<UserRow>, AuthError> {
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("email"),
            Alias::new("password_hash"),
            Alias::new("is_active"),
            Alias::new("is_staff"),
            Alias::new("is_superuser"),
            Alias::new("created_at"),
            Alias::new("updated_at"),
            Alias::new("last_login"),
        ])
        .from(Alias::new(USERS))
        .and_where(Expr::col(Alias::new("email")).eq(email));
    let row = one(db, &query).await?;
    row.map(|row| read_user(&row)).transpose()
}

pub async fn find_user_by_id(
    db: &impl ConnectionTrait,
    id: Uuid,
) -> Result<Option<UserRow>, AuthError> {
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("email"),
            Alias::new("password_hash"),
            Alias::new("is_active"),
            Alias::new("is_staff"),
            Alias::new("is_superuser"),
            Alias::new("created_at"),
            Alias::new("updated_at"),
            Alias::new("last_login"),
        ])
        .from(Alias::new(USERS))
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    let row = one(db, &query).await?;
    row.map(|row| read_user(&row)).transpose()
}

pub async fn list_user_rows(db: &impl ConnectionTrait) -> Result<Vec<UserRow>, AuthError> {
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("email"),
            Alias::new("password_hash"),
            Alias::new("is_active"),
            Alias::new("is_staff"),
            Alias::new("is_superuser"),
            Alias::new("created_at"),
            Alias::new("updated_at"),
            Alias::new("last_login"),
        ])
        .from(Alias::new(USERS))
        .order_by(Alias::new("email"), Order::Asc);
    let rows = all(db, &query).await?;
    rows.iter().map(read_user).collect()
}

/// Rollennamen je Benutzer. Das Passwort-Hash wird nicht gelesen.
pub async fn list_user_role_names(
    db: &impl ConnectionTrait,
) -> Result<Vec<(Uuid, String)>, AuthError> {
    let mut roles = Query::select();
    roles
        .columns([Alias::new("id"), Alias::new("name")])
        .from(Alias::new(ROLES));
    let mut names = HashMap::new();
    for row in all(db, &roles).await? {
        names.insert(required_uuid(&row, "id")?, required_text(&row, "name")?);
    }
    let mut links = Query::select();
    links
        .columns([Alias::new("user_id"), Alias::new("role_id")])
        .from(Alias::new(USER_ROLES));
    let mut pairs = Vec::new();
    for row in all(db, &links).await? {
        let user_id = required_uuid(&row, "user_id")?;
        let role_id = required_uuid(&row, "role_id")?;
        if let Some(name) = names.get(&role_id) {
            pairs.push((user_id, name.clone()));
        }
    }
    Ok(pairs)
}

pub async fn update_password(
    db: &impl ConnectionTrait,
    id: Uuid,
    password_hash: &str,
    updated_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(USERS))
        .values([
            (Alias::new("password_hash"), text(password_hash)),
            (Alias::new("updated_at"), number(updated_at)),
        ])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    execute(db, &query).await
}

pub async fn update_last_login(
    db: &impl ConnectionTrait,
    id: Uuid,
    at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(USERS))
        .values([(Alias::new("last_login"), number(at))])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    execute(db, &query).await
}

pub async fn set_active(
    db: &impl ConnectionTrait,
    id: Uuid,
    active: bool,
    updated_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(USERS))
        .values([
            (Alias::new("is_active"), flag(active)),
            (Alias::new("updated_at"), number(updated_at)),
        ])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    execute(db, &query).await
}

pub async fn insert_session(
    db: &impl ConnectionTrait,
    session: &SessionRow,
) -> Result<(), AuthError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(SESSIONS))
        .columns([
            Alias::new("id"),
            Alias::new("user_id"),
            Alias::new("token_hash"),
            Alias::new("csrf_token_hash"),
            Alias::new("created_at"),
            Alias::new("expires_at"),
            Alias::new("last_seen_at"),
            Alias::new("revoked_at"),
        ])
        .values_panic([
            text(&session.id.to_string()),
            text(&session.user_id.to_string()),
            text(&session.token_hash),
            text(&session.csrf_token_hash),
            number(session.created_at),
            number(session.expires_at),
            number(session.last_seen_at),
            opt_number(session.revoked_at),
        ]);
    execute(db, &query).await
}

pub async fn find_session_by_hash(
    db: &impl ConnectionTrait,
    token_hash: &str,
) -> Result<Option<SessionRow>, AuthError> {
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("user_id"),
            Alias::new("token_hash"),
            Alias::new("csrf_token_hash"),
            Alias::new("created_at"),
            Alias::new("expires_at"),
            Alias::new("last_seen_at"),
            Alias::new("revoked_at"),
        ])
        .from(Alias::new(SESSIONS))
        .and_where(Expr::col(Alias::new("token_hash")).eq(token_hash));
    let row = one(db, &query).await?;
    row.map(|row| read_session(&row)).transpose()
}

pub async fn touch_session(
    db: &impl ConnectionTrait,
    id: Uuid,
    last_seen_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(SESSIONS))
        .values([(Alias::new("last_seen_at"), number(last_seen_at))])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    execute(db, &query).await
}

pub async fn list_role_names(db: &impl ConnectionTrait) -> Result<Vec<(Uuid, String)>, AuthError> {
    let mut query = Query::select();
    query
        .columns([Alias::new("id"), Alias::new("name")])
        .from(Alias::new(ROLES))
        .order_by(Alias::new("name"), Order::Asc);
    let rows = all(db, &query).await?;
    rows.iter()
        .map(|row| Ok((required_uuid(row, "id")?, required_text(row, "name")?)))
        .collect()
}

pub async fn list_permission_codes(
    db: &impl ConnectionTrait,
    role_id: Uuid,
) -> Result<Vec<String>, AuthError> {
    let mut query = Query::select();
    query
        .column(Alias::new("code"))
        .from(Alias::new(PERMISSIONS))
        .inner_join(
            Alias::new(ROLE_PERMISSIONS),
            Expr::col((Alias::new(PERMISSIONS), Alias::new("id")))
                .equals((Alias::new(ROLE_PERMISSIONS), Alias::new("permission_id"))),
        )
        .and_where(
            Expr::col((Alias::new(ROLE_PERMISSIONS), Alias::new("role_id")))
                .eq(role_id.to_string()),
        )
        .order_by(Alias::new("code"), Order::Asc);
    let rows = all(db, &query).await?;
    rows.iter().map(|row| required_text(row, "code")).collect()
}

pub async fn update_profile(
    db: &impl ConnectionTrait,
    id: Uuid,
    email: &str,
    active: bool,
    staff: bool,
    updated_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(USERS))
        .values([
            (Alias::new("email"), text(email)),
            (Alias::new("is_active"), flag(active)),
            (Alias::new("is_staff"), flag(staff)),
            (Alias::new("updated_at"), number(updated_at)),
        ])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    execute(db, &query).await
}

pub async fn set_staff(
    db: &impl ConnectionTrait,
    id: Uuid,
    staff: bool,
    updated_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(USERS))
        .values([
            (Alias::new("is_staff"), flag(staff)),
            (Alias::new("updated_at"), number(updated_at)),
        ])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    execute(db, &query).await
}

pub async fn insert_anonymous(
    db: &impl ConnectionTrait,
    session: &AnonymousRow,
) -> Result<(), AuthError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(ANONYMOUS_SESSIONS))
        .columns([
            Alias::new("id"),
            Alias::new("token_hash"),
            Alias::new("csrf_token_hash"),
            Alias::new("state"),
            Alias::new("created_at"),
            Alias::new("expires_at"),
            Alias::new("last_seen_at"),
            Alias::new("revoked_at"),
        ])
        .values_panic([
            text(&session.id.to_string()),
            text(&session.token_hash),
            text(&session.csrf_token_hash),
            opt_text(session.state.as_deref()),
            number(session.created_at),
            number(session.expires_at),
            number(session.last_seen_at),
            opt_number(session.revoked_at),
        ]);
    execute(db, &query).await
}

pub async fn find_anonymous_by_hash(
    db: &impl ConnectionTrait,
    token_hash: &str,
) -> Result<Option<AnonymousRow>, AuthError> {
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("token_hash"),
            Alias::new("csrf_token_hash"),
            Alias::new("state"),
            Alias::new("created_at"),
            Alias::new("expires_at"),
            Alias::new("last_seen_at"),
            Alias::new("revoked_at"),
        ])
        .from(Alias::new(ANONYMOUS_SESSIONS))
        .and_where(Expr::col(Alias::new("token_hash")).eq(token_hash));
    let row = one(db, &query).await?;
    row.map(|row| read_anonymous(&row)).transpose()
}

pub async fn touch_anonymous(
    db: &impl ConnectionTrait,
    id: Uuid,
    last_seen_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(ANONYMOUS_SESSIONS))
        .values([(Alias::new("last_seen_at"), number(last_seen_at))])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    execute(db, &query).await
}

pub async fn revoke_anonymous(
    db: &impl ConnectionTrait,
    id: Uuid,
    revoked_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(ANONYMOUS_SESSIONS))
        .values([(Alias::new("revoked_at"), number(revoked_at))])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()))
        .and_where(Expr::col(Alias::new("revoked_at")).is_null());
    execute(db, &query).await
}

pub async fn write_anonymous_state(
    db: &impl ConnectionTrait,
    id: Uuid,
    state: &str,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(ANONYMOUS_SESSIONS))
        .values([(Alias::new("state"), text(state))])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()))
        .and_where(Expr::col(Alias::new("revoked_at")).is_null());
    execute(db, &query).await
}

pub async fn revoke_session(
    db: &impl ConnectionTrait,
    id: Uuid,
    revoked_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(SESSIONS))
        .values([(Alias::new("revoked_at"), number(revoked_at))])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()))
        .and_where(Expr::col(Alias::new("revoked_at")).is_null());
    execute(db, &query).await
}

pub async fn revoke_user_sessions(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    revoked_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(SESSIONS))
        .values([(Alias::new("revoked_at"), number(revoked_at))])
        .and_where(Expr::col(Alias::new("user_id")).eq(user_id.to_string()))
        .and_where(Expr::col(Alias::new("revoked_at")).is_null());
    execute(db, &query).await
}

#[derive(Debug, Clone)]
pub struct PasswordResetRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub expires_at: i64,
    pub used_at: Option<i64>,
}

pub async fn insert_password_reset(
    db: &impl ConnectionTrait,
    id: Uuid,
    user_id: Uuid,
    token_hash: &str,
    created_at: i64,
    expires_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(PASSWORD_RESETS))
        .columns([
            Alias::new("id"),
            Alias::new("user_id"),
            Alias::new("token_hash"),
            Alias::new("created_at"),
            Alias::new("expires_at"),
            Alias::new("used_at"),
        ])
        .values_panic([
            text(&id.to_string()),
            text(&user_id.to_string()),
            text(token_hash),
            number(created_at),
            number(expires_at),
            opt_number(None),
        ]);
    execute(db, &query).await
}

pub async fn active_password_resets(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    now: i64,
) -> Result<Vec<(Uuid, i64)>, AuthError> {
    let mut query = Query::select();
    query
        .columns([Alias::new("id"), Alias::new("created_at")])
        .from(Alias::new(PASSWORD_RESETS))
        .and_where(Expr::col(Alias::new("user_id")).eq(user_id.to_string()))
        .and_where(Expr::col(Alias::new("used_at")).is_null())
        .and_where(Expr::col(Alias::new("expires_at")).gt(now))
        .order_by(Alias::new("created_at"), Order::Asc);
    let mut rows = Vec::new();
    for row in all(db, &query).await? {
        rows.push((
            required_uuid(&row, "id")?,
            required_i64(&row, "created_at")?,
        ));
    }
    Ok(rows)
}

pub async fn consume_password_reset(
    db: &impl ConnectionTrait,
    id: Uuid,
    now: i64,
) -> Result<bool, AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(PASSWORD_RESETS))
        .values([(Alias::new("used_at"), number(now))])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()))
        .and_where(Expr::col(Alias::new("used_at")).is_null())
        .and_where(Expr::col(Alias::new("expires_at")).gt(now));
    Ok(execute_rows(db, &query).await? == 1)
}

pub async fn invalidate_other_resets(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    keep: Uuid,
    now: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(PASSWORD_RESETS))
        .values([(Alias::new("used_at"), number(now))])
        .and_where(Expr::col(Alias::new("user_id")).eq(user_id.to_string()))
        .and_where(Expr::col(Alias::new("id")).ne(keep.to_string()))
        .and_where(Expr::col(Alias::new("used_at")).is_null());
    execute(db, &query).await
}

pub async fn find_password_reset_by_hash(
    db: &impl ConnectionTrait,
    token_hash: &str,
) -> Result<Option<PasswordResetRow>, AuthError> {
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("user_id"),
            Alias::new("expires_at"),
            Alias::new("used_at"),
        ])
        .from(Alias::new(PASSWORD_RESETS))
        .and_where(Expr::col(Alias::new("token_hash")).eq(token_hash));
    let Some(row) = one(db, &query).await? else {
        return Ok(None);
    };
    Ok(Some(PasswordResetRow {
        id: required_uuid(&row, "id")?,
        user_id: required_uuid(&row, "user_id")?,
        expires_at: required_i64(&row, "expires_at")?,
        used_at: optional_i64(&row, "used_at")?,
    }))
}

pub async fn insert_reset_exchange(
    db: &impl ConnectionTrait,
    id: Uuid,
    reset_id: Uuid,
    secret_hash: &str,
    created_at: i64,
    expires_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(PASSWORD_RESET_EXCHANGES))
        .columns([
            Alias::new("id"),
            Alias::new("reset_id"),
            Alias::new("secret_hash"),
            Alias::new("created_at"),
            Alias::new("expires_at"),
        ])
        .values_panic([
            text(&id.to_string()),
            text(&reset_id.to_string()),
            text(secret_hash),
            number(created_at),
            number(expires_at),
        ]);
    execute(db, &query).await
}

pub async fn find_exchange(
    db: &impl ConnectionTrait,
    secret_hash: &str,
) -> Result<Option<(Uuid, i64, PasswordResetRow)>, AuthError> {
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("reset_id"),
            Alias::new("expires_at"),
        ])
        .from(Alias::new(PASSWORD_RESET_EXCHANGES))
        .and_where(Expr::col(Alias::new("secret_hash")).eq(secret_hash));
    let Some(row) = one(db, &query).await? else {
        return Ok(None);
    };
    let exchange_id = required_uuid(&row, "id")?;
    let exchange_expires = required_i64(&row, "expires_at")?;
    let reset_id = required_uuid(&row, "reset_id")?;
    let mut reset = Query::select();
    reset
        .columns([
            Alias::new("id"),
            Alias::new("user_id"),
            Alias::new("expires_at"),
            Alias::new("used_at"),
        ])
        .from(Alias::new(PASSWORD_RESETS))
        .and_where(Expr::col(Alias::new("id")).eq(reset_id.to_string()));
    let Some(row) = one(db, &reset).await? else {
        return Ok(None);
    };
    Ok(Some((
        exchange_id,
        exchange_expires,
        PasswordResetRow {
            id: required_uuid(&row, "id")?,
            user_id: required_uuid(&row, "user_id")?,
            expires_at: required_i64(&row, "expires_at")?,
            used_at: optional_i64(&row, "used_at")?,
        },
    )))
}

pub async fn delete_user_exchanges(
    db: &impl ConnectionTrait,
    user_id: Uuid,
) -> Result<(), AuthError> {
    let mut ids = Query::select();
    ids.column(Alias::new("id"))
        .from(Alias::new(PASSWORD_RESETS))
        .and_where(Expr::col(Alias::new("user_id")).eq(user_id.to_string()));
    let mut reset_ids = Vec::new();
    for row in all(db, &ids).await? {
        reset_ids.push(required_uuid(&row, "id")?);
    }
    for reset_id in reset_ids {
        let mut query = Query::delete();
        query
            .from_table(Alias::new(PASSWORD_RESET_EXCHANGES))
            .and_where(Expr::col(Alias::new("reset_id")).eq(reset_id.to_string()));
        execute(db, &query).await?;
    }
    Ok(())
}

pub async fn insert_outbox(
    db: &impl ConnectionTrait,
    id: Uuid,
    kind: &str,
    recipient: &str,
    reset_id: Option<Uuid>,
    created_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(MAIL_OUTBOX))
        .columns([
            Alias::new("id"),
            Alias::new("kind"),
            Alias::new("recipient"),
            Alias::new("reset_id"),
            Alias::new("created_at"),
            Alias::new("sent_at"),
            Alias::new("failed_at"),
            Alias::new("attempts"),
            Alias::new("error_code"),
        ])
        .values_panic([
            text(&id.to_string()),
            text(kind),
            text(recipient),
            match reset_id {
                Some(id) => text(&id.to_string()),
                None => Expr::value(Value::String(None)),
            },
            number(created_at),
            opt_number(None),
            opt_number(None),
            number(0),
            Expr::value(Value::String(None)),
        ]);
    execute(db, &query).await
}

pub async fn finish_outbox(
    db: &impl ConnectionTrait,
    id: Uuid,
    sent_at: Option<i64>,
    failed_at: Option<i64>,
    error_code: Option<&str>,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(MAIL_OUTBOX))
        .values([
            (Alias::new("sent_at"), opt_number(sent_at)),
            (Alias::new("failed_at"), opt_number(failed_at)),
            (Alias::new("attempts"), number(1)),
            (
                Alias::new("error_code"),
                match error_code {
                    Some(code) => text(code),
                    None => Expr::value(Value::String(None)),
                },
            ),
        ])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    execute(db, &query).await
}

pub async fn stale_outbox(
    db: &impl ConnectionTrait,
    older_than: i64,
) -> Result<Vec<(Uuid, Option<Uuid>)>, AuthError> {
    let mut query = Query::select();
    query
        .columns([Alias::new("id"), Alias::new("reset_id")])
        .from(Alias::new(MAIL_OUTBOX))
        .and_where(Expr::col(Alias::new("sent_at")).is_null())
        .and_where(Expr::col(Alias::new("failed_at")).is_null())
        .and_where(Expr::col(Alias::new("created_at")).lt(older_than));
    let mut rows = Vec::new();
    for row in all(db, &query).await? {
        rows.push((
            required_uuid(&row, "id")?,
            optional_text_uuid(&row, "reset_id")?,
        ));
    }
    Ok(rows)
}

pub async fn insert_password_audit(
    db: &impl ConnectionTrait,
    actor_id: Uuid,
    summary: &str,
    operation_id: Uuid,
    created_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::insert();
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
        .values_panic([
            text(&Uuid::new_v4().to_string()),
            number(created_at),
            text(&actor_id.to_string()),
            text("_ferrioxid_users"),
            text(&actor_id.to_string()),
            text("password_reset"),
            text("ok"),
            text(summary),
            text(&operation_id.to_string()),
        ]);
    execute(db, &query).await
}

pub async fn find_named_row(
    db: &impl ConnectionTrait,
    table: &str,
    column: &str,
    value: &str,
) -> Result<Option<Uuid>, AuthError> {
    let mut query = Query::select();
    query
        .column(Alias::new("id"))
        .from(Alias::new(table))
        .and_where(Expr::col(Alias::new(column)).eq(value));
    one(db, &query).await?.map(|row| parse_id(&row)).transpose()
}

pub async fn ensure_named_row(
    db: &impl ConnectionTrait,
    table: &str,
    column: &str,
    value: &str,
) -> Result<Uuid, AuthError> {
    if let Some(id) = find_named_row(db, table, column, value).await? {
        return Ok(id);
    }
    let id = Uuid::new_v4();
    let mut insert = Query::insert();
    insert
        .into_table(Alias::new(table))
        .columns([Alias::new("id"), Alias::new(column)])
        .values_panic([text(&id.to_string()), text(value)]);
    execute(db, &insert).await?;
    Ok(id)
}

pub async fn link_rows(
    db: &impl ConnectionTrait,
    table: &str,
    left_column: &str,
    left: Uuid,
    right_column: &str,
    right: Uuid,
) -> Result<(), AuthError> {
    let mut existing = Query::select();
    existing
        .column(Alias::new("id"))
        .from(Alias::new(table))
        .and_where(Expr::col(Alias::new(left_column)).eq(left.to_string()))
        .and_where(Expr::col(Alias::new(right_column)).eq(right.to_string()));
    if one(db, &existing).await?.is_some() {
        return Ok(());
    }
    let mut insert = Query::insert();
    insert
        .into_table(Alias::new(table))
        .columns([
            Alias::new("id"),
            Alias::new(left_column),
            Alias::new(right_column),
        ])
        .values_panic([
            text(&Uuid::new_v4().to_string()),
            text(&left.to_string()),
            text(&right.to_string()),
        ]);
    execute(db, &insert).await
}

pub async fn unlink_rows(
    db: &impl ConnectionTrait,
    table: &str,
    left_column: &str,
    left: Uuid,
    right_column: &str,
    right: Uuid,
) -> Result<(), AuthError> {
    let mut query = Query::delete();
    query
        .from_table(Alias::new(table))
        .and_where(Expr::col(Alias::new(left_column)).eq(left.to_string()))
        .and_where(Expr::col(Alias::new(right_column)).eq(right.to_string()));
    execute(db, &query).await
}

pub async fn user_has_code(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    code: &str,
) -> Result<bool, AuthError> {
    let mut query = Query::select();
    query
        .expr(Expr::col((Alias::new(PERMISSIONS), Alias::new("id"))))
        .from(Alias::new(USER_ROLES))
        .inner_join(
            Alias::new(ROLE_PERMISSIONS),
            Expr::col((Alias::new(USER_ROLES), Alias::new("role_id")))
                .equals((Alias::new(ROLE_PERMISSIONS), Alias::new("role_id"))),
        )
        .inner_join(
            Alias::new(PERMISSIONS),
            Expr::col((Alias::new(ROLE_PERMISSIONS), Alias::new("permission_id")))
                .equals((Alias::new(PERMISSIONS), Alias::new("id"))),
        )
        .and_where(
            Expr::col((Alias::new(USER_ROLES), Alias::new("user_id"))).eq(user_id.to_string()),
        )
        .and_where(Expr::col((Alias::new(PERMISSIONS), Alias::new("code"))).eq(code))
        .limit(1);
    Ok(one(db, &query).await?.is_some())
}

fn read_user(row: &QueryResult) -> Result<UserRow, AuthError> {
    Ok(UserRow {
        id: required_uuid(row, "id")?,
        email: required_text(row, "email")?,
        password_hash: required_text(row, "password_hash")?,
        is_active: required_flag(row, "is_active")?,
        is_staff: required_flag(row, "is_staff")?,
        is_superuser: required_flag(row, "is_superuser")?,
        created_at: required_i64(row, "created_at")?,
        updated_at: required_i64(row, "updated_at")?,
        last_login: optional_i64(row, "last_login")?,
    })
}

fn read_anonymous(row: &QueryResult) -> Result<AnonymousRow, AuthError> {
    Ok(AnonymousRow {
        id: required_uuid(row, "id")?,
        token_hash: required_text(row, "token_hash")?,
        csrf_token_hash: required_text(row, "csrf_token_hash")?,
        state: optional_text(row, "state")?,
        created_at: required_i64(row, "created_at")?,
        expires_at: required_i64(row, "expires_at")?,
        last_seen_at: required_i64(row, "last_seen_at")?,
        revoked_at: optional_i64(row, "revoked_at")?,
    })
}

fn read_session(row: &QueryResult) -> Result<SessionRow, AuthError> {
    Ok(SessionRow {
        id: required_uuid(row, "id")?,
        user_id: required_uuid(row, "user_id")?,
        token_hash: required_text(row, "token_hash")?,
        csrf_token_hash: required_text(row, "csrf_token_hash")?,
        created_at: required_i64(row, "created_at")?,
        expires_at: required_i64(row, "expires_at")?,
        last_seen_at: required_i64(row, "last_seen_at")?,
        revoked_at: optional_i64(row, "revoked_at")?,
    })
}

fn parse_id(row: &QueryResult) -> Result<Uuid, AuthError> {
    required_uuid(row, "id")
}

fn optional_text_uuid(row: &QueryResult, column: &str) -> Result<Option<Uuid>, AuthError> {
    match row.try_get::<Option<String>>("", column) {
        Ok(Some(value)) if !value.is_empty() => value
            .parse()
            .map(Some)
            .map_err(|_| AuthError::database(format!("column {column} is not a uuid"))),
        Ok(_) => Ok(None),
        Err(error) => Err(AuthError::database(error)),
    }
}

fn required_uuid(row: &QueryResult, column: &str) -> Result<Uuid, AuthError> {
    if let Ok(value) = row.try_get::<Uuid>("", column) {
        return Ok(value);
    }
    parse_uuid(&required_text(row, column)?)
}

fn parse_uuid(value: &str) -> Result<Uuid, AuthError> {
    Uuid::parse_str(value).map_err(|_| AuthError::Validation("stored uuid is invalid".into()))
}

fn required_text(row: &QueryResult, column: &str) -> Result<String, AuthError> {
    row.try_get::<String>("", column)
        .map_err(|_| AuthError::database(format!("column {column} is not text")))
}

fn required_i64(row: &QueryResult, column: &str) -> Result<i64, AuthError> {
    if let Ok(value) = row.try_get::<i64>("", column) {
        return Ok(value);
    }
    row.try_get::<i32>("", column)
        .map(i64::from)
        .map_err(|_| AuthError::database(format!("column {column} is not an integer")))
}

fn optional_i64(row: &QueryResult, column: &str) -> Result<Option<i64>, AuthError> {
    if let Ok(value) = row.try_get::<Option<i64>>("", column) {
        return Ok(value);
    }
    row.try_get::<Option<i32>>("", column)
        .map(|value| value.map(i64::from))
        .map_err(|_| AuthError::database(format!("column {column} is not an integer")))
}

fn required_flag(row: &QueryResult, column: &str) -> Result<bool, AuthError> {
    if let Ok(value) = row.try_get::<bool>("", column) {
        return Ok(value);
    }
    Ok(required_i64(row, column)? != 0)
}

fn text(value: &str) -> sea_orm::sea_query::SimpleExpr {
    Expr::value(value)
}

fn opt_text(value: Option<&str>) -> sea_orm::sea_query::SimpleExpr {
    match value {
        Some(value) => Expr::value(value),
        None => Expr::value(Option::<String>::None),
    }
}

fn optional_text(row: &QueryResult, column: &str) -> Result<Option<String>, AuthError> {
    row.try_get::<Option<String>>("", column)
        .map_err(|_| AuthError::database(format!("column {column} is not text")))
}

fn number(value: i64) -> sea_orm::sea_query::SimpleExpr {
    Expr::value(value)
}

fn opt_number(value: Option<i64>) -> sea_orm::sea_query::SimpleExpr {
    match value {
        Some(value) => Expr::value(value),
        None => Expr::value(Value::BigInt(None)),
    }
}

fn flag(value: bool) -> sea_orm::sea_query::SimpleExpr {
    Expr::value(value)
}

async fn execute(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<(), AuthError> {
    execute_rows(db, query).await?;
    Ok(())
}

async fn execute_rows(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<u64, AuthError> {
    let result = db.execute(statement(db, query)?).await.map_err(map_write)?;
    Ok(result.rows_affected())
}

async fn one(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Option<QueryResult>, AuthError> {
    db.query_one(statement(db, query)?)
        .await
        .map_err(AuthError::database)
}

async fn all(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Vec<QueryResult>, AuthError> {
    db.query_all(statement(db, query)?)
        .await
        .map_err(AuthError::database)
}

fn statement(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Statement, AuthError> {
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => {
            return Err(AuthError::Validation("MySQL is not supported".into()))
        }
    };
    Ok(Statement::from_sql_and_values(backend, sql, values.0))
}

fn map_write(error: sea_orm::DbErr) -> AuthError {
    if unique_violation(&error) {
        return AuthError::Validation("record already exists".into());
    }
    AuthError::database(error)
}

fn unique_violation(error: &sea_orm::DbErr) -> bool {
    let runtime = match error {
        sea_orm::DbErr::Exec(runtime) | sea_orm::DbErr::Query(runtime) => runtime,
        _ => return false,
    };
    let sea_orm::RuntimeErr::SqlxError(sqlx_error) = runtime else {
        return false;
    };
    let sea_orm::sqlx::Error::Database(database) = sqlx_error else {
        return false;
    };
    database.kind() == sea_orm::sqlx::error::ErrorKind::UniqueViolation
}
