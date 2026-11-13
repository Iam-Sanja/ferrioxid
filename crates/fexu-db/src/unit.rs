//! Eine Schreibgrenze. Der Aufrufer arbeitet auf der übergebenen Transaktion.
//! `conclude` committet nur nach `Ok` und rollt bei `Err` zurück.

use crate::driver::DatabaseTransaction;

pub async fn conclude<T, E>(
    tx: DatabaseTransaction,
    result: Result<T, E>,
    map_commit: impl FnOnce(crate::driver::DbErr) -> E,
) -> Result<T, E> {
    match result {
        Ok(value) => match tx.commit().await {
            Ok(()) => Ok(value),
            Err(error) => Err(map_commit(error)),
        },
        Err(error) => {
            let _ = tx.rollback().await;
            Err(error)
        }
    }
}
