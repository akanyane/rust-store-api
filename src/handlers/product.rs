use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::error::AppError;
use crate::extractors::{AuthAdmin, ValidatedJson};
use crate::models::product::{CreateProduct, Product, ProductDetail, UpdateProduct};
use crate::services::product as product_service;
use crate::state::AppState;

pub async fn list_products(State(state): State<AppState>) -> Result<Json<Vec<Product>>, AppError> {
    let products = product_service::list_products(&state.db).await?;
    Ok(Json(products))
}

pub async fn get_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ProductDetail>, AppError> {
    let product = product_service::get_product(&state.db, &id).await?;
    Ok(Json(product))
}

pub async fn create_product(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    ValidatedJson(input): ValidatedJson<CreateProduct>,
) -> Result<(StatusCode, Json<Product>), AppError> {
    let product = product_service::create_product(&state.db, input).await?;
    Ok((StatusCode::CREATED, Json(product)))
}

pub async fn update_product(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(id): Path<String>,
    ValidatedJson(input): ValidatedJson<UpdateProduct>,
) -> Result<Json<Product>, AppError> {
    let product = product_service::update_product(&state.db, &id, input).await?;
    Ok(Json(product))
}

pub async fn delete_product(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    product_service::delete_product(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
