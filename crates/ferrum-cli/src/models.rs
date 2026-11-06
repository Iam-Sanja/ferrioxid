//! Erzeugt ein Modell und trägt es über feste Markierungen in die Registry ein.

use crate::scaffold::validate_project_name;
use ferrum_migrations::discover_project;
use std::path::{Path, PathBuf};

pub fn generate_model(name: &str) -> Result<PathBuf, String> {
    validate_model_name(name)?;
    let project = discover_project(&std::env::current_dir().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let module = snake_case(name);
    validate_project_name(&module)?;
    let model_path = project.root.join(format!("src/models/{module}.rs"));
    if model_path.exists() {
        return Err(format!(
            "Das Modell `{name}` existiert bereits und wird nicht überschrieben."
        ));
    }
    let package = package_name(&project.root)?;
    let template = project.template.as_deref().unwrap_or("api");
    std::fs::write(&model_path, model_source(name, &module, template))
        .map_err(|error| error.to_string())?;
    insert_line(
        &project.root.join("src/models/mod.rs"),
        "// ferrum:models:end",
        &format!("pub mod {module};"),
    )?;
    insert_line(
        &project.root.join("src/main.rs"),
        "// ferrum:uses:end",
        &format!("use {package}::models::{module}::{name};"),
    )?;
    insert_line(
        &project.root.join("src/main.rs"),
        "// ferrum:schema:end",
        &format!(
            "        RegisteredModel {{ table: {name}::descriptor().table, columns: {name}::columns(), constraints: {name}::constraints() }},"
        ),
    )?;
    insert_line(
        &project.root.join("src/main.rs"),
        "// ferrum:registry:end",
        &format!("        .register::<{name}>()"),
    )?;
    let admin_marker = project.root.join("src/main.rs");
    let main = std::fs::read_to_string(&admin_marker).map_err(|error| error.to_string())?;
    if main.contains("// ferrum:admin:end") {
        insert_line(
            &admin_marker,
            "// ferrum:admin:end",
            &format!("        .add::<{name}>()"),
        )?;
    }
    Ok(model_path)
}

pub fn validate_model_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("Der Modellname darf nicht leer sein.".to_owned());
    };
    if !first.is_ascii_uppercase() || !chars.all(|char| char.is_ascii_alphanumeric()) {
        return Err(
            "Der Modellname muss PascalCase sein und darf keine Pfadzeichen enthalten.".to_owned(),
        );
    }
    Ok(())
}

fn snake_case(name: &str) -> String {
    let mut result = String::new();
    for (index, char) in name.chars().enumerate() {
        if char.is_ascii_uppercase() {
            if index > 0 {
                result.push('_');
            }
            result.push(char.to_ascii_lowercase());
        } else {
            result.push(char);
        }
    }
    result
}

fn model_source(name: &str, module: &str, template: &str) -> String {
    let table = format!("{module}s");
    let attributes = if template == "fullstack" {
        format!(
            "    table = \"{table}\",\n    api = \"/api/v1/{table}\",\n    admin = true,\n    admin_label = \"{name}\",\n    permissions = \"{module}s\""
        )
    } else {
        format!(
            "    table = \"{table}\",\n    api = \"/api/v1/{table}\",\n    allow_anonymous = true"
        )
    };
    format!(
        r#"use ferrum::prelude::*;
use serde::{{Deserialize, Serialize}};

#[derive(FerrumModel, Serialize, Deserialize)]
#[ferrum(
{attributes}
)]
pub struct {name} {{
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, min_length = 1)]
    pub name: String,
}}
"#
    )
}

fn insert_line(path: &Path, marker: &str, line: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("{} konnte nicht gelesen werden: {error}", path.display()))?;
    if text.contains(line) {
        return Err(format!("`{line}` ist bereits registriert."));
    }
    let Some(index) = text.find(marker) else {
        return Err(format!(
            "{} enthält die Markierung `{marker}` nicht. Es wird nichts ersetzt.",
            path.display()
        ));
    };
    let mut updated = String::new();
    updated.push_str(&text[..index]);
    updated.push_str(line);
    updated.push('\n');
    updated.push_str(&text[index..]);
    std::fs::write(path, updated).map_err(|error| error.to_string())
}

fn package_name(root: &Path) -> Result<String, String> {
    let text =
        std::fs::read_to_string(root.join("Cargo.toml")).map_err(|error| error.to_string())?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("name = ") {
            let name = value.trim().trim_matches('"').to_owned();
            if name.is_empty() {
                return Err("Der Paketname ist leer.".to_owned());
            }
            return Ok(name);
        }
    }
    Err("Cargo.toml enthält keinen Paketnamen.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{snake_case, validate_model_name};

    #[test]
    fn model_names_stay_on_one_identifier() {
        assert!(validate_model_name("Customer").is_ok());
        assert!(validate_model_name("../Customer").is_err());
        assert!(validate_model_name("customer").is_err());
        assert_eq!(snake_case("ProductNote"), "product_note");
    }
}
