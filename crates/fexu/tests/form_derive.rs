//! Model-aware form derive, including numeric limits, choices and decimals.

use fexu::core::{DatabaseKind, FexuModel};
use fexu::forms::{bind_formset, FexuForm, FormValues, SubmitMode};
use fexu::prelude::*;
use fexu::{FexuChoice, FexuForm};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FexuChoice)]
enum ItemStatus {
    #[choice(value = "draft", de = "Entwurf", en = "Draft")]
    Draft,
    #[choice(value = "active", de = "Aktiv", en = "Active")]
    Active,
}

#[derive(Debug, fexu::FexuModel, fexu::serde::Serialize, fexu::serde::Deserialize)]
#[fexu(table = "derived_form_items")]
struct DerivedItem {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 40)]
    name: String,
    #[field(min = 0, max = 20)]
    stock: i64,
    #[field(precision = 8, scale = 2, min = 0)]
    price: Decimal,
    #[field(choice)]
    status: ItemStatus,
}

#[derive(Debug, PartialEq, Eq, FexuForm)]
#[form(model = DerivedItem)]
struct DerivedItemForm {
    name: String,
    stock: i64,
    price: String,
    status: String,
}

#[derive(Debug, FexuForm)]
#[form(model = DerivedItem)]
struct PatchItemForm {
    name: Option<String>,
    stock: Option<i64>,
    price: Option<String>,
    status: Option<String>,
}

#[test]
fn derive_uses_model_bounds_choices_and_patch() {
    assert!(DerivedItem::schema_sql(DatabaseKind::Sqlite).contains("derived_form_items"));
    let mut values = FormValues::default();
    values.insert("name", "Bolt");
    values.insert("stock", "21");
    values.insert("price", "1.50");
    values.insert("status", "active");
    let errors = DerivedItemForm::from_form(&values).unwrap_err();
    assert_eq!(errors.for_field("stock"), Some("validation.max_value"));
    values.insert("stock", "2");
    values.insert("status", "missing");
    let errors = DerivedItemForm::from_form(&values).unwrap_err();
    assert_eq!(errors.for_field("status"), Some("validation.choice"));
    values.insert("status", "active");
    values.insert("price", "-1.00");
    let errors = DerivedItemForm::from_form(&values).unwrap_err();
    assert_eq!(errors.for_field("price"), Some("validation.min_value"));
    values.insert("price", "1.50");
    values.insert("id", "nope");
    let errors = DerivedItemForm::from_form(&values).unwrap_err();
    assert_eq!(errors.for_field("id"), Some("validation.unknown_field"));
    values = FormValues::default();
    values.insert("name", "Bolt");
    values.insert("stock", "2");
    values.insert("price", "1.50");
    values.insert("status", "active");
    let form = DerivedItemForm::from_form(&values).unwrap();
    assert_eq!(form.stock, 2);
    assert_eq!(form.price, "1.50");
    let patched = PatchItemForm::from_submit(SubmitMode::Patch, &FormValues::default()).unwrap();
    assert_eq!(patched.name, None);
    assert_eq!(patched.stock, None);
    assert_eq!(patched.price, None);
    assert_eq!(patched.status, None);
    let mut rows = FormValues::default();
    rows.insert("items_TOTAL_FORMS", "1");
    rows.insert("items_0_name", "Bolt");
    rows.insert("items_0_stock", "2");
    rows.insert("items_0_price", "1.50");
    rows.insert("items_0_status", "draft");
    let parsed = bind_formset(&rows, "items", 3, SubmitMode::Create, |mode, row| {
        DerivedItemForm::from_submit(mode, row)
    })
    .unwrap();
    assert_eq!(parsed[0].status, "draft");
}
