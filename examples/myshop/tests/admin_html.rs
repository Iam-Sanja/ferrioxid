//! HTML-Admin ohne JavaScript: Formulare, Links und Sitzungen über echtes HTTP.

use ferrum::admin::{ActionOperation, AdminAction, AdminRegistry};
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
    has_many = "items=HtmlProduct.category_id",
    admin = true,
    admin_label = "Kategorien",
    admin_permission = "categories",
    admin_inline = "items=html_products.category_id"
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
    #[admin(list_display, filterable, label = "Kategorie")]
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

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_titles",
    api = "/api/v1/html_titles",
    admin = true,
    admin_label = "Titel"
)]
struct HtmlTitle {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 80)]
    title: String,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_counts",
    api = "/api/v1/html_counts",
    admin = true,
    admin_label = "Zaehler"
)]
struct HtmlCount {
    #[field(primary_key, auto)]
    id: Uuid,
    #[admin(filterable, label = "Total")]
    total: i64,
    #[admin(filterable, label = "Aktiv")]
    active: bool,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_vaults",
    api = "/api/v1/html_vaults",
    admin = true,
    admin_label = "Tresor"
)]
struct HtmlVault {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 80)]
    label: String,
    #[admin(hidden)]
    password_hash: String,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_links",
    api = "/api/v1/html_links",
    admin = true,
    admin_label = "Verknuepfungen",
    admin_permission = "links"
)]
struct HtmlLink {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(foreign_key = "html_secrets.id", on_delete = "set_null")]
    #[admin(list_display, filterable, label = "Geheimnis")]
    secret_id: Option<Uuid>,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_codes",
    api = "/api/v1/html_codes",
    admin = true,
    admin_label = "Codes"
)]
struct HtmlCode {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 40)]
    #[admin(list_display, label = "Code")]
    code: String,
    #[field(required, min_length = 1, max_length = 80)]
    note: String,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_offers",
    api = "/api/v1/html_offers",
    admin = true,
    admin_label = "Angebote",
    admin_permission = "offers",
    admin_ordering = "-title",
    admin_per_page = 2,
    admin_fieldset = "Allgemein|Name und Kategorie",
    admin_fieldset = "Preis und Verfuegbarkeit",
    admin_fieldset = "System|Nur lesen|optional"
)]
struct HtmlOffer {
    #[field(primary_key, auto)]
    #[admin(fieldset = "System")]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 80)]
    #[admin(
        list_display,
        list_link,
        searchable,
        sortable,
        label = "Titel",
        fieldset = "Allgemein"
    )]
    title: String,
    #[field(foreign_key = "html_categories.id", on_delete = "restrict")]
    #[admin(label = "Kategorie", fieldset = "Allgemein")]
    category_id: Option<Uuid>,
    #[field(min = 0)]
    #[admin(label = "Preis", fieldset = "Preis und Verfuegbarkeit")]
    price_cents: i64,
    #[admin(label = "Aktiv", fieldset = "Preis und Verfuegbarkeit")]
    active: bool,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_bins",
    api = "/api/v1/html_bins",
    has_many = "notes=HtmlOwnedNote.bin_id",
    admin = true,
    admin_label = "Faecher",
    admin_permission = "bins",
    admin_inline = "notes=html_owned_notes.bin_id"
)]
struct HtmlBin {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 80)]
    #[admin(list_display, label = "Name")]
    name: String,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_owned_notes",
    api = "/api/v1/html_owned_notes",
    permissions = "notes",
    object_policy = true,
    admin = true,
    admin_label = "Notizen",
    admin_permission = "notes"
)]
struct HtmlOwnedNote {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required)]
    owner: String,
    #[field(required, min_length = 1, max_length = 80)]
    #[admin(list_display, label = "Titel")]
    title: String,
    #[field(foreign_key = "html_bins.id", on_delete = "restrict")]
    #[admin(label = "Fach")]
    bin_id: Option<Uuid>,
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

struct NotePolicy;

impl ObjectPolicy<HtmlOwnedNote> for NotePolicy {
    fn can_read(user: &AuthUser, object: &HtmlOwnedNote) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_update(user: &AuthUser, object: &HtmlOwnedNote) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_delete(user: &AuthUser, object: &HtmlOwnedNote) -> bool {
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
    fn can_create(user: &AuthUser, input: &HtmlOwnedNoteCreate) -> bool {
        input.owner == user.id.to_string()
    }
}

struct Reply {
    status: u16,
    headers: String,
    body: String,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "html_appointments",
    api = "/api/v1/html_appointments",
    admin = true,
    admin_label = "Termine",
    admin_permission = "appointments",
    admin_date_hierarchy = "event_date"
)]
struct HtmlAppointment {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 80)]
    #[admin(list_display, searchable, label = "Titel")]
    title: String,
    #[field(required)]
    #[admin(list_display, sortable, filterable, label = "Datum")]
    event_date: NaiveDate,
    #[admin(label = "Beginn")]
    start_time: Option<NaiveTime>,
    #[field(required)]
    #[admin(label = "Angelegt")]
    created_at: DateTime<Utc>,
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
    let power = auth
        .create_user("power@example.com", "correct-horse-battery", false, true)
        .await
        .unwrap();
    assert!(!power.is_staff);
    assert!(power.is_superuser);
    auth.create_user("idle@example.com", "correct-horse-battery", true, true)
        .await
        .unwrap();
    auth.set_user_active("idle@example.com", false)
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
        "links.read",
        "links.create",
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
        .add::<HtmlPayroll>()
        .add::<HtmlTitle>()
        .add::<HtmlCount>()
        .add::<HtmlVault>()
        .add::<HtmlCode>()
        .add::<HtmlLink>()
        .add::<HtmlOffer>();
    let app = ferrum::App::new(db.clone())
        .with_auth(auth.clone())
        .with_admin(registry)
        .register::<HtmlCategory>()
        .register::<HtmlProduct>()
        .register_policy::<HtmlSecret, OwnerPolicy>()
        .register::<HtmlPayroll>()
        .register::<HtmlTitle>()
        .register::<HtmlCount>()
        .register::<HtmlVault>()
        .register::<HtmlCode>()
        .register::<HtmlLink>()
        .register::<HtmlOffer>();
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
    HtmlTitle::create(
        &db,
        HtmlTitleCreate {
            title: "NurTitel".into(),
        },
    )
    .await
    .unwrap();
    HtmlCount::create(
        &db,
        HtmlCountCreate {
            total: 42,
            active: true,
        },
    )
    .await
    .unwrap();
    HtmlVault::create(
        &db,
        HtmlVaultCreate {
            label: "SichtbaresEtikett".into(),
            password_hash: "$argon2id$v=19$hidden-hash".into(),
        },
    )
    .await
    .unwrap();
    HtmlCode::create(
        &db,
        HtmlCodeCreate {
            code: "SICHTBARER-CODE".into(),
            note: "GEHEIME-NOTIZ".into(),
        },
    )
    .await
    .unwrap();
    let _visible = HtmlSecret::create(
        &db,
        HtmlSecretCreate {
            owner: staff.id.to_string(),
            title: "StaffVisible".into(),
        },
    )
    .await
    .unwrap();
    let mut late_category = None;
    for index in 0..=100 {
        let created = HtmlCategory::create(
            &db,
            HtmlCategoryCreate {
                name: format!("Zettel {index:03}"),
            },
        )
        .await
        .unwrap();
        if index == 100 {
            late_category = Some(created);
        }
    }
    let late_category = late_category.unwrap();
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
    assert!(home.body.contains("href=\"/admin/models/html_products\""));
    assert!(!home.body.contains("href=\"/admin/models\""));
    assert!(home.body.contains("Anwendungen"));
    assert!(home.body.contains("Historie"));
    assert!(!home.body.contains("Benutzer und Zugang"));
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
    assert!(plain_home
        .body
        .contains("Die Verwaltung verlangt einen Staff-Zugang."));
    assert!(plain_home.body.contains("action=\"/admin/logout\""));
    assert!(!plain_home.body.contains("<aside"));
    assert!(!plain_home.body.contains("Stapel"));
    assert!(!plain_home.body.contains("NurTitel"));
    let plain_csrf = field_value(&plain_home.body, "csrf_token");
    assert!(!plain_csrf.is_empty());
    let english = call(
        "GET",
        "/admin".into(),
        None,
        format!("Cookie: ferrum_session={plain_session}; ferrum_locale=en\r\n"),
    )
    .await
    .unwrap();
    assert!(english
        .body
        .contains("Administration requires staff access."));
    let missing_csrf = call(
        "POST",
        "/admin/logout".into(),
        Some(form(&[("csrf_token", "")])),
        cookie_header(&plain_session),
    )
    .await
    .unwrap();
    assert_eq!(missing_csrf.status, 403);
    let foreign_csrf = call(
        "POST",
        "/admin/logout".into(),
        Some(form(&[("csrf_token", &csrf)])),
        cookie_header(&plain_session),
    )
    .await
    .unwrap();
    assert_eq!(foreign_csrf.status, 403);
    let still_plain = call("GET", "/admin".into(), None, cookie_header(&plain_session))
        .await
        .unwrap();
    assert_eq!(still_plain.status, 403);
    let plain_out = call(
        "POST",
        "/admin/logout".into(),
        Some(form(&[("csrf_token", &plain_csrf)])),
        cookie_header(&plain_session),
    )
    .await
    .unwrap();
    assert_eq!(plain_out.status, 303);
    assert_eq!(location(&plain_out.headers), "/admin/login");
    let plain_after = call("GET", "/admin".into(), None, cookie_header(&plain_session))
        .await
        .unwrap();
    assert_eq!(plain_after.status, 303);
    assert!(location(&plain_after.headers).starts_with("/admin/login"));
    let plain_models = call(
        "GET",
        "/admin/models/html_products".into(),
        None,
        cookie_header(&plain_session),
    )
    .await
    .unwrap();
    assert_eq!(plain_models.status, 303);

    let power_login = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let power_csrf = cookie_value(&power_login.headers, "ferrum_login_csrf");
    let power_in = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &power_csrf),
            ("email", "power@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={power_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(power_in.status, 303);
    let power_session = cookie_value(&power_in.headers, "ferrum_session");
    let power_home = call("GET", "/admin".into(), None, cookie_header(&power_session))
        .await
        .unwrap();
    assert_eq!(power_home.status, 403);
    assert!(power_home
        .body
        .contains("Die Verwaltung verlangt einen Staff-Zugang."));
    assert!(!power_home.body.contains("NurTitel"));
    let power_token = field_value(&power_home.body, "csrf_token");
    let power_out = call(
        "POST",
        "/admin/logout".into(),
        Some(form(&[("csrf_token", &power_token)])),
        cookie_header(&power_session),
    )
    .await
    .unwrap();
    assert_eq!(power_out.status, 303);
    let power_again = call(
        "GET",
        "/admin/models/html_titles".into(),
        None,
        cookie_header(&power_session),
    )
    .await
    .unwrap();
    assert_eq!(power_again.status, 303);
    let stored = auth
        .list_users()
        .await
        .unwrap()
        .into_iter()
        .find(|user| user.email == "power@example.com")
        .unwrap();
    assert!(!stored.is_staff && stored.is_superuser && stored.is_active);

    let idle_login = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let idle_csrf = cookie_value(&idle_login.headers, "ferrum_login_csrf");
    let idle_in = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &idle_csrf),
            ("email", "idle@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={idle_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(idle_in.status, 401);

    let list = call(
        "GET",
        "/admin/models/html_products?sort=name&dir=asc".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(list.status, 200);
    assert!(list.body.contains("Name"));
    assert!(list.body.contains("Preis"));
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
    assert!(new_product.body.contains("Obst"));
    assert!(!new_product.body.contains("data-track-dirty=\"record\""));
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
    assert!(detail.body.contains("Obst"));
    assert!(detail.body.contains(&category_id));
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
    assert!(searched.body.contains("Obst"));
    assert!(searched.body.contains(&format!("title=\"{category_id}\"")));
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
    assert!(filtered.body.contains("name=\"filter.category_id\""));
    assert!(filtered.body.contains("Obst"));
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
    assert!(edit.body.contains("Obst"));
    assert!(edit.body.contains("selected"));
    assert!(edit.body.contains("data-track-dirty=\"record\""));
    assert!(edit.body.contains("audit?model=html_products"));
    assert!(edit.body.contains("class=\"js-save\""));
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
    assert!(renamed.body.contains("Obst"));
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA foreign_keys=OFF",
    ))
    .await
    .unwrap();
    HtmlCategory::delete(&db, category_id.parse().unwrap())
        .await
        .expect("category delete");
    let missing_relation = call(
        "GET",
        format!("/admin/models/html_products/{product_id}"),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert!(
        missing_relation.body.contains("Nicht sichtbar"),
        "{}",
        missing_relation.body
    );
    assert!(!missing_relation.body.contains("Obst"));
    assert!(missing_relation.body.contains(&category_id));

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
    let own_secret_id = record_id(&location(&own_secret.headers));
    let own_link = call(
        "POST",
        "/admin/models/html_links/new".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("secret_id", &own_secret_id),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(own_link.status, 303, "{}", own_link.body);
    let foreign_link = call(
        "POST",
        "/admin/models/html_links/new".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("secret_id", &hidden.id.to_string()),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_ne!(foreign_link.status, 303, "{}", foreign_link.body);
    assert!(
        foreign_link.body.contains("sichtbaren Eintrag"),
        "{}",
        foreign_link.body
    );
    let links = call(
        "GET",
        "/admin/models/html_links".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(links.status, 200, "{}", links.body);
    assert!(links.body.contains("Eigen"));
    assert!(!links.body.contains("Nicht sichtbar"));
    assert!(!links.body.contains("RootOnlySecret"));

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
    let root_link_form = call(
        "GET",
        "/admin/models/html_links/new".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    let root_link_csrf = field_value(&root_link_form.body, "csrf_token");
    let root_link = call(
        "POST",
        "/admin/models/html_links/new".into(),
        Some(form(&[
            ("csrf_token", &root_link_csrf),
            ("secret_id", &hidden.id.to_string()),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(root_link.status, 303, "{}", root_link.body);
    let hidden_label = call(
        "GET",
        "/admin/models/html_links".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert!(
        hidden_label.body.contains("Nicht sichtbar"),
        "{}",
        hidden_label.body
    );
    assert!(!hidden_label.body.contains("RootOnlySecret"));
    let root_missing = call(
        "GET",
        format!("/admin/models/html_products/{product_id}"),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(root_missing.body.contains("Nicht vorhanden"));
    assert!(!root_missing.body.contains("Obst"));
    let root_links = call(
        "GET",
        "/admin/models/html_links".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(root_links.body.contains("Eigen"));
    assert!(root_links.body.contains("RootOnlySecret"));
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
    assert!(users.body.contains("Kein Staff"));
    assert!(users.body.contains("Kein Superuser"));
    assert!(users.body.contains(">Superuser<") || users.body.contains("Superuser</span>"));
    assert!(users.body.contains("<select"));
    assert!(users.body.contains("name=\"role\""));
    assert!(!users.body.contains("<input name=\"role\""));
    assert!(!users.body.contains("Nein · Superuser"));
    assert!(!users.body.contains("Rolle anlegen"));
    assert!(users.body.contains("Benutzer und Zugang"));
    assert!(
        users.body.contains("href=\"/admin/directory/roles\"")
            || users
                .body
                .contains("href=\"&#x2F;admin&#x2F;directory&#x2F;roles\"")
    );
    assert!(users.body.contains("aria-current=\"page\""));
    let titles = call(
        "GET",
        "/admin/models/html_titles".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(titles.status, 200, "{}", titles.body);
    assert!(titles.body.contains("NurTitel"));
    let header = titles.body.find("<thead").expect("table header");
    let head_end = titles.body[header..].find("</thead>").expect("header end");
    let head = &titles.body[header..header + head_end];
    let title_col = head.find("Title").expect("title column");
    let id_col = head.find("Id").expect("id column");
    assert!(title_col < id_col);
    let counts = call(
        "GET",
        "/admin/models/html_counts".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(counts.status, 200, "{}", counts.body);
    assert!(counts.body.contains("42"));
    assert!(counts.body.contains("Total"));
    assert!(counts.body.contains("name=\"filter.active\""));
    assert!(counts.body.contains("name=\"filter.total\""));
    assert!(counts.body.contains("type=\"number\""));
    assert!(counts.body.contains(">Alle<"));
    assert!(counts.body.contains(">Ja<"));
    assert!(counts.body.contains(">Nein<"));
    let counted = call(
        "GET",
        "/admin/models/html_counts?filter.total=42&filter.active=true".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(counted.status, 200, "{}", counted.body);
    assert!(counted.body.contains("42"));
    let bad_count = call(
        "GET",
        "/admin/models/html_counts?filter.total=abc".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(bad_count.status, 422);
    let vault = call(
        "GET",
        "/admin/models/html_vaults".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(vault.status, 200, "{}", vault.body);
    assert!(vault.body.contains("SichtbaresEtikett"));
    assert!(!vault.body.contains("$argon2id$"));
    assert!(!vault.body.contains("password_hash"));
    let codes = call(
        "GET",
        "/admin/models/html_codes".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(codes.status, 200, "{}", codes.body);
    assert!(codes.body.contains("SICHTBARER-CODE"));
    assert!(!codes.body.contains("GEHEIME-NOTIZ"));
    let hidden_sort = call(
        "GET",
        "/admin/models/html_codes?sort=note".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(hidden_sort.status, 422);
    let staff_titles = call(
        "GET",
        "/admin/models/html_titles".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(staff_titles.status, 403);
    assert!(!staff_titles.body.contains("NurTitel"));
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
            ("confirm", "1"),
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
            ("confirm", "1"),
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
            ("confirm", "1"),
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
    assert!(assigned.body.contains("Benutzer anlegen"));
    assert!(assigned.body.contains("Anzeigen"));
    let new_user = call(
        "GET",
        "/admin/directory/users/new".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(new_user.status, 200, "{}", new_user.body);
    assert!(new_user.body.contains("name=\"password\""));
    assert!(!new_user.body.contains("is_superuser"));
    assert!(!new_user.body.contains("argon2"));
    let unconfirmed_user = call(
        "POST",
        "/admin/directory/users/new".into(),
        Some(form(&[
            ("csrf_token", &root_token),
            ("email", "clerk@example.com"),
            ("password", "correct-horse-battery"),
            ("password_confirm", "correct-horse-battery"),
            ("active", "true"),
            ("staff", "true"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(unconfirmed_user.status, 422, "{}", unconfirmed_user.body);
    let invalid_confirm = call(
        "POST",
        "/admin/directory/users/new".into(),
        Some(form(&[
            ("csrf_token", &root_token),
            ("email", "clerk@example.com"),
            ("password", "correct-horse-battery"),
            ("password_confirm", "correct-horse-battery"),
            ("active", "true"),
            ("staff", "true"),
            ("confirm", "true"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(invalid_confirm.status, 422, "{}", invalid_confirm.body);
    let elevated = call(
        "POST",
        "/admin/directory/users/new".into(),
        Some(form(&[
            ("csrf_token", &root_token),
            ("email", "elevated@example.com"),
            ("password", "correct-horse-battery"),
            ("password_confirm", "correct-horse-battery"),
            ("active", "true"),
            ("staff", "true"),
            ("is_superuser", "true"),
            ("confirm", "1"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(elevated.status, 422, "{}", elevated.body);
    let staff_create = call(
        "POST",
        "/admin/directory/users/new".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("email", "clerk@example.com"),
            ("password", "correct-horse-battery"),
            ("password_confirm", "correct-horse-battery"),
            ("active", "true"),
            ("staff", "true"),
            ("confirm", "1"),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(staff_create.status, 403);
    let created_user = call(
        "POST",
        "/admin/directory/users/new".into(),
        Some(form(&[
            ("csrf_token", &root_token),
            ("email", "clerk@example.com"),
            ("password", "correct-horse-battery"),
            ("password_confirm", "correct-horse-battery"),
            ("active", "true"),
            ("staff", "true"),
            ("roles", "catalog"),
            ("confirm", "1"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(created_user.status, 303, "{}", created_user.body);
    assert!(!location(&created_user.headers).contains("password"));
    let clerk_id = record_id(&location(&created_user.headers));
    let clerk_page = call(
        "GET",
        location(&created_user.headers),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(clerk_page.status, 200, "{}", clerk_page.body);
    assert!(clerk_page.body.contains("clerk@example.com"));
    assert!(clerk_page.body.contains("catalog"));
    assert!(!clerk_page.body.contains("argon2"));
    assert!(!clerk_page.body.contains("correct-horse-battery"));
    let clerk_login_page = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let clerk_csrf = cookie_value(&clerk_login_page.headers, "ferrum_login_csrf");
    let clerk_in = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &clerk_csrf),
            ("email", "clerk@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={clerk_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(clerk_in.status, 303, "{}", clerk_in.body);
    let clerk_session = cookie_value(&clerk_in.headers, "ferrum_session");
    let clerk_home = call("GET", "/admin".into(), None, cookie_header(&clerk_session))
        .await
        .unwrap();
    assert_eq!(clerk_home.status, 200, "{}", clerk_home.body);
    let reset = call(
        "POST",
        format!("/admin/directory/users/{clerk_id}/password"),
        Some(form(&[
            ("csrf_token", &root_token),
            ("password", "replacement-horse-battery"),
            ("password_confirm", "replacement-horse-battery"),
            ("confirm", "1"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(reset.status, 303, "{}", reset.body);
    assert!(!location(&reset.headers).contains("replacement"));
    let revoked = call("GET", "/admin".into(), None, cookie_header(&clerk_session))
        .await
        .unwrap();
    assert_eq!(revoked.status, 303);
    assert!(location(&revoked.headers).starts_with("/admin/login"));
    let old_password = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let old_csrf = cookie_value(&old_password.headers, "ferrum_login_csrf");
    let old_in = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &old_csrf),
            ("email", "clerk@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={old_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_ne!(old_in.status, 303);
    let fresh_login = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let fresh_csrf = cookie_value(&fresh_login.headers, "ferrum_login_csrf");
    let fresh_in = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &fresh_csrf),
            ("email", "clerk@example.com"),
            ("password", "replacement-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrum_login_csrf={fresh_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(fresh_in.status, 303, "{}", fresh_in.body);
    let fresh_session = cookie_value(&fresh_in.headers, "ferrum_session");
    let staff_removed = call(
        "POST",
        format!("/admin/directory/users/{clerk_id}/edit"),
        Some(form(&[
            ("csrf_token", &root_token),
            ("email", "clerk@example.com"),
            ("active", "true"),
            ("staff", "false"),
            ("confirm", "1"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(staff_removed.status, 303, "{}", staff_removed.body);
    let staff_gone = call("GET", "/admin".into(), None, cookie_header(&fresh_session))
        .await
        .unwrap();
    assert_eq!(staff_gone.status, 403, "{}", staff_gone.body);
    let bad_csrf = call(
        "POST",
        format!("/admin/directory/users/{clerk_id}/active"),
        Some(form(&[
            (
                "csrf_token",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            ("active", "false"),
            ("confirm", "1"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(bad_csrf.status, 403);
    let staff_attack = call(
        "POST",
        format!("/admin/directory/users/{}/active", plain.id),
        Some(form(&[
            ("csrf_token", &csrf),
            ("active", "false"),
            ("confirm", "1"),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(staff_attack.status, 403);
    let protected = call(
        "POST",
        format!("/admin/directory/users/{}/password", root.id),
        Some(form(&[
            ("csrf_token", &root_token),
            ("password", "replacement-horse-battery"),
            ("password_confirm", "replacement-horse-battery"),
            ("confirm", "1"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(protected.status, 403, "{}", protected.body);
    assert!(protected.body.contains("/admin/directory/users"));
    let self_off = call(
        "POST",
        format!("/admin/directory/users/{}/active", root.id),
        Some(form(&[("csrf_token", &root_token), ("active", "false")])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(self_off.status, 403);
    assert!(self_off.body.contains("/admin/directory/users"));
    let unconfirmed = call(
        "POST",
        format!("/admin/directory/users/{}/active", plain.id),
        Some(form(&[("csrf_token", &root_token), ("active", "false")])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(unconfirmed.status, 422, "{}", unconfirmed.body);
    assert!(unconfirmed
        .body
        .contains("Diese Aktion braucht eine Bestätigung."));
    let deactivate = call(
        "POST",
        format!("/admin/directory/users/{}/active", plain.id),
        Some(form(&[
            ("csrf_token", &root_token),
            ("active", "false"),
            ("confirm", "1"),
        ])),
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

    for title in ["Amsel", "Zebra", "Mitte"] {
        HtmlOffer::create(
            &db,
            HtmlOfferCreate {
                title: title.into(),
                category_id: None,
                price_cents: 10,
                active: true,
            },
        )
        .await
        .unwrap();
    }
    let denied = call(
        "GET",
        "/admin/models/html_offers".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(denied.status, 403, "{}", denied.body);
    let offers = call(
        "GET",
        "/admin/models/html_offers".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(offers.status, 200, "{}", offers.body);
    let zebra = offers.body.find("Zebra").unwrap();
    let mitte = offers.body.find("Mitte").unwrap();
    assert!(zebra < mitte);
    assert!(!offers.body.contains("Amsel"));
    assert!(offers.body.contains("Zebra</a>"), "{}", offers.body);
    let offer_page = call(
        "GET",
        "/admin/models/html_offers?page=2".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(offer_page.body.contains("Amsel"), "{}", offer_page.body);
    let bad_sort = call(
        "GET",
        "/admin/models/html_offers?sort=title%3Bdrop".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(bad_sort.status, 422);
    let ascending = call(
        "GET",
        "/admin/models/html_offers?sort=title&dir=asc".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    let amsel = ascending.body.find("Amsel").unwrap();
    let zebra_again = ascending.body.find("Zebra");
    assert!(zebra_again.is_none() || amsel < zebra_again.unwrap());
    let offer_new = call(
        "GET",
        "/admin/models/html_offers/new".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(offer_new.body.contains("Allgemein"));
    assert!(offer_new.body.contains("Preis und Verfuegbarkeit"));
    assert!(offer_new.body.contains("Name und Kategorie"));
    assert!(!offer_new.body.contains("<summary>System</summary>"));
    let offer_csrf = field_value(&offer_new.body, "csrf_token");
    let continued = call(
        "POST",
        "/admin/models/html_offers/new".into(),
        Some(form(&[
            ("csrf_token", &offer_csrf),
            ("title", "Weiter"),
            ("price_cents", "15"),
            ("active", "true"),
            ("save", "edit"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(continued.status, 303, "{}", continued.body);
    let continued_at = location(&continued.headers);
    assert!(
        continued_at.contains("/edit?notice=created"),
        "{continued_at}"
    );
    let another = call(
        "POST",
        "/admin/models/html_offers/new".into(),
        Some(form(&[
            ("csrf_token", &offer_csrf),
            ("title", "Noch eins"),
            ("price_cents", "16"),
            ("active", "false"),
            ("save", "new"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(another.status, 303, "{}", another.body);
    assert_eq!(
        location(&another.headers),
        "/admin/models/html_offers/new?notice=created"
    );
    let before_bad = call(
        "GET",
        "/admin/models/html_offers".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    let rejected = call(
        "POST",
        "/admin/models/html_offers/new".into(),
        Some(form(&[
            ("csrf_token", &offer_csrf),
            ("title", "Nicht speichern"),
            ("price_cents", "17"),
            ("active", "true"),
            ("save", "https://evil.example"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(rejected.status, 422, "{}", rejected.body);
    let after_bad = call(
        "GET",
        "/admin/models/html_offers".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(
        before_bad.body.matches("Nicht speichern").count(),
        after_bad.body.matches("Nicht speichern").count()
    );
    let kept = "Bleibt-12345678901234567890123456789012345678901234567890123456789012345678901234";
    let invalid = call(
        "POST",
        "/admin/models/html_offers/new".into(),
        Some(form(&[
            ("csrf_token", &offer_csrf),
            ("title", kept),
            ("category_id", &late_category.id.to_string()),
            ("price_cents", "4"),
            ("active", "true"),
            ("save", "detail"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_ne!(invalid.status, 303, "{}", invalid.body);
    assert!(invalid.body.contains(kept), "{}", invalid.body);
    assert!(invalid.body.contains(&late_category.id.to_string()));
    assert!(invalid.body.contains("value=\"true\" checked"));
    let edited = call("GET", continued_at, None, cookie_header(&root_session))
        .await
        .unwrap();
    assert!(
        edited.body.contains("<summary>System</summary>"),
        "{}",
        edited.body
    );
    assert!(edited.body.contains("Nur lesen"));
    let found = call(
        "GET",
        "/admin/models/html_offers/pick/category_id?q=Zettel%20100".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(found.status, 200, "{}", found.body);
    assert!(found.body.contains("Zettel 100"), "{}", found.body);
    assert!(!found.body.contains("Zettel 000"));
    let later = call(
        "GET",
        "/admin/models/html_offers/pick/category_id?page=5".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(later.body.contains("Zettel 100"), "{}", later.body);
    assert!(!later.body.contains("Zettel 000"));
    let secret_pick = call(
        "GET",
        "/admin/models/html_links/pick/secret_id".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_eq!(secret_pick.status, 200, "{}", secret_pick.body);
    assert!(secret_pick.body.contains("StaffVisible"));
    assert!(!secret_pick.body.contains("RootOnlySecret"));
    let link_form = call(
        "GET",
        "/admin/models/html_links/new".into(),
        None,
        cookie_header(&session),
    )
    .await
    .unwrap();
    let link_csrf = field_value(&link_form.body, "csrf_token");
    let forged = call(
        "POST",
        "/admin/models/html_links/new".into(),
        Some(form(&[
            ("csrf_token", &link_csrf),
            ("secret_id", &hidden.id.to_string()),
            ("save", "detail"),
        ])),
        cookie_header(&session),
    )
    .await
    .unwrap();
    assert_ne!(forged.status, 303, "{}", forged.body);
    assert!(
        forged.body.contains("sichtbaren Eintrag"),
        "{}",
        forged.body
    );
    let user_form_page = call(
        "GET",
        "/admin/directory/users/new".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    let user_csrf = field_value(&user_form_page.body, "csrf_token");
    let mismatched = call(
        "POST",
        "/admin/directory/users/new".into(),
        Some(form(&[
            ("csrf_token", &user_csrf),
            ("email", "kept@example.com"),
            ("password", "correct-horse-battery"),
            ("password_confirm", "other-horse-battery"),
            ("active", "true"),
            ("staff", "false"),
            ("roles", "catalog"),
            ("confirm", "1"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(mismatched.status, 422, "{}", mismatched.body);
    assert!(mismatched.body.contains("value=\"kept@example.com\""));
    assert!(mismatched.body.contains("value=\"catalog\" checked"));
    assert!(mismatched
        .body
        .contains("Die Passwörter stimmen nicht überein."));
    assert!(!mismatched.body.contains("correct-horse-battery"));
    assert!(!mismatched.body.contains("other-horse-battery"));
    assert!(!mismatched.body.contains("csrf_token="));

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

#[tokio::test]
async fn admin_v018_drafts_inlines_bulk_and_audit() {
    let db = ferrum::db::connect("sqlite::memory:").await.unwrap();
    ferrum::auth::apply_migrations(&db).await.unwrap();
    let auth = AuthService::new(db.clone(), AuthConfig::development());
    let staff = auth
        .create_user("staff@example.com", "correct-horse-battery", true, false)
        .await
        .unwrap();
    let root = auth
        .create_user("root@example.com", "correct-horse-battery", true, true)
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
        "secrets.delete",
        "bins.read",
        "notes.read",
        "notes.create",
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
        .add::<HtmlCount>()
        .add::<HtmlOffer>()
        .add::<HtmlVault>()
        .add::<HtmlBin>()
        .add_policy::<HtmlSecret, OwnerPolicy>()
        .add_policy::<HtmlOwnedNote, NotePolicy>()
        .action(AdminAction {
            model_key: "html_counts",
            name: "feature",
            label_de: "Hervorheben",
            label_en: "Feature",
            permission: "update",
            confirm: true,
            max_ids: 100,
            operation: ActionOperation::SetBool {
                column: "active",
                value: true,
            },
        });
    let app = ferrum::App::new(db.clone())
        .with_auth(auth.clone())
        .with_admin(registry)
        .register::<HtmlCategory>()
        .register::<HtmlProduct>()
        .register::<HtmlCount>()
        .register::<HtmlOffer>()
        .register::<HtmlVault>()
        .register::<HtmlBin>()
        .register_policy::<HtmlSecret, OwnerPolicy>()
        .register_policy::<HtmlOwnedNote, NotePolicy>();
    app.bootstrap_schema().await.unwrap();
    let obst = HtmlCategory::create(
        &db,
        HtmlCategoryCreate {
            name: "Obst".into(),
        },
    )
    .await
    .unwrap();
    let other = HtmlCategory::create(
        &db,
        HtmlCategoryCreate {
            name: "Gemuese".into(),
        },
    )
    .await
    .unwrap();
    let empty = HtmlCategory::create(
        &db,
        HtmlCategoryCreate {
            name: "Leer".into(),
        },
    )
    .await
    .unwrap();
    let first = HtmlCount::create(
        &db,
        HtmlCountCreate {
            total: 1,
            active: false,
        },
    )
    .await
    .unwrap();
    let second = HtmlCount::create(
        &db,
        HtmlCountCreate {
            total: 2,
            active: false,
        },
    )
    .await
    .unwrap();
    let featured = HtmlCount::create(
        &db,
        HtmlCountCreate {
            total: 3,
            active: false,
        },
    )
    .await
    .unwrap();
    let own_secret = HtmlSecret::create(
        &db,
        HtmlSecretCreate {
            owner: staff.id.to_string(),
            title: "EigenesGeheimnis".into(),
        },
    )
    .await
    .unwrap();
    let foreign_secret = HtmlSecret::create(
        &db,
        HtmlSecretCreate {
            owner: root.id.to_string(),
            title: "FremdesGeheimnis".into(),
        },
    )
    .await
    .unwrap();
    let bin = HtmlBin::create(
        &db,
        HtmlBinCreate {
            name: "Ablage".into(),
        },
    )
    .await
    .unwrap();
    HtmlOwnedNote::create(
        &db,
        HtmlOwnedNoteCreate {
            owner: root.id.to_string(),
            title: "RootGeheim".into(),
            bin_id: Some(bin.id),
        },
    )
    .await
    .unwrap();
    HtmlOwnedNote::create(
        &db,
        HtmlOwnedNoteCreate {
            owner: staff.id.to_string(),
            title: "StaffNotiz".into(),
            bin_id: Some(bin.id),
        },
    )
    .await
    .unwrap();
    let kept = HtmlProduct::create(
        &db,
        HtmlProductCreate {
            name: "Bleibtprodukt".into(),
            price_cents: 10,
            category_id: Some(obst.id),
        },
    )
    .await
    .unwrap();
    let also = HtmlProduct::create(
        &db,
        HtmlProductCreate {
            name: "Auchprodukt".into(),
            price_cents: 11,
            category_id: Some(obst.id),
        },
    )
    .await
    .unwrap();
    HtmlVault::create(
        &db,
        HtmlVaultCreate {
            label: "AltEtikett".into(),
            password_hash: "$argon2id$v=19$hidden-hash".into(),
        },
    )
    .await
    .unwrap();
    // bootstrap_schema erzeugt keine Fremdschlüssel. Die Sammelaktion muss am echten RESTRICT scheitern.
    for sql in [
        "CREATE TABLE html_products_restricted (id text PRIMARY KEY NOT NULL, name text NOT NULL, price_cents integer NOT NULL, category_id text, CONSTRAINT fk_html_products_category FOREIGN KEY (category_id) REFERENCES html_categories (id) ON DELETE RESTRICT)",
        "INSERT INTO html_products_restricted (id, name, price_cents, category_id) SELECT id, name, price_cents, category_id FROM html_products",
        "DROP TABLE html_products",
        "ALTER TABLE html_products_restricted RENAME TO html_products",
    ] {
        db.execute(Statement::from_string(DatabaseBackend::Sqlite, sql.to_owned()))
            .await
            .unwrap();
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app.into_router()).await });
    let call = |method: &'static str, path: String, body: Option<String>, extra: String| {
        tokio::task::spawn_blocking(move || http(port, method, &path, body.as_deref(), &extra))
    };
    let mut root_session = String::new();
    let mut staff_session = String::new();
    for email in ["root@example.com", "staff@example.com"] {
        let login = call("GET", "/admin/login".into(), None, String::new())
            .await
            .unwrap();
        let token = cookie_value(&login.headers, "ferrum_login_csrf");
        let signed = call(
            "POST",
            "/admin/login".into(),
            Some(form(&[
                ("csrf_token", &token),
                ("email", email),
                ("password", "correct-horse-battery"),
                ("next", "/admin"),
            ])),
            format!("Cookie: ferrum_login_csrf={token}\r\n"),
        )
        .await
        .unwrap();
        assert_eq!(signed.status, 303, "{}", signed.body);
        let session = cookie_value(&signed.headers, "ferrum_session");
        if email.starts_with("root") {
            root_session = session;
        } else {
            staff_session = session;
        }
    }
    let root_home = call("GET", "/admin".into(), None, cookie_header(&root_session))
        .await
        .unwrap();
    let root_csrf = field_value(&root_home.body, "csrf_token");
    let staff_home = call("GET", "/admin".into(), None, cookie_header(&staff_session))
        .await
        .unwrap();
    let staff_csrf = field_value(&staff_home.body, "csrf_token");

    let login = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    assert!(login.body.contains("<label for=\"email\">"));
    assert!(login.body.contains("<button type=\"submit\">"));
    assert!(!login.body.contains("onkeydown") && !login.body.contains("onkeypress"));
    let form_page = call(
        "GET",
        "/admin/models/html_offers/new".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(form_page.status, 200, "{}", form_page.body);
    assert!(form_page.body.contains("<label class=\"check\">"));
    assert!(!form_page.body.contains("class=\"check\" for="));
    assert!(form_page.body.contains("type=\"checkbox\""));
    assert!(form_page.body.contains("name=\"save\""));
    assert!(form_page.body.contains("type=\"submit\""));
    assert!(form_page.body.contains("<fieldset"));
    let css = call("GET", "/admin/assets/admin.css".into(), None, String::new())
        .await
        .unwrap();
    assert!(css.body.contains(":focus-visible"));
    let script = call("GET", "/admin/assets/admin.js".into(), None, String::new())
        .await
        .unwrap();
    assert!(!script.body.contains("keydown") && !script.body.contains("keyup"));

    let draft = call(
        "POST",
        "/admin/models/html_offers/draft".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("title", "Bleibt-Name"),
            ("price_cents", "199"),
            ("active", "true"),
            ("password", "hunter2-geheim"),
            ("password_hash", "$argon2id$stored"),
            ("pick_field", "category_id"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(draft.status, 303, "{}", draft.body);
    let draft_url = location(&draft.headers);
    assert!(draft_url.contains("draft="));
    assert!(!draft_url.contains("Bleibt-Name") && !draft_url.contains("hunter2"));
    let draft_id = draft_url
        .split("draft=")
        .nth(1)
        .unwrap_or("")
        .split('&')
        .next()
        .unwrap_or("");
    let other_draft = call(
        "POST",
        "/admin/models/html_offers/draft".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("title", "Anderer-Name"),
            ("price_cents", "250"),
            ("active", "false"),
            ("pick_field", "category_id"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    let other_id = location(&other_draft.headers)
        .split("draft=")
        .nth(1)
        .unwrap_or("")
        .split('&')
        .next()
        .unwrap_or("")
        .to_owned();
    let restored = call(
        "GET",
        format!("/admin/models/html_offers/new?draft={draft_id}"),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(
        restored.body.contains("value=\"Bleibt-Name\""),
        "{}",
        restored.body
    );
    assert!(restored.body.contains("value=\"199\""), "{}", restored.body);
    assert!(restored.body.contains("checked"), "{}", restored.body);
    let second_view = call(
        "GET",
        format!("/admin/models/html_offers/new?draft={other_id}"),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(second_view.body.contains("value=\"Anderer-Name\""));
    assert!(restored.body.contains("value=\"Bleibt-Name\""));
    let stolen = call(
        "GET",
        format!("/admin/models/html_offers/new?draft={draft_id}"),
        None,
        cookie_header(&staff_session),
    )
    .await
    .unwrap();
    assert!(!stolen.body.contains("Bleibt-Name"), "{}", stolen.body);
    let payloads = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT payload FROM _ferrum_admin_drafts",
        ))
        .await
        .unwrap();
    for row in &payloads {
        let payload: String = row.try_get("", "payload").unwrap();
        assert!(
            !payload.contains("hunter2") && !payload.contains("argon2"),
            "{payload}"
        );
    }
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "UPDATE _ferrum_admin_drafts SET expires_at = 1",
    ))
    .await
    .unwrap();
    let expired = call(
        "GET",
        format!("/admin/models/html_offers/new?draft={draft_id}"),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(!expired.body.contains("Bleibt-Name"), "{}", expired.body);

    let invalid = call(
        "POST",
        "/admin/models/html_offers/draft".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("title", "Korrigierbar"),
            ("price_cents", "abc"),
            ("active", "true"),
            ("pick_field", "category_id"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    let invalid_id = location(&invalid.headers)
        .split("draft=")
        .nth(1)
        .unwrap_or("")
        .split('&')
        .next()
        .unwrap_or("")
        .to_owned();
    let invalid_form = call(
        "GET",
        format!("/admin/models/html_offers/new?draft={invalid_id}"),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(
        invalid_form.body.contains("value=\"abc\""),
        "{}",
        invalid_form.body
    );
    let rejected = call(
        "POST",
        "/admin/models/html_offers/new".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("_draft", &invalid_id),
            ("title", "Korrigierbar"),
            ("price_cents", "abc"),
            ("active", "true"),
            ("category_id", &obst.id.to_string()),
            ("save", "detail"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_ne!(rejected.status, 303, "{}", rejected.body);
    assert!(rejected.body.contains("value=\"abc\""), "{}", rejected.body);
    assert!(
        rejected.body.contains("value=\"Korrigierbar\""),
        "{}",
        rejected.body
    );

    let picked = call(
        "POST",
        "/admin/models/html_offers/draft".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("title", "Auswahl-Name"),
            ("price_cents", "321"),
            ("active", "true"),
            ("pick_field", "category_id"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    let pick = call(
        "GET",
        location(&picked.headers),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert!(pick.body.contains("Obst"), "{}", pick.body);
    let choice = pick
        .body
        .split("href=\"")
        .find(|part| part.contains("draft=") && part.contains("category_id="))
        .unwrap_or("");
    let choice = choice
        .split('"')
        .next()
        .unwrap_or("")
        .replace("&#x2F;", "/");
    let chosen = call("GET", choice, None, cookie_header(&root_session))
        .await
        .unwrap();
    assert!(
        chosen.body.contains("value=\"Auswahl-Name\""),
        "{}",
        chosen.body
    );
    assert!(
        chosen.body.contains(&obst.id.to_string()),
        "{}",
        chosen.body
    );

    let detail = call(
        "GET",
        format!("/admin/models/html_categories/{}", obst.id),
        None,
        cookie_header(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(detail.status, 200, "{}", detail.body);
    assert!(detail.body.contains("Bleibtprodukt"), "{}", detail.body);
    assert!(detail.body.contains("Hinzufügen"), "{}", detail.body);
    let created = call(
        "POST",
        format!("/admin/models/html_categories/{}/inline/items/new", obst.id),
        Some(form(&[
            ("csrf_token", &staff_csrf),
            ("name", "Inline-Apfel"),
            ("price_cents", "199"),
            ("category_id", &other.id.to_string()),
        ])),
        cookie_header(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(created.status, 303, "{}", created.body);
    let products = HtmlProduct::list(&db).await.unwrap();
    let stored = products
        .data
        .iter()
        .find(|item| item.name == "Inline-Apfel")
        .unwrap();
    assert_eq!(stored.category_id, Some(obst.id));
    let bin_page = call(
        "GET",
        format!("/admin/models/html_bins/{}", bin.id),
        None,
        cookie_header(&staff_session),
    )
    .await
    .unwrap();
    assert!(bin_page.body.contains("StaffNotiz"), "{}", bin_page.body);
    assert!(!bin_page.body.contains("RootGeheim"), "{}", bin_page.body);

    let staged = call(
        "POST",
        "/admin/models/html_counts/bulk".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("action", "activate"),
            ("ids", &first.id.to_string()),
            ("ids", &second.id.to_string()),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(staged.status, 303, "{}", staged.body);
    assert!(!HtmlCount::get(&db, first.id).await.unwrap().unwrap().active);
    let confirm = call(
        "GET",
        location(&staged.headers),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(confirm.status, 200, "{}", confirm.body);
    assert!(confirm.body.contains("Betroffene Datensätze"));
    assert!(!HtmlCount::get(&db, first.id).await.unwrap().unwrap().active);
    let unconfirmed = call(
        "POST",
        location(&staged.headers),
        Some(form(&[("csrf_token", &root_csrf)])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_ne!(unconfirmed.status, 303);
    assert!(
        !HtmlCount::get(&db, second.id)
            .await
            .unwrap()
            .unwrap()
            .active
    );
    let applied = call(
        "POST",
        location(&staged.headers),
        Some(form(&[("csrf_token", &root_csrf), ("confirm", "1")])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(applied.status, 303, "{}", applied.body);
    assert!(HtmlCount::get(&db, first.id).await.unwrap().unwrap().active);
    assert!(
        HtmlCount::get(&db, second.id)
            .await
            .unwrap()
            .unwrap()
            .active
    );

    let denied = call(
        "POST",
        "/admin/models/html_secrets/bulk".into(),
        Some(form(&[
            ("csrf_token", &staff_csrf),
            ("action", "delete"),
            ("ids", &own_secret.id.to_string()),
            ("ids", &foreign_secret.id.to_string()),
        ])),
        cookie_header(&staff_session),
    )
    .await
    .unwrap();
    assert_ne!(denied.status, 303, "{}", denied.body);
    assert!(HtmlSecret::get(&db, own_secret.id).await.unwrap().is_some());
    assert!(HtmlSecret::get(&db, foreign_secret.id)
        .await
        .unwrap()
        .is_some());
    let missing = call(
        "POST",
        "/admin/models/html_counts/bulk".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("action", "deactivate"),
            ("ids", &first.id.to_string()),
            ("ids", &Uuid::new_v4().to_string()),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_ne!(missing.status, 303, "{}", missing.body);
    assert!(HtmlCount::get(&db, first.id).await.unwrap().unwrap().active);
    let forged = call(
        "POST",
        "/admin/models/html_counts/bulk".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("action", "deactivate"),
            ("ids", "not-a-uuid"),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(forged.status, 422, "{}", forged.body);
    let no_csrf = call(
        "POST",
        "/admin/models/html_counts/bulk".into(),
        Some(form(&[
            ("action", "deactivate"),
            ("ids", &first.id.to_string()),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(no_csrf.status, 403, "{}", no_csrf.body);
    let unknown = call(
        "POST",
        "/admin/models/html_counts/bulk".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("action", "nope"),
            ("ids", &featured.id.to_string()),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(unknown.status, 422, "{}", unknown.body);
    assert!(
        !HtmlCount::get(&db, featured.id)
            .await
            .unwrap()
            .unwrap()
            .active
    );
    let custom = call(
        "POST",
        "/admin/models/html_counts/bulk".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("action", "feature"),
            ("ids", &featured.id.to_string()),
            ("ids", &featured.id.to_string()),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(custom.status, 303, "{}", custom.body);
    let custom_done = call(
        "POST",
        location(&custom.headers),
        Some(form(&[("csrf_token", &root_csrf), ("confirm", "1")])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(custom_done.status, 303, "{}", custom_done.body);
    assert!(
        location(&custom_done.headers).contains("changed=1"),
        "{}",
        location(&custom_done.headers)
    );
    assert!(
        HtmlCount::get(&db, featured.id)
            .await
            .unwrap()
            .unwrap()
            .active
    );

    let queued = call(
        "POST",
        "/admin/models/html_products/bulk".into(),
        Some(form(&[
            ("csrf_token", &staff_csrf),
            ("action", "delete"),
            ("ids", &kept.id.to_string()),
            ("ids", &also.id.to_string()),
        ])),
        cookie_header(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(queued.status, 303, "{}", queued.body);
    auth.revoke_permission("catalog", "products.delete")
        .await
        .unwrap();
    let blocked = call(
        "POST",
        location(&queued.headers),
        Some(form(&[("csrf_token", &staff_csrf), ("confirm", "1")])),
        cookie_header(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(blocked.status, 403, "{}", blocked.body);
    assert!(HtmlProduct::get(&db, kept.id).await.unwrap().is_some());
    assert!(HtmlProduct::get(&db, also.id).await.unwrap().is_some());

    let rollback = call(
        "POST",
        "/admin/models/html_categories/bulk".into(),
        Some(form(&[
            ("csrf_token", &root_csrf),
            ("action", "delete"),
            ("ids", &obst.id.to_string()),
            ("ids", &empty.id.to_string()),
        ])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(rollback.status, 303, "{}", rollback.body);
    let failed = call(
        "POST",
        location(&rollback.headers),
        Some(form(&[("csrf_token", &root_csrf), ("confirm", "1")])),
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_ne!(failed.status, 303, "{}", failed.body);
    assert!(HtmlCategory::get(&db, obst.id).await.unwrap().is_some());
    assert!(HtmlCategory::get(&db, empty.id).await.unwrap().is_some());

    let history = call(
        "GET",
        "/admin/audit".into(),
        None,
        cookie_header(&root_session),
    )
    .await
    .unwrap();
    assert_eq!(history.status, 200, "{}", history.body);
    assert!(history.body.contains("<time datetime=\""));
    assert!(history.body.contains(" UTC</time>"));
    assert!(
        history.body.contains("Zeitpunkte werden in UTC")
            || history.body.contains("Times are shown in UTC")
    );
    assert!(history.body.contains("bulk_update"), "{}", history.body);
    assert!(
        history.body.contains("Inline-Apfel") || history.body.contains("create"),
        "{}",
        history.body
    );
    assert!(!history.body.contains("hidden-hash"), "{}", history.body);
    assert!(!history.body.contains("hunter2"), "{}", history.body);
    let staff_history = call(
        "GET",
        "/admin/audit".into(),
        None,
        cookie_header(&staff_session),
    )
    .await
    .unwrap();
    assert!(
        staff_history.body.contains("Inline-Apfel"),
        "{}",
        staff_history.body
    );
    assert!(
        !staff_history.body.contains("bulk_update"),
        "{}",
        staff_history.body
    );
}

#[tokio::test]
async fn admin_dates_use_native_inputs_and_utc_hierarchy() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let db = ferrum::db::connect("sqlite::memory:").await.unwrap();
    ferrum::auth::apply_migrations(&db).await.unwrap();
    let mut config = AuthConfig::development();
    config.with_loopback_address(&format!("127.0.0.1:{port}"));
    let auth = AuthService::new(db.clone(), config);
    auth.create_user("staff@example.com", "correct-horse-battery", true, true)
        .await
        .unwrap();
    let registry = AdminRegistry::new().add::<HtmlAppointment>();
    let app = ferrum::App::new(db)
        .with_auth(auth)
        .with_admin(registry)
        .register::<HtmlAppointment>();
    app.bootstrap_schema().await.unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app.into_router()).await });
    let call = |method: &'static str, path: String, body: Option<String>, extra: String| {
        tokio::task::spawn_blocking(move || http(port, method, &path, body.as_deref(), &extra))
    };
    let login_page = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let token = cookie_value(&login_page.headers, "ferrum_login_csrf");
    let signed = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &token),
            ("email", "staff@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Origin: http://127.0.0.1:{port}\r\nCookie: ferrum_login_csrf={token}\r\n"),
    )
    .await
    .unwrap();
    let session = cookie_value(&signed.headers, "ferrum_session");
    let cookie = format!("Cookie: ferrum_session={session}\r\n");
    let form_page = call(
        "GET",
        "/admin/models/html_appointments/new".into(),
        None,
        cookie.clone(),
    )
    .await
    .unwrap();
    assert_eq!(form_page.status, 200);
    assert!(form_page.body.contains("type=\"date\""));
    assert!(form_page.body.contains("type=\"time\""));
    assert!(form_page.body.contains("type=\"datetime-local\""));
    assert!(form_page.body.contains("UTC"));
    assert!(form_page.body.contains("for=\"event_date\""));
    let csrf = field_value(&form_page.body, "csrf_token");
    let invalid = call(
        "POST",
        "/admin/models/html_appointments/new".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("title", "Falsch"),
            ("event_date", "2023-02-29"),
            ("start_time", ""),
            ("created_at", "2026-11-07T13:30"),
            ("save", "detail"),
        ])),
        format!("Origin: http://127.0.0.1:{port}\r\n{cookie}"),
    )
    .await
    .unwrap();
    assert_ne!(invalid.status, 303);
    assert!(invalid.body.contains("2023-02-29"));
    let created = call(
        "POST",
        "/admin/models/html_appointments/new".into(),
        Some(form(&[
            ("csrf_token", &csrf),
            ("title", "Fruehstueck"),
            ("event_date", "2026-11-07"),
            ("start_time", "08:15"),
            ("created_at", "2026-11-07T07:15"),
            ("save", "detail"),
        ])),
        format!("Origin: http://127.0.0.1:{port}\r\n{cookie}"),
    )
    .await
    .unwrap();
    assert_eq!(created.status, 303, "{}", created.body);
    let list = call(
        "GET",
        "/admin/models/html_appointments".into(),
        None,
        cookie.clone(),
    )
    .await
    .unwrap();
    assert!(list.body.contains("Fruehstueck"));
    assert!(list.body.contains("2026"));
    assert!(list.body.contains("year=2026"));
    let year = call(
        "GET",
        "/admin/models/html_appointments?year=2026".into(),
        None,
        cookie.clone(),
    )
    .await
    .unwrap();
    assert!(year.body.contains("Fruehstueck"));
    let empty = call(
        "GET",
        "/admin/models/html_appointments?year=1999".into(),
        None,
        cookie,
    )
    .await
    .unwrap();
    assert!(!empty.body.contains("Fruehstueck"));
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
