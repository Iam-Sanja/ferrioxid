//! Kleine, typisierte Helfer für Listen und Schreibprüfungen.
//!
//! Sie erzeugen keinen zweiten Router. Der Aufrufer bleibt eine Axum-Funktion
//! und übergibt den `QueryScope` der Policy. Ein Query-String kann keinen Scope bauen.

use fexu_core::{ColumnSpec, FexuError, QueryScope};
use fexu_db::driver::ConnectionTrait;
use fexu_db::{get_record, query_admin_page, AdminPage, AdminPageRequest};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageQuery {
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug)]
pub struct CrudError {
    pub code: &'static str,
    pub detail: &'static str,
}

impl CrudError {
    const fn new(code: &'static str, detail: &'static str) -> Self {
        Self { code, detail }
    }
}

pub fn page_window(query: PageQuery) -> Result<(u64, u64), CrudError> {
    if query.page == 0 || query.page > 10_000 || !(1..=100).contains(&query.page_size) {
        return Err(CrudError::new(
            "crud.page",
            "page starts at 1 and page_size is from 1 to 100",
        ));
    }
    let offset = u64::from(query.page - 1) * u64::from(query.page_size);
    Ok((offset, u64::from(query.page_size)))
}

/// Lokaler Pfad plus Seitenparameter. Externe Ziele und Scope-Schlüssel werden abgelehnt.
pub fn pagination_href(path: &str, page: u32, extra: &[(&str, &str)]) -> Result<String, CrudError> {
    if page == 0 || page > 10_000 || !local_path(path) {
        return Err(CrudError::new(
            "crud.url",
            "pagination stays on a local path",
        ));
    }
    let mut href = String::from(path);
    let mut joiner = if path.contains('?') { '&' } else { '?' };
    for (key, value) in extra {
        if !safe_key(key) || matches!(*key, "scope" | "csrf" | "token" | "csrf_token") {
            return Err(CrudError::new(
                "crud.url",
                "pagination does not accept a scope or token",
            ));
        }
        href.push(joiner);
        href.push_str(key);
        href.push('=');
        href.push_str(&encode(value));
        joiner = '&';
    }
    href.push(joiner);
    href.push_str("page=");
    href.push_str(&page.to_string());
    Ok(href)
}

pub fn require_scope(scope: &QueryScope) -> Result<(), CrudError> {
    match scope {
        QueryScope::None => Err(CrudError::new(
            "crud.scope",
            "the object policy returned no rows",
        )),
        QueryScope::All | QueryScope::Equals { .. } => Ok(()),
    }
}

pub fn render_notice(text: &str) -> String {
    format!("<p role=\"status\">{}</p>", escape(text))
}

pub async fn scoped_list(
    db: &impl ConnectionTrait,
    table: &str,
    columns: &[ColumnSpec],
    scope: &QueryScope,
    query: PageQuery,
) -> Result<AdminPage, FexuError> {
    require_scope(scope)
        .map_err(|_| FexuError::Validation("the object policy returned no rows".into()))?;
    let (offset, limit) = page_window(query).map_err(|_| {
        FexuError::Validation("page starts at 1 and page_size is from 1 to 100".into())
    })?;
    query_admin_page(
        db,
        table,
        columns,
        AdminPageRequest {
            search_columns: &[],
            search: None,
            sort_column: "id",
            descending: false,
            equal_filters: &[],
            bounds: &[],
            offset,
            limit,
            scope,
        },
    )
    .await
}

pub async fn scoped_get(
    db: &impl ConnectionTrait,
    table: &str,
    columns: &[ColumnSpec],
    id: Uuid,
    scope: &QueryScope,
) -> Result<Option<Value>, FexuError> {
    require_scope(scope)
        .map_err(|_| FexuError::Validation("the object policy returned no rows".into()))?;
    get_record(db, table, columns, id, scope).await
}

fn local_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains('\\')
        && !path.contains("..")
        && !path.contains("://")
        && path.len() <= 200
        && !path.chars().any(|char| char.is_control())
}

fn safe_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && key.len() <= 32
}

fn encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                output.push(byte as char)
            }
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

fn escape(value: &str) -> String {
    let mut output = String::new();
    for char in value.chars() {
        match char {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(char),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{pagination_href, render_notice, require_scope, PageQuery};
    use fexu_core::QueryScope;

    #[test]
    fn links_stay_local_and_scopes_are_not_request_data() {
        assert_eq!(
            pagination_href("/notes", 2, &[("q", "a <b>")]).unwrap(),
            "/notes?q=a%20%3Cb%3E&page=2"
        );
        assert!(pagination_href("https://evil.example", 1, &[]).is_err());
        assert!(pagination_href("//evil.example", 1, &[]).is_err());
        assert!(pagination_href("/notes", 1, &[("scope", "all")]).is_err());
        assert!(pagination_href("/notes/../secret", 1, &[]).is_err());
        assert!(require_scope(&QueryScope::None).is_err());
        assert!(require_scope(&QueryScope::All).is_ok());
        let notice = render_notice("<script>");
        assert!(notice.contains("&lt;script&gt;"));
        assert!(!notice.contains("<script>"));
        assert!(super::page_window(PageQuery {
            page: 0,
            page_size: 10
        })
        .is_err());
        assert_eq!(
            super::page_window(PageQuery {
                page: 2,
                page_size: 25
            })
            .unwrap(),
            (25, 25)
        );
    }
}
