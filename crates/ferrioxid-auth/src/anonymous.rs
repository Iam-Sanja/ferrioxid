//! Serverseitige anonyme Sitzungen. Dieselbe HMAC-CSRF-Ableitung wie bei Benutzersitzungen.

use crate::cookie::{clear_named_cookie, read_cookie, set_named_cookie};
use crate::error::AuthError;
use crate::service::{
    csrf_for_session, csrf_header_name, digest, random_token, token_eq, unix_now, AuthService,
    AuthUser,
};
use crate::store::{self, AnonymousRow};
use axum::http::{HeaderMap, Method};
use uuid::Uuid;

/// Sitzung, die ein Formular oder eine Cookie-Anfrage gerade verwendet.
#[derive(Debug)]
pub enum BrowserActor {
    User(AuthUser),
    Anonymous {
        session_id: Uuid,
    },
    /// Kein Session-Cookie. Bearer- oder öffentliche API-Clients fallen hierher und werden nicht per Browser-CSRF blockiert.
    Stateless,
}

/// Neu ausgegebene oder wiedererkannte anonyme Sitzung. Der Roh-Token steht nur im Cookie.
pub struct AnonymousSession {
    pub session_id: Uuid,
    pub csrf_token: String,
    /// Gesetzt, wenn der Browser das Cookie neu speichern muss.
    pub cookie: Option<String>,
}

impl std::fmt::Debug for AnonymousSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AnonymousSession")
            .field("session_id", &self.session_id)
            .field("csrf_token", &"[redacted]")
            .field("cookie", &self.cookie.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

impl AuthService {
    /// Legt eine anonyme Sitzung an, wenn das anonyme Cookie fehlt, abgelaufen oder unbekannt ist.
    /// Eine vorhandene Benutzersitzung bleibt unberührt.
    pub async fn ensure_anonymous_session(
        &self,
        headers: &HeaderMap,
    ) -> Result<AnonymousSession, AuthError> {
        let name = self.anonymous_cookie_name()?;
        if let Some(token) = read_cookie(headers, name) {
            if let Some(session) = self.current_anonymous(&token).await? {
                let now = unix_now()?;
                store::touch_anonymous(&self.db, session.id, now).await?;
                return Ok(AnonymousSession {
                    session_id: session.id,
                    csrf_token: csrf_for_session(
                        &self.config.csrf_secret,
                        session.id,
                        &session.token_hash,
                    ),
                    cookie: None,
                });
            }
        }
        self.issue_anonymous().await
    }

    /// Prüft POST, PUT, PATCH und DELETE. Eine Benutzersitzung wird nicht auf die anonyme Sitzung zurückgestuft.
    pub async fn authenticate_browser_form(
        &self,
        headers: &HeaderMap,
        method: &Method,
        field: &str,
    ) -> Result<BrowserActor, AuthError> {
        if read_cookie(headers, &self.config.cookie_name).is_some() {
            return self
                .authenticate_csrf(headers, method, supplied_csrf(headers, field))
                .await
                .map(BrowserActor::User);
        }
        if read_cookie(headers, &self.config.anonymous_cookie_name).is_some() {
            return self
                .authenticate_anonymous_csrf(headers, method, supplied_csrf(headers, field))
                .await
                .map(|session_id| BrowserActor::Anonymous { session_id });
        }
        Err(AuthError::Unauthenticated)
    }

    /// Cookie-Schreibzugriffe prüfen CSRF. Ohne Session-Cookie bleibt die Anfrage unberührt, auch mit `Authorization: Bearer`.
    pub async fn guard_cookie_write(
        &self,
        headers: &HeaderMap,
        method: &Method,
        field: &str,
    ) -> Result<BrowserActor, AuthError> {
        let has_user = read_cookie(headers, &self.config.cookie_name).is_some();
        let has_anonymous = read_cookie(headers, &self.config.anonymous_cookie_name).is_some();
        if !has_user && !has_anonymous {
            return Ok(BrowserActor::Stateless);
        }
        self.authenticate_browser_form(headers, method, field).await
    }

    pub async fn authenticate_anonymous_csrf(
        &self,
        headers: &HeaderMap,
        method: &Method,
        supplied: &str,
    ) -> Result<Uuid, AuthError> {
        self.origin_allowed(headers, method)?;
        let name = self.anonymous_cookie_name()?;
        let token = read_cookie(headers, name).ok_or(AuthError::Unauthenticated)?;
        let session = self
            .current_anonymous(&token)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        let now = unix_now()?;
        store::touch_anonymous(&self.db, session.id, now).await?;
        if !method.is_safe() {
            let expected =
                csrf_for_session(&self.config.csrf_secret, session.id, &session.token_hash);
            if !token_eq(supplied, &expected) {
                return Err(AuthError::CsrfRejected);
            }
        }
        Ok(session.id)
    }

    pub async fn read_anonymous_state(&self, token: &str) -> Result<String, AuthError> {
        let session = self
            .current_anonymous(token)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        Ok(session.state.unwrap_or_default())
    }

    pub async fn write_anonymous_state(&self, token: &str, state: &str) -> Result<(), AuthError> {
        let session = self
            .current_anonymous(token)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        let state = normalize_state(state)?;
        store::write_anonymous_state(&self.db, session.id, &state).await
    }

    pub async fn revoke_anonymous_token(&self, token: &str) -> Result<(), AuthError> {
        if token.is_empty() {
            return Ok(());
        }
        if let Some(session) = store::find_anonymous_by_hash(&self.db, &digest(token)).await? {
            if session.revoked_at.is_none() {
                store::revoke_anonymous(&self.db, session.id, unix_now()?).await?;
            }
        }
        Ok(())
    }

    pub fn clear_anonymous_cookie(&self) -> Result<String, AuthError> {
        Ok(clear_named_cookie(
            &self.config,
            self.anonymous_cookie_name()?,
        ))
    }

    fn anonymous_cookie_name(&self) -> Result<&str, AuthError> {
        let name = self.config.anonymous_cookie_name.as_str();
        if name.is_empty()
            || name == self.config.cookie_name
            || name.contains([';', ' ', '=', ',', '\r', '\n'])
        {
            return Err(AuthError::Validation(
                "anonymous cookie name is invalid".into(),
            ));
        }
        Ok(name)
    }

    async fn issue_anonymous(&self) -> Result<AnonymousSession, AuthError> {
        let name = self.anonymous_cookie_name()?;
        let now = unix_now()?;
        let expires_at = now.saturating_add(self.config.session_ttl_secs);
        let token = random_token();
        let session_id = Uuid::new_v4();
        let token_hash = digest(&token);
        let csrf_token = csrf_for_session(&self.config.csrf_secret, session_id, &token_hash);
        store::insert_anonymous(
            &self.db,
            &AnonymousRow {
                id: session_id,
                token_hash,
                csrf_token_hash: digest(&csrf_token),
                state: None,
                created_at: now,
                expires_at,
                last_seen_at: now,
                revoked_at: None,
            },
        )
        .await?;
        Ok(AnonymousSession {
            session_id,
            csrf_token,
            cookie: Some(set_named_cookie(
                &self.config,
                name,
                &token,
                self.config.session_ttl_secs,
            )),
        })
    }

    async fn current_anonymous(&self, token: &str) -> Result<Option<AnonymousRow>, AuthError> {
        if token.is_empty() || token.len() > 128 {
            return Ok(None);
        }
        let Some(session) = store::find_anonymous_by_hash(&self.db, &digest(token)).await? else {
            return Ok(None);
        };
        let now = unix_now()?;
        if session.revoked_at.is_some()
            || session.expires_at <= now
            || now.saturating_sub(session.last_seen_at) > self.config.idle_timeout_secs
        {
            return Ok(None);
        }
        Ok(Some(session))
    }
}

fn supplied_csrf<'a>(headers: &'a HeaderMap, field: &'a str) -> &'a str {
    if !field.is_empty() {
        return field;
    }
    headers
        .get(csrf_header_name())
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
}

fn normalize_state(raw: &str) -> Result<String, AuthError> {
    if raw.len() > 4096 {
        return Err(AuthError::Validation("anonymous state is too large".into()));
    }
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|_| AuthError::Validation("anonymous state must be a JSON object".into()))?;
    let Some(object) = value.as_object() else {
        return Err(AuthError::Validation(
            "anonymous state must be a JSON object".into(),
        ));
    };
    if object.len() > 32 {
        return Err(AuthError::Validation(
            "anonymous state has too many keys".into(),
        ));
    }
    for (key, item) in object {
        let key_ok = !key.is_empty()
            && key.len() <= 64
            && key
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_');
        if !key_ok {
            return Err(AuthError::Validation(
                "anonymous state key is invalid".into(),
            ));
        }
        if !(item.is_string() || item.is_number() || item.is_boolean() || item.is_null()) {
            return Err(AuthError::Validation(
                "anonymous state values must be scalar".into(),
            ));
        }
        if item.as_str().is_some_and(|text| text.len() > 1024) {
            return Err(AuthError::Validation(
                "anonymous state value is too large".into(),
            ));
        }
    }
    let stored = serde_json::to_string(&value)
        .map_err(|_| AuthError::Validation("anonymous state must be a JSON object".into()))?;
    if stored.len() > 4096 {
        return Err(AuthError::Validation("anonymous state is too large".into()));
    }
    Ok(stored)
}
