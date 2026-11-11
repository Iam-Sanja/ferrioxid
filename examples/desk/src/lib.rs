//! Kleine Desk-Anwendung. Geschäftslogik steht in [`shop`] und verwendet nur
//! die öffentliche Ferrioxid-API.

pub mod models;
pub mod shop;

use std::path::Path;

use axum::response::Html;
use axum::routing::get;
use ferrioxid::apps::{AppConfig, AppError, FerrioxidApplication, Registration, TemplateMode};
use ferrioxid::urls::{UrlError, UrlSet};

pub struct DeskApp;

impl FerrioxidApplication for DeskApp {
    fn config(&self) -> AppConfig {
        AppConfig::new("desk").label("Desk").prefix("/desk")
    }

    fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), AppError> {
        registry.routes(routes()?)?;
        registry.templates(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("templates"),
            TemplateMode::Isolated,
        )?;
        Ok(())
    }
}

pub fn routes() -> Result<UrlSet, UrlError> {
    UrlSet::new().route("desk.stock", "/stock", get(stock_view))
}

pub fn stock_markup(name: &str, stock: i64) -> Result<String, tera::Error> {
    let mut templates = tera::Tera::default();
    templates.add_raw_template("stock.html", include_str!("../templates/stock.html"))?;
    let mut context = tera::Context::new();
    context.insert("name", name);
    context.insert("stock", &stock);
    templates.render("stock.html", &context)
}

async fn stock_view() -> Html<String> {
    Html(stock_markup("Beispiel", 0).expect("stock template"))
}
