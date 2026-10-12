use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::error::AppError;
use crate::extractors::{AuthAdmin, AuthCustomer, ValidatedJson};
use crate::models::customer::{AdminCustomerView, Customer, UpdateCustomer};
use crate::services::customer as customer_service;
use crate::state::AppState;

pub async fn get_customer(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
) -> Result<Json<Customer>, AppError> {
    let customer = customer_service::get_customer(&state.db, &customer_id).await?;
    Ok(Json(customer))
}

pub async fn update_customer(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
    ValidatedJson(input): ValidatedJson<UpdateCustomer>,
) -> Result<Json<Customer>, AppError> {
    let customer = customer_service::update_customer(&state.db, &customer_id, input).await?;
    Ok(Json(customer))
}

pub async fn delete_customer(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
) -> Result<StatusCode, AppError> {
    customer_service::delete_customer(&state.db, &customer_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_get_customer(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(customer_id): Path<String>,
) -> Result<Json<AdminCustomerView>, AppError> {
    let customer = customer_service::admin_get_customer(&state.db, &customer_id).await?;
    Ok(Json(customer))
}
