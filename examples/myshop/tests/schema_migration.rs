//! `0001_initial.json` bleibt an das Produktmodell gebunden, das beim Anlegen dieser Datei galt.
//! Das aktuelle Product hat `category_id` und wird durch `0002_categories_and_product_relations.json` abgebildet.

use ferrioxid::core::FerrioxidModel;
use ferrioxid::migrations::{
    load_migration, make_migration, make_migration_with_backfills, plan_change,
    plan_change_with_backfills, seal, BackfillLiteral, ColumnBackfill, RegisteredModel,
    SchemaSnapshot,
};
use myshop::models::category::Category;
use myshop::models::event::Event;
use myshop::models::internal_note::InternalNote;
use myshop::models::order::Order;
use myshop::models::order_line::OrderLine;
use myshop::models::product::Product as LiveProduct;
use myshop_schema::Product;

mod myshop_schema {
    use ferrioxid::prelude::*;

    #[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
    #[ferrioxid(table = "products", api = "/api/v1/products", allow_anonymous = true)]
    pub struct Product {
        #[field(primary_key, auto)]
        pub id: Uuid,
        #[field(required, min_length = 3, max_length = 150)]
        pub name: String,
        #[field(min = 0)]
        pub price_cents: i64,
        #[field(default = true)]
        pub active: bool,
    }
}

#[test]
fn initial_migration_matches_the_registered_product_model() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/0001_initial.json");
    let migration = load_migration(&path).expect("0001_initial");
    let schema = SchemaSnapshot::from_registered(&[RegisteredModel {
        table: Product::descriptor().table,
        columns: Product::columns(),
        constraints: Product::constraints(),
    }])
    .unwrap();
    assert!(migration.from_snapshot.tables.is_empty());
    assert!(migration.to_snapshot.semantically_eq(&schema));
    assert_eq!(migration.operations.len(), 1);
}

#[test]
fn events_migration_adds_date_time_columns_without_rewriting_history() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let current = SchemaSnapshot::from_registered(&[
        RegisteredModel {
            table: Category::descriptor().table,
            columns: Category::columns(),
            constraints: Category::constraints(),
        },
        RegisteredModel {
            table: LiveProduct::descriptor().table,
            columns: LiveProduct::columns(),
            constraints: LiveProduct::constraints(),
        },
        RegisteredModel {
            table: InternalNote::descriptor().table,
            columns: InternalNote::columns(),
            constraints: InternalNote::constraints(),
        },
        RegisteredModel {
            table: Event::descriptor().table,
            columns: Event::columns(),
            constraints: Event::constraints(),
        },
    ])
    .unwrap();
    let path = directory.join("0004_events.json");
    if !path.exists() {
        make_migration(&directory, "events", &current).unwrap();
    }
    let migration = load_migration(&path).unwrap();
    assert_eq!(migration.id, "0004_events");
    let plan = plan_change(&migration.from_snapshot, &migration.to_snapshot).unwrap();
    assert!(plan.refusal.is_none());
    let sealed = seal(
        migration.id.clone(),
        migration.name.clone(),
        migration.from_snapshot.clone(),
        migration.to_snapshot.clone(),
        &plan,
    )
    .unwrap();
    assert_eq!(migration.checksum, sealed.checksum);
    let events = migration
        .to_snapshot
        .tables
        .iter()
        .find(|table| table.name == "events")
        .unwrap();
    let kind = |name: &str| {
        events
            .columns
            .iter()
            .find(|column| column.name == name)
            .unwrap()
            .logical_type
    };
    use ferrioxid::migrations::LogicalType;
    assert_eq!(kind("event_date"), LogicalType::Date);
    assert_eq!(kind("start_time"), LogicalType::Time);
    assert_eq!(kind("created_at"), LogicalType::DateTime);
    assert!(
        events
            .columns
            .iter()
            .find(|column| column.name == "start_time")
            .unwrap()
            .nullable
    );
}

#[test]
fn orders_migration_appends_parent_and_child_tables() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let current = SchemaSnapshot::from_registered(&[
        RegisteredModel {
            table: Category::descriptor().table,
            columns: Category::columns(),
            constraints: Category::constraints(),
        },
        RegisteredModel {
            table: LiveProduct::descriptor().table,
            columns: LiveProduct::columns(),
            constraints: LiveProduct::constraints(),
        },
        RegisteredModel {
            table: InternalNote::descriptor().table,
            columns: InternalNote::columns(),
            constraints: InternalNote::constraints(),
        },
        RegisteredModel {
            table: Event::descriptor().table,
            columns: Event::columns(),
            constraints: Event::constraints(),
        },
        RegisteredModel {
            table: Order::descriptor().table,
            columns: Order::columns(),
            constraints: Order::constraints(),
        },
        RegisteredModel {
            table: OrderLine::descriptor().table,
            columns: OrderLine::columns(),
            constraints: OrderLine::constraints(),
        },
    ])
    .unwrap();
    let path = directory.join("0005_orders.json");
    if !path.exists() {
        make_migration(&directory, "orders", &current).unwrap();
    }
    let migration = load_migration(&path).unwrap();
    assert_eq!(migration.id, "0005_orders");
    assert!(migration
        .to_snapshot
        .tables
        .iter()
        .any(|table| table.name == "orders"));
    assert!(migration
        .to_snapshot
        .tables
        .iter()
        .any(|table| table.name == "order_lines"));
    assert!(
        plan_change(&migration.from_snapshot, &migration.to_snapshot)
            .unwrap()
            .refusal
            .is_none()
    );
}

#[test]
fn prices_and_status_are_added_without_rewriting_applied_history() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let current = SchemaSnapshot::from_registered(&[
        RegisteredModel {
            table: Category::descriptor().table,
            columns: Category::columns(),
            constraints: Category::constraints(),
        },
        RegisteredModel {
            table: LiveProduct::descriptor().table,
            columns: LiveProduct::columns(),
            constraints: LiveProduct::constraints(),
        },
        RegisteredModel {
            table: InternalNote::descriptor().table,
            columns: InternalNote::columns(),
            constraints: InternalNote::constraints(),
        },
        RegisteredModel {
            table: Event::descriptor().table,
            columns: Event::columns(),
            constraints: Event::constraints(),
        },
        RegisteredModel {
            table: Order::descriptor().table,
            columns: Order::columns(),
            constraints: Order::constraints(),
        },
        RegisteredModel {
            table: OrderLine::descriptor().table,
            columns: OrderLine::columns(),
            constraints: OrderLine::constraints(),
        },
    ])
    .unwrap();
    let path = directory.join("0006_prices_and_status.json");
    if !path.exists() {
        make_migration_with_backfills(
            &directory,
            "prices_and_status",
            &current,
            &[
                ColumnBackfill {
                    table: "products".to_owned(),
                    column: "price".to_owned(),
                    value: BackfillLiteral::Text("0.00".to_owned()),
                },
                ColumnBackfill {
                    table: "products".to_owned(),
                    column: "status".to_owned(),
                    value: BackfillLiteral::Text("draft".to_owned()),
                },
            ],
        )
        .unwrap();
    }
    let migration = load_migration(&path).unwrap();
    assert_eq!(migration.id, "0006_prices_and_status");
    let plan = plan_change_with_backfills(
        &migration.from_snapshot,
        &migration.to_snapshot,
        &[
            ColumnBackfill {
                table: "products".to_owned(),
                column: "price".to_owned(),
                value: BackfillLiteral::Text("0.00".to_owned()),
            },
            ColumnBackfill {
                table: "products".to_owned(),
                column: "status".to_owned(),
                value: BackfillLiteral::Text("draft".to_owned()),
            },
        ],
    )
    .unwrap();
    assert!(plan.refusal.is_none(), "{:?}", plan.refusal);
    let products = migration
        .to_snapshot
        .tables
        .iter()
        .find(|table| table.name == "products")
        .unwrap();
    let price = products
        .columns
        .iter()
        .find(|column| column.name == "price")
        .unwrap();
    use ferrioxid::migrations::LogicalType;
    assert_eq!(price.logical_type, LogicalType::Decimal);
    assert_eq!(price.precision, Some(12));
    assert_eq!(price.scale, Some(2));
    let status = products
        .columns
        .iter()
        .find(|column| column.name == "status")
        .unwrap();
    assert_eq!(status.logical_type, LogicalType::Choice);
    assert_eq!(
        status
            .choices
            .iter()
            .map(|choice| choice.value.as_str())
            .collect::<Vec<_>>(),
        vec!["draft", "active", "archived"]
    );
}
