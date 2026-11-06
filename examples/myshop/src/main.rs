mod models;

use ferrum::{db::ModelRepository, App};
use models::product::Product;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum".to_owned());
    let db = ferrum::db::connect(&url).await?;
    // register::<Product>() exposes GET, GET /{id}, POST, PATCH /{id} and DELETE /{id}.
    let app = App::new(db.clone()).register::<Product>();

    if std::env::args().any(|arg| arg == "--bootstrap") {
        app.bootstrap_schema().await?;
        // Exercise the generated typed repository against the demo model.
        let products = <Product as ModelRepository>::list(&db).await?;
        for product in products.data {
            tracing::debug!(
                id = %product.id, name = %product.name,
                price_cents = product.price_cents, active = product.active,
                "existing demo product"
            );
        }
        println!("Development schema bootstrap finished (CREATE TABLE IF NOT EXISTS).");
        return Ok(());
    }

    let address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Ferrum myshop at http://{address}");
    axum::serve(listener, app.into_router()).await?;
    Ok(())
}
