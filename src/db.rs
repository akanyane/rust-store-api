use surrealdb::Surreal;
use surrealdb::engine::any::{self, Any};

pub async fn connect_db() -> surrealdb::Result<Surreal<Any>> {
    let db = any::connect("surrealkv://data/rust-store").await?;
    db.use_ns("rust_store").use_db("main").await?;

    db.query(include_str!("../schema.surql")).await?.check()?;

    Ok(db)
}
