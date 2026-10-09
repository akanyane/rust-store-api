use surrealdb::{
    Surreal,
    engine::any::Any,
    method::Transaction,
    types::{RecordId, SurrealValue},
};

/// Onde uma operação de banco é executada: direto na conexão ou dentro de
/// uma transação. Assim os repositories não precisam saber qual dos dois é.
pub enum Executor<'a> {
    Db(&'a Surreal<Any>),
    Tx(&'a Transaction<Any>),
}

impl Executor<'_> {
    pub async fn create<D, R>(&self, table: &str, data: D) -> surrealdb::Result<Option<R>>
    where
        D: SurrealValue,
        R: SurrealValue,
    {
        match self {
            Executor::Db(db) => db.create(table.to_string()).content(data).await,
            Executor::Tx(tx) => tx.create(table.to_string()).content(data).await,
        }
    }

    /// Cria um registro com ID escolhido por nós (ex.: `customer:<chave do user>`).
    pub async fn create_with_id<D, R>(&self, id: RecordId, data: D) -> surrealdb::Result<Option<R>>
    where
        D: SurrealValue,
        R: SurrealValue,
    {
        match self {
            Executor::Db(db) => db.create(id).content(data).await,
            Executor::Tx(tx) => tx.create(id).content(data).await,
        }
    }

    pub async fn select_all<R>(&self, table: &str) -> surrealdb::Result<Vec<R>>
    where
        R: SurrealValue,
    {
        match self {
            Executor::Db(db) => db.select(table.to_string()).await,
            Executor::Tx(tx) => tx.select(table.to_string()).await,
        }
    }

    pub async fn select_one<R>(&self, id: RecordId) -> surrealdb::Result<Option<R>>
    where
        R: SurrealValue,
    {
        match self {
            Executor::Db(db) => db.select(id).await,
            Executor::Tx(tx) => tx.select(id).await,
        }
    }

    /// Substitui o conteúdo de um registro. Devolve `None` se o ID não existir.
    pub async fn update_one<D, R>(&self, id: RecordId, data: D) -> surrealdb::Result<Option<R>>
    where
        D: SurrealValue,
        R: SurrealValue,
    {
        match self {
            Executor::Db(db) => db.update(id).content(data).await,
            Executor::Tx(tx) => tx.update(id).content(data).await,
        }
    }

    pub async fn delete_one<R>(&self, id: RecordId) -> surrealdb::Result<Option<R>>
    where
        R: SurrealValue,
    {
        match self {
            Executor::Db(db) => db.delete(id).await,
            Executor::Tx(tx) => tx.delete(id).await,
        }
    }

    /// Roda um SELECT com um parâmetro nomeado (`$name`) e devolve todas as linhas.
    pub async fn query_all<R, V>(
        &self,
        sql: &str,
        name: &str,
        value: V,
    ) -> surrealdb::Result<Vec<R>>
    where
        R: SurrealValue,
        V: SurrealValue,
    {
        let mut response = match self {
            Executor::Db(db) => {
                db.query(sql.to_string())
                    .bind((name.to_string(), value))
                    .await?
            }
            Executor::Tx(tx) => {
                tx.query(sql.to_string())
                    .bind((name.to_string(), value))
                    .await?
            }
        };
        response.take(0)
    }
}
