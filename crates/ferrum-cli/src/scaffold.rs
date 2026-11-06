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
        "target/\n.env\n*.sqlite3\n*.sqlite3-journal\n",
    )?;
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
    let (template_name, admin, embedded) = match template {
        Template::Api => ("api", "false", "false"),
        Template::Fullstack => ("fullstack", "true", "true"),
    };
    format!(
        r#"# Ferrum-Projekt. Diese Datei enthält keine Secrets.
migrations_dir = "migrations"
schema_command = "cargo run --quiet -- --print-schema"

[project]
name = "{name}"
template = "{template_name}"

[database]
url = "sqlite://db.sqlite3?mode=rwc"

[server]
host = "127.0.0.1"
port = 3000

[admin]
enabled = {admin}
embedded_assets = {embedded}
"#
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
Der Server bindet standardmäßig nur `127.0.0.1`. Ein Benutzer wird mit `ferrum admin create-user --email name@example.com --staff --superuser` angelegt. Das Passwort wird abgefragt.
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
    let auth = ferrum::auth::AuthService::new(db.clone(), ferrum::auth::AuthConfig::from_env());
    let admin = ferrum::admin::AdminRegistry::new()
        .add::<Product>()
        // ferrum:admin
        // ferrum:admin:end
        ;
"#
        .to_owned(),
    };
    let chain = match template {
        Template::Api => ".register::<Product>()\n        // ferrum:registry\n        // ferrum:registry:end\n",
        Template::Fullstack => ".with_auth(auth)\n        .with_admin(admin)\n        .register::<Product>()\n        // ferrum:registry\n        // ferrum:registry:end\n",
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
    tracing_subscriber::fmt().with_env_filter("info").init();
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| ferrum::db::DEFAULT_SQLITE_URL.to_owned());
    let db = ferrum::db::connect(&url).await?;
    {auth}
    let app = ferrum::App::new(db)
        {chain};
    let address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    if address.starts_with("0.0.0.0") {{
        return Err("Ferrum bindet neue Projekte nicht an 0.0.0.0.".into());
    }}
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Ferrum lauscht auf http://{{address}}");
    axum::serve(
        listener,
        app.into_router()
            .into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}}
"#
    )
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
