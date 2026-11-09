//! Cross-app foreign keys. SQLite runs in this suite. PostgreSQL is ignored unless DATABASE_URL is set.

use ferrioxid_core::{ColumnKind, DatabaseKind, ReferentialAction};
use ferrioxid_db::{render_table_bundle, ForeignKeyClause, PhysicalColumn};
use ferrioxid_migrations::{
    apply_packages, make_migration, resolve_installed_schemas, ColumnSnapshot, ForeignKeySnapshot,
    InstalledApp, LogicalType, MakeOutcome, OnDelete, PackageMigration, SchemaSnapshot,
    TableSnapshot,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
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
    let path = std::env::temp_dir().join(format!("ferrioxid-cross-app-{nanos}-{tick}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn column(name: &str, primary_key: bool) -> ColumnSnapshot {
    ColumnSnapshot {
        name: name.to_owned(),
        logical_type: LogicalType::Uuid,
        primary_key,
        nullable: false,
        default: None,
        precision: None,
        scale: None,
        choices: Vec::new(),
    }
}

fn named_table(
    name: &str,
    columns: Vec<ColumnSnapshot>,
    foreign_keys: Vec<ForeignKeySnapshot>,
) -> TableSnapshot {
    TableSnapshot {
        name: name.to_owned(),
        columns,
        unique_constraints: Vec::new(),
        indexes: Vec::new(),
        foreign_keys,
    }
}

fn foreign_key(
    table: &str,
    column_name: &str,
    referenced: &str,
    on_delete: OnDelete,
) -> ForeignKeySnapshot {
    ForeignKeySnapshot {
        name: format!("fk_{table}_{column_name}_{referenced}_id"),
        columns: vec![column_name.to_owned()],
        referenced_table: referenced.to_owned(),
        referenced_columns: vec!["id".to_owned()],
        on_delete,
        on_update: "no_action".to_owned(),
    }
}

fn identity(name: &str) -> TableSnapshot {
    named_table(
        name,
        vec![column("id", true), column("name", false)],
        Vec::new(),
    )
}

fn child(name: &str, column_name: &str, referenced: &str, on_delete: OnDelete) -> TableSnapshot {
    named_table(
        name,
        vec![column("id", true), column(column_name, false)],
        vec![foreign_key(name, column_name, referenced, on_delete)],
    )
}

fn app(name: &str, depends_on: &[&str], tables: Vec<TableSnapshot>) -> InstalledApp {
    InstalledApp {
        name: name.to_owned(),
        depends_on: depends_on.iter().map(|item| (*item).to_owned()).collect(),
        schema: SchemaSnapshot::from_tables(tables),
    }
}

fn shop() -> Vec<InstalledApp> {
    vec![
        app(
            "orders",
            &["cart", "products"],
            vec![child(
                "order_lines",
                "product_id",
                "products",
                OnDelete::Cascade,
            )],
        ),
        app(
            "cart",
            &["products"],
            vec![child(
                "cart_items",
                "product_id",
                "products",
                OnDelete::Restrict,
            )],
        ),
        app("products", &[], vec![identity("products")]),
    ]
}

fn package(name: &str, root: &Path, depends_on: &[&str]) -> PackageMigration {
    PackageMigration {
        name: name.to_owned(),
        directory: root.join(name),
        depends_on: depends_on.iter().map(|item| (*item).to_owned()).collect(),
    }
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

#[test]
fn dependency_graph_accepts_direct_targets_and_rejects_unsafe_graphs() {
    let resolved = resolve_installed_schemas(&shop()).unwrap();
    assert_eq!(
        resolved
            .iter()
            .map(|app| app.name.as_str())
            .collect::<Vec<_>>(),
        vec!["products", "cart", "orders"]
    );
    let cart = resolved.iter().find(|app| app.name == "cart").unwrap();
    assert_eq!(cart.schema.depends_on, vec!["products".to_owned()]);
    assert_eq!(cart.schema.external_tables[0].app, "products");
    assert_eq!(cart.schema.external_tables[0].table.name, "products");
    let orders = resolved.iter().find(|app| app.name == "orders").unwrap();
    assert_eq!(
        orders.schema.depends_on,
        vec!["cart".to_owned(), "products".to_owned()]
    );

    let mut missing_dependency = shop();
    missing_dependency[1].depends_on.clear();
    let missing = resolve_installed_schemas(&missing_dependency)
        .unwrap_err()
        .0;
    assert!(missing.contains("does not declare"), "{missing}");
    assert!(missing.contains("products"), "{missing}");
    assert!(missing.contains("were not changed"), "{missing}");

    let mut missing_target = shop();
    missing_target[1].schema.tables[0].foreign_keys[0].referenced_table = "widgets".to_owned();
    let unknown = resolve_installed_schemas(&missing_target).unwrap_err().0;
    assert!(unknown.contains("unknown table `widgets`"), "{unknown}");

    let ambiguous = vec![
        app("catalog", &[], vec![identity("products")]),
        app("archive", &[], vec![identity("products")]),
    ];
    let clash = resolve_installed_schemas(&ambiguous).unwrap_err().0;
    assert!(clash.contains("both"), "{clash}");

    let cycle = vec![
        app("cart", &["orders"], vec![identity("cart_items")]),
        app("orders", &["cart"], vec![identity("order_lines")]),
    ];
    let cyclic = resolve_installed_schemas(&cycle).unwrap_err().0;
    assert!(cyclic.contains("cycle"), "{cyclic}");
    assert!(cyclic.contains("cart"), "{cyclic}");
    assert!(cyclic.contains("orders"), "{cyclic}");
}

#[test]
fn sqlite_and_postgres_render_the_declared_delete_action() {
    let column = PhysicalColumn {
        name: "product_id",
        kind: ColumnKind::Uuid,
        nullable: false,
        primary_key: false,
        default_bool: None,
    };
    let id = PhysicalColumn {
        name: "id",
        kind: ColumnKind::Uuid,
        nullable: false,
        primary_key: true,
        default_bool: None,
    };
    for kind in [DatabaseKind::Sqlite, DatabaseKind::Postgres] {
        let foreign_key = ForeignKeyClause {
            name: "fk_cart_items_product_id_products_id",
            column: "product_id",
            referenced_table: "products",
            referenced_column: "id",
            on_delete: ReferentialAction::Restrict,
        };
        let sql = render_table_bundle(kind, "cart_items", &[id, column], &[], &[foreign_key], &[])
            .remove(0);
        assert!(sql.contains("REFERENCES"), "{kind:?}: {sql}");
        assert!(sql.contains("\"products\""), "{kind:?}: {sql}");
        assert!(sql.contains("ON DELETE RESTRICT"), "{kind:?}: {sql}");
        assert!(sql.contains("ON UPDATE NO ACTION"), "{kind:?}: {sql}");
    }
}

#[tokio::test]
async fn sqlite_enforces_cross_app_foreign_keys_without_rewriting_history() {
    let root = temp_dir();
    let resolved = resolve_installed_schemas(&shop()).unwrap();
    for app in &resolved {
        let written = make_migration(&root.join(&app.name), "initial", &app.schema).unwrap();
        assert!(
            matches!(written, MakeOutcome::Written { .. }),
            "{}",
            app.name
        );
        let again = make_migration(&root.join(&app.name), "again", &app.schema).unwrap();
        assert!(matches!(again, MakeOutcome::Unchanged));
    }
    let cart_path = root.join("cart").join("0001_initial.json");
    let cart_text = std::fs::read_to_string(&cart_path).unwrap();
    assert!(cart_text.contains("\"depends_on\":"));
    assert!(cart_text.contains("\"external_tables\":"));
    assert!(cart_text.contains("REFERENCES") || cart_text.contains("products"));
    let checksum = ferrioxid_migrations::load_migration(&cart_path)
        .unwrap()
        .checksum;

    let packages = vec![
        package("orders", &root, &["cart", "products"]),
        package("cart", &root, &["products"]),
        package("products", &root, &[]),
    ];
    let url = format!(
        "sqlite://{}?mode=rwc",
        root.join("app.sqlite3")
            .to_string_lossy()
            .replace('\\', "/")
    );
    let db = ferrioxid_db::connect(&url).await.unwrap();
    let applied = apply_packages(&db, &packages, false).await.unwrap();
    assert_eq!(
        applied,
        vec![
            "products:0001_initial".to_owned(),
            "cart:0001_initial".to_owned(),
            "orders:0001_initial".to_owned()
        ]
    );
    assert!(apply_packages(&db, &packages, false)
        .await
        .unwrap()
        .is_empty());

    let definition = table_sql(&db, "cart_items").await;
    assert!(definition.contains("REFERENCES"), "{definition}");
    assert!(definition.contains("products"), "{definition}");
    assert!(
        definition.to_ascii_uppercase().contains("RESTRICT"),
        "{definition}"
    );
    let order_sql = table_sql(&db, "order_lines").await.to_ascii_uppercase();
    assert!(order_sql.contains("CASCADE"), "{order_sql}");

    let product = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let line = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    let item = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
    execute(
        &db,
        &format!("INSERT INTO \"products\" (\"id\", \"name\") VALUES ('{product}', '{product}')"),
    )
    .await
    .unwrap();
    execute(
        &db,
        &format!(
            "INSERT INTO \"order_lines\" (\"id\", \"product_id\") VALUES ('{line}', '{product}')"
        ),
    )
    .await
    .unwrap();
    execute(
        &db,
        &format!("DELETE FROM \"products\" WHERE \"id\" = '{product}'"),
    )
    .await
    .unwrap();
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM \"order_lines\"").await,
        0
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM \"products\"").await,
        0
    );

    execute(
        &db,
        &format!("INSERT INTO \"products\" (\"id\", \"name\") VALUES ('{product}', '{product}')"),
    )
    .await
    .unwrap();
    execute(
        &db,
        &format!(
            "INSERT INTO \"cart_items\" (\"id\", \"product_id\") VALUES ('{item}', '{product}')"
        ),
    )
    .await
    .unwrap();
    assert!(execute(
        &db,
        "INSERT INTO \"cart_items\" (\"id\", \"product_id\") VALUES ('dddddddd-dddd-4ddd-8ddd-dddddddddddd', '99999999-9999-4999-8999-999999999999')",
    )
    .await
    .is_err());
    assert!(execute(
        &db,
        &format!("DELETE FROM \"products\" WHERE \"id\" = '{product}'"),
    )
    .await
    .is_err());
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM \"products\"").await,
        1
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM \"cart_items\"").await,
        1
    );

    let mut broken = packages.clone();
    broken
        .iter_mut()
        .find(|package| package.name == "cart")
        .unwrap()
        .depends_on
        .clear();
    let error = apply_packages(&db, &broken, false).await.unwrap_err().0;
    assert!(error.contains("does not declare"), "{error}");
    assert_eq!(std::fs::read_to_string(&cart_path).unwrap(), cart_text);
    assert_eq!(
        ferrioxid_migrations::load_migration(&cart_path)
            .unwrap()
            .checksum,
        checksum
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS total FROM \"products\"").await,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM \"_ferrioxid_migrations\""
        )
        .await,
        3
    );

    let only_cart = vec![package("cart", &root, &["products"])];
    let refused = apply_packages(&db, &only_cart, false).await.unwrap_err().0;
    assert!(refused.contains("missing app `products`"), "{refused}");
    let _ = std::fs::remove_dir_all(root);
}

async fn table_sql(db: &DatabaseConnection, table: &str) -> String {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT sql AS value FROM sqlite_master WHERE name = ?",
            [sea_orm::Value::from(table)],
        ))
        .await
        .unwrap()
        .unwrap();
    row.try_get::<String>("", "value").unwrap()
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
async fn postgres_enforces_cross_app_foreign_keys() {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run the ignored PostgreSQL cross-app test");
    let root = temp_dir();
    let mut apps = shop();
    for app in &mut apps {
        for table in &mut app.schema.tables {
            table.name = format!("ferrioxid_v42_{}", table.name);
            for foreign_key in &mut table.foreign_keys {
                foreign_key.referenced_table =
                    format!("ferrioxid_v42_{}", foreign_key.referenced_table);
            }
        }
    }
    let resolved = resolve_installed_schemas(&apps).unwrap();
    for app in &resolved {
        make_migration(&root.join(&app.name), "initial", &app.schema).unwrap();
    }
    let packages = vec![
        package("orders", &root, &["cart", "products"]),
        package("cart", &root, &["products"]),
        package("products", &root, &[]),
    ];
    let db = ferrioxid_db::connect(&url)
        .await
        .expect("postgres connection");
    for table in [
        "ferrioxid_v42_order_lines",
        "ferrioxid_v42_cart_items",
        "ferrioxid_v42_products",
    ] {
        let _ = execute(&db, &format!("DROP TABLE IF EXISTS \"{table}\" CASCADE")).await;
    }
    apply_packages(&db, &packages, false)
        .await
        .expect("postgres migrations");
    let product = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    execute(
        &db,
        &format!("INSERT INTO \"ferrioxid_v42_products\" (\"id\", \"name\") VALUES ('{product}', '{product}')"),
    )
    .await
    .unwrap();
    assert!(execute(
        &db,
        "INSERT INTO \"ferrioxid_v42_cart_items\" (\"id\", \"product_id\") VALUES ('dddddddd-dddd-4ddd-8ddd-dddddddddddd', '99999999-9999-4999-8999-999999999999')",
    )
    .await
    .is_err());
    execute(
        &db,
        &format!("INSERT INTO \"ferrioxid_v42_cart_items\" (\"id\", \"product_id\") VALUES ('cccccccc-cccc-4ccc-8ccc-cccccccccccc', '{product}')"),
    )
    .await
    .unwrap();
    assert!(execute(
        &db,
        &format!("DELETE FROM \"ferrioxid_v42_products\" WHERE \"id\" = '{product}'"),
    )
    .await
    .is_err());
    assert!(apply_packages(&db, &packages, false)
        .await
        .unwrap()
        .is_empty());
    for table in [
        "ferrioxid_v42_order_lines",
        "ferrioxid_v42_cart_items",
        "ferrioxid_v42_products",
    ] {
        execute(&db, &format!("DROP TABLE IF EXISTS \"{table}\" CASCADE"))
            .await
            .unwrap();
    }
    let _ = std::fs::remove_dir_all(root);
}
