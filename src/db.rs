use surrealdb::Surreal;
use surrealdb::engine::any::{self, Any};

pub async fn connect_db() -> surrealdb::Result<Surreal<Any>> {
    connect_db_at("surrealkv://data/rust-store").await
}

/// Abre o banco no endereço dado e aplica o schema; os testes usam um diretório temporário.
pub async fn connect_db_at(url: &str) -> surrealdb::Result<Surreal<Any>> {
    let db = any::connect(url).await?;
    db.use_ns("rust_store").use_db("main").await?;

    db.query(include_str!("../schema.surql")).await?.check()?;

    Ok(db)
}
