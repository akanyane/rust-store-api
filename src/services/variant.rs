use surrealdb::{Surreal, engine::any::Any};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::id_to_string;
use crate::models::variant::{
    CreateVariant, NewVariant, UpdateVariant, Variant, VariantChanges, VariantRecord,
};
use crate::repositories::{product as product_repo, variant as variant_repo};

pub async fn list_variants(db: &Surreal<Any>, product_id: &str) -> Result<Vec<Variant>, AppError> {
    let ex = Executor::Db(db);
    let product = product_repo::find_by_id(&ex, product_id)
        .await?
        .filter(|product| product.active)
        .ok_or(AppError::NotFound)?;

    let records = variant_repo::find_by_product(&ex, product.id).await?;
    Ok(records
        .into_iter()
        .filter(|record| record.active)
        .map(Variant::from)
        .collect())
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
            "A variant with SKU '{}' already exists",
            input.sku
        )));
    }

    let new_variant = NewVariant {
        product: product.id,
        name: input.name,
        sku: input.sku,
        price: input.price,
        stock: input.stock,
    };

    let record = variant_repo::create(&ex, new_variant)
        .await?
        .ok_or(AppError::Internal("failed to create variant".to_string()))?;

    Ok(Variant::from(record))
}

/// Acha a variante dentro do produto da URL; variante de outro produto é 404.
async fn find_in_product(
    ex: &Executor<'_>,
    product_id: &str,
    variant_id: &str,
) -> Result<VariantRecord, AppError> {
    let product = product_repo::find_by_id(ex, product_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let variant = variant_repo::find_by_id(ex, variant_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if variant.product != product.id {
        return Err(AppError::NotFound);
    }
    Ok(variant)
}

pub async fn update_variant(
    db: &Surreal<Any>,
    product_id: &str,
    variant_id: &str,
    input: UpdateVariant,
) -> Result<Variant, AppError> {
    let ex = Executor::Db(db);
    let current = find_in_product(&ex, product_id, variant_id).await?;

    // O próprio SKU pode ser mantido; só colide com OUTRA variante.
    if let Some(other) = variant_repo::find_by_sku(&ex, &input.sku).await?
        && id_to_string(&other.id) != id_to_string(&current.id)
    {
        return Err(AppError::Conflict(format!(
            "A variant with SKU '{}' already exists",
            input.sku
        )));
    }

    let changes = VariantChanges {
        product: current.product,
        name: input.name,
        sku: input.sku,
        price: input.price,
        stock: input.stock,
        active: input.active,
    };
    let record = variant_repo::update(&ex, current.id, changes)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Variant::from(record))
}

/// Exclusão lógica (ver `delete_product`). Idempotente.
pub async fn delete_variant(
    db: &Surreal<Any>,
    product_id: &str,
    variant_id: &str,
) -> Result<(), AppError> {
    let ex = Executor::Db(db);
    let current = find_in_product(&ex, product_id, variant_id).await?;

    if current.active {
        let changes = VariantChanges {
            product: current.product,
            name: current.name,
            sku: current.sku,
            price: current.price,
            stock: current.stock,
            active: false,
        };
        variant_repo::update(&ex, current.id, changes)
            .await?
            .ok_or(AppError::NotFound)?;
    }
    Ok(())
}
