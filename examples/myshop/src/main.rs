use ferrum::core::FerrumModel;
use ferrum::migrations::{history_exists, table_exists, RegisteredModel, SchemaSnapshot};
use ferrum::App;
use myshop::models::category::Category;
use myshop::models::internal_note::InternalNote;
use myshop::models::product::Product;
use std::net::SocketAddr;

fn registered_schema() -> Result<SchemaSnapshot, Box<dyn std::error::Error>> {
    Ok(SchemaSnapshot::from_registered(&[
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
            table: InternalNote::descriptor().table,
            columns: InternalNote::columns(),
            constraints: InternalNote::constraints(),
        },
    ])?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--print-schema") {
        println!("{}", registered_schema()?.to_canonical_json()?);
        return Ok(());
    }
    tracing_subscriber::fmt().with_env_filter("info").init();
    let url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| ferrum::db::DEFAULT_SQLITE_URL.to_owned());
    let db = ferrum::db::connect(&url).await?;
    // register::<Product>() exposes GET, GET /{id}, POST, PATCH /{id} and DELETE /{id}.
    let auth = ferrum::auth::AuthService::new(db.clone(), ferrum::auth::AuthConfig::from_env());
    let admin = ferrum::admin::AdminRegistry::new()
        .add::<Category>()
        .add::<Product>()
        .add::<InternalNote>();
    let app = App::new(db.clone())
        .with_auth(auth)
        .with_admin(admin)
        .register::<Category>()
        .register::<Product>()
        .register::<InternalNote>();

    if std::env::args().any(|arg| arg == "--bootstrap") {
        eprintln!(
            "--bootstrap ist Legacy und legt weder Tabellen noch eine Migrationshistorie an."
        );
        eprintln!("Verwende im Projektroot: ferrum migrate make initial");
        eprintln!("Danach: ferrum migrate up");
        if history_exists(&db).await? {
            return Err(
                "Die Migrationshistorie existiert bereits. ferrum migrate status zeigt den Stand."
                    .into(),
            );
        }
        if table_exists(&db, Product::descriptor().table).await? {
            return Err(
                "Die Tabelle products existiert ohne Migrationshistorie. Es wird nichts gelöscht und nichts als migriert markiert. Lege für Migrationen eine neue SQLite-Datei an.".into(),
            );
        }
        return Err(
            "Die Datenbank enthält das Modell noch nicht. ferrum migrate make und ferrum migrate up erzeugen das Schema.".into(),
        );
    }

    let address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Ferrum myshop at http://{address}");
    axum::serve(
        listener,
        app.into_router()
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
