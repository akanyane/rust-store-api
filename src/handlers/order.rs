use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::error::AppError;
use crate::extractors::{AuthAdmin, AuthCustomer, ValidatedJson, ValidatedQuery};
use crate::models::order::{AdminOrderView, ListOrdersQuery, OrderView, UpdateOrderStatus};
use crate::services::order as order_service;
use crate::state::AppState;

pub async fn checkout(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
) -> Result<(StatusCode, Json<OrderView>), AppError> {
    let order = order_service::checkout(&state.db, &customer_id).await?;
    Ok((StatusCode::CREATED, Json(order)))
}

pub async fn list_orders(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
) -> Result<Json<Vec<OrderView>>, AppError> {
    let orders = order_service::list_orders(&state.db, &customer_id).await?;
    Ok(Json(orders))
}

pub async fn get_order(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
    Path(order_id): Path<String>,
) -> Result<Json<OrderView>, AppError> {
    let order = order_service::get_order(&state.db, &customer_id, &order_id).await?;
    Ok(Json(order))
}

pub async fn cancel_order(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
    Path(order_id): Path<String>,
) -> Result<Json<OrderView>, AppError> {
    let order = order_service::cancel_order(&state.db, &customer_id, &order_id).await?;
    Ok(Json(order))
}

pub async fn pay_order(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
    Path(order_id): Path<String>,
) -> Result<Json<OrderView>, AppError> {
    let order = order_service::pay_order(&state.db, &customer_id, &order_id).await?;
    Ok(Json(order))
}

pub async fn admin_list_orders(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    ValidatedQuery(query): ValidatedQuery<ListOrdersQuery>,
) -> Result<Json<Vec<AdminOrderView>>, AppError> {
    let orders = order_service::admin_list_orders(&state.db, query).await?;
    Ok(Json(orders))
}

pub async fn admin_get_order(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(order_id): Path<String>,
) -> Result<Json<AdminOrderView>, AppError> {
    let order = order_service::admin_get_order(&state.db, &order_id).await?;
    Ok(Json(order))
}

pub async fn admin_update_status(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(order_id): Path<String>,
    ValidatedJson(input): ValidatedJson<UpdateOrderStatus>,
) -> Result<Json<AdminOrderView>, AppError> {
    let order = order_service::admin_update_status(&state.db, &order_id, input.status).await?;
    Ok(Json(order))
}
