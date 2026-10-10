use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use validator::Validate;

// Sem `Debug` de propósito: um `{:?}` acidental não pode vazar a senha para o log.

#[derive(Deserialize, Validate)]
pub struct SignUp {
    #[validate(
        email(message = "invalid format"),
        length(max = 254, message = "at most 254 characters")
    )]
    pub email: String,
    pub password: String,
    #[validate(
        custom(
            function = "crate::models::validate_not_blank",
            message = "must not be blank"
        ),
        length(max = 100, message = "at most 100 characters")
    )]
    pub first_name: String,
    #[validate(
        custom(
            function = "crate::models::validate_not_blank",
            message = "must not be blank"
        ),
        length(max = 100, message = "at most 100 characters")
    )]
    pub last_name: String,
    #[validate(custom(
        function = "crate::models::validate_not_future",
        message = "must not be in the future"
    ))]
    pub birthday: NaiveDate,
}

#[derive(Deserialize, Validate)]
pub struct SignIn {
    #[validate(length(max = 254, message = "at most 254 characters"))]
    pub email: String,
    #[validate(length(max = 128, message = "at most 128 characters"))]
    pub password: String,
}

#[derive(Deserialize, Validate)]
pub struct SignOut {
    #[validate(length(min = 1, max = 128, message = "invalid token"))]
    pub refresh_token: String,
}

/// Par de tokens recém-gerado, devolvido pelo service.
pub struct TokenPair {
    pub session_token: String,
    pub refresh_token: String,
}

/// Quem está por trás de um session token válido.
pub struct Authenticated {
    pub user_id: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub session_token: String,
    pub refresh_token: String,
    pub token_type: String,
}
