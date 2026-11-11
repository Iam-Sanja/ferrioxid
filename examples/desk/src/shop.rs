//! Shop operations on the public model API.
//!
//! `create` checks the model field rules. `update_matching`, `delete_matching`
//! and `insert` do not. They check column names and value types, then one
//! statement changes every matching row. A later failure can roll that
//! statement back only when the caller opened a transaction.

use ferrioxid::core::{FerrioxidError, QueryScope, ScopeValue};
use ferrioxid::db::{col, Aggregate, Direction, ModelQuery, ModelRepository, Related, WriteResult};
use ferrioxid::prelude::*;
use ferrioxid::sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};

use crate::models::{Address, Cart, Category, Line, Owner, OwnerCreate, Product};

pub const ADDRESS_LIMIT: i64 = 20;

pub async fn search_products<C>(db: &C, term: &str) -> Result<Vec<Product>, FerrioxidError>
where
    C: ConnectionTrait,
{
    Product::query()
        .filter(col("name").contains_ignore_case(term))
        .order_by("name", Direction::Asc)
        .all(db)
        .await
}

pub async fn products_in_category<C>(db: &C, category: &str) -> Result<Vec<Product>, FerrioxidError>
where
    C: ConnectionTrait,
{
    Product::query()
        .where_related(
            "category_id",
            Related::<Category>::new().filter(col("name").eq(category)),
        )
        .order_by("name", Direction::Asc)
        .all(db)
        .await
}

pub async fn product_page<C>(
    db: &C,
    limit: u64,
) -> Result<ferrioxid::db::QueryPage<Product>, FerrioxidError>
where
    C: ConnectionTrait,
{
    Product::query()
        .order_by("name", Direction::Asc)
        .limit(limit)
        .page(db)
        .await
}

pub async fn owner_quantity<C>(db: &C, owner: Uuid) -> Result<i64, FerrioxidError>
where
    C: ConnectionTrait,
{
    let rows = Owner::query()
        .filter(col("id").eq(owner))
        .annotate(
            db,
            Aggregate::<Line>::sum("line_qty", "owner_id", "qty"),
            None,
        )
        .await?;
    Ok(rows.into_iter().next().map(|row| row.value).unwrap_or(0))
}

/// Decrements stock in one statement when the row is active, still has enough
/// stock, and the stored price is still `price_cents`.
pub async fn take_stock<C>(
    db: &C,
    product: Uuid,
    price_cents: i64,
    quantity: i64,
) -> Result<WriteResult, FerrioxidError>
where
    C: ConnectionTrait,
{
    Product::query()
        .filter(col("id").eq(product))
        .filter(col("active").eq(true))
        .filter(col("stock").gte(quantity))
        .filter(col("price_cents").eq(price_cents))
        .update_matching()
        .decrement("stock", quantity)
        .execute(db)
        .await
}

pub async fn delete_lines<C>(db: &C, owner: Uuid) -> Result<WriteResult, FerrioxidError>
where
    C: ConnectionTrait,
{
    Line::query()
        .scope(QueryScope::Equals {
            column: "owner_id",
            value: ScopeValue::Uuid(owner),
        })
        .delete_matching()
        .execute(db)
        .await
}

pub async fn mark_lines_moved<C>(
    db: &C,
    owner: Uuid,
    cart: Uuid,
) -> Result<WriteResult, FerrioxidError>
where
    C: ConnectionTrait,
{
    Line::query()
        .scope(QueryScope::Equals {
            column: "owner_id",
            value: ScopeValue::Uuid(owner),
        })
        .filter(col("cart_id").eq(cart))
        .update_matching()
        .set("moved", true)
        .execute(db)
        .await
}

pub async fn open_cart<C>(db: &C, id: Uuid, owner: Uuid) -> Result<WriteResult, FerrioxidError>
where
    C: ConnectionTrait,
{
    Cart::insert()
        .value("id", id)
        .value("owner_id", owner)
        .on_conflict_do_nothing(&["owner_id"])
        .execute(db)
        .await
}

pub async fn add_address(
    db: &DatabaseConnection,
    owner: Uuid,
    id: Uuid,
    label: &str,
) -> Result<WriteResult, FerrioxidError> {
    let tx = db.begin().await.map_err(FerrioxidError::database)?;
    let locked = Owner::query()
        .filter(col("id").eq(owner))
        .lock_existing(&tx)
        .await?;
    if locked.rows != 1 {
        tx.rollback().await.map_err(FerrioxidError::database)?;
        return Err(FerrioxidError::Validation("owner is missing".into()));
    }
    let count = Address::query()
        .filter(col("owner_id").eq(owner))
        .count(&tx)
        .await?;
    if count >= ADDRESS_LIMIT {
        tx.rollback().await.map_err(FerrioxidError::database)?;
        return Err(FerrioxidError::Validation("address limit reached".into()));
    }
    let written = Address::insert()
        .value("id", id)
        .value("owner_id", owner)
        .value("label", label)
        .execute(&tx)
        .await?;
    tx.commit().await.map_err(FerrioxidError::database)?;
    Ok(written)
}

pub async fn load_product<C>(db: &C, id: Uuid) -> Result<Option<Product>, FerrioxidError>
where
    C: ConnectionTrait,
{
    Ok(Product::query()
        .filter(col("id").eq(id))
        .all(db)
        .await?
        .into_iter()
        .next())
}

pub async fn create_owner(db: &DatabaseConnection, name: &str) -> Result<Owner, FerrioxidError> {
    Owner::create(
        db,
        OwnerCreate {
            name: name.to_owned(),
        },
    )
    .await
}
