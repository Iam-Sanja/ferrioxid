//! Seitenleiste aus registrierten Apps, erlaubten Modellen und eigenen Seiten.

use crate::http::{list_blocked, permit};
use crate::localization::catalog;
use crate::registry::AdminRegistry;
use ferrum_auth::{AppState, AuthUser};
use serde_json::{json, Value};

pub(crate) struct Navigation {
    pub sections: Value,
    pub groups: Value,
}

struct Item {
    href: String,
    label: String,
    app: String,
}

pub(crate) async fn navigation(
    state: &AppState,
    user: &AuthUser,
    registry: &AdminRegistry,
    locale: &str,
    path: &str,
) -> Navigation {
    let mut models = Vec::new();
    let mut views = Vec::new();
    if let Some(auth) = state.auth.as_ref() {
        for model in registry.models() {
            if list_blocked(model.as_ref()) {
                continue;
            }
            if permit(auth, user, model.meta(), "read").await.is_err() {
                continue;
            }
            let key = model.meta().key;
            models.push(Item {
                href: format!("/admin/models/{key}"),
                label: model.meta().label.to_owned(),
                app: registry.model_app(key).unwrap_or("").to_owned(),
            });
        }
        for view in registry.views() {
            if !view.in_navigation() {
                continue;
            }
            let allowed = auth
                .holds(user, view.permission_code())
                .await
                .unwrap_or(false);
            if !allowed {
                continue;
            }
            let key = view.key();
            views.push(Item {
                href: format!("/admin/pages/{key}"),
                label: view.title(locale).to_owned(),
                app: registry.view_app(key).unwrap_or("").to_owned(),
            });
        }
    }
    compose(
        locale,
        path,
        user.is_superuser,
        registry.branding().app_name.as_str(),
        registry.app_catalog(),
        &models,
        &views,
    )
}

fn compose(
    locale: &str,
    path: &str,
    superuser: bool,
    host_label: &str,
    apps: &[(String, String)],
    models: &[Item],
    views: &[Item],
) -> Navigation {
    let text = |key: &str| {
        catalog(locale)
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or(key)
            .to_owned()
    };
    let mut groups = Vec::new();
    for (key, label) in apps {
        push_group(&mut groups, key, label, models, views, path);
    }
    push_group(&mut groups, "", host_label, models, views, path);
    let model_groups: Vec<Value> = groups
        .iter()
        .filter(|group| {
            group["models"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
        })
        .cloned()
        .collect();
    let mut sections = vec![json!({
        "key": "home",
        "label": "",
        "items": [{
            "href": "/admin",
            "label": text("dashboard"),
            "current": path == "/admin" || path == "/admin/"
        }]
    })];
    sections.push(json!({
        "key": "apps",
        "label": text("applications"),
        "empty": groups.is_empty(),
        "groups": groups,
    }));
    if superuser {
        sections.push(json!({
            "key": "access",
            "label": text("access"),
            "items": [
                {
                    "href": "/admin/directory/users",
                    "label": text("users"),
                    "current": path.starts_with("/admin/directory/users")
                },
                {
                    "href": "/admin/directory/roles",
                    "label": text("roles"),
                    "current": path.starts_with("/admin/directory/roles")
                        || path.starts_with("/admin/directory/permissions")
                }
            ]
        }));
    }
    sections.push(json!({
        "key": "system",
        "label": text("system"),
        "items": [{
            "href": "/admin/audit",
            "label": text("history"),
            "current": path.starts_with("/admin/audit")
        }]
    }));
    Navigation {
        sections: Value::Array(sections),
        groups: Value::Array(model_groups),
    }
}

fn push_group(
    groups: &mut Vec<Value>,
    key: &str,
    label: &str,
    models: &[Item],
    views: &[Item],
    path: &str,
) {
    let mut items = Vec::new();
    let mut model_rows = Vec::new();
    for item in models.iter().filter(|item| item.app == key) {
        items.push(link(&item.href, &item.label, path));
        let model_key = item.href.trim_start_matches("/admin/models/");
        model_rows.push(json!({ "key": model_key, "label": item.label }));
    }
    for item in views.iter().filter(|item| item.app == key) {
        items.push(link(&item.href, &item.label, path));
    }
    if items.is_empty() {
        return;
    }
    groups.push(json!({
        "key": key,
        "label": label,
        "items": items,
        "models": model_rows
    }));
}

fn link(href: &str, label: &str, path: &str) -> Value {
    json!({
        "href": href,
        "label": label,
        "current": path == href || path.starts_with(&format!("{href}/"))
    })
}

#[cfg(test)]
mod tests {
    use super::{compose, Item};

    fn item(href: &str, label: &str, app: &str) -> Item {
        Item {
            href: href.to_owned(),
            label: label.to_owned(),
            app: app.to_owned(),
        }
    }

    #[test]
    fn navigation_groups_apps_and_keeps_audit_out_of_them() {
        let apps = vec![
            ("shop".to_owned(), "Shop".to_owned()),
            ("notes".to_owned(), "Notizen".to_owned()),
        ];
        let models = vec![
            item("/admin/models/product", "Produkte", "shop"),
            item("/admin/models/note", "Notizen", "notes"),
            item("/admin/models/loose", "Lose", ""),
        ];
        let views = vec![item("/admin/pages/stock", "Bestand", "shop")];
        let nav = compose(
            "de",
            "/admin/models/product",
            false,
            "Ferrum",
            &apps,
            &models,
            &views,
        );
        let body = nav.sections.to_string();
        assert!(body.contains("\"label\":\"Shop\""));
        assert!(body.contains("\"label\":\"Notizen\""));
        assert!(body.contains("\"label\":\"Ferrum\""));
        assert!(body.contains("\"href\":\"/admin/models/product\""));
        assert!(body.contains("\"href\":\"/admin/pages/stock\""));
        assert!(body.contains("\"current\":true"));
        assert!(body.contains("\"href\":\"/admin/audit\""));
        assert!(!body.contains("\"href\":\"/admin/models\""));
        assert!(!body.contains("Benutzer und Zugang"));
        let groups = nav.groups.to_string();
        assert!(groups.contains("\"key\":\"product\""));
        assert!(!groups.contains("\"key\":\"stock\""));
        let root = compose("en", "/admin/audit", true, "Ferrum", &[], &[], &[]);
        let root_body = root.sections.to_string();
        assert!(root_body.contains("Users and access"));
        assert!(root_body.contains("\"empty\":true"));
        assert!(root_body.contains("\"href\":\"/admin/audit\""));
        assert!(root_body.contains("\"current\":true"));
    }
}
