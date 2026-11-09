mod admin;
mod contact;

use ferrioxid::core::FerrioxidModel;
use ferrioxid::middleware::{apply, Standard};
use ferrioxid::migrations::{history_exists, table_exists, RegisteredModel, SchemaSnapshot};
use ferrioxid::static_files::{mount, StaticOptions};
use ferrioxid::storage::LocalStorage;
use ferrioxid::App;
use myshop::models::category::Category;
use myshop::models::event::Event;
use myshop::models::internal_note::InternalNote;
use myshop::models::order::Order;
use myshop::models::order_line::OrderLine;
use myshop::models::product::Product;
use std::net::SocketAddr;
use std::sync::Arc;

async fn home() -> axum::response::Html<&'static str> {
    axum::response::Html("<!DOCTYPE html><html lang=\"de\"><head><meta charset=\"utf-8\"><title>myshop</title><link rel=\"stylesheet\" href=\"/static/css/app.css\"></head><body><main><h1>myshop</h1><p><a href=\"/contact\">Kontakt</a></p></main></body></html>")
}

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
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| ferrioxid::db::DEFAULT_SQLITE_URL.to_owned());
    let db = ferrioxid::db::connect(&url).await?;
    // register::<Product>() exposes GET, GET /{id}, POST, PATCH /{id} and DELETE /{id}.
    let address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let mut auth_config = ferrioxid::auth::AuthConfig::from_env();
    auth_config.apply_bound_address(&address);
    tracing::info!(
        bound = %address,
        origin_count = auth_config.allowed_origins.len(),
        "Login-Origins aus der Bind-Adresse vorbereitet."
    );
    let auth = ferrioxid::auth::AuthService::new(db.clone(), auth_config);
    let project = ferrioxid::migrations::discover_project(std::path::Path::new(".")).ok();
    let branding = project
        .as_ref()
        .map(|project| ferrioxid::admin::AdminBranding {
            site_title: project.site_title.clone(),
            site_header: project.site_header.clone(),
            app_name: project.app_name.clone(),
            primary_color: project.primary_color.clone(),
            default_locale: project.default_locale.clone(),
            fallback_locale: project.fallback_locale.clone(),
            currency: project.currency.clone(),
        })
        .unwrap_or_default();
    let mail = project
        .as_ref()
        .map(ferrioxid::mail_for)
        .unwrap_or_else(ferrioxid::mail::MailService::disabled);
    let admin = admin::extend(
        ferrioxid::admin::AdminRegistry::new()
            .with_branding(branding)
            .add::<Category>()
            .add::<Event>()
            .add::<Order>()
            .add::<OrderLine>()
            .add::<Product>()
            .add::<InternalNote>(),
    )?;
    let media = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("media");
    let storage = LocalStorage::open(
        &media,
        1024 * 1024,
        &["text/plain", "image/png", "image/jpeg", "application/pdf"],
    )?;
    let app = App::new(db.clone())
        .with_auth(auth)
        .with_mail(mail)
        .with_admin(admin)
        .urls(
            ferrioxid::urls::UrlSet::new().route(
                "shop.thanks",
                "/contact/thanks",
                axum::routing::get(contact::thanks),
            )?,
        )?
        .merge(
            axum::Router::new()
                .route("/", axum::routing::get(home))
                .route(
                    "/contact",
                    axum::routing::get(contact::show).post(contact::submit),
                ),
        )
        .merge(ferrioxid::storage::routes(Arc::new(storage)))
        .register::<Category>()
        .register::<Event>()
        .register::<Order>()
        .register::<OrderLine>()
        .register::<Product>()
        .register::<InternalNote>();

    if std::env::args().any(|arg| arg == "--bootstrap") {
        eprintln!(
            "--bootstrap ist Legacy und legt weder Tabellen noch eine Migrationshistorie an."
        );
        eprintln!("Verwende im Projektroot: ferrioxid migrate make initial");
        eprintln!("Danach: ferrioxid migrate up");
        if history_exists(&db).await? {
            return Err(
                "Die Migrationshistorie existiert bereits. ferrioxid migrate status zeigt den Stand."
                    .into(),
            );
        }
        if table_exists(&db, Product::descriptor().table).await? {
            return Err(
                "Die Tabelle products existiert ohne Migrationshistorie. Es wird nichts gelöscht und nichts als migriert markiert. Lege für Migrationen eine neue SQLite-Datei an.".into(),
            );
        }
        return Err(
            "Die Datenbank enthält das Modell noch nicht. ferrioxid migrate make und ferrioxid migrate up erzeugen das Schema.".into(),
        );
    }

    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Ferrioxid myshop at http://{address}");
    let static_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");
    let router = mount(app.into_router(), StaticOptions::new(static_root));
    let router = apply(
        router,
        Standard {
            body_limit: 2 * 1024 * 1024,
        },
    );
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
