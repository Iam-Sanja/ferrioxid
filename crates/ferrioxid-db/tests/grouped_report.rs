#[path = "../examples/grouped_report/scenario.rs"]
mod scenario;

#[tokio::test(flavor = "current_thread")]
async fn sqlite_grouped_report_scope_json_and_one_to_one() {
    let db = ferrioxid_db::connect("sqlite::memory:").await.unwrap();
    scenario::exercise(&db).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires DATABASE_URL pointing at an approved PostgreSQL admin database"]
async fn postgres_grouped_report_scope_json_and_one_to_one() {
    let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let isolated = ferrioxid_db::DisposablePostgres::create(&admin)
        .await
        .expect("isolated database");
    let result = async {
        let db = ferrioxid_db::connect(isolated.url())
            .await
            .map_err(|error| error.to_string())?;
        scenario::exercise(&db).await
    }
    .await;
    let dropped = isolated.drop_database().await;
    result.expect("report");
    dropped.expect("drop isolated database");
}
