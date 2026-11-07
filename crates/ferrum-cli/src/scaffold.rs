//! Erzeugt ein eigenständiges Ferrum-Projekt außerhalb des Framework-Workspaces.

use ferrum_migrations::{
    plan_change, save_migration, seal, ColumnSnapshot, LogicalType, SchemaSnapshot, TableSnapshot,
};
use std::path::{Path, PathBuf};

pub fn new_project(
    name: &str,
    template: &str,
    ferrum_path: Option<&Path>,
) -> Result<PathBuf, String> {
    validate_project_name(name)?;
    let template = parse_template(template)?;
    let ferrum = resolve_ferrum_path(ferrum_path)?;
    let destination = std::env::current_dir()
        .map_err(|error| error.to_string())?
        .join(name);
    if destination.exists() {
        return Err(format!(
            "{} existiert bereits. Ferrum überschreibt kein vorhandenes Verzeichnis.",
            destination.display()
        ));
    }
    std::fs::create_dir_all(destination.join("src/models")).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(destination.join("migrations")).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(destination.join("tests")).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(destination.join("templates/errors"))
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(destination.join("static/css")).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(destination.join("static/js")).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(destination.join("media")).map_err(|error| error.to_string())?;
    let ferrum_display = cargo_path(&ferrum);
    let features = match template {
        Template::Api => "",
        Template::Fullstack => ", features = [\"admin-ui\"]",
    };
    write(
        &destination.join("Cargo.toml"),
        &cargo_toml(name, &ferrum_display, features),
    )?;
    write(&destination.join("ferrum.toml"), &manifest(name, template))?;
    write(
        &destination.join(".gitignore"),
        "target/\n.env\n*.sqlite3\n*.sqlite3-journal\nmedia/*\n!media/.gitkeep\n",
    )?;
    write(
        &destination.join("static/css/app.css"),
        "body { font-family: sans-serif; }\n",
    )?;
    write(
        &destination.join("static/js/app.js"),
        "document.documentElement.dataset.ferrumStatic = \"ready\";\n",
    )?;
    write(&destination.join("media/.gitkeep"), "")?;
    write(&destination.join(".env.example"), env_example())?;
    write(&destination.join("README.md"), &readme(name, template))?;
    write(&destination.join("src/lib.rs"), "pub mod models;\n")?;
    write(
        &destination.join("src/models/mod.rs"),
        "pub mod product;\n// ferrum:models\n// ferrum:models:end\n",
    )?;
    write(
        &destination.join("src/models/product.rs"),
        &product_model(template),
    )?;
    write(&destination.join("src/main.rs"), &main_rs(name, template))?;
    write(&destination.join("tests/api.rs"), &api_test(name))?;
    write_templates(&destination, name, template)?;
    write_initial_migration(&destination.join("migrations"))?;
    Ok(destination)
}

#[derive(Clone, Copy)]
enum Template {
    Api,
    Fullstack,
}

fn parse_template(value: &str) -> Result<Template, String> {
    match value {
        "api" => Ok(Template::Api),
        "fullstack" => Ok(Template::Fullstack),
        _ => Err("Unbekanntes Template. Erlaubt sind api und fullstack.".to_owned()),
    }
}

pub fn validate_project_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("Der Projektname darf nicht leer sein.".to_owned());
    };
    if !first.is_ascii_alphabetic()
        || !chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
        || name.len() > 48
    {
        return Err(
            "Der Projektname muss mit einem Buchstaben beginnen und darf nur Buchstaben, Ziffern und Unterstriche enthalten."
                .to_owned(),
        );
    }
    let reserved = matches!(
        name.to_ascii_lowercase().as_str(),
        "ferrum"
            | "test"
            | "main"
            | "src"
            | "migrations"
            | "cargo"
            | "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "lpt1"
    );
    if reserved {
        return Err(format!("`{name}` ist als Projektname reserviert."));
    }
    Ok(())
}

fn resolve_ferrum_path(explicit: Option<&Path>) -> Result<PathBuf, String> {
    let raw = if let Some(path) = explicit {
        path.to_path_buf()
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    };
    let path = raw.canonicalize().map_err(|_| {
        "Der Ferrum-Pfad existiert nicht. Übergib --ferrum-path mit dem Repository.".to_owned()
    })?;
    let crate_manifest = path.join("crates/ferrum/Cargo.toml");
    if !crate_manifest.is_file() {
        return Err(
            "Der Ferrum-Pfad enthält keine crates/ferrum/Cargo.toml. Eine veröffentlichte Crate ist noch nicht vorgesehen."
                .to_owned(),
        );
    }
    Ok(path.join("crates/ferrum"))
}

fn cargo_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    text.replace('\\', "/")
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "{} existiert bereits und wird nicht überschrieben.",
            path.display()
        ));
    }
    std::fs::write(path, contents).map_err(|error| error.to_string())
}

fn cargo_toml(name: &str, ferrum: &str, features: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
ferrum = {{ path = "{ferrum}"{features} }}
axum = "0.8"
serde = {{ version = "1", features = ["derive"] }}
tokio = {{ version = "1", features = ["macros", "rt-multi-thread"] }}
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}
"#
    )
}

fn manifest(name: &str, template: Template) -> String {
    let (template_name, admin, embedded, auth) = match template {
        Template::Api => ("api", "false", "false", "false"),
        Template::Fullstack => ("fullstack", "true", "true", "true"),
    };
    format!(
        r##"# Ferrum-Projekt. Diese Datei enthält keine Secrets.
migrations_dir = "migrations"
schema_command = "cargo run --quiet -- --print-schema"

[project]
name = "{name}"
template = "{template_name}"
default_locale = "de"
fallback_locale = "en"

[database]
url = "sqlite://db.sqlite3?mode=rwc"

[server]
host = "127.0.0.1"
port = 3000
debug = true

[admin]
enabled = {admin}
embedded_assets = {embedded}
site_title = "Ferrum Admin"
site_header = "Ferrum Verwaltung"

[auth]
enabled = {auth}
session_hours = 12
idle_minutes = 30

[templates]
enabled = true
engine = "tera"
templates_dir = "templates"

[branding]
app_name = "{name}"
primary_color = "#0f172a"
currency = "EUR"

[static]
enabled = true
url_prefix = "/static"
directory = "static"
cache_seconds = 3600

[storage]
backend = "local"
directory = "media"
max_upload_size_mb = 10
"##
    )
}

fn env_example() -> &'static str {
    "DATABASE_URL=sqlite://db.sqlite3?mode=rwc\nBIND_ADDRESS=127.0.0.1:3000\n# FERRUM_CSRF_SECRET muss in Produktion 64 Hex-Zeichen sein.\n# FERRUM_PASSWORD wird nur für die nicht interaktive Benutzeranlage gelesen.\n"
}

fn readme(name: &str, template: Template) -> String {
    let frontend = match template {
        Template::Api => "Dieses API-Projekt braucht kein Node.js.\n",
        Template::Fullstack => "Vor dem ersten Release-Build muss im Ferrum-Repository `apps/admin-ui` mit `npm run build` gebaut sein. Der laufende Server braucht kein Node.js.\n",
    };
    format!(
        r#"# {name}

SQLite ist die lokale Datenbank. Docker und PostgreSQL sind nicht erforderlich.

```powershell
ferrum migrate up
ferrum doctor
ferrum dev
```

{frontend}
Der Server bindet standardmäßig nur `127.0.0.1`. Die Startseite liegt unter `/`. Statische Dateien liegen in `static/` und werden unter `/static/` ausgeliefert. `media/` ist für Uploads vorgesehen und wird nicht versioniert. Ein Benutzer wird mit `ferrum admin create-user --email name@example.com --staff --superuser` angelegt. Das Passwort wird abgefragt. Vorlagen stehen in `templates/` und nutzen Tera. Das Kontaktformular eines Fullstack-Projekts liegt unter `/contact` und versendet keine E-Mail.
"#
    )
}

fn product_model(template: Template) -> String {
    let attributes = match template {
        Template::Api => "    table = \"products\",\n    api = \"/api/v1/products\",\n    allow_anonymous = true",
        Template::Fullstack => {
            "    table = \"products\",\n    api = \"/api/v1/products\",\n    allow_anonymous = true,\n    admin = true,\n    admin_label = \"Produkte\",\n    admin_permission = \"products\""
        }
    };
    format!(
        r#"use ferrum::prelude::*;
use serde::{{Deserialize, Serialize}};

#[derive(FerrumModel, Serialize, Deserialize)]
#[ferrum(
{attributes}
)]
pub struct Product {{
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, min_length = 1, max_length = 150)]
    pub name: String,
}}
"#
    )
}

fn main_rs(name: &str, template: Template) -> String {
    let auth = match template {
        Template::Api => String::new(),
        Template::Fullstack => r#"
    let mut auth_config = ferrum::auth::AuthConfig::from_env();
    auth_config.apply_project_session(settings.session_hours, settings.idle_minutes);
    let auth = ferrum::auth::AuthService::new(db.clone(), auth_config);
    let admin = ferrum::admin::AdminRegistry::new()
        .with_branding(ferrum::admin::AdminBranding {
            site_title: settings.site_title.clone(),
            site_header: settings.site_header.clone(),
            app_name: settings.app_name.clone(),
            primary_color: settings.primary_color.clone(),
            default_locale: settings.default_locale.clone(),
            fallback_locale: settings.fallback_locale.clone(),
            currency: settings.currency.clone(),
        })
        .add::<Product>()
        // ferrum:admin
        // ferrum:admin:end
        ;
"#
        .to_owned(),
    };
    let chain = match template {
        Template::Api => ".register::<Product>()\n        // ferrum:registry\n        // ferrum:registry:end\n",
        Template::Fullstack => ".with_auth(auth)\n        .with_admin(admin)\n        .merge(axum::Router::new().route(\"/contact\", axum::routing::get(show_contact).post(submit_contact)))\n        .register::<Product>()\n        // ferrum:registry\n        // ferrum:registry:end\n",
    };
    format!(
        r#"use {name}::models::product::Product;
use ferrum::core::FerrumModel;
use ferrum::migrations::{{RegisteredModel, SchemaSnapshot}};
// ferrum:uses
// ferrum:uses:end

fn registered_schema() -> Result<SchemaSnapshot, Box<dyn std::error::Error>> {{
    Ok(SchemaSnapshot::from_registered(&[
        RegisteredModel {{
            table: Product::descriptor().table,
            columns: Product::columns(),
            constraints: Product::constraints(),
        }},
        // ferrum:schema
        // ferrum:schema:end
    ])?)
}}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {{
    if std::env::args().any(|argument| argument == "--print-schema") {{
        println!("{{}}", registered_schema()?.to_canonical_json()?);
        return Ok(());
    }}
    let settings = ferrum::migrations::discover_project(std::path::Path::new("."))?;
    tracing_subscriber::fmt().with_env_filter("info").init();
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| settings.database_url.clone());
    let db = ferrum::db::connect(&url).await?;
    {auth}
    let app = ferrum::App::new(db)
        {chain};
    let address = std::env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| format!("{{}}:{{}}", settings.host, settings.port));
    if address.starts_with("0.0.0.0") {{
        return Err("Ferrum bindet neue Projekte nicht an 0.0.0.0.".into());
    }}
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Ferrum lauscht auf http://{{address}}");
    let router = ferrum::pages::mount_pages(app.into_router(), &settings)?;
    let router = ferrum::web::prepare(router, &settings);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}}
"#
    ) + &match template {
        Template::Api => String::new(),
        Template::Fullstack => contact_handlers(),
    }
}

fn contact_handlers() -> String {
    r#"
use ferrum::forms::FerrumForm;

async fn show_contact(
    axum::extract::State(state): axum::extract::State<ferrum::auth::AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    let csrf = csrf_for(&state, &headers).await;
    let form = ferrum::forms::render_form(
        "/contact",
        csrf.as_deref(),
        &ferrum::forms::FormValues::default(),
        &ferrum::forms::FormErrors::default(),
        &ferrum::forms::contact_fields("de"),
        "de",
    );
    axum::response::Html(ferrum::forms::render_page("Kontakt", &form))
}

async fn submit_contact(
    axum::extract::State(state): axum::extract::State<ferrum::auth::AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let values = match ferrum::forms::FormValues::parse_urlencoded(&String::from_utf8_lossy(&body)) {
        Ok(values) => values,
        Err(errors) => return axum::response::Html(contact_html(&state, &headers, &ferrum::forms::FormValues::default(), &errors).await).into_response(),
    };
    if let Some(auth) = state.auth.as_ref() {
        if auth.csrf_from_request(&headers).await.is_ok()
            && auth.authenticate_csrf(&headers, &axum::http::Method::POST, values.get("csrf_token").unwrap_or("")).await.is_err()
        {
            let mut errors = ferrum::forms::FormErrors::default();
            errors.push("", "form.csrf");
            return (axum::http::StatusCode::FORBIDDEN, axum::response::Html(contact_html(&state, &headers, &values, &errors).await)).into_response();
        }
    }
    match ferrum::forms::ContactForm::from_form(&values) {
        Ok(_form) => axum::response::Html(ferrum::forms::render_page("Kontakt", &format!("<p role=\"status\">{}</p>", ferrum::forms::message("de", "form.thanks")))).into_response(),
        Err(errors) => (axum::http::StatusCode::UNPROCESSABLE_ENTITY, axum::response::Html(contact_html(&state, &headers, &values, &errors).await)).into_response(),
    }
}

async fn csrf_for(state: &ferrum::auth::AppState, headers: &axum::http::HeaderMap) -> Option<String> {
    match state.auth.as_ref() {
        Some(auth) => auth.csrf_from_request(headers).await.ok(),
        None => None,
    }
}

async fn contact_html(state: &ferrum::auth::AppState, headers: &axum::http::HeaderMap, values: &ferrum::forms::FormValues, errors: &ferrum::forms::FormErrors) -> String {
    let csrf = csrf_for(state, headers).await;
    let form = ferrum::forms::render_form("/contact", csrf.as_deref(), values, errors, &ferrum::forms::contact_fields("de"), "de");
    ferrum::forms::render_page("Kontakt", &form)
}
"#.to_owned()
}

fn write_templates(destination: &Path, name: &str, template: Template) -> Result<(), String> {
    let admin_block = match template {
        Template::Api => "",
        Template::Fullstack => "{% if admin %}<p><a href=\"/admin\">{{ admin_link }}</a></p><p>{{ auth_hint }}</p>{% endif %}\n<p><a href=\"/?lang=de\">Deutsch</a> · <a href=\"/?lang=en\">English</a></p>\n",
    };
    write(
        &destination.join("templates/base.html"),
        "<!DOCTYPE html>\n<html lang=\"{{ locale }}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{{ app_name }}</title>\n<link rel=\"stylesheet\" href=\"/static/css/app.css\">\n<style>\n:root { --ink: {{ primary_color }}; }\nbody { margin: 0; font-family: \"Segoe UI\", sans-serif; background: #f8fafc; color: #0f172a; }\nheader { background: var(--ink); color: white; padding: 1.25rem 1.5rem; }\nmain { max-width: 46rem; margin: 0 auto; padding: 1.5rem; }\n.panel { background: white; border-radius: 1rem; padding: 1.25rem; box-shadow: 0 1px 2px rgba(15,23,42,.08); }\na { color: #0f766e; }\n</style>\n</head>\n<body>\n<header><strong>{{ app_name }}</strong></header>\n<main>{% block content %}{% endblock %}</main>\n</body>\n</html>\n",
    )?;
    write(
        &destination.join("templates/index.html"),
        &format!(
            "{{% extends \"base.html\" %}}{{% block content %}}\n<h1>{name}</h1>\n<p>{{{{ running }}}}</p>\n<section class=\"panel\">\n<p>{{{{ api_hint }}}}</p>\n{admin_block}<p>{{{{ setup_hint }}}}</p>\n<p>{{{{ status_label }}}}: SQLite</p>\n</section>\n{{% endblock %}}\n"
        ),
    )?;
    write(
        &destination.join("templates/errors/404.html"),
        "{% extends \"base.html\" %}{% block content %}<h1>404</h1><p>{{ not_found }}</p>{% endblock %}\n",
    )?;
    write(
        &destination.join("templates/errors/500.html"),
        "{% extends \"base.html\" %}{% block content %}<h1>500</h1><p>{{ server_error }}</p>{% endblock %}\n",
    )?;
    Ok(())
}

fn api_test(name: &str) -> String {
    format!(
        r#"use {name}::models::product::Product;
use ferrum::core::FerrumModel;

#[test]
fn product_table_is_explicit() {{
    assert_eq!(Product::descriptor().table, "products");
    assert_eq!(Product::descriptor().api_path, "/api/v1/products");
}}
"#
    )
}

fn write_initial_migration(directory: &Path) -> Result<(), String> {
    let from = SchemaSnapshot {
        format_version: 1,
        tables: Vec::new(),
    };
    let to = SchemaSnapshot {
        format_version: 1,
        tables: vec![TableSnapshot {
            name: "products".to_owned(),
            columns: vec![
                ColumnSnapshot {
                    name: "id".to_owned(),
                    logical_type: LogicalType::Uuid,
                    primary_key: true,
                    nullable: false,
                    default: None,
                },
                ColumnSnapshot {
                    name: "name".to_owned(),
                    logical_type: LogicalType::String,
                    primary_key: false,
                    nullable: false,
                    default: None,
                },
            ],
            unique_constraints: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
        }],
    };
    let plan = plan_change(&from, &to).map_err(|error| error.to_string())?;
    let file = seal(
        "0001_initial".to_owned(),
        "initial".to_owned(),
        from,
        to,
        &plan,
    )
    .map_err(|error| error.to_string())?;
    save_migration(directory, &file).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_project_name;

    #[test]
    fn rejects_unsafe_project_names() {
        assert!(validate_project_name("").is_err());
        assert!(validate_project_name("../secret").is_err());
        assert!(validate_project_name("shop;rm").is_err());
        assert!(validate_project_name("con").is_err());
        assert!(validate_project_name("shop").is_ok());
    }
}
