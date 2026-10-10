use surrealdb::{Surreal, engine::any::Any};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::id_to_string;
use crate::models::product::{CreateProduct, NewProduct, Product, ProductDetail};
use crate::models::variant::{NewVariant, Variant};
use crate::repositories::{product as product_repo, variant as variant_repo};

pub async fn list_products(db: &Surreal<Any>) -> Result<Vec<Product>, AppError> {
    let records = product_repo::find_all(&Executor::Db(db)).await?;
    Ok(records.into_iter().map(Product::from).collect())
}

pub async fn get_product(db: &Surreal<Any>, id: &str) -> Result<ProductDetail, AppError> {
    let ex = Executor::Db(db);
    let record = product_repo::find_by_id(&ex, id)
        .await?
        .ok_or(AppError::NotFound)?;

    let variants = variant_repo::find_by_product(&ex, record.id.clone()).await?;

    Ok(ProductDetail::new(
        Product::from(record),
        variants.into_iter().map(Variant::from).collect(),
    ))
}

pub async fn create_product(db: &Surreal<Any>, input: CreateProduct) -> Result<Product, AppError> {
    let tx = db.clone().begin().await?;

    match insert_product_with_default_variant(&Executor::Tx(&tx), input).await {
        Ok(product) => {
            tx.commit().await?;
            Ok(product)
        }
        Err(e) => {
            if let Err(cancel_err) = tx.cancel().await {
                eprintln!("Failed to roll back transaction: {cancel_err:?}");
            }
            Err(e)
        }
    }
}

async fn insert_product_with_default_variant(
    ex: &Executor<'_>,
    input: CreateProduct,
) -> Result<Product, AppError> {
    let new_product = NewProduct {
        name: input.name,
        description: input.description,
    };

    let product = product_repo::create(ex, new_product)
        .await?
        .ok_or(AppError::Internal("failed to create product".to_string()))?;

    let default_variant = NewVariant {
        product: product.id.clone(),
        name: "Default".to_string(),
        sku: format!("SKU-{}", id_to_string(&product.id)),
        price: input.price,
        stock: input.stock,
    };

    variant_repo::create(ex, default_variant).await?;

    Ok(Product::from(product))
}
