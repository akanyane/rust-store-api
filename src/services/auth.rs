use std::sync::OnceLock;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use surrealdb::{Surreal, engine::any::Any, types::Datetime};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::auth::{Authenticated, SignIn, SignUp, TokenPair};
use crate::models::customer::{Customer, CustomerRecord, NewCustomer};
use crate::models::id_to_string;
use crate::models::session::NewSession;
use crate::models::user::{NewUser, ROLE_ADMIN, ROLE_CUSTOMER};
use crate::repositories::{customer as customer_repo, session as session_repo, user as user_repo};

const EMAIL_IN_USE: &str = "Já existe uma conta com este e-mail";
const MIN_PASSWORD_LEN: usize = 8;
// Limite superior para ninguém mandar uma senha gigante só para gastar CPU no argon2.
const MAX_PASSWORD_LEN: usize = 128;
const SESSION_TTL_HOURS: i64 = 1;
const REFRESH_TTL_DAYS: i64 = 15;

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn ensure_valid_password(password: &str) -> Result<(), AppError> {
    let len = password.chars().count();
    if !(MIN_PASSWORD_LEN..=MAX_PASSWORD_LEN).contains(&len) {
        return Err(AppError::Validation(format!(
            "A senha deve ter entre {MIN_PASSWORD_LEN} e {MAX_PASSWORD_LEN} caracteres"
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
        .map_err(|e| AppError::Internal(format!("falha ao gerar hash da senha: {e}")))
}

/// Hash de uma senha qualquer, usado quando o e-mail não existe. Assim o sign-in
/// gasta o mesmo tempo nos dois casos e o tempo de resposta não revela quais
/// e-mails estão cadastrados.
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password_blocking("senha-descartavel")
            .unwrap_or_else(|_| panic!("falha ao gerar o hash de apoio do sign-in"))
    })
}

// O argon2 é de propósito pesado em CPU; `spawn_blocking` evita travar o runtime async.
async fn hash_password(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || hash_password_blocking(&password))
        .await
        .map_err(|e| AppError::Internal(format!("tarefa de hash falhou: {e}")))?
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
    .map_err(|e| AppError::Internal(format!("tarefa de verificação falhou: {e}")))
}

pub async fn sign_up(db: &Surreal<Any>, input: SignUp) -> Result<Customer, AppError> {
    ensure_valid_password(&input.password)?;
    let email = normalize_email(&input.email);

    if user_repo::find_by_email(&Executor::Db(db), &email)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(EMAIL_IN_USE.to_string()));
    }

    let user = NewUser {
        email: email.clone(),
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
                eprintln!("Falha ao cancelar transação: {cancel_err:?}");
            }
            return Err(e);
        }
    };

    Ok(Customer::from_parts(customer, email))
}

/// Cria o user e o customer com a mesma chave. Numa transação: ou ficam os dois, ou nenhum.
async fn sign_up_in_tx(
    ex: &Executor<'_>,
    user: NewUser,
    profile: NewCustomer,
) -> Result<CustomerRecord, AppError> {
    let user = user_repo::create(ex, user)
        .await?
        .ok_or_else(|| AppError::Internal("falha ao criar user".to_string()))?;

    customer_repo::create(ex, &id_to_string(&user.id), profile)
        .await?
        .ok_or_else(|| AppError::Internal("falha ao criar cliente".to_string()))
}

pub async fn sign_in(db: &Surreal<Any>, input: SignIn) -> Result<TokenPair, AppError> {
    let ex = Executor::Db(db);
    let user = user_repo::find_by_email(&ex, &normalize_email(&input.email)).await?;

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

    let tokens = TokenPair {
        session_token: generate_token(),
        refresh_token: generate_token(),
    };
    let now = Utc::now();

    session_repo::create(
        &ex,
        NewSession {
            user: user.id,
            token_hash: hash_token(&tokens.session_token),
            refresh_hash: hash_token(&tokens.refresh_token),
            expires_at: Datetime::from(now + Duration::hours(SESSION_TTL_HOURS)),
            refresh_expires_at: Datetime::from(now + Duration::days(REFRESH_TTL_DAYS)),
        },
    )
    .await?
    .ok_or_else(|| AppError::Internal("falha ao criar sessão".to_string()))?;

    Ok(tokens)
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

/// Garante que existe um admin com este e-mail (seed da partida). Idempotente: se o
/// e-mail já existe, não troca a senha nem promove um usuário comum em silêncio.
pub async fn ensure_admin(
    db: &Surreal<Any>,
    email: &str,
    password: String,
) -> Result<(), AppError> {
    let ex = Executor::Db(db);
    let email = normalize_email(email);

    if let Some(existing) = user_repo::find_by_email(&ex, &email).await? {
        if existing.role != ROLE_ADMIN {
            eprintln!("ADMIN_EMAIL já pertence a um usuário comum: ele NÃO foi promovido a admin");
        }
        return Ok(());
    }

    ensure_valid_password(&password)?;
    user_repo::create(
        &ex,
        NewUser {
            email,
            password_hash: hash_password(password).await?,
            role: ROLE_ADMIN.to_string(),
        },
    )
    .await?
    .ok_or_else(|| AppError::Internal("falha ao criar admin".to_string()))?;

    println!("Usuário admin criado");
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
