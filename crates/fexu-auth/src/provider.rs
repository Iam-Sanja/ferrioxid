//! Vertrauenswürdige Prüfung im Serverprozess. Ein Provider legt keine Sitzung an.

use crate::error::AuthError;
use crate::password::{verify_against_dummy, verify_password};
use crate::service::normalize_email;
use crate::store::{self, UserRow};
use async_trait::async_trait;
use fexu_db::driver::DatabaseConnection;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedIdentity {
    pub user_id: Uuid,
    pub email: String,
    pub method: &'static str,
}

#[async_trait]
pub trait IdentityProvider: Send + Sync {
    fn method(&self) -> &'static str;

    async fn verify(
        &self,
        db: &DatabaseConnection,
        email: &str,
        secret: &str,
    ) -> Result<VerifiedIdentity, AuthError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PasswordProvider;

#[async_trait]
impl IdentityProvider for PasswordProvider {
    fn method(&self) -> &'static str {
        "password"
    }

    async fn verify(
        &self,
        db: &DatabaseConnection,
        email: &str,
        secret: &str,
    ) -> Result<VerifiedIdentity, AuthError> {
        let user = verify_local_password(db, email, secret).await?;
        Ok(VerifiedIdentity {
            user_id: user.id,
            email: user.email,
            method: self.method(),
        })
    }
}

pub(crate) async fn verify_local_password(
    db: &DatabaseConnection,
    email: &str,
    password: &str,
) -> Result<UserRow, AuthError> {
    let normalized = match normalize_email(email) {
        Ok(value) => value,
        Err(_) => {
            verify_against_dummy(password);
            return Err(AuthError::InvalidCredentials);
        }
    };
    let user = match store::find_user_by_email(db, &normalized).await? {
        Some(user) => user,
        None => {
            verify_against_dummy(password);
            return Err(AuthError::InvalidCredentials);
        }
    };
    let matches = verify_password(password, &user.password_hash).unwrap_or(false);
    if !matches || !user.is_active {
        return Err(AuthError::InvalidCredentials);
    }
    Ok(user)
}
