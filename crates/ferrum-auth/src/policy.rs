//! Objektregeln. Der Query-Scope filtert Listen und Schreibzugriffe in SQL.

use crate::service::AuthUser;
use ferrum_core::QueryScope;

pub trait ObjectPolicy<T> {
    fn can_read(user: &AuthUser, object: &T) -> bool;
    fn can_update(user: &AuthUser, object: &T) -> bool;
    fn can_delete(user: &AuthUser, object: &T) -> bool;

    /// Standard ist keine Zeile. Eine Policy muss `All` oder `Equals` ausdrücklich liefern.
    fn read_scope(_user: &AuthUser) -> QueryScope {
        QueryScope::None
    }

    fn write_scope(user: &AuthUser) -> QueryScope {
        Self::read_scope(user)
    }
}
