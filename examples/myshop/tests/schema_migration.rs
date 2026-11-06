//! Die eingecheckte myshop-Migration muss zum registrierten Product-Modell passen.

use ferrum::core::FerrumModel;
use ferrum::migrations::{load_migration, RegisteredModel, SchemaSnapshot};
use myshop_schema::Product;

mod myshop_schema {
    use ferrum::prelude::*;

    #[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
    #[ferrum(table = "products", api = "/api/v1/products", allow_anonymous = true)]
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
    }])
    .unwrap();
    assert!(migration.from_snapshot.tables.is_empty());
    assert!(migration.to_snapshot.semantically_eq(&schema));
    assert_eq!(migration.operations.len(), 1);
}
