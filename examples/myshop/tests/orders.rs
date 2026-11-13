//! Transaktionale Bestellung und Positionen auf einer temporären SQLite-Datei.

use fexu::admin::{
    fail_next_insert, inline_form_groups, save_with_children, AdminRegistry, SaveResult,
};
use fexu::auth::{AuthConfig, AuthService, AuthUser};
use fexu::db::ModelRepository;
use fexu::migrations::apply_directory;
use fexu::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use myshop::models::order::Order;
use myshop::models::order_line::OrderLine;
use serde_json::{Map, Value};
use std::collections::HashMap;
use uuid::Uuid;

fn registry() -> AdminRegistry {
    AdminRegistry::new().add::<Order>().add::<OrderLine>()
}

async fn ready() -> (fexu::sea_orm::DatabaseConnection, AuthService, AuthUser) {
    let db = fexu::db::connect("sqlite::memory:").await.unwrap();
    fexu::auth::apply_migrations(&db).await.unwrap();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    apply_directory(&db, &directory, false).await.unwrap();
    let auth = AuthService::new(db.clone(), AuthConfig::development());
    let user = auth
        .create_user("root@example.com", "correct-horse-battery", true, true)
        .await
        .unwrap();
    let actor = AuthUser {
        id: user.id,
        email: user.email,
        is_active: true,
        is_staff: true,
        is_superuser: true,
        session_id: Uuid::new_v4(),
    };
    (db, auth, actor)
}

fn form(rows: &[(&str, &str, &str)]) -> HashMap<String, String> {
    let mut values = HashMap::new();
    values.insert("customer".into(), "Ada".into());
    values.insert("ordered_on".into(), "2026-11-07".into());
    values.insert("status".into(), "Offen".into());
    for (index, (product, quantity, price)) in rows.iter().enumerate() {
        values.insert(format!("inline_lines_{index}_product"), (*product).into());
        values.insert(format!("inline_lines_{index}_quantity"), (*quantity).into());
        values.insert(format!("inline_lines_{index}_unit_price"), (*price).into());
    }
    values
}

fn parent_body() -> Map<String, Value> {
    Map::from_iter([
        ("customer".into(), Value::String("Ada".into())),
        ("ordered_on".into(), Value::String("2026-11-07".into())),
        ("status".into(), Value::String("Offen".into())),
    ])
}

#[tokio::test]
async fn parent_and_children_commit_or_roll_back_together() {
    let (db, auth, actor) = ready().await;
    let models = registry();
    let parent = models.find("orders").unwrap();
    let invalid = save_with_children(
        &db,
        &auth,
        &actor,
        parent.as_ref(),
        |key| models.find(key).cloned(),
        None,
        parent_body(),
        &form(&[("Apfel", "2", "199"), ("Birne", "", "249")]),
    )
    .await;
    assert!(matches!(invalid, SaveResult::Invalid(_)));
    assert!(Order::list(&db).await.unwrap().data.is_empty());

    let saved = save_with_children(
        &db,
        &auth,
        &actor,
        parent.as_ref(),
        |key| models.find(key).cloned(),
        None,
        parent_body(),
        &form(&[("Apfel", "2", "199"), ("Birne", "1", "249")]),
    )
    .await;
    let SaveResult::Saved(order_id) = saved else {
        panic!("expected save");
    };
    assert_eq!(OrderLine::list(&db).await.unwrap().data.len(), 2);

    let duplicate = {
        let lines = OrderLine::list(&db).await.unwrap().data;
        let mut posted = form(&[("Apfel", "3", "199")]);
        posted.insert("inline_lines_0_id".into(), lines[0].id.to_string());
        posted.insert("inline_lines_1_id".into(), lines[0].id.to_string());
        posted.insert("inline_lines_1_product".into(), "Birne".into());
        posted.insert("inline_lines_1_quantity".into(), "1".into());
        posted.insert("inline_lines_1_unit_price".into(), "249".into());
        save_with_children(
            &db,
            &auth,
            &actor,
            parent.as_ref(),
            |key| models.find(key).cloned(),
            Some(order_id),
            parent_body(),
            &posted,
        )
        .await
    };
    assert!(matches!(duplicate, SaveResult::Invalid(_)));
    assert_eq!(OrderLine::list(&db).await.unwrap().data.len(), 2);

    let values = Map::new();
    let (_, basis) = inline_form_groups(
        &db,
        &auth,
        &actor,
        parent.as_ref(),
        |key| models.find(key).cloned(),
        Some(order_id),
        &values,
    )
    .await;
    let lines = OrderLine::list(&db).await.unwrap().data;
    let mut posted = form(&[("Apfel", "3", "199"), ("Birne", "1", "249")]);
    posted.insert("basis".into(), basis);
    posted.insert("inline_lines_0_id".into(), lines[0].id.to_string());
    posted.insert("inline_lines_1_id".into(), lines[1].id.to_string());
    let updated = save_with_children(
        &db,
        &auth,
        &actor,
        parent.as_ref(),
        |key| models.find(key).cloned(),
        Some(order_id),
        parent_body(),
        &posted,
    )
    .await;
    assert!(matches!(updated, SaveResult::Saved(_)));
    let changed = OrderLine::list(&db).await.unwrap().data;
    assert!(changed.iter().any(|line| line.quantity == 3));

    let second = save_with_children(
        &db,
        &auth,
        &actor,
        parent.as_ref(),
        |key| models.find(key).cloned(),
        None,
        parent_body(),
        &form(&[("Pflaume", "1", "100")]),
    )
    .await;
    let SaveResult::Saved(second_id) = second else {
        panic!("second order");
    };
    let foreign_line = OrderLine::list(&db)
        .await
        .unwrap()
        .data
        .into_iter()
        .find(|line| line.order_id == second_id)
        .unwrap();
    let (_, basis) = inline_form_groups(
        &db,
        &auth,
        &actor,
        parent.as_ref(),
        |key| models.find(key).cloned(),
        Some(order_id),
        &Map::new(),
    )
    .await;
    let mut stolen = form(&[("Apfel", "3", "199")]);
    stolen.insert("basis".into(), basis);
    stolen.insert("inline_lines_0_id".into(), foreign_line.id.to_string());
    let rejected = save_with_children(
        &db,
        &auth,
        &actor,
        parent.as_ref(),
        |key| models.find(key).cloned(),
        Some(order_id),
        parent_body(),
        &stolen,
    )
    .await;
    assert!(matches!(rejected, SaveResult::Invalid(_)));
    let still = OrderLine::get(&db, foreign_line.id).await.unwrap().unwrap();
    assert_eq!(still.order_id, second_id);

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT \"operation_id\", \"summary\" FROM \"_fexu_admin_audit\" WHERE \"operation_id\" IS NOT NULL",
        ))
        .await
        .unwrap();
    assert!(rows.len() >= 2);
    let operation = rows[0].try_get::<String>("", "operation_id").unwrap();
    assert!(rows.iter().any(|row| {
        row.try_get::<String>("", "operation_id").unwrap() == operation
            && !row
                .try_get::<String>("", "summary")
                .unwrap()
                .contains("password")
    }));
}

#[tokio::test]
async fn limits_permissions_and_audit_failure_leave_no_partial_order() {
    let (db, auth, actor) = ready().await;
    let models = registry();
    let parent = models.find("orders").unwrap();
    let mut too_many = form(&[]);
    for index in 0..21 {
        too_many.insert(format!("inline_lines_{index}_product"), format!("P{index}"));
        too_many.insert(format!("inline_lines_{index}_quantity"), "1".into());
        too_many.insert(format!("inline_lines_{index}_unit_price"), "1".into());
    }
    assert!(matches!(
        save_with_children(
            &db,
            &auth,
            &actor,
            parent.as_ref(),
            |key| models.find(key).cloned(),
            None,
            parent_body(),
            &too_many
        )
        .await,
        SaveResult::Invalid(_)
    ));
    let mut unknown = form(&[("Apfel", "1", "10")]);
    unknown.insert("inline_lines_0_order_id".into(), Uuid::nil().to_string());
    assert!(matches!(
        save_with_children(
            &db,
            &auth,
            &actor,
            parent.as_ref(),
            |key| models.find(key).cloned(),
            None,
            parent_body(),
            &unknown
        )
        .await,
        SaveResult::Invalid(_)
    ));
    let staff = auth
        .create_user("clerk@example.com", "correct-horse-battery", true, false)
        .await
        .unwrap();
    let clerk = AuthUser {
        id: staff.id,
        email: staff.email,
        is_active: true,
        is_staff: true,
        is_superuser: false,
        session_id: Uuid::new_v4(),
    };
    assert!(matches!(
        save_with_children(
            &db,
            &auth,
            &clerk,
            parent.as_ref(),
            |key| models.find(key).cloned(),
            None,
            parent_body(),
            &form(&[("Apfel", "1", "10")])
        )
        .await,
        SaveResult::Denied
    ));
    fail_next_insert();
    let failed = save_with_children(
        &db,
        &auth,
        &actor,
        parent.as_ref(),
        |key| models.find(key).cloned(),
        None,
        parent_body(),
        &form(&[("Apfel", "1", "10")]),
    )
    .await;
    assert!(matches!(
        failed,
        SaveResult::Invalid(_) | SaveResult::Conflict(_)
    ));
    assert!(Order::list(&db).await.unwrap().data.is_empty());
}

#[tokio::test]
async fn a_stale_form_conflicts_and_keeps_the_stored_quantity() {
    let (db, auth, actor) = ready().await;
    let models = registry();
    let parent = models.find("orders").unwrap();
    let SaveResult::Saved(order_id) = save_with_children(
        &db,
        &auth,
        &actor,
        parent.as_ref(),
        |key| models.find(key).cloned(),
        None,
        parent_body(),
        &form(&[("Apfel", "2", "199")]),
    )
    .await
    else {
        panic!("create");
    };
    let line = OrderLine::list(&db).await.unwrap().data.remove(0);
    let mut posted = form(&[("Apfel", "9", "199")]);
    posted.insert("basis".into(), "0000000000000000".into());
    posted.insert("inline_lines_0_id".into(), line.id.to_string());
    let conflict = save_with_children(
        &db,
        &auth,
        &actor,
        parent.as_ref(),
        |key| models.find(key).cloned(),
        Some(order_id),
        parent_body(),
        &posted,
    )
    .await;
    assert!(matches!(conflict, SaveResult::Conflict(_)));
    assert_eq!(
        OrderLine::get(&db, line.id)
            .await
            .unwrap()
            .unwrap()
            .quantity,
        2
    );
}

#[tokio::test]
async fn the_order_form_adds_a_row_without_javascript_or_a_write() {
    let (db, auth, _actor) = ready().await;
    let app = fexu::App::new(db.clone())
        .with_auth(auth)
        .with_admin(registry());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { axum::serve(listener, app.into_router()).await });
    let ask = |method: &'static str, path: String, body: Option<String>, extra: String| {
        tokio::task::spawn_blocking(move || http(port, method, &path, body.as_deref(), &extra))
    };
    let login_page = ask("GET", "/admin/login".into(), None, String::new())
        .await
        .unwrap();
    let login_csrf = cookie_value(&login_page.headers, "fexu_login_csrf");
    let signed = ask(
        "POST",
        "/admin/login".into(),
        Some(encoded(&[
            ("csrf_token", &login_csrf),
            ("email", "root@example.com"),
            ("password", "correct-horse-battery"),
            ("next", "/admin"),
        ])),
        format!("Cookie: fexu_login_csrf={login_csrf}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(signed.status, 303);
    let session = cookie_value(&signed.headers, "fexu_session");
    let cookie = format!("Cookie: fexu_session={session}\r\n");
    let page = ask(
        "GET",
        "/admin/models/orders/new".into(),
        None,
        cookie.clone(),
    )
    .await
    .unwrap();
    assert_eq!(page.status, 200);
    assert!(page.body.contains("name=\"inline_lines_0_product\""));
    assert!(page.body.contains("name=\"save\" value=\"rows\""));
    assert!(page.body.contains("<fieldset"));
    assert!(page.body.contains("admin.js\" defer"));
    let csrf = field_value(&page.body, "csrf_token");
    let rows = ask(
        "POST",
        "/admin/models/orders/new".into(),
        Some(encoded(&[
            ("csrf_token", &csrf),
            ("customer", "Ada"),
            ("ordered_on", "2026-11-07"),
            ("status", "Offen"),
            ("inline_lines_0_product", "Apfel"),
            ("inline_lines_0_quantity", "2"),
            ("inline_lines_0_unit_price", "199"),
            ("save", "rows"),
        ])),
        cookie.clone(),
    )
    .await
    .unwrap();
    assert_eq!(rows.status, 303);
    assert!(location(&rows.headers).contains("draft="));
    assert!(Order::list(&db).await.unwrap().data.is_empty());
    let draft = ask("GET", location(&rows.headers), None, cookie.clone())
        .await
        .unwrap();
    assert!(draft.body.contains("Apfel"));
    let csrf = field_value(&draft.body, "csrf_token");
    let saved = ask(
        "POST",
        "/admin/models/orders/new".into(),
        Some(encoded(&[
            ("csrf_token", &csrf),
            ("customer", "Ada"),
            ("ordered_on", "2026-11-07"),
            ("status", "Offen"),
            ("inline_lines_0_product", "Apfel"),
            ("inline_lines_0_quantity", "2"),
            ("inline_lines_0_unit_price", "199"),
            ("inline_lines_1_product", "Birne"),
            ("inline_lines_1_quantity", "1"),
            ("inline_lines_1_unit_price", "249"),
            ("save", "detail"),
        ])),
        cookie,
    )
    .await
    .unwrap();
    assert_eq!(saved.status, 303);
    assert_eq!(OrderLine::list(&db).await.unwrap().data.len(), 2);
    server.abort();
}

struct Reply {
    status: u16,
    headers: String,
    body: String,
}

fn http(port: u16, method: &str, path: &str, body: Option<&str>, extra: &str) -> Reply {
    use std::io::{Read, Write};
    use std::net::TcpStream;
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
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra}\r\n{data}",
        data.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => response.extend_from_slice(&buffer[..count]),
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                break;
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
        body: raw_body.to_owned(),
    }
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

fn encoded(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{key}={}", encode(value)))
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
