use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::error::AppError;
use crate::models::auth::Authenticated;
use crate::models::user::ROLE_ADMIN;
use crate::services::auth as auth_service;
use crate::state::AppState;

async fn authenticate_request(parts: &Parts, state: &AppState) -> Result<Authenticated, AppError> {
    let token = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    auth_service::authenticate(&state.db, token).await
}

/// Cliente autenticado pelo header `Authorization: Bearer <session_token>`.
/// Guarda a chave do customer, que é a mesma do user dono da sessão.
pub struct AuthCustomer(pub String);

impl FromRequestParts<AppState> for AuthCustomer {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = authenticate_request(parts, state).await?;
        Ok(AuthCustomer(auth.user_id))
    }
}

/// Exige um session token válido de um user com papel `admin`: 401 sem token
/// válido, 403 se o token é de outro papel.
pub struct AuthAdmin;

impl FromRequestParts<AppState> for AuthAdmin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = authenticate_request(parts, state).await?;
        if auth.role != ROLE_ADMIN {
            return Err(AppError::Forbidden);
        }
        Ok(AuthAdmin)
    }
}
