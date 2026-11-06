//! Objektregeln. Listen bleiben ohne SQL-Scope blockiert.

use crate::service::AuthUser;

pub trait ObjectPolicy<T> {
    fn can_read(user: &AuthUser, object: &T) -> bool;
    fn can_update(user: &AuthUser, object: &T) -> bool;
    fn can_delete(user: &AuthUser, object: &T) -> bool;
}
