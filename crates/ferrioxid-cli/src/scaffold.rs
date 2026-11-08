//! Generates a standalone Ferrioxid project outside the framework workspace.

use ferrioxid_migrations::{
    manifest_json, plan_change, save_migration, seal, sha256_hex, ColumnSnapshot, LogicalType,
    ProjectManifest, SchemaSnapshot, TableSnapshot, TrackedFile,
};
use std::path::{Path, PathBuf};

pub fn new_project(
    name: &str,
    template: &str,
    ferrioxid_path: Option<&Path>,
) -> Result<PathBuf, String> {
    validate_project_name(name)?;
    let template = parse_template(template)?;
    let ferrioxid = resolve_ferrioxid_path(ferrioxid_path)?;
    let destination = std::env::current_dir()
        .map_err(|error| error.to_string())?
        .join(name);
    if destination.exists() {
        return Err(format!(
            "{} already exists. Ferrioxid does not overwrite existing directories.",
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
    let ferrioxid_display = cargo_path(&ferrioxid);
    let features = match template {
        Template::Api => "",
        Template::Fullstack => ", features = [\"admin-ui\"]",
    };
    write(
        &destination.join("Cargo.toml"),
        &cargo_toml(name, &ferrioxid_display, features),
    )?;
    write(&destination.join("ferrum.toml"), &manifest(name, template))?;
    write(
        &destination.join(".gitignore"),
        "target/\n.env\n.env.*\n!.env.example\n.ferrum/csrf.key\n.ferrioxid/csrf.key\n*.sqlite3\n*.sqlite3-journal\nmedia/*\n!media/.gitkeep\n",
    )?;
    write(
        &destination.join("static/css/app.css"),
        "body { font-family: sans-serif; }\n",
    )?;
    write(
        &destination.join("static/js/app.js"),
        "document.documentElement.dataset.ferrioxidStatic = \"ready\";\n",
    )?;
    write(&destination.join("media/.gitkeep"), "")?;
    write(&destination.join(".env.example"), env_example())?;
    write(&destination.join("README.md"), &readme(name, template))?;
    write(&destination.join("src/lib.rs"), "pub mod models;\n")?;
    write(
        &destination.join("src/models/mod.rs"),
        "pub mod product;\n// ferrioxid:models\n// ferrioxid:models:end\n",
    )?;
    write(
        &destination.join("src/models/product.rs"),
        &product_model(template),
    )?;
    write(&destination.join("src/main.rs"), &main_rs(name, template))?;
    write(&destination.join("src/settings.rs"), settings_rs())?;
    write(&destination.join("src/urls.rs"), &urls_rs(template))?;
    if matches!(template, Template::Fullstack) {
        std::fs::create_dir_all(destination.join("src/views"))
            .map_err(|error| error.to_string())?;
        write(&destination.join("src/views/mod.rs"), "pub mod home;\n")?;
        write(&destination.join("src/views/home.rs"), home_view())?;
    }
    write(&destination.join("tests/api.rs"), &api_test(name))?;
    write_templates(&destination, name, template)?;
    write_initial_migration(&destination.join("migrations"))?;
    write_upgrade_manifest(&destination, template)?;
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
        _ => Err("Unknown template. Choose api or fullstack.".to_owned()),
    }
}

pub fn validate_project_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("The project name cannot be empty.".to_owned());
    };
    if !first.is_ascii_alphabetic()
        || !chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
        || name.len() > 48
    {
        return Err(
            "The project name must start with a letter and contain only letters, digits, and underscores."
                .to_owned(),
        );
    }
    let reserved = matches!(
        name.to_ascii_lowercase().as_str(),
        "ferrioxid"
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
        return Err(format!("`{name}` is a reserved project name."));
    }
    Ok(())
}

fn resolve_ferrioxid_path(explicit: Option<&Path>) -> Result<PathBuf, String> {
    let raw = if let Some(path) = explicit {
        path.to_path_buf()
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    };
    let path = raw.canonicalize().map_err(|_| {
        "The Ferrioxid path does not exist. Use --ferrioxid-path to specify the repository."
            .to_owned()
    })?;
    let crate_manifest = path.join("crates/ferrioxid/Cargo.toml");
    if !crate_manifest.is_file() {
        return Err(
            "The Ferrioxid path does not contain crates/ferrioxid/Cargo.toml. The crate is not published yet."
                .to_owned(),
        );
    }
    Ok(path.join("crates/ferrioxid"))
}

fn cargo_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    text.replace('\\', "/")
}

fn write_upgrade_manifest(destination: &Path, template: Template) -> Result<(), String> {
    let template_name = match template {
        Template::Api => "api",
        Template::Fullstack => "fullstack",
    };
    let mut relative = vec![
        "Cargo.toml",
        "ferrum.toml",
        ".gitignore",
        ".env.example",
        "README.md",
        "src/lib.rs",
        "src/main.rs",
        "src/settings.rs",
        "src/urls.rs",
        "src/models/mod.rs",
        "src/models/product.rs",
        "tests/api.rs",
        "templates/base.html",
        "templates/index.html",
        "templates/errors/404.html",
        "templates/errors/500.html",
        "static/css/app.css",
        "static/js/app.js",
        "media/.gitkeep",
        "migrations/0001_initial.json",
    ];
    if matches!(template, Template::Fullstack) {
        relative.extend([
            "src/views/mod.rs",
            "src/views/home.rs",
            "templates/home.html",
        ]);
    }
    let mut files = Vec::new();
    for path in relative {
        let bytes = std::fs::read(destination.join(path))
            .map_err(|error| format!("{path} is missing from the new project: {error}"))?;
        files.push(TrackedFile {
            path: path.to_owned(),
            sha256: sha256_hex(&bytes),
        });
    }
    let json = manifest_json(&ProjectManifest::generated(template_name, files))
        .map_err(|error| error.0)?;
    std::fs::create_dir_all(destination.join(".ferrum")).map_err(|error| error.to_string())?;
    std::fs::write(destination.join(".ferrum/project.json"), json)
        .map_err(|error| error.to_string())
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "{} already exists and will not be overwritten.",
            path.display()
        ));
    }
    std::fs::write(path, contents).map_err(|error| error.to_string())
}

fn cargo_toml(name: &str, ferrioxid: &str, features: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
ferrioxid = {{ path = "{ferrioxid}"{features} }}
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
        r##"# Ferrioxid project. This file does not contain secrets.
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
site_title = "Ferrioxid Admin"
site_header = "Ferrioxid Verwaltung"

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

[mail]
backend = "disabled"
public_origin = "http://127.0.0.1:3000"
reset_ttl_minutes = 30
"##
    )
}

fn env_example() -> &'static str {
    "DATABASE_URL=sqlite://db.sqlite3?mode=rwc\nBIND_ADDRESS=127.0.0.1:3000\n# FERRUM_CSRF_SECRET must contain 64 hexadecimal characters in production.\n# FERRUM_PASSWORD is read only when creating users non-interactively.\n# FERRUM_SMTP_PASSWORD must not be stored in ferrum.toml.\n# FERRUM_PUBLIC_ORIGIN sets the password reset URL, for example https://example.com.\n"
}

fn readme(name: &str, template: Template) -> String {
    let frontend = match template {
        Template::Api => "This API project does not require Node.js.\n",
        Template::Fullstack => {
            "The admin is embedded in the Rust binary. Node.js is not required.\n"
        }
    };
    format!(
        r#"# {name}

SQLite is the local database. Docker and PostgreSQL are not required.

```powershell
ferrioxid migrate up
ferrioxid doctor
ferrioxid dev
```

{frontend}
The server binds to `127.0.0.1` by default. The home page is available at `/`. Static files are stored in `static/` and served at `/static/`. `media/` is used for uploads and is not version-controlled. Create a user with `ferrioxid admin create-user --email name@example.com --staff --superuser`. The command prompts for a password. Templates are stored in `templates/` and use Tera. The contact form of a fullstack project is available at `/contact` and does not send email. `[mail] backend = "disabled"` also prevents email from being sent. Password reset is available at `/auth/password-reset` once a mail backend is enabled.
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
    let name_admin = match template {
        Template::Api => "",
        Template::Fullstack => {
            "#[admin(list_display, searchable, sortable, label = \"Name\")]\n    "
        }
    };
    format!(
        r#"use ferrioxid::prelude::*;
use serde::{{Deserialize, Serialize}};

#[derive(FerrioxidModel, Serialize, Deserialize)]
#[ferrioxid(
{attributes}
)]
pub struct Product {{
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, min_length = 1, max_length = 150)]
    {name_admin}pub name: String,
}}
"#
    )
}

fn main_rs(name: &str, template: Template) -> String {
    let auth = match template {
        Template::Api => String::new(),
        Template::Fullstack => r#"
    let mut auth_config = ferrioxid::auth::AuthConfig::from_env();
    auth_config.apply_project_session(settings.session_hours, settings.idle_minutes);
    auth_config.apply_bound_address(&address);
    let auth = ferrioxid::auth::AuthService::new(db.clone(), auth_config);
    let admin = ferrioxid::admin::AdminRegistry::new()
        .with_branding(ferrioxid::admin::AdminBranding {
            site_title: settings.site_title.clone(),
            site_header: settings.site_header.clone(),
            app_name: settings.app_name.clone(),
            primary_color: settings.primary_color.clone(),
            default_locale: settings.default_locale.clone(),
            fallback_locale: settings.fallback_locale.clone(),
            currency: settings.currency.clone(),
        })
        .add::<Product>()
        // ferrioxid:admin
        // ferrioxid:admin:end
        ;
"#
        .to_owned(),
    };
    let modules = match template {
        Template::Api => "mod settings;\nmod urls;\n",
        Template::Fullstack => "mod settings;\nmod urls;\nmod views;\n",
    };
    let chain = match template {
        Template::Api => ".with_settings(settings.clone())\n        .with_mail(ferrioxid::mail_for(&settings))\n        .urls(urls::routes()?)?\n        .register::<Product>()\n        // ferrioxid:registry\n        // ferrioxid:registry:end\n",
        Template::Fullstack => ".with_settings(settings.clone())\n        .with_mail(ferrioxid::mail_for(&settings))\n        .with_auth(auth)\n        .with_admin(admin)\n        .urls(urls::routes()?)?\n        .merge(axum::Router::new().route(\"/contact\", axum::routing::get(show_contact).post(submit_contact)))\n        .register::<Product>()\n        // ferrioxid:registry\n        // ferrioxid:registry:end\n",
    };
    format!(
        r#"{modules}use {name}::models::product::Product;
use ferrioxid::core::FerrioxidModel;
use ferrioxid::migrations::{{RegisteredModel, SchemaSnapshot}};
// ferrioxid:uses
// ferrioxid:uses:end

fn registered_schema() -> Result<SchemaSnapshot, Box<dyn std::error::Error>> {{
    Ok(SchemaSnapshot::from_registered(&[
        RegisteredModel {{
            table: Product::descriptor().table,
            columns: Product::columns(),
            constraints: Product::constraints(),
        }},
        // ferrioxid:schema
        // ferrioxid:schema:end
    ])?)
}}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {{
    if std::env::args().any(|argument| argument == "--print-schema") {{
        println!("{{}}", registered_schema()?.to_canonical_json()?);
        return Ok(());
    }}
    let settings = settings::load()?;
    tracing_subscriber::fmt().with_env_filter("info").init();
    let url = settings.resolved_database_url();
    let db = ferrioxid::db::connect(&url).await?;
    let address = settings.bind_address()?;
    if address.starts_with("0.0.0.0") {{
        return Err("Ferrioxid does not bind new projects to 0.0.0.0.".into());
    }}
    {auth}
    let app = ferrioxid::App::new(db)
        {chain};
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Ferrioxid lauscht auf http://{{address}}");
    let router = ferrioxid::pages::mount_pages(app.into_router(), &settings)?;
    let router = ferrioxid::web::prepare(router, &settings);
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
use ferrioxid::forms::FerrioxidForm;

async fn show_contact(
    axum::extract::State(state): axum::extract::State<ferrioxid::auth::AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    let csrf = csrf_for(&state, &headers).await;
    let form = ferrioxid::forms::render_form(
        "/contact",
        csrf.as_deref(),
        &ferrioxid::forms::FormValues::default(),
        &ferrioxid::forms::FormErrors::default(),
        &ferrioxid::forms::contact_fields("de"),
        "de",
    );
    axum::response::Html(ferrioxid::forms::render_page("Kontakt", &form))
}

async fn submit_contact(
    axum::extract::State(state): axum::extract::State<ferrioxid::auth::AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let values = match ferrioxid::forms::FormValues::parse_urlencoded(&String::from_utf8_lossy(&body)) {
        Ok(values) => values,
        Err(errors) => return axum::response::Html(contact_html(&state, &headers, &ferrioxid::forms::FormValues::default(), &errors).await).into_response(),
    };
    if let Some(auth) = state.auth.as_ref() {
        if auth.csrf_from_request(&headers).await.is_ok()
            && auth.authenticate_csrf(&headers, &axum::http::Method::POST, values.get("csrf_token").unwrap_or("")).await.is_err()
        {
            let mut errors = ferrioxid::forms::FormErrors::default();
            errors.push("", "form.csrf");
            return (axum::http::StatusCode::FORBIDDEN, axum::response::Html(contact_html(&state, &headers, &values, &errors).await)).into_response();
        }
    }
    match ferrioxid::forms::ContactForm::from_form(&values) {
        Ok(_form) => axum::response::Html(ferrioxid::forms::render_page("Kontakt", &format!("<p role=\"status\">{}</p>", ferrioxid::forms::message("de", "form.thanks")))).into_response(),
        Err(errors) => (axum::http::StatusCode::UNPROCESSABLE_ENTITY, axum::response::Html(contact_html(&state, &headers, &values, &errors).await)).into_response(),
    }
}

async fn csrf_for(state: &ferrioxid::auth::AppState, headers: &axum::http::HeaderMap) -> Option<String> {
    match state.auth.as_ref() {
        Some(auth) => auth.csrf_from_request(headers).await.ok(),
        None => None,
    }
}

async fn contact_html(state: &ferrioxid::auth::AppState, headers: &axum::http::HeaderMap, values: &ferrioxid::forms::FormValues, errors: &ferrioxid::forms::FormErrors) -> String {
    let csrf = csrf_for(state, headers).await;
    let form = ferrioxid::forms::render_form("/contact", csrf.as_deref(), values, errors, &ferrioxid::forms::contact_fields("de"), "de");
    ferrioxid::forms::render_page("Kontakt", &form)
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
    if matches!(template, Template::Fullstack) {
        write(
            &destination.join("templates/home.html"),
            "{% extends \"base.html\" %}{% block content %}<h1>{{ welcome }}</h1><p>{{ running }}</p>{% endblock %}\n",
        )?;
    }
    Ok(())
}

fn settings_rs() -> &'static str {
    r#"pub fn load() -> Result<ferrioxid::migrations::ProjectConfig, Box<dyn std::error::Error>> {
    let settings = ferrioxid::migrations::Settings::discover(std::path::Path::new("."))?;
    Ok(settings.project().clone())
}
"#
}

fn urls_rs(template: Template) -> String {
    match template {
        Template::Api => r#"use axum::routing::get;
use ferrioxid::urls::{UrlError, UrlSet};

pub fn routes() -> Result<UrlSet, UrlError> {
    UrlSet::new().route("api.ping", "/api/v1/ping", get(ping))
}

async fn ping() -> axum::response::Response {
    ferrioxid::views::json(&ferrioxid::serde_json::json!({"ok": true}))
}
"#
        .to_owned(),
        Template::Fullstack => r#"use axum::routing::get;
use ferrioxid::urls::{UrlError, UrlSet};

pub fn routes() -> Result<UrlSet, UrlError> {
    UrlSet::new().route("site.home", "/home", get(crate::views::home::home))
}
"#
        .to_owned(),
    }
}

fn home_view() -> &'static str {
    r#"use std::sync::Arc;

use axum::extract::Extension;
use ferrioxid::migrations::ProjectConfig;

pub async fn home(
    Extension(settings): Extension<Arc<ProjectConfig>>,
) -> axum::response::Response {
    let site = match ferrioxid::pages::Site::open(&settings.templates_directory()) {
        Ok(site) => site,
        Err(_) => {
            return ferrioxid::views::fail(
                &axum::http::HeaderMap::new(),
                None,
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "view.template",
                "Internal server error",
                settings.debug,
            )
        }
    };
    let context = ferrioxid::pages::home_context(&settings);
    ferrioxid::views::render(&site, "home.html", &context, settings.debug)
}
"#
}

fn api_test(name: &str) -> String {
    format!(
        r#"use {name}::models::product::Product;
use ferrioxid::core::FerrioxidModel;

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
