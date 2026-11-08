//! Erweiterter Admin: eigene Seite, Aktion, Filter, Widget und Vorlage.

use ferrioxid::admin::{
    ActionContext, ActionError, ActionHandler, AdminAction, AdminFilter, AdminRegistry, AdminView,
    AdminWidget, ExtensionError, FilterOption, ViewHandler, ViewPage, ViewRequest,
};
use ferrioxid::auth::{AuthConfig, AuthService, AuthUser, ObjectPolicy};
use ferrioxid::core::{QueryScope, ScopeValue};
use ferrioxid::db::ModelRepository;
use ferrioxid::prelude::*;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "extension_tickets",
    api = "/api/v1/extension_tickets",
    permissions = "tickets",
    object_policy = true,
    admin = true,
    admin_label = "Tickets",
    admin_permission = "tickets",
    admin_per_page = 1
)]
struct Ticket {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 80)]
    #[admin(list_display, searchable, label = "Titel")]
    title: String,
    #[field(required, min_length = 1, max_length = 40)]
    #[admin(list_display, label = "Status")]
    status: String,
    #[field(required, min_length = 1, max_length = 200)]
    #[admin(label = "Notiz")]
    note: String,
    #[field(required, min_length = 3, max_length = 120)]
    #[admin(label = "E-Mail")]
    email: String,
    #[field(min = 0)]
    #[admin(label = "Betrag")]
    amount: i64,
    #[field(required, min_length = 1, max_length = 80)]
    #[admin(hidden)]
    owner: String,
}

struct TicketPolicy;

impl ObjectPolicy<Ticket> for TicketPolicy {
    fn can_read(user: &AuthUser, object: &Ticket) -> bool {
        object.owner == user.id.to_string()
    }

    fn can_update(user: &AuthUser, object: &Ticket) -> bool {
        Self::can_read(user, object)
    }

    fn can_delete(user: &AuthUser, object: &Ticket) -> bool {
        Self::can_read(user, object)
    }

    fn read_scope(user: &AuthUser) -> QueryScope {
        QueryScope::Equals {
            column: "owner",
            value: ScopeValue::Text(user.id.to_string()),
        }
    }

    fn can_create(user: &AuthUser, input: &<Ticket as ModelRepository>::Create) -> bool {
        input.owner == user.id.to_string()
    }
}

struct Report;

#[ferrioxid::async_trait::async_trait]
impl ViewHandler for Report {
    async fn handle(&self, request: &ViewRequest<'_>) -> Result<ViewPage, ExtensionError> {
        let mut data = serde_json::Map::new();
        data.insert(
            "body".to_owned(),
            serde_json::json!("<script>alert(1)</script>"),
        );
        let mut page = ViewPage::new(data);
        if request.form.contains_key("csrf_token") {
            page.notice = "gespeichert".to_owned();
        }
        Ok(page)
    }
}

struct CloseTicket;

#[ferrioxid::async_trait::async_trait]
impl ActionHandler for CloseTicket {
    async fn apply(&self, context: &mut ActionContext) -> Result<(), ActionError> {
        for id in context.ids() {
            context.set_text(id, "status", "closed")?;
        }
        Ok(())
    }
}

struct Hostile;

#[ferrioxid::async_trait::async_trait]
impl ActionHandler for Hostile {
    async fn apply(&self, context: &mut ActionContext) -> Result<(), ActionError> {
        let id = context.ids()[0];
        context.set_text(id, "email", "stolen@example.com")?;
        Ok(())
    }
}

const STATUS: &[FilterOption] = &[
    FilterOption::new("open", "Offen", "Open"),
    FilterOption::new("closed", "Geschlossen", "Closed"),
];

#[tokio::test]
async fn extensions_keep_auth_scopes_and_validation() {
    assert!(AdminRegistry::new()
        .view(AdminView::new("directory"))
        .is_err());
    assert!(AdminRegistry::new()
        .template("../secret.html", "{% extends \"base.html\" %}")
        .is_err());

    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    ferrioxid::auth::apply_migrations(&db).await.unwrap();
    let auth = AuthService::new(db.clone(), AuthConfig::development());
    let staff = auth
        .create_user("staff@example.com", "correct-horse-battery", true, false)
        .await
        .unwrap();
    let other = auth
        .create_user("other@example.com", "correct-horse-battery", true, false)
        .await
        .unwrap();
    auth.create_user("none@example.com", "correct-horse-battery", true, false)
        .await
        .unwrap();
    auth.create_user("root@example.com", "correct-horse-battery", true, true)
        .await
        .unwrap();
    auth.ensure_role("desk").await.unwrap();
    for code in ["tickets.read", "tickets.update"] {
        auth.ensure_permission(code).await.unwrap();
        auth.grant_permission("desk", code).await.unwrap();
    }
    auth.assign_role("staff@example.com", "desk").await.unwrap();

    let registry = AdminRegistry::new()
        .add_policy::<Ticket, TicketPolicy>()
        .template(
            "reports.html",
            "{% extends \"base.html\" %}{% block content %}<p>{{ body }}</p>{% endblock %}",
        )
        .unwrap()
        .view(
            AdminView::new("reports")
                .titles("Berichte", "Reports")
                .permission("tickets.read")
                .navigation(true)
                .template("reports.html")
                .on_get(Report)
                .on_post(Report),
        )
        .unwrap()
        .try_action(
            AdminAction::custom("extension_tickets", "close_ticket", Arc::new(CloseTicket))
                .labels("Schließen", "Close")
                .writes(&["status"]),
        )
        .unwrap()
        .try_action(
            AdminAction::custom("extension_tickets", "hostile", Arc::new(Hostile))
                .labels("Fremd", "Foreign")
                .writes(&["status"]),
        )
        .unwrap()
        .filter(AdminFilter::choice(
            "extension_tickets",
            "ticket_status",
            "status",
            STATUS,
        ))
        .unwrap()
        .filter(AdminFilter::equals(
            "extension_tickets",
            "title_exact",
            "title",
        ))
        .unwrap()
        .widget(AdminWidget::textarea("extension_tickets", "note", 80))
        .unwrap()
        .widget(AdminWidget::email("extension_tickets", "email"))
        .unwrap()
        .widget(AdminWidget::number(
            "extension_tickets",
            "amount",
            Some(0),
            Some(1000),
        ))
        .unwrap()
        .widget(AdminWidget::readonly("extension_tickets", "status"))
        .unwrap()
        .seal()
        .unwrap();
    assert!(AdminRegistry::new()
        .add_policy::<Ticket, TicketPolicy>()
        .widget(AdminWidget::email("extension_tickets", "owner"))
        .unwrap()
        .seal()
        .is_err());

    let app = ferrioxid::App::new(db.clone())
        .with_auth(auth)
        .with_admin(registry)
        .register_policy::<Ticket, TicketPolicy>();
    app.bootstrap_schema().await.unwrap();
    let alpha = Ticket::create(
        &db,
        TicketCreate {
            title: "Alpha".into(),
            status: "open".into(),
            note: "<script>alert(1)</script>".into(),
            email: "alpha@example.com".into(),
            amount: 10,
            owner: staff.id.to_string(),
        },
    )
    .await
    .unwrap();
    Ticket::create(
        &db,
        TicketCreate {
            title: "Beta".into(),
            status: "open".into(),
            note: "beta".into(),
            email: "beta@example.com".into(),
            amount: 11,
            owner: staff.id.to_string(),
        },
    )
    .await
    .unwrap();
    Ticket::create(
        &db,
        TicketCreate {
            title: "Gamma".into(),
            status: "open".into(),
            note: "gamma".into(),
            email: "gamma@example.com".into(),
            amount: 12,
            owner: other.id.to_string(),
        },
    )
    .await
    .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app.into_router()).await });
    let call = |method: &'static str, path: String, body: Option<String>, extra: String| {
        tokio::task::spawn_blocking(move || http(port, method, &path, body.as_deref(), &extra))
    };
    let anonymous = call("GET", "/admin/pages/reports".into(), None, String::new())
        .await
        .unwrap();
    assert_eq!(anonymous.status, 303);
    assert!(location(&anonymous.headers).starts_with("/admin/login"));
    let missing = call("GET", "/admin/pages/missing".into(), None, String::new())
        .await
        .unwrap();
    assert!(missing.status == 303 || missing.status == 404);

    let staff_session = login(&call, "staff@example.com").await;
    let none_session = login(&call, "none@example.com").await;
    let denied = call(
        "GET",
        "/admin/pages/reports".into(),
        None,
        cookie(&none_session),
    )
    .await
    .unwrap();
    assert_eq!(denied.status, 403);
    let directory = call(
        "GET",
        "/admin/directory/users".into(),
        None,
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(directory.status, 403);
    let absent = call(
        "GET",
        "/admin/pages/missing".into(),
        None,
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(absent.status, 404);
    let report = call(
        "GET",
        "/admin/pages/reports".into(),
        None,
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(report.status, 200);
    assert!(report.body.contains("Berichte"));
    assert!(report.body.contains("&lt;script&gt;"));
    assert!(!report.body.contains("<script>"));
    let bare = call(
        "POST",
        "/admin/pages/reports".into(),
        Some("refresh=1".into()),
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(bare.status, 403);
    let csrf = field_value(&report.body, "csrf_token");
    let posted = call(
        "POST",
        "/admin/pages/reports".into(),
        Some(form(&[("csrf_token", &csrf)])),
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(posted.status, 200, "{}", posted.body);
    assert!(posted.body.contains("gespeichert"));

    let list = call(
        "GET",
        "/admin/models/extension_tickets?filter.ticket_status=open".into(),
        None,
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(list.status, 200, "{}", list.body);
    assert!(!list.body.contains("Gamma"));
    let second = call(
        "GET",
        "/admin/models/extension_tickets?filter.ticket_status=open&page=2".into(),
        None,
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(second.status, 200, "{}", second.body);
    assert!(!second.body.contains("Gamma"));
    let both = format!("{}{}", list.body, second.body);
    assert!(both.contains("Alpha"));
    assert!(both.contains("Beta"));
    let exact = call(
        "GET",
        "/admin/models/extension_tickets?filter.title_exact=Alpha".into(),
        None,
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert!(exact.body.contains("Alpha"));
    assert!(!exact.body.contains("Beta"));
    let injected = call(
        "GET",
        "/admin/models/extension_tickets?filter.ticket_status=open%27%20OR%20%271%27%3D%271".into(),
        None,
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(injected.status, 422);
    let literal = call(
        "GET",
        "/admin/models/extension_tickets?filter.title_exact=Alpha%27%20OR%20%271%27%3D%271".into(),
        None,
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(literal.status, 200, "{}", literal.body);
    assert!(literal.body.contains("Treffer: 0") || literal.body.contains("Results: 0"));
    assert!(!literal.body.contains("Gamma"));
    let unknown = call(
        "GET",
        "/admin/models/extension_tickets?filter.missing=1".into(),
        None,
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(unknown.status, 422);

    let edit = call(
        "GET",
        format!("/admin/models/extension_tickets/{}/edit", alpha.id),
        None,
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(edit.status, 200, "{}", edit.body);
    assert!(edit.body.contains("<textarea"));
    assert!(edit.body.contains("&lt;script&gt;"));
    assert!(!edit.body.contains("<script>"));
    let form_csrf = field_value(&edit.body, "csrf_token");
    let bad_email = call(
        "POST",
        format!("/admin/models/extension_tickets/{}/edit", alpha.id),
        Some(form(&[
            ("csrf_token", &form_csrf),
            ("save", "detail"),
            ("title", "Alpha"),
            ("note", "Hinweis"),
            ("email", "not-an-email"),
            ("amount", "10"),
        ])),
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_ne!(bad_email.status, 303);
    let stored = Ticket::get(&db, alpha.id).await.unwrap().unwrap();
    assert_eq!(stored.email, "alpha@example.com");
    let bad_amount = call(
        "POST",
        format!("/admin/models/extension_tickets/{}/edit", alpha.id),
        Some(form(&[
            ("csrf_token", &form_csrf),
            ("save", "detail"),
            ("title", "Alpha"),
            ("note", "Hinweis"),
            ("email", "alpha@example.com"),
            ("amount", "1001"),
        ])),
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_ne!(bad_amount.status, 303);
    assert_eq!(
        Ticket::get(&db, alpha.id).await.unwrap().unwrap().amount,
        10
    );
    let locked = call(
        "POST",
        format!("/admin/models/extension_tickets/{}/edit", alpha.id),
        Some(form(&[
            ("csrf_token", &form_csrf),
            ("save", "detail"),
            ("title", "Alpha"),
            ("status", "hacked"),
            ("note", "Hinweis"),
            ("email", "alpha@example.com"),
            ("amount", "10"),
        ])),
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_ne!(locked.status, 303);
    assert_eq!(
        Ticket::get(&db, alpha.id).await.unwrap().unwrap().status,
        "open"
    );

    let list_csrf = field_value(&list.body, "csrf_token");
    let rejected = call(
        "POST",
        "/admin/models/extension_tickets/bulk".into(),
        Some(form(&[
            ("csrf_token", &list_csrf),
            ("action", "drop_table"),
            ("ids", &alpha.id.to_string()),
        ])),
        cookie(&staff_session),
    )
    .await
    .unwrap();
    assert_eq!(rejected.status, 422);
    let hostile = stage_action(&call, &staff_session, &list_csrf, "hostile", &alpha.id).await;
    assert_eq!(hostile, 422);
    let after_hostile = Ticket::get(&db, alpha.id).await.unwrap().unwrap();
    assert_eq!(after_hostile.status, "open");
    assert_eq!(after_hostile.email, "alpha@example.com");
    run_action(&call, &staff_session, &list_csrf, "close_ticket", &alpha.id).await;
    assert_eq!(
        Ticket::get(&db, alpha.id).await.unwrap().unwrap().status,
        "closed"
    );
}

async fn stage_action(
    call: &impl Fn(&'static str, String, Option<String>, String) -> tokio::task::JoinHandle<Reply>,
    session: &str,
    csrf: &str,
    action: &str,
    id: &Uuid,
) -> u16 {
    let staged = call(
        "POST",
        "/admin/models/extension_tickets/bulk".into(),
        Some(form(&[
            ("csrf_token", csrf),
            ("action", action),
            ("ids", &id.to_string()),
        ])),
        cookie(session),
    )
    .await
    .unwrap();
    assert_eq!(staged.status, 303, "{}", staged.body);
    let path = location(&staged.headers);
    let confirm = call("GET", path.clone(), None, cookie(session))
        .await
        .unwrap();
    let token = field_value(&confirm.body, "csrf_token");
    let applied = call(
        "POST",
        path,
        Some(form(&[("csrf_token", &token), ("confirm", "1")])),
        cookie(session),
    )
    .await
    .unwrap();
    applied.status
}

async fn run_action(
    call: &impl Fn(&'static str, String, Option<String>, String) -> tokio::task::JoinHandle<Reply>,
    session: &str,
    csrf: &str,
    action: &str,
    id: &Uuid,
) {
    assert_eq!(stage_action(call, session, csrf, action, id).await, 303);
}

async fn login(
    call: &impl Fn(&'static str, String, Option<String>, String) -> tokio::task::JoinHandle<Reply>,
    email: &str,
) -> String {
    let login = call("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let token = cookie_value(&login.headers, "ferrioxid_login_csrf");
    let signed = call(
        "POST",
        "/admin/login".into(),
        Some(form(&[
            ("csrf_token", &token),
            ("email", email),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: ferrioxid_login_csrf={token}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(signed.status, 303, "{}", signed.body);
    cookie_value(&signed.headers, "ferrioxid_session")
}

struct Reply {
    status: u16,
    headers: String,
    body: String,
}

fn http(port: u16, method: &str, path: &str, body: Option<&str>, extra: &str) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .unwrap();
    let data = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n{extra}\r\n{data}",
        data.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => response.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
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
        body: raw_body.to_owned(),
    }
}

fn cookie(token: &str) -> String {
    format!("Cookie: ferrioxid_session={token}\r\n")
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

fn location(headers: &str) -> String {
    headers
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("location:"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| value.trim().to_owned())
        .unwrap_or_default()
}

fn field_value(body: &str, name: &str) -> String {
    let marker = format!("name=\"{name}\" value=\"");
    let start = body.find(&marker).unwrap() + marker.len();
    body[start..].split('"').next().unwrap().to_owned()
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", key, value.replace('&', "%26").replace(' ', "+")))
        .collect::<Vec<_>>()
        .join("&")
}
