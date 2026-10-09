use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

// Sem `Debug` de propósito: um `{:?}` acidental não pode vazar a senha para o log.

#[derive(Deserialize)]
pub struct SignUp {
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub birthday: NaiveDate,
}

#[derive(Deserialize)]
pub struct SignIn {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct SignOut {
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
