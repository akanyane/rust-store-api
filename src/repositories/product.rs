use surrealdb::types::{RecordId, SurrealValue};

use crate::executor::Executor;
use crate::models::product::{
    ActivePage, CountRow, NewProduct, ProductChanges, ProductPage, ProductRecord, ProductSort,
};

/// Uma página de produtos ativos, por nome ou do mais recente para o mais antigo (os sem
/// data ficam no fim); o id desempata, para a ordem não oscilar entre páginas.
pub async fn find_active(
    ex: &Executor<'_>,
    sort: ProductSort,
    limit: i64,
    offset: i64,
) -> surrealdb::Result<Vec<ProductRecord>> {
    let query = match sort {
        ProductSort::Name => {
            "SELECT * FROM product WHERE active = $page.active ORDER BY name, id \
             LIMIT $page.limit START $page.offset"
        }
        ProductSort::Newest => {
            "SELECT * FROM product WHERE active = $page.active ORDER BY created_at DESC, id \
             LIMIT $page.limit START $page.offset"
        }
    };
    let page = ActivePage {
        active: true,
        limit,
        offset,
    };
    ex.query_all(query, "page", page).await
}

/// Quantos produtos ativos existem, independente de página.
pub async fn count_active(ex: &Executor<'_>) -> surrealdb::Result<i64> {
    let rows: Vec<CountRow> = ex
        .query_all(
            "SELECT count() AS count FROM product WHERE active = $active GROUP ALL",
            "active",
            true,
        )
        .await?;
    // Sem nenhum ativo o `GROUP ALL` pode não devolver linha nenhuma.
    Ok(rows.first().map_or(0, |row| row.count))
}

pub async fn find_by_id(ex: &Executor<'_>, id: &str) -> surrealdb::Result<Option<ProductRecord>> {
    ex.select_one(RecordId::new("product", id.to_string()))
        .await
}

pub async fn find_by_ids(
    ex: &Executor<'_>,
    ids: Vec<RecordId>,
) -> surrealdb::Result<Vec<ProductRecord>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    ex.query_all("SELECT * FROM product WHERE id IN $ids", "ids", ids)
        .await
}

pub async fn create(
    ex: &Executor<'_>,
    data: NewProduct,
) -> surrealdb::Result<Option<ProductRecord>> {
    ex.create("product", data).await
}

/// Parâmetro único do `update` (o `query_all` aceita só um).
#[derive(SurrealValue)]
struct ProductUpdate {
    id: RecordId,
    name: String,
    description: String,
    active: bool,
}

/// Atualiza só nome, descrição e `active`. Usa `SET` e não `CONTENT`, que substitui o
/// registro inteiro e apagaria os outros campos, como o `created_at`.
pub async fn update(
    ex: &Executor<'_>,
    id: RecordId,
    data: ProductChanges,
) -> surrealdb::Result<Option<ProductRecord>> {
    let rows: Vec<ProductRecord> = ex
        .query_all(
            "UPDATE $change.id SET name = $change.name, \
             description = $change.description, active = $change.active",
            "change",
            ProductUpdate {
                id,
                name: data.name,
                description: data.description,
                active: data.active,
            },
        )
        .await?;
    Ok(rows.into_iter().next())
}

/// Produtos ativos e inativos, com filtro opcional de `active` e paginação. Ordem: por nome
/// ou do mais recente para o mais antigo (produtos sem data ficam no fim); o id desempata,
/// para a ordem não oscilar entre páginas.
pub async fn find_page(
    ex: &Executor<'_>,
    page: ProductPage,
    sort: ProductSort,
) -> surrealdb::Result<Vec<ProductRecord>> {
    let query = match sort {
        ProductSort::Name => {
            "SELECT * FROM product WHERE ($page.active = NONE OR active = $page.active) \
             ORDER BY name, id LIMIT $page.limit START $page.offset"
        }
        ProductSort::Newest => {
            "SELECT * FROM product WHERE ($page.active = NONE OR active = $page.active) \
             ORDER BY created_at DESC, id LIMIT $page.limit START $page.offset"
        }
    };
    ex.query_all(query, "page", page).await
}
