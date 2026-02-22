//! REST API router for the matching engine (Phase 2).
//!
//! Used by the binary and by integration tests. Create with [`create_router`].
//! Uses Extension for state so the router is `Router<()>` and works with `into_make_service()`.

use axum::{
    extract::Extension,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::sync::Mutex;

use crate::{Engine, InstrumentId, Order, OrderId};

/// Shared app state: one engine per process.
#[derive(Clone)]
pub struct AppState {
    pub(crate) engine: std::sync::Arc<Mutex<Engine>>,
}

/// Builds the REST router with state. Returns `Router<()>` so you can call `.into_make_service()` for `axum::serve`.
pub fn create_router(instrument_id: InstrumentId) -> Router<()> {
    let state = AppState {
        engine: std::sync::Arc::new(Mutex::new(Engine::new(instrument_id))),
    };
    Router::new()
        .route("/health", get(health))
        .route("/orders", post(submit_order))
        .route("/orders/cancel", post(cancel_order))
        .route("/orders/modify", post(modify_order))
        .layer(Extension(state))
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

#[derive(serde::Deserialize)]
struct CancelRequest {
    order_id: u64,
}

async fn cancel_order(
    Extension(state): Extension<AppState>,
    Json(body): Json<CancelRequest>,
) -> Response {
    let mut guard = state.engine.lock().expect("lock");
    let removed = guard.cancel_order(OrderId(body.order_id));
    #[derive(serde::Serialize)]
    struct Out {
        canceled: bool,
    }
    (StatusCode::OK, Json(Out { canceled: removed })).into_response()
}

#[derive(serde::Deserialize)]
struct ModifyRequest {
    order_id: u64,
    replacement: Order,
}

async fn modify_order(
    Extension(state): Extension<AppState>,
    Json(body): Json<ModifyRequest>,
) -> Response {
    let mut guard = state.engine.lock().expect("lock");
    let order_id = OrderId(body.order_id);
    match guard.modify_order(order_id, &body.replacement) {
        Ok((trades, reports)) => {
            #[derive(serde::Serialize)]
            struct Out {
                trades: Vec<crate::Trade>,
                reports: Vec<crate::ExecutionReport>,
            }
            (StatusCode::OK, Json(Out { trades, reports })).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn submit_order(
    Extension(state): Extension<AppState>,
    Json(order): Json<Order>,
) -> Response {
    let mut guard = state.engine.lock().expect("lock");
    match guard.submit_order(order) {
        Ok((trades, reports)) => {
            #[derive(serde::Serialize)]
            struct Out {
                trades: Vec<crate::Trade>,
                reports: Vec<crate::ExecutionReport>,
            }
            (StatusCode::OK, Json(Out { trades, reports })).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}
