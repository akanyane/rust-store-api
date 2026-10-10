use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    NotFound,
    Conflict(String),
    Validation(String),
    Unauthorized,
    Forbidden,
    Internal(String),
    Database(surrealdb::Error),
}

impl From<surrealdb::Error> for AppError {
    fn from(e: surrealdb::Error) -> Self {
        AppError::Database(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Resource not found".to_string()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            AppError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            // Mensagem única de propósito: não revela se o e-mail existe.
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Access denied".to_string()),
            AppError::Internal(msg) => {
                eprintln!("Internal error: {msg}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal error".to_string(),
                )
            }
            AppError::Database(e) => {
                // A mensagem do banco pode conter dados pessoais (ex.: o e-mail
                // inválido). Em release ela é omitida; em debug os dados são fictícios.
                if cfg!(debug_assertions) {
                    eprintln!("Database error: {e:?}");
                } else {
                    eprintln!("Database error (details omitted: may contain personal data)");
                }
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal error".to_string(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl AppError {
    /// Duas transações que escrevem no mesmo registro ao mesmo tempo colidem, e a
    /// perdedora pode ser repetida. O SDK embarcado devolve isso como erro `Internal`
    /// sem detalhe estruturado, então só a mensagem o identifica.
    pub fn is_write_conflict(&self) -> bool {
        matches!(self, AppError::Database(e) if e.message().contains("Transaction conflict"))
    }
}
