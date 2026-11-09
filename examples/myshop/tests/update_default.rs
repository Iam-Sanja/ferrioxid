use ferrioxid::db::PatchField;
use myshop::models::product::ProductUpdate;

#[test]
fn generated_update_default_marks_every_field_missing() {
    let patch = ProductUpdate::default();

    assert!(matches!(patch.name, PatchField::Missing));
    assert!(matches!(patch.price_cents, PatchField::Missing));
    assert!(matches!(patch.active, PatchField::Missing));
    assert!(matches!(patch.category_id, PatchField::Missing));
}

#[test]
fn generated_update_default_matches_empty_json_patch() {
    let parsed: ProductUpdate = ferrioxid::serde_json::from_str("{}").unwrap();

    assert!(matches!(parsed.name, PatchField::Missing));
    assert!(matches!(parsed.price_cents, PatchField::Missing));
    assert!(matches!(parsed.active, PatchField::Missing));
    assert!(matches!(parsed.category_id, PatchField::Missing));
}

#[test]
fn json_null_still_differs_from_missing_for_optional_fields() {
    let patch: ProductUpdate = ferrioxid::serde_json::from_str(r#"{"category_id":null}"#).unwrap();
    assert!(matches!(patch.category_id, PatchField::Null));
    assert!(matches!(patch.name, PatchField::Missing));
}
