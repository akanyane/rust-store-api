use std::sync::OnceLock;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use surrealdb::{
    Surreal,
    engine::any::Any,
    types::{Datetime, RecordId},
};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::auth::{Authenticated, SignIn, SignUp, TokenPair};
use crate::models::customer::{Customer, CustomerRecord, NewCustomer};
use crate::models::session::NewSession;
use crate::models::user::{NewUser, ROLE_ADMIN, ROLE_CUSTOMER};
use crate::models::{id_to_string, validate_username};
use crate::repositories::{customer as customer_repo, session as session_repo, user as user_repo};

const USERNAME_IN_USE: &str = "An account with this username already exists";
const MIN_PASSWORD_LEN: usize = 8;
// Limite superior para ninguém mandar uma senha gigante só para gastar CPU no argon2.
const MAX_PASSWORD_LEN: usize = 128;
const SESSION_TTL_HOURS: i64 = 1;
const REFRESH_TTL_DAYS: i64 = 15;

fn normalize_username(username: &str) -> String {
    username.trim().to_lowercase()
}

fn ensure_valid_password(password: &str) -> Result<(), AppError> {
    let len = password.chars().count();
    if !(MIN_PASSWORD_LEN..=MAX_PASSWORD_LEN).contains(&len) {
        return Err(AppError::Validation(format!(
            "Password must be between {MIN_PASSWORD_LEN} and {MAX_PASSWORD_LEN} characters"
        )));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Token opaco: 32 bytes aleatórios em hexadecimal (64 caracteres).
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    hex(&bytes)
}

/// SHA-256 basta aqui porque o token já é aleatório e longo (não dá para "adivinhar"
/// como uma senha). Por isso o argon2 fica só para a senha.
fn hash_token(token: &str) -> String {
    hex(Sha256::digest(token.as_bytes()).as_slice())
}

fn hash_password_blocking(password: &str) -> Result<String, AppError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|e| AppError::Internal(format!("failed to hash password: {e}")))
}

/// Hash de uma senha qualquer, usado quando o e-mail não existe. Assim o sign-in
/// gasta o mesmo tempo nos dois casos e o tempo de resposta não revela quais
/// e-mails estão cadastrados.
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password_blocking("throwaway-password")
            .unwrap_or_else(|_| panic!("failed to generate the sign-in dummy hash"))
    })
}

// O argon2 é de propósito pesado em CPU; `spawn_blocking` evita travar o runtime async.
async fn hash_password(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || hash_password_blocking(&password))
        .await
        .map_err(|e| AppError::Internal(format!("hash task failed: {e}")))?
}

async fn verify_password(password: String, hash: Option<String>) -> Result<bool, AppError> {
    tokio::task::spawn_blocking(move || {
        let hash = hash.unwrap_or_else(|| dummy_hash().to_string());
        match PasswordHash::new(&hash) {
            Ok(parsed) => Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok(),
            Err(_) => false,
        }
    })
    .await
    .map_err(|e| AppError::Internal(format!("verification task failed: {e}")))
}

pub async fn sign_up(db: &Surreal<Any>, input: SignUp) -> Result<Customer, AppError> {
    ensure_valid_password(&input.password)?;
    let username = normalize_username(&input.username);

    if user_repo::find_by_username(&Executor::Db(db), &username)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(USERNAME_IN_USE.to_string()));
    }

    let user = NewUser {
        username: username.clone(),
        password_hash: hash_password(input.password).await?,
        role: ROLE_CUSTOMER.to_string(),
    };
    let profile = NewCustomer {
        first_name: input.first_name,
        last_name: input.last_name,
        birthday: input.birthday,
    };

    let tx = db.clone().begin().await?;
    let result = sign_up_in_tx(&Executor::Tx(&tx), user, profile).await;

    let customer = match result {
        Ok(customer) => {
            tx.commit().await?;
            customer
        }
        Err(e) => {
            if let Err(cancel_err) = tx.cancel().await {
                eprintln!("Failed to roll back transaction: {cancel_err:?}");
            }
            return Err(e);
        }
    };

    Ok(Customer::from_parts(customer, username))
}

/// Cria o user e o customer com a mesma chave. Numa transação: ou ficam os dois, ou nenhum.
async fn sign_up_in_tx(
    ex: &Executor<'_>,
    user: NewUser,
    profile: NewCustomer,
) -> Result<CustomerRecord, AppError> {
    let user = user_repo::create(ex, user)
        .await?
        .ok_or_else(|| AppError::Internal("failed to create user".to_string()))?;

    customer_repo::create(ex, &id_to_string(&user.id), profile)
        .await?
        .ok_or_else(|| AppError::Internal("failed to create customer".to_string()))
}

pub async fn sign_in(db: &Surreal<Any>, input: SignIn) -> Result<TokenPair, AppError> {
    let ex = Executor::Db(db);
    let user = user_repo::find_by_username(&ex, &normalize_username(&input.username)).await?;

    let valid = verify_password(
        input.password,
        user.as_ref().map(|u| u.password_hash.clone()),
    )
    .await?;

    // Mesmo erro para "e-mail não existe" e "senha errada".
    let user = match user {
        Some(user) if valid => user,
        _ => return Err(AppError::Unauthorized),
    };

    issue_tokens(&ex, user.id).await
}

/// Gera um par de tokens novo e grava a sessão (só os hashes vão para o banco).
async fn issue_tokens(ex: &Executor<'_>, user: RecordId) -> Result<TokenPair, AppError> {
    let tokens = TokenPair {
        session_token: generate_token(),
        refresh_token: generate_token(),
    };
    let now = Utc::now();

    session_repo::create(
        ex,
        NewSession {
            user,
            token_hash: hash_token(&tokens.session_token),
            refresh_hash: hash_token(&tokens.refresh_token),
            expires_at: Datetime::from(now + Duration::hours(SESSION_TTL_HOURS)),
            refresh_expires_at: Datetime::from(now + Duration::days(REFRESH_TTL_DAYS)),
        },
    )
    .await?
    .ok_or_else(|| AppError::Internal("failed to create session".to_string()))?;

    Ok(tokens)
}

/// Troca um refresh token válido por um par novo (rotação): a sessão antiga é
/// apagada, então um refresh token só vale uma vez. Tudo numa transação.
pub async fn refresh(db: &Surreal<Any>, refresh_token: &str) -> Result<TokenPair, AppError> {
    let tx = db.clone().begin().await?;
    let result = refresh_in_tx(&Executor::Tx(&tx), refresh_token).await;

    match result {
        Ok(tokens) => match tx.commit().await {
            Ok(_) => Ok(tokens),
            Err(e) if AppError::from(e.clone()).is_write_conflict() => Err(AppError::Unauthorized),
            Err(e) => Err(e.into()),
        },
        Err(e) => {
            if let Err(cancel_err) = tx.cancel().await {
                eprintln!("Failed to roll back transaction: {cancel_err:?}");
            }
            if e.is_write_conflict() {
                return Err(AppError::Unauthorized);
            }
            Err(e)
        }
    }
}

async fn refresh_in_tx(ex: &Executor<'_>, refresh_token: &str) -> Result<TokenPair, AppError> {
    let session = session_repo::find_by_refresh_hash(ex, &hash_token(refresh_token))
        .await?
        .ok_or(AppError::Unauthorized)?;

    if DateTime::<Utc>::from(session.refresh_expires_at) <= Utc::now() {
        return Err(AppError::Unauthorized);
    }

    // Se outro refresh com o mesmo token já apagou a sessão, este não passa.
    session_repo::delete(ex, session.id.clone())
        .await?
        .ok_or(AppError::Unauthorized)?;

    user_repo::find_by_id(ex, &id_to_string(&session.user))
        .await?
        .ok_or(AppError::Unauthorized)?;

    issue_tokens(ex, session.user).await
}

/// Devolve o user dono do token (a chave dele é também a do customer) e o papel,
/// ou 401 se o token for desconhecido, estiver expirado ou o user não existir mais.
pub async fn authenticate(db: &Surreal<Any>, token: &str) -> Result<Authenticated, AppError> {
    let ex = Executor::Db(db);
    let session = session_repo::find_by_token_hash(&ex, &hash_token(token))
        .await?
        .ok_or(AppError::Unauthorized)?;

    if DateTime::<Utc>::from(session.expires_at) <= Utc::now() {
        return Err(AppError::Unauthorized);
    }

    let user = user_repo::find_by_id(&ex, &id_to_string(&session.user))
        .await?
        .ok_or(AppError::Unauthorized)?;

    Ok(Authenticated {
        user_id: id_to_string(&user.id),
        role: user.role,
    })
}

/// Garante que existe um admin com este username (seed da partida). Idempotente: se o
/// username já existe, não troca a senha nem promove um usuário comum em silêncio.
pub async fn ensure_admin(
    db: &Surreal<Any>,
    username: &str,
    password: String,
) -> Result<(), AppError> {
    let ex = Executor::Db(db);
    let username = normalize_username(username);
    validate_username(&username)
        .map_err(|_| AppError::Validation("ADMIN_USERNAME is not a valid username".to_string()))?;

    if let Some(existing) = user_repo::find_by_username(&ex, &username).await? {
        if existing.role != ROLE_ADMIN {
            eprintln!("ADMIN_USERNAME belongs to a regular user: it was NOT promoted to admin");
        }
        return Ok(());
    }

    ensure_valid_password(&password)?;
    user_repo::create(
        &ex,
        NewUser {
            username,
            password_hash: hash_password(password).await?,
            role: ROLE_ADMIN.to_string(),
        },
    )
    .await?
    .ok_or_else(|| AppError::Internal("failed to create admin".to_string()))?;

    println!("Admin user created");
    Ok(())
}

/// Encerra a sessão do refresh token. Idempotente: token desconhecido também dá sucesso.
pub async fn sign_out(db: &Surreal<Any>, refresh_token: &str) -> Result<(), AppError> {
    let ex = Executor::Db(db);

    if let Some(session) =
        session_repo::find_by_refresh_hash(&ex, &hash_token(refresh_token)).await?
    {
        session_repo::delete(&ex, session.id).await?;
    }
    Ok(())
}

/// Remove as sessões com refresh token vencido e devolve quantas foram apagadas.
pub async fn purge_expired_sessions(db: &Surreal<Any>) -> Result<usize, AppError> {
    let ex = Executor::Db(db);
    let deleted = session_repo::delete_expired(&ex, Datetime::from(Utc::now())).await?;
    Ok(deleted.len())
}
