use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Query, Request},
    http::{header::AUTHORIZATION, request::Parts},
};
use serde::de::DeserializeOwned;
use validator::{Validate, ValidationErrors};

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

/// Replaces `Json<T>` and validates the body with `#[derive(Validate)]`. Answers 422
/// in the standard API format, without the serde text and without echoing the
/// submitted values (the body may contain personal data).
pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|_| AppError::Validation("Invalid request body".to_string()))?;

        value
            .validate()
            .map_err(|errors| AppError::Validation(describe_errors(&errors)))?;

        Ok(ValidatedJson(value))
    }
}

/// One sentence per field, alphabetically: `field: message`.
fn describe_errors(errors: &ValidationErrors) -> String {
    let mut lines: Vec<String> = errors
        .field_errors()
        .into_iter()
        .map(|(field, field_errors)| {
            let message = field_errors
                .first()
                .and_then(|e| e.message.as_deref())
                .unwrap_or("invalid value");
            format!("{field}: {message}")
        })
        .collect();
    lines.sort();
    lines.join("; ")
}

/// O irmão do `ValidatedJson` para parâmetros de URL: 422 no formato padrão, sem o
/// texto cru do Axum.
pub struct ValidatedQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::Validation("Invalid query parameters".to_string()))?;

        value
            .validate()
            .map_err(|errors| AppError::Validation(describe_errors(&errors)))?;

        Ok(ValidatedQuery(value))
    }
}
