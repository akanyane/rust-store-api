use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderName, StatusCode},
};

use crate::error::AppError;
use crate::extractors::{AuthAdmin, AuthCustomer, ValidatedJson, ValidatedQuery};
use crate::handlers::total_count;
use crate::models::customer::{
    AdminCustomerListItem, AdminCustomerView, Customer, ListCustomersQuery, UpdateCustomer,
};
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

pub async fn admin_list_customers(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    ValidatedQuery(query): ValidatedQuery<ListCustomersQuery>,
) -> Result<([(HeaderName, String); 1], Json<Vec<AdminCustomerListItem>>), AppError> {
    let (customers, total) = customer_service::admin_list_customers(&state.db, query).await?;
    Ok((total_count(total), Json(customers)))
}
