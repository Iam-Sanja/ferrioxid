//! Benutzer, Sitzungen und Berechtigungen.

use crate::config::AuthConfig;
use crate::cookie::{clear_cookie, read_cookie, set_cookie};
use crate::error::AuthError;
use crate::password::{
    hash_password, validate_new_password, verify_against_dummy, verify_password,
};
use crate::schema::{PERMISSIONS, ROLES, ROLE_PERMISSIONS, USER_ROLES};
use crate::store::{self, SessionRow, UserRow};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::http::{HeaderMap, Method};
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, TransactionTrait};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct PublicUser {
    pub id: Uuid,
    pub email: String,
    pub is_active: bool,
    pub is_staff: bool,
    pub is_superuser: bool,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub is_active: bool,
    pub is_staff: bool,
    pub is_superuser: bool,
    pub session_id: Uuid,
}

#[derive(Debug)]
pub struct IssuedSession {
    pub token: String,
    pub csrf_token: String,
    pub cookie: String,
    pub user: PublicUser,
}

#[derive(Clone)]
pub struct AuthService {
    db: DatabaseConnection,
    config: AuthConfig,
    limiter: std::sync::Arc<Mutex<HashMap<String, Vec<i64>>>>,
}

impl AuthService {
    pub fn new(db: DatabaseConnection, config: AuthConfig) -> Self {
        if config.environment == crate::config::Environment::Production && !config.cookie_secure {
            tracing::warn!("Produktionskonfiguration sendet Session-Cookies ohne Secure.");
        }
        Self {
            db,
            config,
            limiter: std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn config(&self) -> &AuthConfig {
        &self.config
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn create_user(
        &self,
        email: &str,
        password: &str,
        is_staff: bool,
        is_superuser: bool,
    ) -> Result<PublicUser, AuthError> {
        validate_new_password(password)?;
        let email = normalize_email(email)?;
        let now = unix_now()?;
        let user = UserRow {
            id: Uuid::new_v4(),
            email,
            password_hash: hash_password(password)?,
            is_active: true,
            is_staff,
            is_superuser,
            created_at: now,
            updated_at: now,
            last_login: None,
        };
        store::insert_user(&self.db, &user).await?;
        Ok(public_user(&user))
    }

    pub async fn list_users(&self) -> Result<Vec<PublicUser>, AuthError> {
        let rows = store::list_user_rows(&self.db).await?;
        Ok(rows.iter().map(public_user).collect())
    }

    pub async fn list_user_role_names(&self) -> Result<Vec<(Uuid, String)>, AuthError> {
        store::list_user_role_names(&self.db).await
    }

    pub async fn get_user(&self, id: Uuid) -> Result<Option<PublicUser>, AuthError> {
        Ok(store::find_user_by_id(&self.db, id)
            .await?
            .as_ref()
            .map(public_user))
    }

    /// Admin-Anlage. Superuser bleibt der CLI vorbehalten. Der Hash entsteht vor dem Schreibvorgang.
    pub async fn create_directory_user(
        &self,
        email: &str,
        password: &str,
        is_active: bool,
        is_staff: bool,
    ) -> Result<PublicUser, AuthError> {
        validate_new_password(password)?;
        let hash = hash_password(password)?;
        self.insert_directory_user_on(&self.db, email, &hash, is_active, is_staff)
            .await
    }

    pub async fn update_profile(
        &self,
        id: Uuid,
        email: &str,
        is_active: bool,
        is_staff: bool,
    ) -> Result<PublicUser, AuthError> {
        let user = store::find_user_by_id(&self.db, id)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        if user.is_superuser {
            return Err(AuthError::Forbidden);
        }
        let email = normalize_email(email)?;
        if let Some(existing) = store::find_user_by_email(&self.db, &email).await? {
            if existing.id != user.id {
                return Err(AuthError::Validation("email is already registered".into()));
            }
        }
        let now = unix_now()?;
        store::update_profile(&self.db, user.id, &email, is_active, is_staff, now).await?;
        if !is_active {
            store::revoke_user_sessions(&self.db, user.id, now).await?;
        }
        Ok(PublicUser {
            id: user.id,
            email,
            is_active,
            is_staff,
            is_superuser: false,
        })
    }

    /// Setzt das Passwort ohne das bisherige zu kennen und widerruft alle Sitzungen dieses Benutzers.
    pub async fn reset_password(&self, id: Uuid, password: &str) -> Result<(), AuthError> {
        validate_new_password(password)?;
        let hash = hash_password(password)?;
        let transaction = self.db.begin().await.map_err(AuthError::database)?;
        let result = self.reset_password_hash_on(&transaction, id, &hash).await;
        ferrioxid_db::conclude(transaction, result, AuthError::database).await
    }

    pub async fn list_user_permissions(&self, id: Uuid) -> Result<Vec<String>, AuthError> {
        let links = store::list_user_role_names(&self.db).await?;
        let mut codes = std::collections::BTreeSet::new();
        for (user_id, role) in links {
            if user_id != id {
                continue;
            }
            for code in self.list_role_permissions(&role).await? {
                codes.insert(code);
            }
        }
        Ok(codes.into_iter().collect())
    }

    pub async fn set_user_active(&self, email: &str, active: bool) -> Result<(), AuthError> {
        let email = normalize_email(email)?;
        let user = store::find_user_by_email(&self.db, &email)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        let now = unix_now()?;
        store::set_active(&self.db, user.id, active, now).await?;
        if !active {
            store::revoke_user_sessions(&self.db, user.id, now).await?;
        }
        Ok(())
    }

    pub async fn login(
        &self,
        email: &str,
        password: &str,
        client: &str,
    ) -> Result<IssuedSession, AuthError> {
        let key = format!("{client}:{}", email.trim().to_lowercase());
        if self.limited(&key)? {
            return Err(AuthError::RateLimited);
        }
        let normalized = match normalize_email(email) {
            Ok(value) => value,
            Err(_) => {
                verify_against_dummy(password);
                self.record_failure(&key)?;
                return Err(AuthError::InvalidCredentials);
            }
        };
        let user = store::find_user_by_email(&self.db, &normalized).await?;
        let Some(user) = user else {
            verify_against_dummy(password);
            self.record_failure(&key)?;
            return Err(AuthError::InvalidCredentials);
        };
        let matches = verify_password(password, &user.password_hash).unwrap_or(false);
        if !matches || !user.is_active {
            self.record_failure(&key)?;
            return Err(AuthError::InvalidCredentials);
        }
        self.clear_failures(&key)?;
        let now = unix_now()?;
        store::update_last_login(&self.db, user.id, now).await?;
        self.issue(user.id, now).await
    }

    pub async fn authenticate_token(&self, token: &str) -> Result<AuthUser, AuthError> {
        let session = self.open_session(token).await?;
        let user = store::find_user_by_id(&self.db, session.user_id)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        if !user.is_active {
            return Err(AuthError::Unauthenticated);
        }
        let now = unix_now()?;
        store::touch_session(&self.db, session.id, now).await?;
        Ok(AuthUser {
            id: user.id,
            email: user.email,
            is_active: user.is_active,
            is_staff: user.is_staff,
            is_superuser: user.is_superuser,
            session_id: session.id,
        })
    }

    pub async fn logout(&self, token: Option<&str>) -> Result<String, AuthError> {
        if let Some(token) = token {
            if let Some(session) = store::find_session_by_hash(&self.db, &digest(token)).await? {
                store::revoke_session(&self.db, session.id, unix_now()?).await?;
            }
        }
        Ok(clear_cookie(&self.config))
    }

    pub async fn change_password(
        &self,
        user: &AuthUser,
        current: &str,
        new_password: &str,
    ) -> Result<String, AuthError> {
        validate_new_password(new_password)?;
        let row = store::find_user_by_id(&self.db, user.id)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        if !verify_password(current, &row.password_hash)? {
            return Err(AuthError::InvalidCredentials);
        }
        if verify_password(new_password, &row.password_hash)? {
            return Err(AuthError::Validation(
                "new password must differ from the current password".into(),
            ));
        }
        let now = unix_now()?;
        let transaction = self.db.begin().await.map_err(AuthError::database)?;
        store::update_password(&transaction, user.id, &hash_password(new_password)?, now).await?;
        store::revoke_user_sessions(&transaction, user.id, now).await?;
        transaction.commit().await.map_err(AuthError::database)?;
        Ok(clear_cookie(&self.config))
    }

    pub async fn rotate_session(&self, token: &str) -> Result<IssuedSession, AuthError> {
        let current = self.open_session(token).await?;
        let now = unix_now()?;
        if current.expires_at <= now {
            return Err(AuthError::Unauthenticated);
        }
        let transaction = self.db.begin().await.map_err(AuthError::database)?;
        store::revoke_session(&transaction, current.id, now).await?;
        let issued = insert_session(
            &transaction,
            &self.config.csrf_secret,
            current.user_id,
            now,
            current.expires_at,
        )
        .await?;
        transaction.commit().await.map_err(AuthError::database)?;
        let user = store::find_user_by_id(&self.db, current.user_id)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        Ok(IssuedSession {
            cookie: set_cookie(
                &self.config,
                &issued.token,
                (current.expires_at - now).max(0),
            ),
            token: issued.token,
            csrf_token: issued.csrf_token,
            user: public_user(&user),
        })
    }

    pub async fn authorize(
        &self,
        user: &AuthUser,
        permission: Option<&str>,
        action: &str,
    ) -> Result<(), AuthError> {
        if !user.is_active {
            return Err(AuthError::Forbidden);
        }
        let Some(permission) = permission else {
            return Err(AuthError::Forbidden);
        };
        if permission == "authenticated" {
            return Ok(());
        }
        if user.is_superuser {
            return Ok(());
        }
        let code = format!("{permission}.{action}");
        if store::user_has_code(&self.db, user.id, &code).await? {
            Ok(())
        } else {
            Err(AuthError::Forbidden)
        }
    }

    /// Prüft einen vollständigen Code wie `orders.read`. Superuser bestehen, inaktive Benutzer nicht.
    pub async fn holds(&self, user: &AuthUser, code: &str) -> Result<bool, AuthError> {
        if !user.is_active || code.is_empty() || code.len() > 80 {
            return Ok(false);
        }
        if user.is_superuser {
            return Ok(true);
        }
        store::user_has_code(&self.db, user.id, code).await
    }

    pub async fn ensure_role(&self, name: &str) -> Result<Uuid, AuthError> {
        let name = require_ident(name, "role")?;
        store::ensure_named_row(&self.db, ROLES, "name", &name).await
    }

    pub async fn ensure_permission(&self, code: &str) -> Result<Uuid, AuthError> {
        validate_permission_code(code)?;
        store::ensure_named_row(&self.db, PERMISSIONS, "code", code).await
    }

    pub async fn grant_permission(&self, role: &str, code: &str) -> Result<(), AuthError> {
        let role_id = self.ensure_role(role).await?;
        let permission_id = self.ensure_permission(code).await?;
        store::link_rows(
            &self.db,
            ROLE_PERMISSIONS,
            "role_id",
            role_id,
            "permission_id",
            permission_id,
        )
        .await
    }

    pub async fn revoke_permission(&self, role: &str, code: &str) -> Result<(), AuthError> {
        let role_name = require_ident(role, "role")?;
        validate_permission_code(code)?;
        let Some(role_id) = store::find_named_row(&self.db, ROLES, "name", &role_name).await?
        else {
            return Ok(());
        };
        let Some(permission_id) =
            store::find_named_row(&self.db, PERMISSIONS, "code", code).await?
        else {
            return Ok(());
        };
        store::unlink_rows(
            &self.db,
            ROLE_PERMISSIONS,
            "role_id",
            role_id,
            "permission_id",
            permission_id,
        )
        .await
    }

    pub async fn assign_role(&self, email: &str, role: &str) -> Result<(), AuthError> {
        let email = normalize_email(email)?;
        let user = store::find_user_by_email(&self.db, &email)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        let role_id = self.ensure_role(role).await?;
        store::link_rows(&self.db, USER_ROLES, "user_id", user.id, "role_id", role_id).await
    }

    pub async fn revoke_role(&self, email: &str, role: &str) -> Result<(), AuthError> {
        let email = normalize_email(email)?;
        let user = store::find_user_by_email(&self.db, &email)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        let role_name = require_ident(role, "role")?;
        let Some(role_id) = store::find_named_row(&self.db, ROLES, "name", &role_name).await?
        else {
            return Ok(());
        };
        store::unlink_rows(&self.db, USER_ROLES, "user_id", user.id, "role_id", role_id).await
    }

    pub fn client_key(&self, headers: &HeaderMap, peer: &str) -> String {
        if self.config.trust_proxy {
            if let Some(value) = headers
                .get("x-forwarded-for")
                .and_then(|value| value.to_str().ok())
            {
                let first = value.split(',').next().unwrap_or("").trim();
                if !first.is_empty() {
                    return first.to_owned();
                }
            }
        }
        peer.to_owned()
    }

    pub async fn refresh_csrf(&self, token: &str) -> Result<String, AuthError> {
        let session = self.open_session(token).await?;
        let user = store::find_user_by_id(&self.db, session.user_id)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        if !user.is_active {
            return Err(AuthError::Unauthenticated);
        }
        Ok(csrf_for_session(
            &self.config.csrf_secret,
            session.id,
            &session.token_hash,
        ))
    }

    pub async fn set_staff(&self, id: Uuid, staff: bool) -> Result<(), AuthError> {
        let now = unix_now()?;
        store::set_staff(&self.db, id, staff, now).await
    }

    pub async fn begin(&self) -> Result<DatabaseTransaction, AuthError> {
        self.db.begin().await.map_err(AuthError::database)
    }

    /// Prüft Staff-Superuser erneut auf derselben Verbindung wie die folgende Schreiboperation.
    pub async fn require_directory_actor(
        &self,
        db: &impl ConnectionTrait,
        id: Uuid,
    ) -> Result<(), AuthError> {
        let user = store::find_user_by_id(db, id)
            .await?
            .ok_or(AuthError::Forbidden)?;
        if user.is_active && user.is_staff && user.is_superuser {
            Ok(())
        } else {
            Err(AuthError::Forbidden)
        }
    }

    pub async fn authorize_on(
        &self,
        db: &impl ConnectionTrait,
        user: &AuthUser,
        permission: Option<&str>,
        action: &str,
    ) -> Result<(), AuthError> {
        if !user.is_active {
            return Err(AuthError::Forbidden);
        }
        let Some(permission) = permission else {
            return Err(AuthError::Forbidden);
        };
        if permission == "authenticated" {
            return Ok(());
        }
        if user.is_superuser {
            let stored = store::find_user_by_id(db, user.id)
                .await?
                .ok_or(AuthError::Forbidden)?;
            if stored.is_active && stored.is_superuser {
                return Ok(());
            }
            return Err(AuthError::Forbidden);
        }
        let code = format!("{permission}.{action}");
        if store::user_has_code(db, user.id, &code).await? {
            Ok(())
        } else {
            Err(AuthError::Forbidden)
        }
    }

    pub async fn insert_directory_user_on(
        &self,
        db: &impl ConnectionTrait,
        email: &str,
        password_hash: &str,
        is_active: bool,
        is_staff: bool,
    ) -> Result<PublicUser, AuthError> {
        let email = normalize_email(email)?;
        if store::find_user_by_email(db, &email).await?.is_some() {
            return Err(AuthError::Validation("email is already registered".into()));
        }
        let now = unix_now()?;
        let user = UserRow {
            id: Uuid::new_v4(),
            email,
            password_hash: password_hash.to_owned(),
            is_active,
            is_staff,
            is_superuser: false,
            created_at: now,
            updated_at: now,
            last_login: None,
        };
        store::insert_user(db, &user).await?;
        Ok(public_user(&user))
    }

    pub async fn update_profile_on(
        &self,
        db: &impl ConnectionTrait,
        actor: Uuid,
        id: Uuid,
        email: &str,
        is_active: bool,
        is_staff: bool,
    ) -> Result<PublicUser, AuthError> {
        let user = store::find_user_by_id(db, id)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        if user.is_superuser || (actor == id && (!is_active || !is_staff)) {
            return Err(AuthError::Forbidden);
        }
        let email = normalize_email(email)?;
        if let Some(existing) = store::find_user_by_email(db, &email).await? {
            if existing.id != user.id {
                return Err(AuthError::Validation("email is already registered".into()));
            }
        }
        let now = unix_now()?;
        store::update_profile(db, user.id, &email, is_active, is_staff, now).await?;
        if !is_active {
            store::revoke_user_sessions(db, user.id, now).await?;
        }
        Ok(PublicUser {
            id: user.id,
            email,
            is_active,
            is_staff,
            is_superuser: false,
        })
    }

    pub async fn reset_password_hash_on(
        &self,
        db: &impl ConnectionTrait,
        id: Uuid,
        password_hash: &str,
    ) -> Result<(), AuthError> {
        let user = store::find_user_by_id(db, id)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        if user.is_superuser {
            return Err(AuthError::Forbidden);
        }
        let now = unix_now()?;
        store::update_password(db, user.id, password_hash, now).await?;
        store::revoke_user_sessions(db, user.id, now).await?;
        Ok(())
    }

    pub async fn set_active_on(
        &self,
        db: &impl ConnectionTrait,
        id: Uuid,
        active: bool,
    ) -> Result<(), AuthError> {
        let user = store::find_user_by_id(db, id)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        if user.is_superuser {
            return Err(AuthError::Forbidden);
        }
        let now = unix_now()?;
        store::set_active(db, user.id, active, now).await?;
        if !active {
            store::revoke_user_sessions(db, user.id, now).await?;
        }
        Ok(())
    }

    pub async fn set_staff_on(
        &self,
        db: &impl ConnectionTrait,
        actor: Uuid,
        id: Uuid,
        staff: bool,
    ) -> Result<(), AuthError> {
        let user = store::find_user_by_id(db, id)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        if user.is_superuser || (actor == id && !staff) {
            return Err(AuthError::Forbidden);
        }
        let now = unix_now()?;
        store::set_staff(db, id, staff, now).await
    }

    pub async fn assign_role_on(
        &self,
        db: &impl ConnectionTrait,
        email: &str,
        role: &str,
    ) -> Result<Uuid, AuthError> {
        let email = normalize_email(email)?;
        let user = store::find_user_by_email(db, &email)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        let role_name = require_ident(role, "role")?;
        let role_id = store::ensure_named_row(db, ROLES, "name", &role_name).await?;
        store::link_rows(db, USER_ROLES, "user_id", user.id, "role_id", role_id).await?;
        Ok(user.id)
    }

    pub async fn revoke_role_on(
        &self,
        db: &impl ConnectionTrait,
        email: &str,
        role: &str,
    ) -> Result<Uuid, AuthError> {
        let email = normalize_email(email)?;
        let user = store::find_user_by_email(db, &email)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        let role_name = require_ident(role, "role")?;
        let Some(role_id) = store::find_named_row(db, ROLES, "name", &role_name).await? else {
            return Ok(user.id);
        };
        store::unlink_rows(db, USER_ROLES, "user_id", user.id, "role_id", role_id).await?;
        Ok(user.id)
    }

    pub async fn grant_permission_on(
        &self,
        db: &impl ConnectionTrait,
        role: &str,
        code: &str,
    ) -> Result<(), AuthError> {
        let role_name = require_ident(role, "role")?;
        validate_permission_code(code)?;
        let role_id = store::ensure_named_row(db, ROLES, "name", &role_name).await?;
        let permission_id = store::ensure_named_row(db, PERMISSIONS, "code", code).await?;
        store::link_rows(
            db,
            ROLE_PERMISSIONS,
            "role_id",
            role_id,
            "permission_id",
            permission_id,
        )
        .await
    }

    pub async fn revoke_permission_on(
        &self,
        db: &impl ConnectionTrait,
        role: &str,
        code: &str,
    ) -> Result<(), AuthError> {
        let role_name = require_ident(role, "role")?;
        validate_permission_code(code)?;
        let Some(role_id) = store::find_named_row(db, ROLES, "name", &role_name).await? else {
            return Ok(());
        };
        let Some(permission_id) = store::find_named_row(db, PERMISSIONS, "code", code).await?
        else {
            return Ok(());
        };
        store::unlink_rows(
            db,
            ROLE_PERMISSIONS,
            "role_id",
            role_id,
            "permission_id",
            permission_id,
        )
        .await
    }

    pub async fn list_roles(&self) -> Result<Vec<(Uuid, String)>, AuthError> {
        store::list_role_names(&self.db).await
    }

    pub async fn list_role_permissions(&self, role: &str) -> Result<Vec<String>, AuthError> {
        let role_name = require_ident(role, "role")?;
        let Some(role_id) = store::find_named_row(&self.db, ROLES, "name", &role_name).await?
        else {
            return Err(AuthError::Validation("role was not found".into()));
        };
        store::list_permission_codes(&self.db, role_id).await
    }

    pub fn presented_origin_allowed(&self, headers: &HeaderMap) -> Result<(), AuthError> {
        self.allow_presented(headers)
    }

    pub fn origin_allowed(&self, headers: &HeaderMap, method: &Method) -> Result<(), AuthError> {
        if method.is_safe() {
            return Ok(());
        }
        self.allow_presented(headers)
    }

    /// Origin oder Referer müssen exakt auf der Liste stehen. Der Host-Header wird nicht gelesen.
    /// `Origin: null` gilt nur zusammen mit `Sec-Fetch-Site: same-origin`: Browser setzen genau
    /// diese Kombination bei einem Same-Origin-Formular, wenn `Referrer-Policy: no-referrer` gilt.
    fn allow_presented(&self, headers: &HeaderMap) -> Result<(), AuthError> {
        let Some(origin) = presented_origin(headers) else {
            return Ok(());
        };
        if origin_on_list(&self.config.allowed_origins, &origin)
            || nulled_same_origin(headers, &origin)
        {
            Ok(())
        } else {
            Err(AuthError::OriginRejected)
        }
    }

    pub async fn authenticate(
        &self,
        headers: &HeaderMap,
        method: &Method,
    ) -> Result<AuthUser, AuthError> {
        let supplied = headers
            .get(csrf_header_name())
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        self.authenticate_csrf(headers, method, supplied).await
    }

    /// Dieselbe HMAC-Prüfung wie der Header. HTML-Formulare übergeben das Token als Feld.
    pub async fn authenticate_csrf(
        &self,
        headers: &HeaderMap,
        method: &Method,
        supplied: &str,
    ) -> Result<AuthUser, AuthError> {
        self.origin_allowed(headers, method)?;
        let token =
            read_cookie(headers, &self.config.cookie_name).ok_or(AuthError::Unauthenticated)?;
        let user = self.authenticate_token(&token).await?;
        if !method.is_safe() {
            let session = store::find_session_by_hash(&self.db, &digest(&token))
                .await?
                .ok_or(AuthError::Unauthenticated)?;
            let expected =
                csrf_for_session(&self.config.csrf_secret, session.id, &session.token_hash);
            if !constant_eq(supplied, &expected) {
                return Err(AuthError::CsrfRejected);
            }
        }
        Ok(user)
    }

    pub async fn csrf_from_request(&self, headers: &HeaderMap) -> Result<String, AuthError> {
        let token =
            read_cookie(headers, &self.config.cookie_name).ok_or(AuthError::Unauthenticated)?;
        self.refresh_csrf(&token).await
    }

    async fn issue(&self, user_id: Uuid, now: i64) -> Result<IssuedSession, AuthError> {
        let expires_at = now.saturating_add(self.config.session_ttl_secs);
        let issued =
            insert_session(&self.db, &self.config.csrf_secret, user_id, now, expires_at).await?;
        let user = store::find_user_by_id(&self.db, user_id)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        Ok(IssuedSession {
            cookie: set_cookie(&self.config, &issued.token, self.config.session_ttl_secs),
            token: issued.token,
            csrf_token: issued.csrf_token,
            user: public_user(&user),
        })
    }

    async fn open_session(&self, token: &str) -> Result<SessionRow, AuthError> {
        let session = store::find_session_by_hash(&self.db, &digest(token))
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        let now = unix_now()?;
        if session.revoked_at.is_some() || session.expires_at <= now {
            return Err(AuthError::Unauthenticated);
        }
        if now.saturating_sub(session.last_seen_at) > self.config.idle_timeout_secs {
            return Err(AuthError::Unauthenticated);
        }
        Ok(session)
    }

    pub(crate) fn note_attempt(&self, key: &str, max: u32) -> Result<bool, AuthError> {
        let mut guard = self
            .limiter
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = unix_now()?;
        let hits = guard.entry(key.to_owned()).or_default();
        hits.retain(|at| now.saturating_sub(*at) < self.config.login_window_secs);
        if hits.len() as u32 >= max {
            return Ok(true);
        }
        hits.push(now);
        Ok(false)
    }

    fn limited(&self, key: &str) -> Result<bool, AuthError> {
        let mut guard = self
            .limiter
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = unix_now()?;
        let hits = guard.entry(key.to_owned()).or_default();
        hits.retain(|at| now.saturating_sub(*at) < self.config.login_window_secs);
        Ok(hits.len() as u32 >= self.config.login_max_attempts)
    }

    fn record_failure(&self, key: &str) -> Result<(), AuthError> {
        let mut guard = self
            .limiter
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.entry(key.to_owned()).or_default().push(unix_now()?);
        Ok(())
    }

    fn clear_failures(&self, key: &str) -> Result<(), AuthError> {
        let mut guard = self
            .limiter
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.remove(key);
        Ok(())
    }
}

struct RawSession {
    token: String,
    csrf_token: String,
}

async fn insert_session(
    db: &impl sea_orm::ConnectionTrait,
    secret: &[u8; 32],
    user_id: Uuid,
    now: i64,
    expires_at: i64,
) -> Result<RawSession, AuthError> {
    let token = random_token();
    let session_id = Uuid::new_v4();
    let token_hash = digest(&token);
    let csrf_token = csrf_for_session(secret, session_id, &token_hash);
    store::insert_session(
        db,
        &SessionRow {
            id: session_id,
            user_id,
            token_hash,
            csrf_token_hash: digest(&csrf_token),
            created_at: now,
            expires_at,
            last_seen_at: now,
            revoked_at: None,
        },
    )
    .await?;
    Ok(RawSession { token, csrf_token })
}

fn csrf_for_session(secret: &[u8; 32], session_id: Uuid, token_hash: &str) -> String {
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret).expect("csrf hmac key");
    // Preserve the pre-rename HMAC namespace so existing CSRF tokens remain valid.
    mac.update(b"ferrum-csrf-v1");
    mac.update(session_id.as_bytes());
    mac.update(token_hash.as_bytes());
    base64url(&mac.finalize().into_bytes())
}

pub fn normalize_email(input: &str) -> Result<String, AuthError> {
    let email = input.trim().to_lowercase();
    if email.len() < 3 || email.len() > 254 || email.chars().any(char::is_whitespace) {
        return Err(AuthError::Validation("email is invalid".into()));
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err(AuthError::Validation("email is invalid".into()));
    };
    if local.is_empty() || !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.')
    {
        return Err(AuthError::Validation("email is invalid".into()));
    }
    Ok(email)
}

fn require_ident(value: &str, label: &str) -> Result<String, AuthError> {
    let value = value.trim();
    let mut chars = value.chars();
    let valid = matches!(chars.next(), Some(character) if character.is_ascii_alphabetic() || character == '_')
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '.'
        });
    if !valid || value.len() > 80 {
        return Err(AuthError::Validation(format!("{label} is invalid")));
    }
    Ok(value.to_owned())
}

fn validate_permission_code(code: &str) -> Result<(), AuthError> {
    let mut parts = code.split('.');
    let resource = parts.next().unwrap_or("");
    let action = parts.next().unwrap_or("");
    if parts.next().is_some()
        || !matches!(action, "read" | "create" | "update" | "delete")
        || require_ident(resource, "permission").is_err()
    {
        return Err(AuthError::Validation(
            "permission must look like resource.read".into(),
        ));
    }
    Ok(())
}

fn public_user(user: &UserRow) -> PublicUser {
    PublicUser {
        id: user.id,
        email: user.email.clone(),
        is_active: user.is_active,
        is_staff: user.is_staff,
        is_superuser: user.is_superuser,
    }
}

pub fn unix_now() -> Result<i64, AuthError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|_| AuthError::Validation("system clock is invalid".into()))
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64url(&bytes)
}

fn digest(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn constant_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in left.iter().zip(right.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(first >> 2) as usize] as char);
        output.push(TABLE[(((first & 0b11) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((second & 0b1111) << 2) | (third >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            output.push(TABLE[(third & 0b111111) as usize] as char);
        }
    }
    output
}

fn presented_origin(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get("origin").and_then(|value| value.to_str().ok()) {
        return Some(value.to_owned());
    }
    referer_origin(headers)
}

fn referer_origin(headers: &HeaderMap) -> Option<String> {
    let referer = headers.get("referer")?.to_str().ok()?;
    let (scheme, rest) = referer.split_once("://")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        return None;
    }
    Some(format!("{scheme}://{host}"))
}

fn nulled_same_origin(headers: &HeaderMap, origin: &str) -> bool {
    origin.trim().eq_ignore_ascii_case("null")
        && headers
            .get("sec-fetch-site")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.eq_ignore_ascii_case("same-origin"))
}

fn origin_on_list(allowed: &[String], presented: &str) -> bool {
    let Some(presented) = canonical_origin(presented) else {
        return false;
    };
    allowed
        .iter()
        .any(|item| canonical_origin(item).as_deref() == Some(presented.as_str()))
}

fn canonical_origin(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    let (scheme, rest) = value.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    if rest.is_empty()
        || rest.contains(['/', '?', '#', '@', ' ', '\\'])
        || rest.chars().any(|char| char.is_control())
    {
        return None;
    }
    Some(format!(
        "{}://{}",
        scheme.to_ascii_lowercase(),
        rest.to_ascii_lowercase()
    ))
}

pub fn csrf_header_name() -> &'static str {
    "x-csrf-token"
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn legacy_csrf_hmac_namespace_stays_stable_after_framework_rename() {
        let mut secret = [0_u8; 32];
        for (index, byte) in secret.iter_mut().enumerate() {
            *byte = index as u8;
        }
        assert_eq!(
            csrf_for_session(&secret, Uuid::nil(), "legacy-test-token-hash"),
            "7YhBGSxfUfH-DNB2AOwAAfkYTbefW2Qm5DSiseZFR3k"
        );
    }
}
