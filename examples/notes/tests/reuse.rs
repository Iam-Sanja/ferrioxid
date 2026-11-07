use axum::body::Body;
use axum::routing::get;
use ferrum::apps::{AppConfig, AppError, FerrumApplication, Registration};
use ferrum::urls::UrlSet;
use http_body_util::BodyExt;
use notes::NotesApp;
use tower::ServiceExt;

struct TagsApp;

impl FerrumApplication for TagsApp {
    fn config(&self) -> AppConfig {
        AppConfig::new("tags")
            .prefix("/tags")
            .depends_on(&["notes"])
    }

    fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), AppError> {
        registry.routes(UrlSet::new().route("tags.list", "/items", get(tags))?)
    }
}

async fn tags() -> axum::response::Response {
    ferrum::views::json(&ferrum::serde_json::json!({"app": "tags"}))
}

#[tokio::test]
async fn two_apps_share_the_router_without_a_migration() {
    let db = ferrum::db::connect("sqlite::memory:").await.unwrap();
    let app = ferrum::App::new(db)
        .install(NotesApp)
        .unwrap()
        .install(TagsApp)
        .unwrap()
        .seal()
        .unwrap();
    assert_eq!(
        app.installed_apps(),
        ["notes".to_owned(), "tags".to_owned()]
    );
    let router = app.into_router();
    let response = router
        .oneshot(
            axum::http::Request::get("/notes/items")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8(bytes.to_vec()).unwrap().contains("notes"));
}
