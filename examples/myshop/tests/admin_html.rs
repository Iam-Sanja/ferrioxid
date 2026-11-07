//! HTML-Admin ohne JavaScript: Formulare, Links und Sitzungen über echtes HTTP.

use ferrum::admin::AdminRegistry;
use ferrum::auth::{AuthConfig, AuthService, AuthUser, ObjectPolicy};
use ferrum::core::{QueryScope, ScopeValue};
use ferrum::db::ModelRepository;
use ferrum::prelude::*;
use ferrum::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_categories",
    api = "/api/v1/html_categories",
    admin = true,
    admin_label = "Kategorien",
    admin_permission = "categories"
)]
struct HtmlCategory {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 100)]
    #[admin(list_display, searchable, sortable, label = "Name")]
    name: String,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_products",
    api = "/api/v1/html_products",
    admin = true,
    admin_label = "Produkte",
    admin_permission = "products"
)]
struct HtmlProduct {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 150)]
    #[admin(list_display, searchable, sortable, label = "Name")]
    name: String,
    #[field(min = 0)]
    #[admin(list_display, sortable, label = "Preis")]
    price_cents: i64,
    #[field(foreign_key = "html_categories.id", on_delete = "restrict")]
    #[admin(filterable, label = "Kategorie")]
    category_id: Option<Uuid>,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_secrets",
    api = "/api/v1/html_secrets",
    permissions = "secrets",
    object_policy = true,
    admin = true,
    admin_label = "Geheimnisse"
)]
struct HtmlSecret {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required)]
    owner: String,
    #[field(required, min_length = 1)]
    #[admin(list_display, searchable, label = "Titel")]
    title: String,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_payroll",
    api = "/api/v1/html_payroll",
    admin = true,
    admin_permission = "payroll"
)]
struct HtmlPayroll {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1)]
    amount: String,
}

struct OwnerPolicy;

impl ObjectPolicy<HtmlSecret> for OwnerPolicy {
    fn can_read(user: &AuthUser, object: &HtmlSecret) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_update(user: &AuthUser, object: &HtmlSecret) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_delete(user: &AuthUser, object: &HtmlSecret) -> bool {
        object.owner == user.id.to_string()
    }
    fn read_scope(user: &AuthUser) -> QueryScope {
        QueryScope::Equals {
            column: "owner",
            value: ScopeValue::Text(user.id.to_string()),
        }
    }
    fn write_scope(user: &AuthUser) -> QueryScope {
        Self::read_scope(user)
    }
    fn can_create(user: &AuthUser, input: &HtmlSecretCreate) -> bool {
        input.owner == user.id.to_string()
    }
}

struct Reply {
    status: u16,
    headers: String,
    body: String,
}

#[tokio::test]
async fn html_admin_works_without_javascript() {
    let db = ferrum::db::connect("sqlite::memory:").await.unwrap();
    ferrum::auth::apply_migrations(&db).await.unwrap();
    let auth = AuthService::new(db.clone(), AuthConfig::development());
    let staff = auth
        .create_user("staff@example.com", "correct-horse-battery", true, false)
        .await
        .unwrap();
    let plain = auth
        .create_user("plain@example.com", "correct-horse-battery", false, false)
        .await
        .unwrap();
    let root = auth
        .create_user("root@example.com", "correct-horse-battery", true, true)
        .await
        .unwrap();
    auth.create_user("gone@example.com", "correct-horse-battery", true, false)
        .await
        .unwrap();
    auth.ensure_role("catalog").await.unwrap();
    for code in [
        "products.read",
        "products.create",
        "products.update",
        "products.delete",
        "categories.read",
        "categories.create",
        "secrets.read",
        "secrets.create",
    ] {
        auth.ensure_permission(code).await.unwrap();
        auth.grant_permission("catalog", code).await.unwrap();
    }
    auth.assign_role("staff@example.com", "catalog")
        .await
        .unwrap();
    let registry = AdminRegistry::new()
        .add::<HtmlCategory>()
        .add::<HtmlProduct>()
        .add_policy::<HtmlSecret, OwnerPolicy>()
        .add::<HtmlPayroll>();
    let app = ferrum::App::new(db.clone())
        .with_auth(auth.clone())
        .with_admin(registry)
        .register::<HtmlCategory>()
        .register::<HtmlProduct>()
        .register_policy::<HtmlSecret, OwnerPolicy>()
        .register::<HtmlPayroll>();
    app.bootstrap_schema().await.unwrap();
    for index in 0..26 {
        HtmlProduct::create(
            &db,
            HtmlProductCreate {
                name: format!("Stapel {index:02}"),
                price_cents: index,
                category_id: None,
            },
        )
        .await
        .unwrap();
    }
    let hidden = HtmlSecret::create(
        &db,
        HtmlSecretCreate {
            owner: root.id.to_string(),
            title: "RootOnlySecret".into(),
        },
    )
    .await
    .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { axum::serve(listener, app.into_router()).await });
    let call = |method: &'static str, path: String, body: Option<String>, extra: String| {
        tokio::task::spawn_blocking(move || http(port, method, &path, body.as_deref(), &extra))
    };

    let english = call(
        "GET",
        "/admin/login".into(),
        None,
        "Accept-Language: en\r\n".into(),
    )
    .await
    .unwrap();
    assert_eq!(english.status, 200);
    assert!(english.body.contains("Sign in"));
    let login = call(
        "GET",
        "/admin/login?next=https://evil.example".into(),
        None,
        String::new(),
    )
    .await
    .unwrap();
    assert_eq!(login.status, 200);
    assert!(login.body.contains("Anmelden"));
    assert!(!login.body.contains("evil.example"));
    let login_csrf = cookie_value(&login.headers, "ferrum_login_csrf");
    assert_eq!(login_csrf, field_value(&login.body, "csrf_token"));
    assert_eq!(login_csrf.len(), 64);
    let again = call(
        "GET",
        "/admin/login".into(),
        None,
        format!("Cookie: ferrum_login_csrf={login_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(field_value(&again.body, "csrf_token"), login_csrf);

    let unknown = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &login_csrf),
            ("email", "nobody@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={login_csrf}\r\n"),
    )
    .await
    .unwrap();
    let wrong = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &login_csrf),
            ("email", "staff@example.com"),
            ("password", "wrong-horse-battery-1"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={login_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(unknown.status, 401);
    assert_eq!(wrong.status, 401);
    assert!(unknown.body.contains("E-Mail oder Passwort ist ungültig."));
    assert!(wrong.body.contains("E-Mail oder Passwort ist ungültig."));
    let missing_login_csrf = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", "abcd"),
            ("email", "staff@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={login_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(missing_login_csrf.status, 403);
    assert!(missing_login_csrf
        .body
        .contains("Die Sicherheitsprüfung ist fehlgeschlagen."));
    let foreign = call("POST", "/admin/login".into(), Some(form(&[("csrf_token", &login_csrf), ("email", "staff@example.com"), ("password", "correct-horse-battery"), ("next", "/admin")])), "Cookie: ferrum_login_csrf=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n".into()).await.unwrap();
    assert_eq!(foreign.status, 403);

    let signed = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &login_csrf),
            ("email", "staff@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "https://evil.example/phish"),
        ])),
        format!("Cookie: ferrum_login_csrf={login_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(signed.status, 303);
    assert_eq!(location(&signed.headers), "/admin");
    let session = cookie_value(&signed.headers, "ferrum_session");
    assert!(!session.is_empty());

    let home = call("GET", "/admin".into(), None, cookie_header(&session))
        .await
        .unwrap();
    assert_eq!(home.status, 200);
    assert!(home.body.contains("Produkte"));
    assert!(home.body.contains("Kategorien"));
    assert!(!home.body.contains("Payroll") && !home.body.contains("html_payroll"));
    assert!(!home.body.contains("$argon"));
    let csrf = field_value(&home.body, "csrf_token");
    assert!(!csrf.is_empty());
    let anon = call(
        "GET",
        "/admin/models/html_products".into(),
        None,
        String::new(),
    )
    .await
    .unwrap();
    assert_eq!(anon.status, 303);
    assert!(location(&anon.headers).starts_with("/admin/login?next="));

    let plain_login = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let plain_csrf = cookie_value(&plain_login.headers, "ferrum_login_csrf");
    let plain_in = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &plain_csrf),
            ("email", "plain@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={plain_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(plain_in.status, 303);
    let plain_session = cookie_value(&plain_in.headers, "ferrum_session");
    let plain_home = call("GET", "/admin".into(), None, cookie_header(&plain_session))
        .await
        .unwrap();
    assert_eq!(plain_home.status, 403);
    assert!(plain_home.body.contains("staff access is required"));

    let list = call(
        "GET",
        "/admin/models/html_products?sort=name&dir=asc".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(list.status, 200);
    assert!(list.body.contains("sort=name"));
    assert!(list.body.contains("page=2"));
    assert!(list.body.contains("Stapel 00"));
    let invalid_sort = call(
        "GET",
        "/admin/models/html_products?sort=owner".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(invalid_sort.status, 422);
    let invalid_filter = call(
        "GET",
        "/admin/models/html_products?filter.nope=1".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(invalid_filter.status, 422);
    let payroll = call(
        "GET",
        "/admin/models/html_payroll".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(payroll.status, 403);
    let secret_hidden = call(
        "GET",
        format!("/admin/models/html_secrets/{}", hidden.id),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(secret_hidden.status, 404);
    assert!(!secret_hidden.body.contains("RootOnlySecret"));

    let prefs = call(
        "POST",
        "/admin/preferences".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("next", "/admin"),
            ("locale", "en"),
            ("theme", "dark"),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(prefs.status, 303);
    assert_eq!(location(&prefs.headers), "/admin");
    let themed = call(
        "GET",
        "/admin".into(),
        None,
        format!("Cookie: ferrum_session={session}; ferrum_locale=en; ferrum_theme=dark\r\n"),
    )
    .await
    .unwrap();
    assert!(themed.body.contains("data-theme=\"dark\""));
    assert!(themed.body.contains("Dashboard"));
    let open = call(
        "POST",
        "/admin/preferences".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("next", "https://evil.example"),
            ("locale", "de"),
            ("theme", "light"),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(location(&open.headers), "/admin");

    let new_category = call(
        "GET",
        "/admin/models/html_categories/new".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(new_category.status, 200);
    let created_category = call(
        "POST",
        "/admin/models/html_categories/new".into(),
        Some(form(&[("csrf_token", &csrf), ("name", "Obst")])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(created_category.status, 303, "{}", created_category.body);
    let category_id = record_id(&location(&created_category.headers));
    let new_product = call(
        "GET",
        "/admin/models/html_products/new".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert!(new_product.body.contains(&category_id));
    let created = call(
        "POST",
        "/admin/models/html_products/new".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("name", "Apfel"),
            ("price_cents", "199"),
            ("category_id", &category_id),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(created.status, 303, "{}", created.body);
    assert!(location(&created.headers).contains("notice=created"));
    let product_id = record_id(&location(&created.headers));
    let detail = call(
        "GET",
        location(&created.headers),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(detail.status, 200);
    assert!(detail.body.contains("Apfel"));
    assert!(detail.body.contains("Der Eintrag wurde angelegt."));

    let xss = call(
        "POST",
        "/admin/models/html_products/new".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("name", "<script>alert(1)</script>"),
            ("price_cents", "1"),
            ("category_id", ""),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(xss.status, 303, "{}", xss.body);
    let xss_id = record_id(&location(&xss.headers));
    let searched = call(
        "GET",
        "/admin/models/html_products?q=Apfel".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert!(searched.body.contains("Apfel"));
    assert!(!searched.body.contains("Stapel 00"));
    let filtered = call(
        "GET",
        format!("/admin/models/html_products?filter.category_id={category_id}"),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(filtered.status, 200, "{}", filtered.body);
    assert!(filtered.body.contains("Apfel"));
    let escaped = call(
        "GET",
        "/admin/models/html_products?q=%3Cscript%3E".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert!(escaped.body.contains("&lt;script&gt;"));
    assert!(!escaped.body.contains("<script>alert"));

    let forged = call(
        "POST",
        "/admin/models/html_products/new".into(),
        Some(form(&[
            ("csrf_token", "nope"),
            ("name", "Falsch"),
            ("price_cents", "1"),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(forged.status, 403);
    let missing = call(
        "POST",
        "/admin/models/html_products/new".into(),
        Some(form(&[("name", "Ohne"), ("price_cents", "1")])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(missing.status, 403);
    let locked = call(
        "POST",
        "/admin/models/html_products/new".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("name", "Gesperrt"),
            ("price_cents", "1"),
            ("id", "11111111-1111-1111-1111-111111111111"),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_ne!(locked.status, 303);
    assert!(locked
        .body
        .contains("Dieses Feld kann nicht gesetzt werden."));

    let edit = call(
        "GET",
        format!("/admin/models/html_products/{product_id}/edit"),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(edit.status, 200);
    assert!(edit.body.contains("Apfel"));
    assert!(edit.body.contains("199"));
    assert!(edit.body.contains(&format!("value=\"{category_id}\"")));
    assert!(edit.body.contains("selected"));
    let saved = call(
        "POST",
        format!("/admin/models/html_products/{product_id}/edit"),
        Some(form(&[
            ("csrf_token", &csrf),
            ("name", "Birne"),
            ("price_cents", "250"),
            ("category_id", &category_id),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(saved.status, 303, "{}", saved.body);
    assert!(location(&saved.headers).contains("notice=saved"));
    let renamed = call(
        "GET",
        format!("/admin/models/html_products/{product_id}"),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert!(renamed.body.contains("Birne"));

    let confirm = call(
        "GET",
        format!("/admin/models/html_products/{xss_id}/delete"),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(confirm.status, 200);
    assert!(confirm.body.contains("dauerhaft gelöscht"));
    let removed = call(
        "POST",
        format!("/admin/models/html_products/{xss_id}/delete"),
        Some(form(&[("csrf_token", &csrf)])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(removed.status, 303);
    assert!(location(&removed.headers).contains("notice=deleted"));
    let gone_record = call(
        "GET",
        format!("/admin/models/html_products/{xss_id}"),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(gone_record.status, 404);

    let own_secret = call(
        "POST",
        "/admin/models/html_secrets/new".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("owner", &staff.id.to_string()),
            ("title", "Eigen"),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(own_secret.status, 303, "{}", own_secret.body);
    let foreign_secret = call(
        "POST",
        "/admin/models/html_secrets/new".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("owner", &root.id.to_string()),
            ("title", "Fremd"),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(foreign_secret.status, 403);

    let root_login = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let root_csrf_login = cookie_value(&root_login.headers, "ferrum_login_csrf");
    let root_in = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &root_csrf_login),
            ("email", "root@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={root_csrf_login}\r\n"),
    )
    .await
    .unwrap();
    let root_session = cookie_value(&root_in.headers, "ferrum_session");
    let users = call(
        "GET",
        "/admin/directory/users".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(users.status, 200);
    assert!(users.body.contains("staff@example.com"));
    let root_token = field_value(&users.body, "csrf_token");
    let staff_users = call(
        "GET",
        "/admin/directory/users".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(staff_users.status, 403);
    let staff_post = call(
        "POST",
        "/admin/directory/roles".into(),
        Some(form(&[("csrf_token", &csrf), ("name", "editor")])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(staff_post.status, 403);
    assert!(staff_post.body.contains("Diese Aktion ist nicht erlaubt."));
    let staff_bare = call(
        "POST",
        "/admin/directory/roles".into(),
        Some(form(&[("name", "editor")])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(staff_bare.status, 403);
    assert!(staff_bare
        .body
        .contains("Die Sicherheitsprüfung ist fehlgeschlagen."));
    let role = call(
        "POST",
        "/admin/directory/roles".into(),
        Some(form(&[("csrf_token", &root_token), ("name", "editor")])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(role.status, 303, "{}", role.body);
    let grant = call(
        "POST",
        "/admin/directory/roles/editor/permissions".into(),
        Some(form(&[
            ("csrf_token", &root_token),
            ("action", "grant"),
            ("code", "notes.read"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(grant.status, 303, "{}", grant.body);
    let permissions = call(
        "GET",
        "/admin/directory/permissions?role=editor".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(permissions.body.contains("notes.read"));
    let revoke = call(
        "POST",
        "/admin/directory/roles/editor/permissions".into(),
        Some(form(&[
            ("csrf_token", &root_token),
            ("action", "revoke"),
            ("code", "notes.read"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(revoke.status, 303);
    let assign = call(
        "POST",
        "/admin/directory/assignments".into(),
        Some(form(&[
            ("csrf_token", &root_token),
            ("email", "staff@example.com"),
            ("role", "editor"),
            ("action", "assign"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(assign.status, 303, "{}", assign.body);
    let assigned = call(
        "GET",
        "/admin/directory/users?q=staff@example.com".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(assigned.body.contains("editor"));
    let self_off = call(
        "POST",
        format!("/admin/directory/users/{}/active", root.id),
        Some(form(&[("csrf_token", &root_token), ("active", "false")])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(self_off.status, 403);
    let deactivate = call(
        "POST",
        format!("/admin/directory/users/{}/active", plain.id),
        Some(form(&[("csrf_token", &root_token), ("active", "false")])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(deactivate.status, 303, "{}", deactivate.body);

    let inactive_page = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let inactive_csrf = cookie_value(&inactive_page.headers, "ferrum_login_csrf");
    let inactive = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &inactive_csrf),
            ("email", "plain@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={inactive_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(inactive.status, 401);
    assert!(inactive.body.contains("E-Mail oder Passwort ist ungültig."));

    let gone_page = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let gone_csrf = cookie_value(&gone_page.headers, "ferrum_login_csrf");
    let gone_in = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &gone_csrf),
            ("email", "gone@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={gone_csrf}\r\n"),
    )
    .await
    .unwrap();
    let gone_session = cookie_value(&gone_in.headers, "ferrum_session");
    auth.set_user_active("gone@example.com", false)
        .await
        .unwrap();
    let disabled = call("GET", "/admin".into(), None, cookie_header(&gone_session))
        .await
        .unwrap();
    assert_eq!(disabled.status, 303);
    assert!(location(&disabled.headers).starts_with("/admin/login"));

    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "UPDATE _ferrum_sessions SET expires_at = 1",
    ))
    .await
    .unwrap();
    let expired = call("GET", "/admin".into(), None, cookie_header(&session))
        .await
        .unwrap();
    assert_eq!(expired.status, 303);
    assert!(location(&expired.headers).starts_with("/admin/login"));
    let expired_write = call(
        "POST",
        "/admin/logout".into(),
        Some(form(&[("csrf_token", &csrf)])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(expired_write.status, 403);

    let fresh_login = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let fresh_csrf = cookie_value(&fresh_login.headers, "ferrum_login_csrf");
    let fresh = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &fresh_csrf),
            ("email", "root@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin/directory/roles"),
        ])),
        format!("Cookie: ferrum_login_csrf={fresh_csrf}\r\n"),
    )
    .await
    .unwrap();
    let fresh_session = cookie_value(&fresh.headers, "ferrum_session");
    let roles = call(
        "GET",
        "/admin/directory/roles".into(),
        None,
        cookie_header(&fresh_session),
    )
    .await
    .unwrap();
    let fresh_token = field_value(&roles.body, "csrf_token");
    let bye = call(
        "POST",
        "/admin/logout".into(),
        Some(form(&[("csrf_token", &fresh_token)])),
        cookie_header(&fresh_session),
    )
    .await
    .unwrap();
    assert_eq!(bye.status, 303);
    assert_eq!(location(&bye.headers), "/admin/login");
    let after = call("GET", "/admin".into(), None, cookie_header(&fresh_session))
        .await
        .unwrap();
    assert_eq!(after.status, 303);

    server.abort();
}

#[tokio::test]
async fn html_login_csrf_respects_the_bound_port() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    assert_ne!(port, 3000);
    let db = ferrum::db::connect("sqlite::memory:").await.unwrap();
    ferrum::auth::apply_migrations(&db).await.unwrap();
    let mut config = AuthConfig::development();
    assert!(config.with_loopback_address(&format!("127.0.0.1:{port}")));
    assert_eq!(
        config.allowed_origins,
        vec![
            format!("http://127.0.0.1:{port}"),
            format!("http://localhost:{port}"),
        ]
    );
    let auth = AuthService::new(db.clone(), config);
    auth.create_user("staff@example.com", "correct-horse-battery", true, false)
        .await
        .unwrap();
    let app = ferrum::App::new(db)
        .with_auth(auth)
        .with_admin(AdminRegistry::new());
    let server = tokio::spawn(async move { axum::serve(listener, app.into_router()).await });
    let call = |method: &'static str, path: String, body: Option<String>, extra: String| {
        tokio::task::spawn_blocking(move || http(port, method, &path, body.as_deref(), &extra))
    };

    let first = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    assert_eq!(first.status, 200);
    assert!(first
        .headers
        .to_ascii_lowercase()
        .contains("cache-control: no-store"));
    let token = cookie_value(&first.headers, "ferrum_login_csrf");
    assert_eq!(token, field_value(&first.body, "csrf_token"));
    assert_eq!(token.len(), 64);
    let issued = set_cookie_line(&first.headers, "ferrum_login_csrf");
    assert!(issued.contains("HttpOnly"));
    assert!(issued.contains("Path=/"));
    assert!(issued.contains("SameSite=Lax"));
    assert!(issued.contains("Max-Age=600"));
    assert!(!issued.contains("Secure"));
    assert!(!issued.to_ascii_lowercase().contains("domain="));

    let second = call(
        "GET",
        "/admin/login".into(),
        None,
        format!("Cookie: ferrum_login_csrf={token}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(field_value(&second.body, "csrf_token"), token);

    let login_body = form(&[
        ("csrf_token", &token),
        ("email", "staff@example.com"),
        ("password", "correct-horse-battery"),
        ("next", "/admin"),
    ]);
    let missing_cookie = call(
        "POST",
        "/admin/login".into(),
        Some(login_body.clone()),
        format!("Origin: http://127.0.0.1:{port}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(missing_cookie.status, 403);
    assert!(missing_cookie
        .body
        .contains("Die Sicherheitsprüfung ist fehlgeschlagen."));

    let tampered = call(
        "POST",
        "/admin/login".into(),
        Some(login_body.clone()),
        format!(
            "Origin: http://127.0.0.1:{port}\r\nCookie: ferrum_login_csrf={}\r\n",
            "b".repeat(64)
        ),
    )
    .await
    .unwrap();
    assert_eq!(tampered.status, 403);

    let missing_field = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("email", "staff@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Origin: http://127.0.0.1:{port}\r\nCookie: ferrum_login_csrf={token}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(missing_field.status, 403);

    let wrong_field = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &"c".repeat(64)),
            ("email", "staff@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Origin: http://127.0.0.1:{port}\r\nCookie: ferrum_login_csrf={token}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(wrong_field.status, 403);

    let foreign = call(
        "POST",
        "/admin/login".into(),
        Some(login_body.clone()),
        format!("Origin: http://evil.example\r\nCookie: ferrum_login_csrf={token}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(foreign.status, 403);

    let null_origin = call(
        "POST",
        "/admin/login".into(),
        Some(login_body.clone()),
        format!("Origin: null\r\nCookie: ferrum_login_csrf={token}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(null_origin.status, 403);
    let null_cross_site = call(
        "POST",
        "/admin/login".into(),
        Some(login_body.clone()),
        format!(
            "Origin: null\r\nSec-Fetch-Site: cross-site\r\nCookie: ferrum_login_csrf={token}\r\n"
        ),
    )
    .await
    .unwrap();
    assert_eq!(null_cross_site.status, 403);
    let null_same_site = call(
        "POST",
        "/admin/login".into(),
        Some(login_body.clone()),
        format!(
            "Origin: null\r\nSec-Fetch-Site: same-site\r\nCookie: ferrum_login_csrf={token}\r\n"
        ),
    )
    .await
    .unwrap();
    assert_eq!(null_same_site.status, 403);

    let stale_port = call(
        "POST",
        "/admin/login".into(),
        Some(login_body.clone()),
        format!("Origin: http://127.0.0.1:3000\r\nCookie: ferrum_login_csrf={token}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(stale_port.status, 403);

    let foreign_referer = call(
        "POST",
        "/admin/login".into(),
        Some(login_body.clone()),
        format!(
            "Referer: http://evil.example/admin/login\r\nCookie: ferrum_login_csrf={token}\r\n"
        ),
    )
    .await
    .unwrap();
    assert_eq!(foreign_referer.status, 403);

    let nulled = call(
        "POST",
        "/admin/login".into(),
        Some(login_body),
        format!(
            "Origin: null\r\nSec-Fetch-Site: same-origin\r\nCookie: ferrum_login_csrf={token}\r\n"
        ),
    )
    .await
    .unwrap();
    assert_eq!(nulled.status, 303, "{}", nulled.body);
    let nulled_session = cookie_value(&nulled.headers, "ferrum_session");
    let nulled_home = call("GET", "/admin".into(), None, cookie_header(&nulled_session))
        .await
        .unwrap();
    let nulled_csrf = field_value(&nulled_home.body, "csrf_token");
    let nulled_bye = call(
        "POST",
        "/admin/logout".into(),
        Some(form(&[("csrf_token", &nulled_csrf)])),
        cookie_header(&nulled_session),
    )
    .await
    .unwrap();
    assert_eq!(nulled_bye.status, 303);

    let rebound = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let rebound_token = cookie_value(&rebound.headers, "ferrum_login_csrf");
    let rebound_body = form(&[
        ("csrf_token", &rebound_token),
        ("email", "staff@example.com"),
        ("password", "correct-horse-battery"),
        ("next", "/admin"),
    ]);
    let signed = tokio::task::spawn_blocking(move || {
        http_on(
            port,
            "POST",
            "/admin/login",
            Some(&rebound_body),
            &format!(
                "Origin: http://127.0.0.1:{port}\r\nCookie: ferrum_login_csrf={rebound_token}\r\n"
            ),
            "evil.example",
        )
    })
    .await
    .unwrap();
    assert_eq!(signed.status, 303, "{}", signed.body);
    assert_eq!(location(&signed.headers), "/admin");
    let session = cookie_value(&signed.headers, "ferrum_session");
    assert!(!session.is_empty());

    let home = call("GET", "/admin".into(), None, cookie_header(&session))
        .await
        .unwrap();
    assert_eq!(home.status, 200);
    let csrf = field_value(&home.body, "csrf_token");
    let bye = call(
        "POST",
        "/admin/logout".into(),
        Some(form(&[("csrf_token", &csrf)])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(bye.status, 303);

    let again = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let again_token = cookie_value(&again.headers, "ferrum_login_csrf");
    let relogin = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &again_token),
            ("email", "staff@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Origin: http://localhost:{port}\r\nCookie: ferrum_login_csrf={again_token}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(relogin.status, 303);
    assert!(!cookie_value(&relogin.headers, "ferrum_session").is_empty());

    server.abort();
}

fn http(port: u16, method: &str, path: &str, body: Option<&str>, extra: &str) -> Reply {
    http_on(port, method, path, body, extra, "localhost")
}

fn http_on(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
    extra: &str,
    host: &str,
) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .unwrap();
    let data = body.unwrap_or("");
    let content_type = if method == "POST" {
        "application/x-www-form-urlencoded"
    } else {
        "text/plain"
    };
    let request = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra}\r\n{data}", data.len());
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                response.extend_from_slice(&buffer[..count]);
                if complete(&response) {
                    break;
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                if complete(&response) {
                    break;
                }
                panic!("timeout: {error}");
            }
            Err(error) => panic!("read: {error}"),
        }
    }
    let text = String::from_utf8_lossy(&response);
    let (headers, raw_body) = text.split_once("\r\n\r\n").unwrap_or((text.as_ref(), ""));
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    Reply {
        status,
        headers: headers.to_owned(),
        body: String::from_utf8_lossy(&decode(headers, raw_body.as_bytes())).into_owned(),
    }
}

fn complete(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let Some((headers, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    if headers
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return body.ends_with("0\r\n\r\n");
    }
    let length = headers.lines().find_map(|line| {
        line.to_ascii_lowercase()
            .strip_prefix("content-length:")
            .map(|value| value.trim().parse::<usize>().unwrap_or(0))
    });
    length.is_some_and(|length| body.len() >= length)
}

fn decode(header: &str, body: &[u8]) -> Vec<u8> {
    if !header
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return body.to_vec();
    }
    let mut decoded = Vec::new();
    let mut rest = body;
    loop {
        let end = rest.windows(2).position(|bytes| bytes == b"\r\n").unwrap();
        let length = usize::from_str_radix(
            std::str::from_utf8(&rest[..end])
                .unwrap()
                .split(';')
                .next()
                .unwrap(),
            16,
        )
        .unwrap();
        if length == 0 {
            break;
        }
        let start = end + 2;
        decoded.extend_from_slice(&rest[start..start + length]);
        rest = &rest[start + length + 2..];
    }
    decoded
}

fn set_cookie_line(headers: &str, name: &str) -> String {
    headers
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("set-cookie:") && line.contains(name))
        .unwrap_or("")
        .to_owned()
}

fn cookie_value(headers: &str, name: &str) -> String {
    headers
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("set-cookie:"))
        .find_map(|line| {
            let value = line.split_once(':')?.1.trim().split(';').next()?.trim();
            let (key, token) = value.split_once('=')?;
            (key == name && !token.is_empty()).then(|| token.to_owned())
        })
        .unwrap_or_default()
}

fn cookie_header(token: &str) -> String {
    format!("Cookie: ferrum_session={token}\r\n")
}

fn location(headers: &str) -> String {
    headers
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("location:"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| value.trim().to_owned())
        .unwrap_or_default()
}

fn field_value(body: &str, name: &str) -> String {
    let needle = format!("name=\"{name}\" value=\"");
    body.find(&needle)
        .map(|start| {
            body[start + needle.len()..]
                .split('"')
                .next()
                .unwrap_or("")
                .to_owned()
        })
        .unwrap_or_default()
}

fn record_id(location: &str) -> String {
    location
        .split('/')
        .nth(4)
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .to_owned()
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", key, encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}
