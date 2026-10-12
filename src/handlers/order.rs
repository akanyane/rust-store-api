use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
};
use chrono::Utc;

use crate::error::AppError;
use crate::extractors::{AuthAdmin, AuthCustomer, ValidatedJson, ValidatedQuery};
use crate::handlers::total_count;
use crate::models::order::{
    AdminOrderView, ExportOrdersQuery, ListOrdersQuery, OrderView, UpdateOrderStatus,
};
use crate::services::order as order_service;
use crate::state::AppState;

pub async fn checkout(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
) -> Result<(StatusCode, Json<OrderView>), AppError> {
    let order = order_service::checkout(&state.db, &customer_id).await?;
    Ok((StatusCode::CREATED, Json(order)))
}

pub async fn list_orders(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
) -> Result<Json<Vec<OrderView>>, AppError> {
    let orders = order_service::list_orders(&state.db, &customer_id).await?;
    Ok(Json(orders))
}

pub async fn get_order(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
    Path(order_id): Path<String>,
) -> Result<Json<OrderView>, AppError> {
    let order = order_service::get_order(&state.db, &customer_id, &order_id).await?;
    Ok(Json(order))
}

pub async fn cancel_order(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
    Path(order_id): Path<String>,
) -> Result<Json<OrderView>, AppError> {
    let order = order_service::cancel_order(&state.db, &customer_id, &order_id).await?;
    Ok(Json(order))
}

pub async fn pay_order(
    State(state): State<AppState>,
    AuthCustomer(customer_id): AuthCustomer,
    Path(order_id): Path<String>,
) -> Result<Json<OrderView>, AppError> {
    let order = order_service::pay_order(&state.db, &customer_id, &order_id).await?;
    Ok(Json(order))
}

pub async fn admin_list_orders(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    ValidatedQuery(query): ValidatedQuery<ListOrdersQuery>,
) -> Result<([(HeaderName, String); 1], Json<Vec<AdminOrderView>>), AppError> {
    let (orders, total) = order_service::admin_list_orders(&state.db, query).await?;
    Ok((total_count(total), Json(orders)))
}

pub async fn admin_get_order(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(order_id): Path<String>,
) -> Result<Json<AdminOrderView>, AppError> {
    let order = order_service::admin_get_order(&state.db, &order_id).await?;
    Ok(Json(order))
}

pub async fn admin_update_status(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(order_id): Path<String>,
    ValidatedJson(input): ValidatedJson<UpdateOrderStatus>,
) -> Result<Json<AdminOrderView>, AppError> {
    let order = order_service::admin_update_status(&state.db, &order_id, input.status).await?;
    Ok(Json(order))
}

/// O CSV dos pedidos que casam com o filtro e a busca, para baixar. `X-Total-Count` diz quantos
/// casam, e `X-Export-Truncated` aparece quando passam do teto e o arquivo traz só os mais novos.
pub async fn admin_export_orders(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    ValidatedQuery(query): ValidatedQuery<ExportOrdersQuery>,
) -> Result<(HeaderMap, String), AppError> {
    let (csv, total, truncated) = order_service::admin_export_orders_csv(&state.db, query).await?;

    let day = Utc::now().format("%Y-%m-%d").to_string();
    Ok((export_headers(total, truncated, &day)?, csv))
}

/// Os cabeçalhos do download: tipo, nome do arquivo (com o dia), total que casa e, se o arquivo
/// foi cortado no teto, o aviso.
fn export_headers(total: i64, truncated: bool, day: &str) -> Result<HeaderMap, AppError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"orders-{day}.csv\""))
            .map_err(|_| AppError::Internal("invalid export filename".to_string()))?,
    );
    headers.insert(
        HeaderName::from_static("x-total-count"),
        HeaderValue::from(total),
    );
    if truncated {
        headers.insert(
            HeaderName::from_static("x-export-truncated"),
            HeaderValue::from_static("true"),
        );
    }
    Ok(headers)
}

#[cfg(test)]
mod export_tests {
    use super::*;

    #[test]
    fn the_truncation_warning_appears_only_when_the_file_was_cut() {
        let cut = export_headers(12_000, true, "2026-10-12").unwrap();
        assert_eq!(cut.get("x-export-truncated").unwrap(), "true");
        assert_eq!(cut.get("x-total-count").unwrap(), "12000");

        let whole = export_headers(7, false, "2026-10-12").unwrap();
        assert!(whole.get("x-export-truncated").is_none());
        assert_eq!(whole.get("x-total-count").unwrap(), "7");
    }

    #[test]
    fn it_is_a_csv_download_named_after_the_day() {
        let headers = export_headers(1, false, "2026-10-12").unwrap();
        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap(),
            "text/csv; charset=utf-8"
        );
        assert_eq!(
            headers.get(header::CONTENT_DISPOSITION).unwrap(),
            "attachment; filename=\"orders-2026-10-12.csv\""
        );
    }
}
