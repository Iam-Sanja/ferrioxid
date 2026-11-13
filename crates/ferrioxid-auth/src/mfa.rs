//! Zweiter Faktor, Sitzungsliste, erneute Anmeldung und Kontoschluss.

use crate::account_store::{self, ChallengeRow, TotpRow};
use crate::error::AuthError;
use crate::password::verify_password;
use crate::service::{constant_eq, digest, unix_now, AuthService, AuthUser, IssuedSession};
use crate::store::{self, SessionRow};
use crate::totp::{
    accept_totp, current_nonce, current_recovery_record, legacy_mfa_key, normalize_recovery,
    open_stored, otpauth_uri, random_secret, reauth_proof, reauth_valid, recovery_codes,
    recovery_format, recovery_hash, seal, seal_format, RecoveryFormat, SealFormat,
};
use sea_orm::TransactionTrait;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: Uuid,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_seen_at: i64,
    pub current: bool,
}

pub struct TotpEnrollment {
    pub secret_base32: String,
    pub otpauth_uri: String,
}

impl std::fmt::Debug for TotpEnrollment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TotpEnrollment")
            .field("secret_base32", &"[redacted]")
            .field("otpauth_uri", &"[redacted]")
            .finish()
    }
}

pub struct RecoverySet {
    pub codes: Vec<String>,
}

impl std::fmt::Debug for RecoverySet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecoverySet")
            .field("codes", &self.codes.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountPhase {
    Active,
    Disabled,
}

pub struct ReauthGrant {
    pub proof: String,
    pub expires_at: i64,
}

impl std::fmt::Debug for ReauthGrant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReauthGrant")
            .field("proof", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl AuthService {
    pub async fn verify_identity(
        &self,
        provider: &dyn crate::provider::IdentityProvider,
        email: &str,
        secret: &str,
        client: &str,
    ) -> Result<crate::provider::VerifiedIdentity, AuthError> {
        let key = format!("{client}:{}", email.trim().to_lowercase());
        if self.limited(&key)? {
            return Err(AuthError::RateLimited);
        }
        match provider.verify(self.database(), email, secret).await {
            Ok(identity) => {
                self.clear_failures(&key)?;
                Ok(identity)
            }
            Err(AuthError::InvalidCredentials) => {
                self.record_failure(&key)?;
                Err(AuthError::InvalidCredentials)
            }
            Err(error) => Err(error),
        }
    }

    pub async fn begin_totp(
        &self,
        user: &AuthUser,
        password: &str,
    ) -> Result<TotpEnrollment, AuthError> {
        self.require_password(user, password).await?;
        if account_store::find_totp(self.database(), user.id)
            .await?
            .is_some_and(|row| row.confirmed_at.is_some())
        {
            return Err(AuthError::Validation("totp is already enabled".into()));
        }
        let secret = random_secret();
        let (nonce, ciphertext) = seal(&self.config().mfa_key, user.id, &secret)?;
        account_store::replace_totp(
            self.database(),
            user.id,
            &current_nonce(&nonce),
            &ciphertext,
            unix_now()?,
        )
        .await?;
        Ok(TotpEnrollment {
            otpauth_uri: otpauth_uri(&user.email, &secret),
            secret_base32: crate::totp::base32(&secret),
        })
    }

    pub async fn confirm_totp(
        &self,
        user: &AuthUser,
        code: &str,
    ) -> Result<RecoverySet, AuthError> {
        let row = self.totp_row(user.id).await?;
        let secret = self.open_secret(user.id, &row).await?;
        let now = unix_now()?;
        let step = accept_totp(&secret, code, now, None)?;
        let codes = recovery_codes();
        let transaction = self.database().begin().await.map_err(AuthError::database)?;
        account_store::confirm_totp(&transaction, user.id, now, step).await?;
        account_store::delete_recovery(&transaction, user.id).await?;
        for code in &codes {
            let normalized = normalize_recovery(code)?;
            account_store::insert_recovery(
                &transaction,
                Uuid::new_v4(),
                user.id,
                &current_recovery_record(&self.config().mfa_key, user.id, &normalized),
                now,
            )
            .await?;
        }
        transaction.commit().await.map_err(AuthError::database)?;
        Ok(RecoverySet { codes })
    }

    pub async fn disable_totp(
        &self,
        user: &AuthUser,
        password: &str,
        second_factor: &str,
    ) -> Result<(), AuthError> {
        self.require_password(user, password).await?;
        self.accept_second_factor(user.id, second_factor).await?;
        let now = unix_now()?;
        let transaction = self.database().begin().await.map_err(AuthError::database)?;
        account_store::delete_totp(&transaction, user.id).await?;
        account_store::delete_recovery(&transaction, user.id).await?;
        account_store::delete_user_challenges(&transaction, user.id).await?;
        store::revoke_user_sessions(&transaction, user.id, now).await?;
        transaction.commit().await.map_err(AuthError::database)?;
        Ok(())
    }

    /// Schließt den offenen zweiten Faktor ab und gibt eine neue Sitzung zurück.
    pub async fn complete_pending_login(
        &self,
        challenge: &str,
        code: &str,
        client: &str,
        anonymous_token: Option<&str>,
    ) -> Result<IssuedSession, AuthError> {
        let key = format!("mfa:{client}");
        if self.limited(&key)? {
            return Err(AuthError::RateLimited);
        }
        let now = unix_now()?;
        let Some(row) = account_store::find_challenge(self.database(), &digest(challenge)).await?
        else {
            self.record_failure(&key)?;
            return Err(AuthError::InvalidCredentials);
        };
        if !challenge_is_open(&row, now) {
            let _ = account_store::consume_challenge(self.database(), row.id, now).await;
            self.record_failure(&key)?;
            return Err(AuthError::InvalidCredentials);
        }
        account_store::bump_challenge(self.database(), row.id, row.attempts.saturating_add(1))
            .await?;
        if self.accept_second_factor(row.user_id, code).await.is_err() {
            self.record_failure(&key)?;
            return Err(AuthError::InvalidCredentials);
        }
        account_store::consume_challenge(self.database(), row.id, now).await?;
        self.clear_failures(&key)?;
        self.issue_login_session(row.user_id, anonymous_token, now)
            .await
    }

    pub async fn list_sessions(&self, user: &AuthUser) -> Result<Vec<SessionInfo>, AuthError> {
        let now = unix_now()?;
        let rows = account_store::list_open_sessions(self.database(), user.id).await?;
        Ok(rows
            .into_iter()
            .filter(|row| session_live(row, now, self.config().idle_timeout_secs))
            .map(|row| SessionInfo {
                current: row.id == user.session_id,
                id: row.id,
                created_at: row.created_at,
                expires_at: row.expires_at,
                last_seen_at: row.last_seen_at,
            })
            .collect())
    }

    pub async fn revoke_session_for(
        &self,
        actor: &AuthUser,
        session_id: Uuid,
    ) -> Result<(), AuthError> {
        let rows = account_store::list_open_sessions(self.database(), actor.id).await?;
        let own = rows.iter().any(|row| row.id == session_id);
        if !own && !actor.is_superuser {
            return Err(AuthError::Forbidden);
        }
        if !own && actor.is_superuser {
            store::revoke_session(self.database(), session_id, unix_now()?).await?;
            return Ok(());
        }
        store::revoke_session(self.database(), session_id, unix_now()?).await
    }

    pub async fn revoke_other_sessions(&self, user: &AuthUser) -> Result<u64, AuthError> {
        let now = unix_now()?;
        let rows = account_store::list_open_sessions(self.database(), user.id).await?;
        let mut revoked = 0u64;
        for row in rows {
            if row.id != user.session_id {
                store::revoke_session(self.database(), row.id, now).await?;
                revoked += 1;
            }
        }
        Ok(revoked)
    }

    pub async fn reauthenticate(
        &self,
        user: &AuthUser,
        password: &str,
        second_factor: Option<&str>,
    ) -> Result<ReauthGrant, AuthError> {
        self.require_password(user, password).await?;
        if self.totp_enabled(user.id).await? {
            let Some(code) = second_factor else {
                return Err(AuthError::Validation("second factor is required".into()));
            };
            self.accept_second_factor(user.id, code).await?;
        }
        let now = unix_now()?;
        let expires_at = now.saturating_add(300);
        let proof = reauth_proof(&self.config().mfa_key, user.id, user.session_id, expires_at);
        Ok(ReauthGrant { proof, expires_at })
    }

    pub fn require_reauth(&self, user: &AuthUser, proof: &str) -> Result<(), AuthError> {
        let now = unix_now()?;
        if reauth_valid(
            &self.config().mfa_key,
            &legacy_mfa_key(&self.config().csrf_secret),
            user.id,
            user.session_id,
            proof,
            now,
        ) {
            Ok(())
        } else {
            Err(AuthError::Unauthenticated)
        }
    }

    pub async fn account_phase(&self, id: Uuid) -> Result<AccountPhase, AuthError> {
        let user = store::find_user_by_id(self.database(), id)
            .await?
            .ok_or_else(|| AuthError::Validation("user was not found".into()))?;
        Ok(if user.is_active {
            AccountPhase::Active
        } else {
            AccountPhase::Disabled
        })
    }

    pub async fn close_account(&self, user: &AuthUser, password: &str) -> Result<(), AuthError> {
        self.require_password(user, password).await?;
        if self.totp_enabled(user.id).await? {
            return Err(AuthError::Validation(
                "disable totp before closing the account".into(),
            ));
        }
        let now = unix_now()?;
        let transaction = self.database().begin().await.map_err(AuthError::database)?;
        store::set_active(&transaction, user.id, false, now).await?;
        store::revoke_user_sessions(&transaction, user.id, now).await?;
        account_store::delete_totp(&transaction, user.id).await?;
        account_store::delete_recovery(&transaction, user.id).await?;
        account_store::delete_user_challenges(&transaction, user.id).await?;
        transaction.commit().await.map_err(AuthError::database)?;
        Ok(())
    }

    pub(crate) async fn issue_login_session(
        &self,
        user_id: Uuid,
        anonymous_token: Option<&str>,
        now: i64,
    ) -> Result<IssuedSession, AuthError> {
        store::update_last_login(self.database(), user_id, now).await?;
        let expires_at = now.saturating_add(self.config().session_ttl_secs);
        let transaction = self.database().begin().await.map_err(AuthError::database)?;
        let mut clear_cookies = Vec::new();
        if let Some(token) = anonymous_token.filter(|token| !token.is_empty()) {
            if let Some(row) = store::find_anonymous_by_hash(&transaction, &digest(token)).await? {
                if row.revoked_at.is_none() {
                    store::revoke_anonymous(&transaction, row.id, now).await?;
                }
            }
            clear_cookies.push(crate::cookie::clear_named_cookie(
                self.config(),
                &self.config().anonymous_cookie_name,
            ));
        }
        let issued = crate::service::insert_session(
            &transaction,
            &self.config().csrf_secret,
            user_id,
            now,
            expires_at,
        )
        .await?;
        transaction.commit().await.map_err(AuthError::database)?;
        let user = store::find_user_by_id(self.database(), user_id)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        Ok(IssuedSession {
            cookie: crate::cookie::set_cookie(
                self.config(),
                &issued.token,
                self.config().session_ttl_secs,
            ),
            token: issued.token,
            csrf_token: issued.csrf_token,
            clear_cookies,
            user: crate::service::public_user(&user),
        })
    }

    async fn require_password(&self, user: &AuthUser, password: &str) -> Result<(), AuthError> {
        let row = store::find_user_by_id(self.database(), user.id)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        if !row.is_active || !verify_password(password, &row.password_hash)? {
            return Err(AuthError::InvalidCredentials);
        }
        Ok(())
    }

    async fn totp_enabled(&self, user_id: Uuid) -> Result<bool, AuthError> {
        Ok(account_store::find_totp(self.database(), user_id)
            .await?
            .is_some_and(|row| row.confirmed_at.is_some()))
    }

    async fn totp_row(&self, user_id: Uuid) -> Result<TotpRow, AuthError> {
        account_store::find_totp(self.database(), user_id)
            .await?
            .ok_or_else(|| AuthError::Validation("totp is not enrolled".into()))
    }

    async fn open_secret(&self, user_id: Uuid, row: &TotpRow) -> Result<Vec<u8>, AuthError> {
        let legacy = legacy_mfa_key(&self.config().csrf_secret);
        let secret = open_stored(
            &self.config().mfa_key,
            &legacy,
            user_id,
            &row.nonce,
            &row.ciphertext,
        )?;
        if seal_format(&row.nonce)? == SealFormat::Legacy {
            let (nonce, ciphertext) = seal(&self.config().mfa_key, user_id, &secret)?;
            account_store::update_totp_secret(
                self.database(),
                user_id,
                &current_nonce(&nonce),
                &ciphertext,
            )
            .await?;
        }
        Ok(secret)
    }

    async fn accept_second_factor(&self, user_id: Uuid, code: &str) -> Result<(), AuthError> {
        let row = self.totp_row(user_id).await?;
        if row.confirmed_at.is_none() {
            return Err(AuthError::Validation("totp is not confirmed".into()));
        }
        let secret = self.open_secret(user_id, &row).await?;
        let now = unix_now()?;
        if let Ok(step) = accept_totp(&secret, code, now, row.last_used_step) {
            account_store::set_totp_step(self.database(), user_id, step).await?;
            return Ok(());
        }
        let normalized = normalize_recovery(code)?;
        let legacy = legacy_mfa_key(&self.config().csrf_secret);
        for (id, hash) in account_store::unused_recovery(self.database(), user_id).await? {
            let Some(format) = recovery_format(&hash) else {
                continue;
            };
            let expected = match format {
                RecoveryFormat::Current => {
                    current_recovery_record(&self.config().mfa_key, user_id, &normalized)
                }
                RecoveryFormat::Legacy => recovery_hash(&legacy, user_id, &normalized),
            };
            if constant_eq(&hash, &expected) {
                account_store::consume_recovery(self.database(), id, now).await?;
                return Ok(());
            }
        }
        Err(AuthError::InvalidCredentials)
    }
}

fn session_live(row: &SessionRow, now: i64, idle_timeout: i64) -> bool {
    row.revoked_at.is_none()
        && row.expires_at > now
        && now.saturating_sub(row.last_seen_at) <= idle_timeout
}

pub(crate) fn challenge_is_open(row: &ChallengeRow, now: i64) -> bool {
    row.consumed_at.is_none() && row.expires_at > now && row.attempts < 5
}
