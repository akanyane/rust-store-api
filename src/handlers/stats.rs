use axum::{Json, extract::State};

use crate::error::AppError;
use crate::extractors::AuthAdmin;
use crate::models::stats::StatsView;
use crate::services::stats as stats_service;
use crate::state::AppState;

pub async fn admin_stats(
    State(state): State<AppState>,
    _admin: AuthAdmin,
) -> Result<Json<StatsView>, AppError> {
    Ok(Json(stats_service::stats(&state.db).await?))
}
