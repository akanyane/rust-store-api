use axum::{
    Json,
    extract::{Path, State},
};

use crate::error::AppError;
use crate::extractors::AuthCustomer;
use crate::models::cart::CartView;
use crate::models::cart_item::{AddCartItem, UpdateCartItem};
use crate::services::cart as cart_service;
use crate::state::AppState;

pub async fn get_cart(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
) -> Result<Json<CartView>, AppError> {
    let cart = cart_service::get_cart(&state.db, &customer_id).await?;
    Ok(Json(cart))
}

pub async fn add_item(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
    Json(input): Json<AddCartItem>,
) -> Result<Json<CartView>, AppError> {
    let cart = cart_service::add_item(&state.db, &customer_id, input).await?;
    Ok(Json(cart))
}

pub async fn update_item(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
    Path(variant_id): Path<String>,
    Json(input): Json<UpdateCartItem>,
) -> Result<Json<CartView>, AppError> {
    let cart = cart_service::update_item(&state.db, &customer_id, &variant_id, input).await?;
    Ok(Json(cart))
}

pub async fn remove_item(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
    Path(variant_id): Path<String>,
) -> Result<Json<CartView>, AppError> {
    let cart = cart_service::remove_item(&state.db, &customer_id, &variant_id).await?;
    Ok(Json(cart))
}
