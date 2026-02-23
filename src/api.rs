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
use tokio::sync::broadcast;

use crate::{Engine, InstrumentId, MatchingEngine, Order, OrderId};

/// Payload broadcast to all WebSocket market-data clients when the book changes.
#[derive(Clone, Debug)]
pub struct BookUpdate {
    pub instrument_id: u64,
    pub best_bid: Option<rust_decimal::Decimal>,
    pub best_ask: Option<rust_decimal::Decimal>,
}

/// Shared app state: one engine per process; broadcast channel for market-data updates.
#[derive(Clone)]
pub struct AppState {
    pub engine: std::sync::Arc<Mutex<Engine>>,
    pub(crate) broadcast_tx: broadcast::Sender<BookUpdate>,
}

/// Builds shared app state (engine + broadcast). Use this when you need to share the engine with FIX or other adapters.
pub fn create_app_state(instrument_id: InstrumentId) -> AppState {
    let (broadcast_tx, _) = broadcast::channel(32);
    AppState {
        engine: std::sync::Arc::new(Mutex::new(Engine::new(instrument_id))),
        broadcast_tx,
    }
}

/// Builds the REST/WebSocket router with the given state. Use with [`create_app_state`] when sharing engine with FIX.
pub fn create_router_with_state(state: AppState) -> Router<()> {
    Router::new()
        .route("/health", get(health))
        .route("/orders", post(submit_order))
        .route("/orders/cancel", post(cancel_order))
        .route("/orders/modify", post(modify_order))
        .route("/ws/market-data", get(ws_market_data))
        .layer(Extension(state))
}

/// Builds the REST/WebSocket router with a new state (convenience for tests). Returns `Router<()>` for `axum::serve`.
pub fn create_router(instrument_id: InstrumentId) -> Router<()> {
    create_router_with_state(create_app_state(instrument_id))
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
        let book = guard.book_snapshot();
        MarketDataSnapshot {
            msg_type: "snapshot",
            instrument_id: book.instrument_id.0,
            best_bid: book.best_bid,
            best_ask: book.best_ask,
        }
    };
    let json = match serde_json::to_string(&snapshot) {
        Ok(s) => s,
        Err(_) => return,
    };
    if socket.send(Message::Text(json.into())).await.is_err() {
        return;
    }

    let mut rx = state.broadcast_tx.subscribe();
    loop {
        tokio::select! {
            res = rx.recv() => {
                match res {
                    Ok(update) => {
                        let msg = MarketDataSnapshot {
                            msg_type: "snapshot",
                            instrument_id: update.instrument_id,
                            best_bid: update.best_bid,
                            best_ask: update.best_ask,
                        };
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if socket.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = socket.recv() => match msg {
                Some(Ok(_)) => {}
                _ => break,
            },
        }
    }
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
    let update = BookUpdate {
        instrument_id: guard.instrument_id().0,
        best_bid: guard.best_bid(),
        best_ask: guard.best_ask(),
    };
    drop(guard);
    let _ = state.broadcast_tx.send(update);
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
    let out = match guard.modify_order(order_id, &body.replacement) {
        Ok((trades, reports)) => {
            let update = BookUpdate {
                instrument_id: guard.instrument_id().0,
                best_bid: guard.best_bid(),
                best_ask: guard.best_ask(),
            };
            drop(guard);
            let _ = state.broadcast_tx.send(update);
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
    };
    out
}

async fn submit_order(
    Extension(state): Extension<AppState>,
    Json(order): Json<Order>,
) -> Response {
    let mut guard = state.engine.lock().expect("lock");
    match guard.submit_order(order) {
        Ok((trades, reports)) => {
            let update = BookUpdate {
                instrument_id: guard.instrument_id().0,
                best_bid: guard.best_bid(),
                best_ask: guard.best_ask(),
            };
            drop(guard);
            let _ = state.broadcast_tx.send(update);
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
