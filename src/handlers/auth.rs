use axum::{Json, extract::State, http::StatusCode};

use crate::error::AppError;
use crate::extractors::{AuthUser, ValidatedJson};
use crate::models::auth::{AuthResponse, Refresh, SessionView, SignIn, SignOut, SignUp, TokenPair};
use crate::models::customer::Customer;
use crate::services::auth as auth_service;
use crate::state::AppState;

pub async fn sign_up(
    State(state): State<AppState>,
    ValidatedJson(input): ValidatedJson<SignUp>,
) -> Result<(StatusCode, Json<Customer>), AppError> {
    let customer = auth_service::sign_up(&state.db, input).await?;
    Ok((StatusCode::CREATED, Json(customer)))
}

pub async fn sign_in(
    State(state): State<AppState>,
    ValidatedJson(input): ValidatedJson<SignIn>,
) -> Result<Json<AuthResponse>, AppError> {
    let TokenPair {
        session_token,
        refresh_token,
    } = auth_service::sign_in(&state.db, input).await?;

    Ok(Json(AuthResponse {
        session_token,
        refresh_token,
        token_type: "Bearer".to_string(),
    }))
}

pub async fn sign_out(
    State(state): State<AppState>,
    ValidatedJson(input): ValidatedJson<SignOut>,
) -> Result<StatusCode, AppError> {
    auth_service::sign_out(&state.db, &input.refresh_token).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn refresh(
    State(state): State<AppState>,
    ValidatedJson(input): ValidatedJson<Refresh>,
) -> Result<Json<AuthResponse>, AppError> {
    let TokenPair {
        session_token,
        refresh_token,
    } = auth_service::refresh(&state.db, &input.refresh_token).await?;

    Ok(Json(AuthResponse {
        session_token,
        refresh_token,
        token_type: "Bearer".to_string(),
    }))
}

/// Quem está logado e com qual papel. Vale para cliente e para admin (o `/me` é só de
/// cliente e dá 404 para admin, que não tem perfil).
pub async fn session(AuthUser(auth): AuthUser) -> Json<SessionView> {
    Json(SessionView {
        id: auth.user_id,
        username: auth.username,
        role: auth.role,
    })
}
