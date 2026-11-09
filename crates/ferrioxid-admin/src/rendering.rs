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

pub fn embedded(name: &str) -> bool {
    TEMPLATES.iter().any(|(template, _)| *template == name)
}

pub fn render(template: &str, value: Value, extra: &[(&str, &str)]) -> Result<String, String> {
    if !embedded(template) && !extra.iter().any(|(name, _)| *name == template) {
        return Err(format!("unknown admin template: {template}"));
    }
    let mut tera = engine().clone();
    for (name, source) in extra {
        if embedded(name) || !crate::registry::valid_template_name(name) {
            return Err(format!("rejected admin template: {name}"));
        }
        tera.add_raw_template(name, source)
            .map_err(|error| error.to_string())?;
    }
    let context = Context::from_serialize(value).map_err(|error| error.to_string())?;
    tera.render(template, &context)
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
    ("pick.html", include_str!("../templates/models/pick.html")),
    ("bulk.html", include_str!("../templates/models/bulk.html")),
    ("audit.html", include_str!("../templates/audit.html")),
    (
        "field.html",
        include_str!("../templates/partials/field.html"),
    ),
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
        "user_form.html",
        include_str!("../templates/directory/user_form.html"),
    ),
    (
        "user_detail.html",
        include_str!("../templates/directory/user_detail.html"),
    ),
    (
        "roles.html",
        include_str!("../templates/directory/roles.html"),
    ),
    (
        "role_new.html",
        include_str!("../templates/directory/role_new.html"),
    ),
    (
        "permissions.html",
        include_str!("../templates/directory/permissions.html"),
    ),
    ("error.html", include_str!("../templates/errors/error.html")),
    ("custom.html", include_str!("../templates/custom.html")),
];
