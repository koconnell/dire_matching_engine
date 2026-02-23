//! REST API router for the matching engine (Phase 2).
//!
//! Used by the binary and by integration tests. Create with [`create_router`].
//! Uses Extension for state so the router is `Router<()>` and works with `into_make_service()`.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension,
    },
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
        .route("/ws/market-data", get(ws_market_data))
        .layer(Extension(state))
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// WebSocket market-data: on connect send one snapshot (best bid/ask), then keep connection open.
async fn ws_market_data(
    Extension(state): Extension<AppState>,
    upgrade: WebSocketUpgrade,
) -> Response {
    upgrade.on_upgrade(move |socket| handle_market_data_socket(state, socket))
}

#[derive(serde::Serialize)]
struct MarketDataSnapshot {
    #[serde(rename = "type")]
    msg_type: &'static str,
    instrument_id: u64,
    best_bid: Option<rust_decimal::Decimal>,
    best_ask: Option<rust_decimal::Decimal>,
}

async fn handle_market_data_socket(state: AppState, mut socket: WebSocket) {
    let snapshot = {
        let guard = state.engine.lock().expect("lock");
        MarketDataSnapshot {
            msg_type: "snapshot",
            instrument_id: guard.instrument_id().0,
            best_bid: guard.best_bid(),
            best_ask: guard.best_ask(),
        }
    };
    let json = match serde_json::to_string(&snapshot) {
        Ok(s) => s,
        Err(_) => return,
    };
    if socket.send(Message::Text(json.into())).await.is_err() {
        return;
    }
    // Keep connection open (e.g. for future broadcast); ignore incoming frames
    while let Some(Ok(_)) = socket.recv().await {}
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
