//! Datum, Uhrzeit und UTC-Zeitpunkt: Migration, Roundtrip, Sortierung, Bereich und abgelehnte Typänderung.

use ferrioxid::chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use ferrioxid::core::{FerrioxidModel, QueryScope};
use ferrioxid::db::{date_buckets, query_admin_page, AdminPageRequest, ModelRepository};
use ferrioxid::migrations::{apply_directory, plan_change, LogicalType, SchemaSnapshot};
use myshop::models::event::{Event, EventCreate};
use std::path::PathBuf;

#[tokio::test]
async fn sqlite_event_roundtrip_sort_filter_and_blocked_conversion() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    apply_directory(&db, &directory, false).await.unwrap();
    let morning = Event::create(
        &db,
        EventCreate {
            title: "Frühstück".into(),
            event_date: NaiveDate::from_ymd_opt(2026, 11, 7).unwrap(),
            start_time: Some(NaiveTime::from_hms_opt(8, 15, 0).unwrap()),
            created_at: Utc.with_ymd_and_hms(2026, 11, 7, 7, 15, 0).unwrap(),
        },
    )
    .await
    .unwrap();
    let evening = Event::create(
        &db,
        EventCreate {
            title: "Abend".into(),
            event_date: NaiveDate::from_ymd_opt(2026, 11, 8).unwrap(),
            start_time: None,
            created_at: Utc.with_ymd_and_hms(2026, 11, 8, 18, 0, 0).unwrap(),
        },
    )
    .await
    .unwrap();
    let loaded = Event::get(&db, morning.id).await.unwrap().unwrap();
    assert_eq!(loaded.event_date, morning.event_date);
    assert_eq!(
        loaded.start_time,
        Some(NaiveTime::from_hms_opt(8, 15, 0).unwrap())
    );
    assert_eq!(loaded.created_at, morning.created_at);
    assert!(Event::get(&db, evening.id)
        .await
        .unwrap()
        .unwrap()
        .start_time
        .is_none());
    let listed = Event::list(&db).await.unwrap();
    assert_eq!(listed.data.len(), 2);
    let page = query_admin_page(
        &db,
        Event::descriptor().table,
        Event::columns(),
        AdminPageRequest {
            search_columns: &["title"],
            search: None,
            sort_column: "event_date",
            descending: false,
            equal_filters: &[(
                "event_date",
                ferrioxid::sea_orm::Value::from(NaiveDate::from_ymd_opt(2026, 11, 7).unwrap()),
            )],
            bounds: &[],
            offset: 0,
            limit: 25,
            scope: &QueryScope::All,
        },
    )
    .await
    .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.rows[0]["title"], "Frühstück");
    assert_eq!(page.rows[0]["event_date"], "2026-11-07");
    assert_eq!(page.rows[0]["start_time"], "08:15:00");
    assert_eq!(page.rows[0]["created_at"], "2026-11-07T07:15:00Z");
    let years = date_buckets(
        &db,
        Event::descriptor().table,
        Event::columns(),
        "event_date",
        4,
        &[],
        None,
        &[],
        &[],
        &QueryScope::All,
    )
    .await
    .unwrap();
    assert_eq!(years, vec![("2026".to_owned(), 2)]);
    let invalid = serde_json::from_value::<EventCreate>(serde_json::json!({
        "title": "Falsch",
        "event_date": "2023-02-29",
        "created_at": "2026-11-07T13:30:00Z"
    }));
    match invalid {
        Err(error) => assert!(error.to_string().contains("validation.date")),
        Ok(_) => panic!("invalid date was accepted"),
    }
    let mut changed = SchemaSnapshot::from_registered(&[ferrioxid::migrations::RegisteredModel {
        table: Event::descriptor().table,
        columns: Event::columns(),
        constraints: Event::constraints(),
    }])
    .unwrap();
    changed.tables[0]
        .columns
        .iter_mut()
        .find(|column| column.name == "event_date")
        .unwrap()
        .logical_type = LogicalType::String;
    let current = SchemaSnapshot::from_registered(&[ferrioxid::migrations::RegisteredModel {
        table: Event::descriptor().table,
        columns: Event::columns(),
        constraints: Event::constraints(),
    }])
    .unwrap();
    let refused = plan_change(&current, &changed).unwrap();
    assert!(refused.refusal.unwrap().contains("blocked"));
}

#[tokio::test]
async fn boundaries_optional_fields_patch_and_utc_ranges() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    apply_directory(&db, &directory, false).await.unwrap();
    let leap = serde_json::from_value::<EventCreate>(serde_json::json!({
        "title": "Schaltjahr",
        "event_date": "2024-02-29",
        "created_at": "2024-02-29T00:00:00Z"
    }))
    .unwrap();
    assert!(leap.start_time.is_none());
    let stored = Event::create(&db, leap).await.unwrap();
    assert_eq!(
        stored.event_date,
        NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
    );
    assert_eq!(
        stored.created_at,
        Utc.with_ymd_and_hms(2024, 2, 29, 0, 0, 0).unwrap()
    );
    let late = Event::create(
        &db,
        EventCreate {
            title: "Vor Mitternacht".into(),
            event_date: NaiveDate::from_ymd_opt(2026, 11, 6).unwrap(),
            start_time: Some(NaiveTime::from_hms_opt(23, 59, 59).unwrap()),
            created_at: Utc.with_ymd_and_hms(2026, 11, 6, 23, 59, 59).unwrap(),
        },
    )
    .await
    .unwrap();
    let midnight = Event::create(
        &db,
        EventCreate {
            title: "Mitternacht".into(),
            event_date: NaiveDate::from_ymd_opt(2026, 11, 7).unwrap(),
            start_time: Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            created_at: Utc.with_ymd_and_hms(2026, 11, 7, 0, 0, 0).unwrap(),
        },
    )
    .await
    .unwrap();
    let day = query_admin_page(
        &db,
        Event::descriptor().table,
        Event::columns(),
        AdminPageRequest {
            search_columns: &[],
            search: None,
            sort_column: "created_at",
            descending: false,
            equal_filters: &[],
            bounds: &[
                (
                    "created_at",
                    true,
                    ferrioxid::sea_orm::Value::from(
                        Utc.with_ymd_and_hms(2026, 11, 7, 0, 0, 0).unwrap(),
                    ),
                ),
                (
                    "created_at",
                    false,
                    ferrioxid::sea_orm::Value::from(
                        Utc.with_ymd_and_hms(2026, 11, 8, 0, 0, 0).unwrap(),
                    ),
                ),
            ],
            offset: 0,
            limit: 25,
            scope: &QueryScope::All,
        },
    )
    .await
    .unwrap();
    assert_eq!(day.total, 1);
    assert_eq!(day.rows[0]["title"], "Mitternacht");
    assert_eq!(day.rows[0]["created_at"], "2026-11-07T00:00:00Z");
    assert_ne!(day.rows[0]["id"], late.id.to_string());
    let patched = serde_json::from_value::<myshop::models::event::EventUpdate>(
        serde_json::json!({"created_at": "2026-11-07T23:59:59Z", "start_time": null}),
    )
    .unwrap();
    let updated = Event::update(&db, midnight.id, patched).await.unwrap();
    assert!(updated.start_time.is_none());
    assert_eq!(
        updated.created_at,
        Utc.with_ymd_and_hms(2026, 11, 7, 23, 59, 59).unwrap()
    );
    let offset = serde_json::from_value::<myshop::models::event::EventUpdate>(
        serde_json::json!({"created_at": "2026-11-07T00:00:00+02:00"}),
    );
    match offset {
        Err(error) => assert!(error.to_string().contains("validation.datetime_offset")),
        Ok(_) => panic!("offset was accepted"),
    }
    let months = date_buckets(
        &db,
        Event::descriptor().table,
        Event::columns(),
        "event_date",
        7,
        &[],
        None,
        &[],
        &[(
            "event_date",
            true,
            ferrioxid::sea_orm::Value::from(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()),
        )],
        &QueryScope::All,
    )
    .await
    .unwrap();
    assert_eq!(months, vec![("2026-11".to_owned(), 2)]);
}
