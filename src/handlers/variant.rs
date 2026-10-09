use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::error::AppError;
use crate::extractors::AuthAdmin;
use crate::models::variant::{CreateVariant, Variant};
use crate::services::variant as variant_service;
use crate::state::AppState;

pub async fn list_variants(
    State(state): State<AppState>,
    Path(product_id): Path<String>,
) -> Result<Json<Vec<Variant>>, AppError> {
    let variants = variant_service::list_variants(&state.db, &product_id).await?;
    Ok(Json(variants))
}

pub async fn create_variant(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(product_id): Path<String>,
    Json(input): Json<CreateVariant>,
) -> Result<(StatusCode, Json<Variant>), AppError> {
    let variant = variant_service::create_variant(&state.db, &product_id, input).await?;
    Ok((StatusCode::CREATED, Json(variant)))
}
