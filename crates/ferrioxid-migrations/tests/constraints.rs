//! SQLite-Prüfungen für Unique Constraints, Indizes und Fremdschlüssel. Kein Docker.

use ferrioxid_db::connect;
use ferrioxid_migrations::{
    apply_directory, baseline_database, make_migration, plan_change, write_explicit_migration,
    ColumnSnapshot, ForeignKeySnapshot, IndexSnapshot, LogicalType, MakeOutcome,
    MigrationOperation, OnDelete, SchemaSnapshot, TableSnapshot, UniqueConstraintSnapshot,
    FORMAT_VERSION,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tick = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ferrioxid-constraints-{nanos}-{tick}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn sqlite_url(path: &Path) -> String {
    format!(
        "sqlite://{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    )
}

fn column(
    name: &str,
    logical_type: LogicalType,
    primary_key: bool,
    nullable: bool,
) -> ColumnSnapshot {
    ColumnSnapshot {
        name: name.to_owned(),
        logical_type,
        primary_key,
        nullable,
        default: None,
    }
}

fn table(
    name: &str,
    columns: Vec<ColumnSnapshot>,
    unique_constraints: Vec<UniqueConstraintSnapshot>,
    indexes: Vec<IndexSnapshot>,
    foreign_keys: Vec<ForeignKeySnapshot>,
) -> TableSnapshot {
    TableSnapshot {
        name: name.to_owned(),
        columns,
        unique_constraints,
        indexes,
        foreign_keys,
    }
}

fn schema(tables: Vec<TableSnapshot>) -> SchemaSnapshot {
    SchemaSnapshot {
        format_version: FORMAT_VERSION,
        tables,
    }
}

fn parent(name: &str) -> TableSnapshot {
    table(
        name,
        vec![
            column("id", LogicalType::Uuid, true, false),
            column("name", LogicalType::String, false, false),
        ],
        vec![UniqueConstraintSnapshot {
            name: format!("uq_{name}_name"),
            columns: vec!["name".to_owned()],
        }],
        Vec::new(),
        Vec::new(),
    )
}

fn foreign_key(
    name: &str,
    column_name: &str,
    referenced_table: &str,
    on_delete: OnDelete,
) -> ForeignKeySnapshot {
    ForeignKeySnapshot {
        name: name.to_owned(),
        columns: vec![column_name.to_owned()],
        referenced_table: referenced_table.to_owned(),
        referenced_columns: vec!["id".to_owned()],
        on_delete,
        on_update: "no_action".to_owned(),
    }
}

fn catalog() -> SchemaSnapshot {
    schema(vec![
        parent("brands"),
        parent("labels"),
        parent("groups"),
        table(
            "products",
            vec![
                column("id", LogicalType::Uuid, true, false),
                column("name", LogicalType::String, false, false),
                column("brand_id", LogicalType::Uuid, false, false),
            ],
            Vec::new(),
            vec![IndexSnapshot {
                name: "ix_products_name".to_owned(),
                columns: vec!["name".to_owned()],
            }],
            vec![foreign_key(
                "fk_products_brand_id_brands_id",
                "brand_id",
                "brands",
                OnDelete::Restrict,
            )],
        ),
        table(
            "items",
            vec![
                column("id", LogicalType::Uuid, true, false),
                column("label_id", LogicalType::Uuid, false, false),
            ],
            Vec::new(),
            Vec::new(),
            vec![foreign_key(
                "fk_items_label_id_labels_id",
                "label_id",
                "labels",
                OnDelete::Cascade,
            )],
        ),
        table(
            "notes",
            vec![
                column("id", LogicalType::Uuid, true, false),
                column("group_id", LogicalType::Uuid, false, true),
                column("code", LogicalType::String, false, true),
            ],
            vec![UniqueConstraintSnapshot {
                name: "uq_notes_code".to_owned(),
                columns: vec!["code".to_owned()],
            }],
            Vec::new(),
            vec![foreign_key(
                "fk_notes_group_id_groups_id",
                "group_id",
                "groups",
                OnDelete::SetNull,
            )],
        ),
        table(
            "variants",
            vec![
                column("id", LogicalType::Uuid, true, false),
                column("product_id", LogicalType::Uuid, false, false),
                column("sku", LogicalType::String, false, false),
            ],
            vec![UniqueConstraintSnapshot {
                name: "uq_variants_product_id_sku".to_owned(),
                columns: vec!["product_id".to_owned(), "sku".to_owned()],
            }],
            Vec::new(),
            vec![foreign_key(
                "fk_variants_product_id_products_id",
                "product_id",
                "products",
                OnDelete::Restrict,
            )],
        ),
    ])
}

async fn open_db(path: &Path) -> DatabaseConnection {
    connect(&sqlite_url(path)).await.unwrap()
}

async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), sea_orm::DbErr> {
    db.execute(Statement::from_string(db.get_database_backend(), sql))
        .await
        .map(|_| ())
}

async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
    let row = db
        .query_one(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap()
        .unwrap();
    row.try_get::<i64>("", "total")
        .or_else(|_| row.try_get::<i32>("", "total").map(i64::from))
        .unwrap()
}

fn text(value: Option<String>) -> String {
    value.unwrap_or_else(|| "NULL".to_owned())
}

async fn scalar(db: &DatabaseConnection, sql: &str) -> String {
    let row = db
        .query_one(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap()
        .unwrap();
    text(row.try_get("", "value").unwrap())
}

#[tokio::test]
async fn sqlite_constraints_indexes_and_foreign_keys() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = catalog();
    let written = make_migration(&migrations, "catalog", &initial).unwrap();
    assert!(matches!(written, MakeOutcome::Written { .. }));
    let db = open_db(&directory.join("app.sqlite3")).await;
    let report = apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(report.applied.len(), 1);
    assert!(apply_directory(&db, &migrations, false)
        .await
        .unwrap()
        .applied
        .is_empty());

    execute(
        &db,
        "INSERT INTO \"brands\" (\"id\", \"name\") VALUES ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', 'Acme')",
    )
    .await
    .unwrap();
    assert!(execute(
        &db,
        "INSERT INTO \"brands\" (\"id\", \"name\") VALUES ('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', 'Acme')",
    )
    .await
    .is_err());
    execute(
        &db,
        "INSERT INTO \"notes\" (\"id\", \"group_id\", \"code\") VALUES ('11111111-1111-4111-8111-111111111111', NULL, NULL)",
    )
    .await
    .unwrap();
    execute(
        &db,
        "INSERT INTO \"notes\" (\"id\", \"group_id\", \"code\") VALUES ('22222222-2222-4222-8222-222222222222', NULL, NULL)",
    )
    .await
    .unwrap();
    execute(
        &db,
        "INSERT INTO \"notes\" (\"id\", \"group_id\", \"code\") VALUES ('33333333-3333-4333-8333-333333333333', NULL, 'N-1')",
    )
    .await
    .unwrap();
    assert!(execute(
        &db,
        "INSERT INTO \"notes\" (\"id\", \"group_id\", \"code\") VALUES ('44444444-4444-4444-8444-444444444444', NULL, 'N-1')",
    )
    .await
    .is_err());

    execute(
        &db,
        "INSERT INTO \"products\" (\"id\", \"name\", \"brand_id\") VALUES ('cccccccc-cccc-4ccc-8ccc-cccccccccccc', 'Laptop', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa')",
    )
    .await
    .unwrap();
    assert!(execute(
        &db,
        "INSERT INTO \"products\" (\"id\", \"name\", \"brand_id\") VALUES ('dddddddd-dddd-4ddd-8ddd-dddddddddddd', 'Other', '99999999-9999-4999-8999-999999999999')",
    )
    .await
    .is_err());
    assert!(execute(
        &db,
        "DELETE FROM \"brands\" WHERE \"id\" = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'",
    )
    .await
    .is_err());
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'index' AND name = 'ix_products_name'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'index' AND name = 'uq_brands_name'"
        )
        .await,
        1
    );

    execute(
        &db,
        "INSERT INTO \"products\" (\"id\", \"name\", \"brand_id\") VALUES ('eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee', 'Mouse', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa')",
    )
    .await
    .unwrap();
    execute(
        &db,
        "INSERT INTO \"variants\" (\"id\", \"product_id\", \"sku\") VALUES ('ffffffff-ffff-4fff-8fff-ffffffffffff', 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', 'SKU-1')",
    )
    .await
    .unwrap();
    assert!(execute(
        &db,
        "INSERT INTO \"variants\" (\"id\", \"product_id\", \"sku\") VALUES ('12121212-1212-4121-8121-121212121212', 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', 'SKU-1')",
    )
    .await
    .is_err());
    execute(
        &db,
        "INSERT INTO \"variants\" (\"id\", \"product_id\", \"sku\") VALUES ('13131313-1313-4131-8131-131313131313', 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee', 'SKU-1')",
    )
    .await
    .unwrap();

    execute(
        &db,
        "INSERT INTO \"labels\" (\"id\", \"name\") VALUES ('14141414-1414-4141-8141-141414141414', 'Red')",
    )
    .await
    .unwrap();
    execute(
        &db,
        "INSERT INTO \"items\" (\"id\", \"label_id\") VALUES ('15151515-1515-4151-8151-151515151515', '14141414-1414-4141-8141-141414141414')",
    )
    .await
    .unwrap();
    execute(
        &db,
        "DELETE FROM \"labels\" WHERE \"id\" = '14141414-1414-4141-8141-141414141414'",
    )
    .await
    .unwrap();
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM \"items\"").await,
        0
    );

    execute(
        &db,
        "INSERT INTO \"groups\" (\"id\", \"name\") VALUES ('16161616-1616-4161-8161-161616161616', 'Blue')",
    )
    .await
    .unwrap();
    execute(
        &db,
        "UPDATE \"notes\" SET \"group_id\" = '16161616-1616-4161-8161-161616161616' WHERE \"id\" = '11111111-1111-4111-8111-111111111111'",
    )
    .await
    .unwrap();
    execute(
        &db,
        "DELETE FROM \"groups\" WHERE \"id\" = '16161616-1616-4161-8161-161616161616'",
    )
    .await
    .unwrap();
    assert_eq!(
        scalar(
            &db,
            "SELECT \"group_id\" AS value FROM \"notes\" WHERE \"id\" = '11111111-1111-4111-8111-111111111111'"
        )
        .await,
        "NULL"
    );

    execute(&db, "PRAGMA wal_checkpoint(FULL)").await.unwrap();
    let baseline_path = directory.join("baseline.sqlite3");
    std::fs::copy(directory.join("app.sqlite3"), &baseline_path).unwrap();
    let baseline_db = open_db(&baseline_path).await;
    execute(&baseline_db, "DROP TABLE \"_ferrum_migrations\"")
        .await
        .unwrap();
    let baseline = baseline_database(&baseline_db, &migrations, false)
        .await
        .unwrap_err()
        .to_string();
    assert!(baseline.contains("Validation successful"), "{baseline}");

    let mut without_index = initial.clone();
    without_index
        .tables
        .iter_mut()
        .find(|table| table.name == "products")
        .unwrap()
        .indexes
        .clear();
    let dropped = make_migration(&migrations, "drop_name_index", &without_index).unwrap();
    assert!(matches!(dropped, MakeOutcome::Written { .. }));
    apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'index' AND name = 'ix_products_name'"
        )
        .await,
        0
    );

    execute(
        &db,
        "INSERT INTO \"products\" (\"id\", \"name\", \"brand_id\") VALUES ('17171717-1717-4171-8171-171717171717', 'Laptop', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa')",
    )
    .await
    .unwrap();
    let mut unique_name = without_index.clone();
    unique_name
        .tables
        .iter_mut()
        .find(|table| table.name == "products")
        .unwrap()
        .unique_constraints
        .push(UniqueConstraintSnapshot {
            name: "uq_products_name".to_owned(),
            columns: vec!["name".to_owned()],
        });
    make_migration(&migrations, "unique_product_name", &unique_name).unwrap();
    let history_before = count(&db, "SELECT COUNT(*) AS total FROM \"_ferrum_migrations\"").await;
    let failed = apply_directory(&db, &migrations, false).await;
    assert!(failed.is_err(), "{failed:?}");
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM \"_ferrum_migrations\"").await,
        history_before
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM sqlite_master WHERE name = 'uq_products_name'"
        )
        .await,
        0
    );

    let mut changed_rule = unique_name.clone();
    changed_rule
        .tables
        .iter_mut()
        .find(|table| table.name == "products")
        .unwrap()
        .foreign_keys[0]
        .on_delete = OnDelete::Cascade;
    let refused = make_migration(&migrations, "cascade_brand", &changed_rule);
    assert!(refused.is_err(), "{refused:?}");
    let plan = plan_change(&unique_name, &changed_rule).unwrap();
    assert!(plan.refusal.is_some());
}

#[tokio::test]
async fn renaming_an_indexed_column_renames_the_database_index() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = schema(vec![table(
        "products",
        vec![
            column("id", LogicalType::Uuid, true, false),
            column("name", LogicalType::String, false, false),
        ],
        Vec::new(),
        vec![IndexSnapshot {
            name: "ix_products_name".to_owned(),
            columns: vec!["name".to_owned()],
        }],
        Vec::new(),
    )]);
    make_migration(&migrations, "products", &initial).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    apply_directory(&db, &migrations, false).await.unwrap();
    let mut renamed = initial.clone();
    let product = renamed
        .tables
        .iter_mut()
        .find(|table| table.name == "products")
        .unwrap();
    product.columns[1].name = "title".to_owned();
    product.indexes[0].name = "ix_products_title".to_owned();
    product.indexes[0].columns = vec!["title".to_owned()];
    write_explicit_migration(
        &migrations,
        "rename_title",
        &renamed,
        MigrationOperation::RenameColumn {
            table: "products".to_owned(),
            from: "name".to_owned(),
            to: "title".to_owned(),
        },
    )
    .unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'index' AND name = 'ix_products_title'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'index' AND name = 'ix_products_name'"
        )
        .await,
        0
    );
}

#[test]
fn set_null_on_required_column_is_rejected() {
    let snapshot = schema(vec![
        parent("groups"),
        table(
            "notes",
            vec![
                column("id", LogicalType::Uuid, true, false),
                column("group_id", LogicalType::Uuid, false, false),
            ],
            Vec::new(),
            Vec::new(),
            vec![foreign_key(
                "fk_notes_group_id_groups_id",
                "group_id",
                "groups",
                OnDelete::SetNull,
            )],
        ),
    ]);
    assert!(snapshot.validate().is_err());
}

#[test]
fn foreign_key_to_an_unknown_table_is_rejected() {
    let snapshot = schema(vec![table(
        "products",
        vec![
            column("id", LogicalType::Uuid, true, false),
            column("brand_id", LogicalType::Uuid, false, false),
        ],
        Vec::new(),
        Vec::new(),
        vec![foreign_key(
            "fk_products_brand_id_brands_id",
            "brand_id",
            "brands",
            OnDelete::Restrict,
        )],
    )]);
    assert!(snapshot.validate().is_err());
}
