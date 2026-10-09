use surrealdb::{Surreal, engine::any::Any};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::variant::{CreateVariant, NewVariant, Variant};
use crate::repositories::{product as product_repo, variant as variant_repo};

pub async fn list_variants(db: &Surreal<Any>, product_id: &str) -> Result<Vec<Variant>, AppError> {
    let ex = Executor::Db(db);
    let product = product_repo::find_by_id(&ex, product_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let records = variant_repo::find_by_product(&ex, product.id).await?;
    Ok(records.into_iter().map(Variant::from).collect())
}

pub async fn create_variant(
    db: &Surreal<Any>,
    product_id: &str,
    input: CreateVariant,
) -> Result<Variant, AppError> {
    let ex = Executor::Db(db);
    let product = product_repo::find_by_id(&ex, product_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if variant_repo::exists_by_sku(&ex, &input.sku).await? {
        return Err(AppError::Conflict(format!(
            "Já existe uma variante com o SKU '{}'",
            input.sku
        )));
    }

    let new_variant = NewVariant {
        product: product.id,
        name: input.name,
        sku: input.sku,
        price_cents: input.price_cents,
        stock: input.stock,
    };

    let record = variant_repo::create(&ex, new_variant)
        .await?
        .ok_or(AppError::Internal("falha ao criar variante".to_string()))?;

    Ok(Variant::from(record))
}
