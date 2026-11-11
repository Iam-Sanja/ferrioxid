use std::path::PathBuf;

use desk::models::{Address, Cart, Category, Line, Owner, Product};
use desk::shop::{self, ADDRESS_LIMIT};
use desk::{routes, stock_markup};
use ferrioxid::core::{DatabaseKind, FerrioxidError, FerrioxidModel};
use ferrioxid::db::{any_of, col, connect, kind_of, Direction, ModelQuery};
use ferrioxid::migrations::{apply_directory, make_migration, RegisteredModel, SchemaSnapshot};
use ferrioxid::sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use uuid::Uuid;

fn fixed(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn migrations() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

fn registered() -> SchemaSnapshot {
    SchemaSnapshot::from_registered(&[
        RegisteredModel {
            table: Owner::descriptor().table,
            columns: Owner::columns(),
            constraints: Owner::constraints(),
        },
        RegisteredModel {
            table: Category::descriptor().table,
            columns: Category::columns(),
            constraints: Category::constraints(),
        },
        RegisteredModel {
            table: Product::descriptor().table,
            columns: Product::columns(),
            constraints: Product::constraints(),
        },
        RegisteredModel {
            table: Cart::descriptor().table,
            columns: Cart::columns(),
            constraints: Cart::constraints(),
        },
        RegisteredModel {
            table: Line::descriptor().table,
            columns: Line::columns(),
            constraints: Line::constraints(),
        },
        RegisteredModel {
            table: Address::descriptor().table,
            columns: Address::columns(),
            constraints: Address::constraints(),
        },
    ])
    .expect("desk schema")
}

async fn install(db: &DatabaseConnection) {
    let directory = migrations();
    std::fs::create_dir_all(&directory).unwrap();
    make_migration(&directory, "initial", &registered()).expect("desk migration");
    apply_directory(db, &directory, false)
        .await
        .expect("apply desk migration");
}

fn assert_sources_have_no_handwritten_sql() {
    for relative in ["src/lib.rs", "src/models.rs", "src/shop.rs"] {
        let text =
            std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative))
                .unwrap()
                .to_ascii_uppercase();
        for word in ["SELECT ", "INSERT INTO", "UPDATE ", "DELETE FROM"] {
            assert!(!text.contains(word), "{relative} contains {word}");
        }
    }
}

async fn insert_owner(db: &DatabaseConnection, id: Uuid, name: &str) {
    Owner::insert()
        .value("id", id)
        .value("name", name)
        .execute(db)
        .await
        .unwrap();
}

async fn insert_category(db: &DatabaseConnection, id: Uuid, name: &str) {
    Category::insert()
        .value("id", id)
        .value("name", name)
        .execute(db)
        .await
        .unwrap();
}

struct ProductSeed<'a> {
    id: Uuid,
    name: &'a str,
    stock: i64,
    active: bool,
    price: i64,
    owner: Uuid,
    category: Uuid,
}

async fn insert_product(db: &DatabaseConnection, seed: ProductSeed<'_>) {
    Product::insert()
        .value("id", seed.id)
        .value("name", seed.name)
        .value("stock", seed.stock)
        .value("active", seed.active)
        .value("price_cents", seed.price)
        .value("owner_id", seed.owner)
        .value("category_id", seed.category)
        .execute(db)
        .await
        .unwrap();
}

async fn second_connection(url: &str, first: &DatabaseConnection) -> DatabaseConnection {
    if url.starts_with("sqlite:") {
        connect(url).await.expect("second sqlite connection")
    } else {
        first.clone()
    }
}

async fn cases(url: &str) {
    assert_sources_have_no_handwritten_sql();
    let routes = routes().expect("routes");
    assert_eq!(routes.names().path("desk.stock", &[]).unwrap(), "/stock");

    let db = connect(url).await.expect("connect");
    install(&db).await;
    let other = second_connection(url, &db).await;
    let kind = kind_of(db.get_database_backend()).unwrap();

    let owner_a = fixed(1);
    let owner_b = fixed(2);
    let owner_c = fixed(3);
    let tools = fixed(10);
    let other_category = fixed(11);
    let hemd = fixed(21);
    let muenchen = fixed(22);
    let hose = fixed(23);
    let spelled = fixed(24);
    let cart_a = fixed(31);
    let cart_b = fixed(32);

    let created = shop::create_owner(&db, "Ada").await.expect("create owner");
    assert_eq!(created.name, "Ada");
    let rejected = shop::create_owner(&db, "").await.expect_err("empty name");
    assert!(matches!(rejected, FerrioxidError::Validation(_)));

    insert_owner(&db, owner_a, "A").await;
    insert_owner(&db, owner_b, "B").await;
    insert_owner(&db, owner_c, "C").await;
    insert_category(&db, tools, "Tools").await;
    insert_category(&db, other_category, "Other").await;
    insert_product(
        &db,
        ProductSeed {
            id: hemd,
            name: "Hemd",
            stock: 1,
            active: true,
            price: 500,
            owner: owner_a,
            category: tools,
        },
    )
    .await;
    insert_product(
        &db,
        ProductSeed {
            id: muenchen,
            name: "München",
            stock: 5,
            active: true,
            price: 100,
            owner: owner_a,
            category: other_category,
        },
    )
    .await;
    insert_product(
        &db,
        ProductSeed {
            id: hose,
            name: "Hose",
            stock: 4,
            active: false,
            price: 500,
            owner: owner_a,
            category: tools,
        },
    )
    .await;
    insert_product(
        &db,
        ProductSeed {
            id: spelled,
            name: "Muenchen",
            stock: 2,
            active: true,
            price: 100,
            owner: owner_b,
            category: other_category,
        },
    )
    .await;

    let found = shop::search_products(&db, "hemd").await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, hemd);
    let unicode = shop::search_products(&db, "MÜNCHEN").await.unwrap();
    match kind {
        DatabaseKind::Sqlite => assert!(unicode.is_empty()),
        DatabaseKind::Postgres => assert_eq!(unicode.len(), 1),
    }
    let ordered = Product::query()
        .filter(any_of([
            col("name").eq("Muenchen"),
            col("name").eq("München"),
        ]))
        .order_by("name", Direction::Asc)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(ordered.len(), 2);
    if kind == DatabaseKind::Sqlite {
        assert_eq!(ordered[0].name, "Muenchen");
        assert_eq!(ordered[1].name, "München");
    }

    let tools_only = shop::products_in_category(&db, "Tools").await.unwrap();
    assert_eq!(tools_only.len(), 2);
    let page = shop::product_page(&db, 2).await.unwrap();
    assert_eq!(page.data.len(), 2);
    assert!(page.has_more);
    assert!(page.total >= 4);

    Cart::insert()
        .value("id", cart_a)
        .value("owner_id", owner_a)
        .execute(&db)
        .await
        .unwrap();
    Cart::insert()
        .value("id", cart_b)
        .value("owner_id", owner_b)
        .execute(&db)
        .await
        .unwrap();
    for number in 0..101_u128 {
        Line::insert()
            .value("id", fixed(1_000 + number))
            .value("cart_id", cart_a)
            .value("owner_id", owner_a)
            .value("sku", format!("sku-{number}"))
            .value("qty", 1_i64)
            .value("moved", false)
            .execute(&db)
            .await
            .unwrap();
    }
    Line::insert()
        .value("id", fixed(2_000))
        .value("cart_id", cart_b)
        .value("owner_id", owner_b)
        .value("sku", "other")
        .value("qty", 7_i64)
        .value("moved", false)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(shop::owner_quantity(&db, owner_a).await.unwrap(), 101);
    assert_eq!(shop::owner_quantity(&db, owner_b).await.unwrap(), 7);

    let inactive = shop::take_stock(&db, hose, 500, 1).await.unwrap();
    assert_eq!(inactive.rows_affected, 0);
    let missing = shop::take_stock(&db, fixed(99), 500, 1).await.unwrap();
    assert_eq!(missing.rows_affected, 0);

    let (left, right) = tokio::join!(
        shop::take_stock(&db, hemd, 500, 1),
        shop::take_stock(&other, hemd, 500, 1),
    );
    let taken = left.unwrap().rows_affected + right.unwrap().rows_affected;
    assert_eq!(taken, 1);
    let hemd_row = shop::load_product(&db, hemd).await.unwrap().unwrap();
    assert_eq!(hemd_row.stock, 0);
    let markup = stock_markup(&hemd_row.name, hemd_row.stock).unwrap();
    assert!(markup.contains("Hemd"));
    assert!(markup.contains("Bestand: 0"));

    let stale = shop::take_stock(&db, muenchen, 999, 1).await.unwrap();
    assert_eq!(stale.rows_affected, 0);
    let fresh = shop::take_stock(&db, muenchen, 100, 1).await.unwrap();
    assert_eq!(fresh.rows_affected, 1);
    assert_eq!(
        shop::load_product(&db, muenchen)
            .await
            .unwrap()
            .unwrap()
            .stock,
        4
    );

    let tx = db.begin().await.unwrap();
    let moved = shop::mark_lines_moved(&tx, owner_a, cart_a).await.unwrap();
    assert_eq!(moved.rows_affected, 101);
    let conflict = Cart::insert()
        .value("id", fixed(40))
        .value("owner_id", owner_a)
        .execute(&tx)
        .await
        .expect_err("duplicate cart");
    assert!(matches!(conflict, FerrioxidError::Validation(_)));
    tx.rollback().await.unwrap();
    let still = Line::query()
        .filter(col("id").eq(fixed(1_000)))
        .all(&db)
        .await
        .unwrap();
    assert!(!still[0].moved);

    let removed = shop::delete_lines(&db, owner_a).await.unwrap();
    assert_eq!(removed.rows_affected, 101);
    assert_eq!(
        Line::query()
            .filter(col("owner_id").eq(owner_a))
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        Line::query()
            .filter(col("owner_id").eq(owner_b))
            .count(&db)
            .await
            .unwrap(),
        1
    );

    let (first_cart, second_cart) = tokio::join!(
        shop::open_cart(&db, fixed(41), owner_c),
        shop::open_cart(&other, fixed(42), owner_c),
    );
    let opened = first_cart.unwrap().rows_affected + second_cart.unwrap().rows_affected;
    assert_eq!(opened, 1);
    assert_eq!(
        Cart::query()
            .filter(col("owner_id").eq(owner_c))
            .count(&db)
            .await
            .unwrap(),
        1
    );

    for number in 0..19_u128 {
        Address::insert()
            .value("id", fixed(3_000 + number))
            .value("owner_id", owner_a)
            .value("label", format!("Adresse {number}"))
            .execute(&db)
            .await
            .unwrap();
    }
    let (one, two) = tokio::join!(
        shop::add_address(&db, owner_a, fixed(4_001), "neu-1"),
        shop::add_address(&other, owner_a, fixed(4_002), "neu-2"),
    );
    let accepted = [&one, &two].iter().filter(|result| result.is_ok()).count();
    assert_eq!(accepted, 1);
    assert_eq!(
        Address::query()
            .filter(col("owner_id").eq(owner_a))
            .count(&db)
            .await
            .unwrap(),
        ADDRESS_LIMIT
    );

    let blocked = Owner::query()
        .filter(col("id").eq(owner_a))
        .delete_matching()
        .execute(&db)
        .await
        .expect_err("addresses restrict the owner");
    assert!(matches!(blocked, FerrioxidError::Validation(text) if text.contains("foreign key")));

    let injected = Product::query()
        .filter(col("id").eq(hemd))
        .update_matching()
        .set("stock = 1", 1_i64)
        .execute(&db)
        .await
        .expect_err("column name");
    assert!(matches!(injected, FerrioxidError::Validation(_)));
    let unfiltered = Product::query()
        .update_matching()
        .set("active", false)
        .execute(&db)
        .await
        .expect_err("unfiltered");
    assert!(matches!(unfiltered, FerrioxidError::Validation(_)));
    let text_math = Product::query()
        .filter(col("id").eq(hemd))
        .update_matching()
        .increment("name", 1)
        .execute(&db)
        .await
        .expect_err("text arithmetic");
    assert!(matches!(text_math, FerrioxidError::Validation(_)));
}

#[tokio::test]
async fn sqlite_same_operations() {
    let path =
        std::env::temp_dir().join(format!("ferrioxid-desk-{}.sqlite", Uuid::new_v4().simple()));
    let url = format!(
        "sqlite://{}?mode=rwc",
        path.display().to_string().replace('\\', "/")
    );
    cases(&url).await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL, CREATEDB, and an approved PostgreSQL test database"]
async fn postgres_same_operations() {
    let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let disposable = ferrioxid::db::DisposablePostgres::create(&admin)
        .await
        .expect("isolated postgres");
    cases(disposable.url()).await;
    disposable
        .drop_database()
        .await
        .expect("drop isolated database");
}
