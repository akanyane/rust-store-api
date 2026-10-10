use chrono::Utc;
use surrealdb::{Surreal, engine::any::Any, types::Datetime};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::product::{
    CreateProduct, ListProductsQuery, ListPublicProductsQuery, NO_LIMIT, NewProduct, Product,
    ProductChanges, ProductDetail, ProductPage, ProductRecord, UpdateProduct,
};
use crate::models::variant::{NewVariant, Variant};
use crate::models::{DEFAULT_PAGE_SIZE, id_to_string};
use crate::repositories::{product as product_repo, variant as variant_repo};

/// Produtos ativos e o total de ativos (independente da página). Sem `limit`, devolve todos.
/// A contagem e a página são duas leituras: um produto criado entre elas pode deixar o
/// total defasado em uma unidade.
pub async fn list_products(
    db: &Surreal<Any>,
    query: ListPublicProductsQuery,
) -> Result<(Vec<Product>, i64), AppError> {
    let ex = Executor::Db(db);
    let sort = query.sort.unwrap_or_default();
    let limit = query.limit.unwrap_or(NO_LIMIT);
    let offset = query.offset.unwrap_or(0);

    let records = product_repo::find_active(&ex, sort, limit, offset).await?;
    let total = product_repo::count_active(&ex).await?;
    Ok((records.into_iter().map(Product::from).collect(), total))
}

pub async fn get_product(db: &Surreal<Any>, id: &str) -> Result<ProductDetail, AppError> {
    let ex = Executor::Db(db);
    let record = product_repo::find_by_id(&ex, id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Produto desativado some do catálogo público.
    if !record.active {
        return Err(AppError::NotFound);
    }

    detail_of(&ex, record, true).await
}

/// Monta o detalhe do produto. `only_active` esconde as variantes desativadas (catálogo
/// público); o admin vê todas.
async fn detail_of(
    ex: &Executor<'_>,
    record: ProductRecord,
    only_active: bool,
) -> Result<ProductDetail, AppError> {
    let variants = variant_repo::find_by_product(ex, record.id.clone()).await?;

    Ok(ProductDetail::new(
        Product::from(record),
        variants
            .into_iter()
            .filter(|variant| !only_active || variant.active)
            .map(Variant::from)
            .collect(),
    ))
}

/// Produtos de qualquer status, para o admin achar o que foi desativado e reativar, e o total
/// que casa com o filtro `active` (independente de `limit` e `offset`).
pub async fn admin_list_products(
    db: &Surreal<Any>,
    query: ListProductsQuery,
) -> Result<(Vec<Product>, i64), AppError> {
    let ex = Executor::Db(db);
    let page = ProductPage {
        active: query.active,
        limit: query.limit.unwrap_or(DEFAULT_PAGE_SIZE),
        offset: query.offset.unwrap_or(0),
    };
    let sort = query.sort.unwrap_or_default();
    let records = product_repo::find_page(&ex, page, sort).await?;
    let total = product_repo::count_matching(&ex, query.active).await?;
    Ok((records.into_iter().map(Product::from).collect(), total))
}

/// Detalhe para o admin: mesmo com o produto inativo, e com todas as variantes.
pub async fn admin_get_product(db: &Surreal<Any>, id: &str) -> Result<ProductDetail, AppError> {
    let ex = Executor::Db(db);
    let record = product_repo::find_by_id(&ex, id)
        .await?
        .ok_or(AppError::NotFound)?;

    detail_of(&ex, record, false).await
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
        created_at: Datetime::from(Utc::now()),
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

pub async fn update_product(
    db: &Surreal<Any>,
    id: &str,
    input: UpdateProduct,
) -> Result<Product, AppError> {
    let ex = Executor::Db(db);
    let current = product_repo::find_by_id(&ex, id)
        .await?
        .ok_or(AppError::NotFound)?;

    let changes = ProductChanges {
        name: input.name,
        description: input.description,
        active: input.active,
    };
    let record = product_repo::update(&ex, current.id, changes)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Product::from(record))
}

/// Exclusão lógica: pedidos e carrinhos apontam para o produto e suas variantes.
/// Idempotente: desativar de novo não é erro.
pub async fn delete_product(db: &Surreal<Any>, id: &str) -> Result<(), AppError> {
    let ex = Executor::Db(db);
    let current = product_repo::find_by_id(&ex, id)
        .await?
        .ok_or(AppError::NotFound)?;

    if current.active {
        let changes = ProductChanges {
            name: current.name,
            description: current.description,
            active: false,
        };
        product_repo::update(&ex, current.id, changes)
            .await?
            .ok_or(AppError::NotFound)?;
    }
    Ok(())
}
