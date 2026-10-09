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
            AppError::NotFound => (StatusCode::NOT_FOUND, "Recurso não encontrado".to_string()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            AppError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            // Mensagem única de propósito: não revela se o e-mail existe.
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Credenciais inválidas".to_string(),
            ),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Acesso negado".to_string()),
            AppError::Internal(msg) => {
                eprintln!("Erro interno: {msg}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Erro interno".to_string(),
                )
            }
            AppError::Database(e) => {
                // A mensagem do banco pode conter dados pessoais (ex.: o e-mail
                // inválido). Em release ela é omitida; em debug os dados são fictícios.
                if cfg!(debug_assertions) {
                    eprintln!("Erro no banco: {e:?}");
                } else {
                    eprintln!("Erro no banco (detalhes omitidos: podem conter dados pessoais)");
                }
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Erro interno".to_string(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
