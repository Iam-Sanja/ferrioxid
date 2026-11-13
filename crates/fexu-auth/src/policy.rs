//! Objektregeln. Der Query-Scope filtert Listen und Schreibzugriffe in SQL.
//! Neue Datensätze werden vor dem INSERT geprüft.

use crate::service::AuthUser;
use fexu_core::QueryScope;
use fexu_db::ModelRepository;

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

    /// Standard ist Ablehnung. Eine Create-Policy muss fremde Besitzer ausdrücklich zurückweisen.
    fn can_create(_user: &AuthUser, _input: &<T as ModelRepository>::Create) -> bool
    where
        T: ModelRepository,
    {
        false
    }
}
