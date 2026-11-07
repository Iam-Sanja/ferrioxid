//! Tera-Vorlagen aus dem Crate. Es werden keine Template-Pfade aus Anfragen gelesen.

use serde_json::Value;
use std::sync::OnceLock;
use tera::{Context, Tera};

fn engine() -> &'static Tera {
    static ENGINE: OnceLock<Tera> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut tera = Tera::default();
        tera.autoescape_on(vec![".html"]);
        for (name, source) in TEMPLATES {
            tera.add_raw_template(name, source).expect("admin template");
        }
        tera
    })
}

pub fn render(template: &str, value: Value) -> Result<String, String> {
    let context = Context::from_serialize(value).map_err(|error| error.to_string())?;
    engine()
        .render(template, &context)
        .map_err(|error| error.to_string())
}

const TEMPLATES: &[(&str, &str)] = &[
    ("base.html", include_str!("../templates/base.html")),
    ("login.html", include_str!("../templates/login.html")),
    (
        "dashboard.html",
        include_str!("../templates/dashboard.html"),
    ),
    ("list.html", include_str!("../templates/models/list.html")),
    ("form.html", include_str!("../templates/models/form.html")),
    (
        "detail.html",
        include_str!("../templates/models/detail.html"),
    ),
    (
        "delete.html",
        include_str!("../templates/models/delete.html"),
    ),
    (
        "users.html",
        include_str!("../templates/directory/users.html"),
    ),
    (
        "roles.html",
        include_str!("../templates/directory/roles.html"),
    ),
    (
        "permissions.html",
        include_str!("../templates/directory/permissions.html"),
    ),
    ("error.html", include_str!("../templates/errors/error.html")),
];
