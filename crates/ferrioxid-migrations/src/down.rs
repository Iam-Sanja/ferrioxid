//! Inverse einer gespeicherten Migration. Datenverlust bleibt abgelehnt, bis er ausdrücklich erlaubt ist.

use crate::diff::{alter_is_supported, MigrationOperation};
use crate::error::MigrationError;
use crate::snapshot::SchemaSnapshot;

pub fn invert_operations(
    operations: &[MigrationOperation],
) -> Result<Vec<MigrationOperation>, MigrationError> {
    let mut inverted = Vec::with_capacity(operations.len());
    for operation in operations.iter().rev() {
        inverted.push(invert_one(operation)?);
    }
    Ok(inverted)
}

fn invert_one(operation: &MigrationOperation) -> Result<MigrationOperation, MigrationError> {
    Ok(match operation {
        MigrationOperation::CreateTable { table } => MigrationOperation::DropTable {
            name: table.name.clone(),
        },
        MigrationOperation::DropTable { name } => {
            return Err(irreversible(&format!("drop_table {name}")))
        }
        MigrationOperation::AddColumn { table, column, .. } => MigrationOperation::DropColumn {
            table: table.clone(),
            name: column.name.clone(),
        },
        MigrationOperation::DropColumn { table, name } => {
            return Err(irreversible(&format!("drop_column {table}.{name}")))
        }
        MigrationOperation::AlterColumn { table, from, to } => {
            if !alter_is_supported(to, from) {
                return Err(irreversible(&format!("alter_column {table}.{}", from.name)));
            }
            MigrationOperation::AlterColumn {
                table: table.clone(),
                from: to.clone(),
                to: from.clone(),
            }
        }
        MigrationOperation::RenameColumn { table, from, to } => MigrationOperation::RenameColumn {
            table: table.clone(),
            from: to.clone(),
            to: from.clone(),
        },
        MigrationOperation::RenameTable { from, to } => MigrationOperation::RenameTable {
            from: to.clone(),
            to: from.clone(),
        },
        MigrationOperation::AddUniqueConstraint { table, constraint } => {
            MigrationOperation::DropUniqueConstraint {
                table: table.clone(),
                name: constraint.name.clone(),
            }
        }
        MigrationOperation::DropUniqueConstraint { table, name } => {
            return Err(irreversible(&format!("drop_unique {table}.{name}")))
        }
        MigrationOperation::AddIndex { table, index } => MigrationOperation::DropIndex {
            table: table.clone(),
            name: index.name.clone(),
        },
        MigrationOperation::DropIndex { table, name } => {
            return Err(irreversible(&format!("drop_index {table}.{name}")))
        }
        MigrationOperation::AddForeignKey { table, foreign_key } => {
            MigrationOperation::DropForeignKey {
                table: table.clone(),
                name: foreign_key.name.clone(),
                previous: Some(foreign_key.clone()),
            }
        }
        MigrationOperation::DropForeignKey {
            table,
            previous: Some(foreign_key),
            ..
        } => MigrationOperation::AddForeignKey {
            table: table.clone(),
            foreign_key: foreign_key.clone(),
        },
        MigrationOperation::DropForeignKey { table, name, .. } => {
            return Err(irreversible(&format!("drop_foreign_key {table}.{name}")))
        }
    })
}

fn irreversible(operation: &str) -> MigrationError {
    MigrationError::new(format!(
        "Operation `{operation}` cannot be reversed without a stored snapshot. No tables or rows were changed."
    ))
}

pub fn file_id(history_id: &str) -> &str {
    history_id.rsplit(':').next().unwrap_or(history_id)
}

pub fn simulated_target(
    from: &SchemaSnapshot,
    operations: &[MigrationOperation],
) -> Result<SchemaSnapshot, MigrationError> {
    crate::simulate::apply_operations(from, operations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{plan_change, plan_change_with_backfills, BackfillLiteral, ColumnBackfill};
    use crate::snapshot::{
        ColumnSnapshot, ForeignKeySnapshot, LogicalType, OnDelete, SchemaSnapshot, TableSnapshot,
        FORMAT_VERSION,
    };
    use crate::store::{save_migration, seal};
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    fn column(name: &str, logical_type: LogicalType, primary_key: bool) -> ColumnSnapshot {
        ColumnSnapshot {
            name: name.to_owned(),
            logical_type,
            primary_key,
            nullable: false,
            default: None,
            precision: None,
            scale: None,
            choices: Vec::new(),
        }
    }

    fn parent() -> TableSnapshot {
        TableSnapshot {
            name: "ferrioxid_down_parent".to_owned(),
            columns: vec![column("id", LogicalType::Uuid, true)],
            unique_constraints: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
        }
    }

    fn child(on_delete: OnDelete, quantity: bool) -> TableSnapshot {
        let mut columns = vec![
            column("id", LogicalType::Uuid, true),
            column("product_id", LogicalType::Uuid, false),
        ];
        if quantity {
            columns.push(column("quantity", LogicalType::I64, false));
        }
        TableSnapshot {
            name: "ferrioxid_down_child".to_owned(),
            columns,
            unique_constraints: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: vec![ForeignKeySnapshot {
                name: "fk_ferrioxid_down_child_product".to_owned(),
                columns: vec!["product_id".to_owned()],
                referenced_table: "ferrioxid_down_parent".to_owned(),
                referenced_columns: vec!["id".to_owned()],
                on_delete,
                on_update: "no_action".to_owned(),
            }],
        }
    }

    fn schema(child_table: TableSnapshot) -> SchemaSnapshot {
        SchemaSnapshot {
            format_version: FORMAT_VERSION,
            tables: vec![parent(), child_table],
            depends_on: Vec::new(),
            external_tables: Vec::new(),
        }
    }

    async fn execute(db: &impl ConnectionTrait, sql: &str) {
        db.execute(Statement::from_string(
            db.get_database_backend(),
            sql.to_owned(),
        ))
        .await
        .unwrap();
    }

    async fn roundtrip(url: &str, backend: DatabaseBackend) {
        if backend == DatabaseBackend::Postgres {
            let db = ferrioxid_db::connect(url).await.unwrap();
            execute(&db, "DROP TABLE IF EXISTS ferrioxid_down_child").await;
            execute(&db, "DROP TABLE IF EXISTS ferrioxid_down_parent").await;
        }
        let directory =
            std::env::temp_dir().join(format!("ferrioxid-down-{}", uuid::Uuid::new_v4()));
        let empty = SchemaSnapshot::empty();
        let created = schema(child(OnDelete::Restrict, false));
        let plan = plan_change(&empty, &created).unwrap();
        assert!(plan.refusal.is_none(), "{:?}", plan.refusal);
        let file = seal(
            "0001_create".to_owned(),
            "create".to_owned(),
            empty,
            created.clone(),
            &plan,
        )
        .unwrap();
        save_migration(&directory, &file).unwrap();
        let cascaded = schema(child(OnDelete::Cascade, false));
        let fk_plan = plan_change(&created, &cascaded).unwrap();
        assert!(fk_plan.refusal.is_none(), "{:?}", fk_plan.refusal);
        assert!(fk_plan
            .operations
            .iter()
            .any(|operation| matches!(operation, MigrationOperation::DropForeignKey { .. })));
        save_migration(
            &directory,
            &seal(
                "0002_fk".to_owned(),
                "fk".to_owned(),
                created,
                cascaded.clone(),
                &fk_plan,
            )
            .unwrap(),
        )
        .unwrap();
        let filled = schema(child(OnDelete::Cascade, true));
        let backfill = plan_change_with_backfills(
            &cascaded,
            &filled,
            &[ColumnBackfill {
                table: "ferrioxid_down_child".to_owned(),
                column: "quantity".to_owned(),
                value: BackfillLiteral::I64(1),
            }],
        )
        .unwrap();
        assert!(backfill.refusal.is_none(), "{:?}", backfill.refusal);
        save_migration(
            &directory,
            &seal(
                "0003_quantity".to_owned(),
                "quantity".to_owned(),
                cascaded,
                filled,
                &backfill,
            )
            .unwrap(),
        )
        .unwrap();

        let db = ferrioxid_db::connect(url).await.unwrap();
        let only_first = directory.join("hold");
        std::fs::create_dir_all(&only_first).unwrap();
        std::fs::rename(
            directory.join("0001_create.json"),
            only_first.join("0001_create.json"),
        )
        .unwrap();
        crate::apply_directory(&db, &only_first, false)
            .await
            .unwrap();
        let parent_id = uuid::Uuid::from_u128(7);
        let child_id = uuid::Uuid::from_u128(8);
        insert_id(
            &db,
            backend,
            "ferrioxid_down_parent",
            &["id"],
            &[parent_id.to_string()],
        )
        .await;
        insert_id(
            &db,
            backend,
            "ferrioxid_down_child",
            &["id", "product_id"],
            &[child_id.to_string(), parent_id.to_string()],
        )
        .await;
        std::fs::rename(
            only_first.join("0001_create.json"),
            directory.join("0001_create.json"),
        )
        .unwrap();
        crate::apply_directory(&db, &directory, false)
            .await
            .unwrap();
        let quantity = scalar(&db, "SELECT quantity FROM ferrioxid_down_child").await;
        assert_eq!(quantity, 1);

        let refused = crate::revert_latest(&db, &directory, false, false).await;
        assert!(refused.is_err());
        assert_eq!(
            scalar(&db, "SELECT quantity FROM ferrioxid_down_child").await,
            1
        );
        let reverted = crate::revert_latest(&db, &directory, true, false)
            .await
            .unwrap();
        assert!(reverted.executed);
        let missing = db
            .query_one(Statement::from_string(
                backend,
                "SELECT quantity FROM ferrioxid_down_child".to_owned(),
            ))
            .await;
        assert!(missing.is_err());
        let fk = crate::revert_latest(&db, &directory, false, false)
            .await
            .unwrap();
        assert!(fk.executed);
        assert!(!fk.destructive);
        let preview = crate::revert_latest(&db, &directory, false, true)
            .await
            .unwrap();
        assert!(!preview.executed);
        assert!(preview.destructive);
        let still = scalar(&db, "SELECT COUNT(*) AS total FROM ferrioxid_down_parent").await;
        assert_eq!(still, 1);
        let _ = std::fs::remove_dir_all(&directory);
        if backend == DatabaseBackend::Postgres {
            execute(&db, "DROP TABLE IF EXISTS ferrioxid_down_child").await;
            execute(&db, "DROP TABLE IF EXISTS ferrioxid_down_parent").await;
        }
    }

    async fn insert_id(
        db: &impl ConnectionTrait,
        backend: DatabaseBackend,
        table: &str,
        columns: &[&str],
        values: &[String],
    ) {
        let names = columns
            .iter()
            .map(|column| format!("\"{column}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let markers = match backend {
            DatabaseBackend::Postgres => (1..=values.len())
                .map(|index| format!("${index}"))
                .collect::<Vec<_>>()
                .join(", "),
            _ => vec!["?"; values.len()].join(", "),
        };
        let bound = values
            .iter()
            .cloned()
            .map(sea_orm::Value::from)
            .collect::<Vec<_>>();
        db.execute(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO \"{table}\" ({names}) VALUES ({markers})"),
            bound,
        ))
        .await
        .unwrap();
    }

    async fn scalar(db: &impl ConnectionTrait, sql: &str) -> i64 {
        let row = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                sql.to_owned(),
            ))
            .await
            .unwrap()
            .unwrap();
        row.try_get::<i64>("", "quantity")
            .or_else(|_| row.try_get::<i64>("", "total"))
            .or_else(|_| row.try_get::<i32>("", "quantity").map(i64::from))
            .or_else(|_| row.try_get::<i32>("", "total").map(i64::from))
            .unwrap()
    }

    #[tokio::test]
    async fn sqlite_reverts_backfill_and_foreign_key_without_touching_a_dry_run() {
        let directory =
            std::env::temp_dir().join(format!("ferrioxid-down-url-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("down.sqlite");
        let url = format!(
            "sqlite://{}?mode=rwc",
            path.display().to_string().replace('\\', "/")
        );
        roundtrip(&url, DatabaseBackend::Sqlite).await;
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn sqlite_reverts_a_namespaced_app_migration() {
        let directory =
            std::env::temp_dir().join(format!("ferrioxid-down-app-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("down.sqlite");
        let url = format!(
            "sqlite://{}?mode=rwc",
            path.display().to_string().replace('\\', "/")
        );
        let empty = SchemaSnapshot::empty();
        let created = SchemaSnapshot::from_tables(vec![parent()]);
        let plan = plan_change(&empty, &created).unwrap();
        save_migration(
            &directory,
            &seal(
                "0001_parent".to_owned(),
                "parent".to_owned(),
                empty,
                created,
                &plan,
            )
            .unwrap(),
        )
        .unwrap();
        let db = ferrioxid_db::connect(&url).await.unwrap();
        crate::apply_namespaced_directory(&db, &directory, "catalog", false)
            .await
            .unwrap();
        insert_id(
            &db,
            DatabaseBackend::Sqlite,
            "ferrioxid_down_parent",
            &["id"],
            &[uuid::Uuid::from_u128(11).to_string()],
        )
        .await;
        let refused = crate::revert_namespaced(&db, &directory, "catalog", false, false).await;
        assert!(refused.is_err(), "{refused:?}");
        assert_eq!(
            scalar(&db, "SELECT COUNT(*) AS total FROM ferrioxid_down_parent").await,
            1
        );
        let preview = crate::revert_namespaced(&db, &directory, "catalog", false, true)
            .await
            .unwrap();
        assert!(!preview.executed);
        assert!(preview.destructive);
        assert_eq!(
            scalar(&db, "SELECT COUNT(*) AS total FROM ferrioxid_down_parent").await,
            1
        );
        let reverted = crate::revert_namespaced(&db, &directory, "catalog", true, false)
            .await
            .unwrap();
        assert!(reverted.executed);
        let missing = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) AS total FROM ferrioxid_down_parent".to_owned(),
            ))
            .await;
        assert!(missing.is_err(), "{missing:?}");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
    async fn postgres_reverts_backfill_and_foreign_key() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        roundtrip(&url, DatabaseBackend::Postgres).await;
    }
}
