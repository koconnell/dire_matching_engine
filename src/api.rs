//! REST API router for the matching engine (Phase 2).
//!
//! Used by the binary and by integration tests. Create with [`create_router`].
//! Uses Extension for state so the router is `Router<()>` and works with `into_make_service()`.
//! Phase 3: API key auth on order/WebSocket routes when auth is enabled; /health stays public.

use axum::{
    body::Body,
    extract::{
        Path,
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension,
        Request,
    },
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::broadcast;

use crate::audit::{AuditEvent, AuditSink, StdoutAuditSink};
use crate::auth::{self, AuthConfig, AuthUser};
use crate::{InstrumentId, MatchingEngine, MultiEngine, Order, OrderId};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Phase 3 §4: Admin API — market state, instruments, config
// ---------------------------------------------------------------------------

/// Market state (US-011, US-012). When not Open, order submission is rejected (503 / FIX reject).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketState {
    Open,
    Halted,
    Closed,
}

impl MarketState {
    pub fn as_str(&self) -> &'static str {
        match self {
            MarketState::Open => "Open",
            MarketState::Halted => "Halted",
            MarketState::Closed => "Closed",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Open" => Some(MarketState::Open),
            "Halted" => Some(MarketState::Halted),
            "Closed" => Some(MarketState::Closed),
            _ => None,
        }
    }
}

/// Payload broadcast to all WebSocket market-data clients when the book changes.
#[derive(Clone, Debug)]
pub struct BookUpdate {
    pub instrument_id: u64,
    pub best_bid: Option<rust_decimal::Decimal>,
    pub best_ask: Option<rust_decimal::Decimal>,
}

/// Shared app state: multi-instrument engine; broadcast; audit sink; market state and admin config (Phase 3 §4).
#[derive(Clone)]
pub struct AppState {
    pub engine: std::sync::Arc<Mutex<MultiEngine>>,
    pub(crate) broadcast_tx: broadcast::Sender<BookUpdate>,
    pub(crate) audit_sink: Arc<dyn AuditSink + Send + Sync>,
    /// Market state: when not Open, REST and FIX reject new orders (503 / FIX reject).
    pub market_state: Arc<Mutex<MarketState>>,
    /// Admin config key-value store (US-009). Keys are strings; values are JSON.
    pub admin_config: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

/// Builds shared app state (multi-instrument engine + broadcast + stdout audit + Open market state). Use this when you need to share the engine with FIX or other adapters.
pub fn create_app_state(instrument_id: InstrumentId) -> AppState {
    create_app_state_with_instruments(vec![(instrument_id, None)])
}

/// Builds shared app state with multiple initial instruments. Each entry is (instrument_id, optional symbol).
pub fn create_app_state_with_instruments(initial: Vec<(InstrumentId, Option<String>)>) -> AppState {
    create_app_state_with_sink_and_instruments(initial, Arc::new(StdoutAuditSink))
}

/// Like [`create_app_state`] but with a single instrument and an explicit audit sink (e.g. [`crate::audit::InMemoryAuditSink`] for tests).
pub fn create_app_state_with_sink(instrument_id: InstrumentId, audit_sink: Arc<dyn AuditSink + Send + Sync>) -> AppState {
    create_app_state_with_sink_and_instruments(vec![(instrument_id, None)], audit_sink)
}

/// Like [`create_app_state_with_instruments`] but with an explicit audit sink.
pub fn create_app_state_with_sink_and_instruments(
    initial: Vec<(InstrumentId, Option<String>)>,
    audit_sink: Arc<dyn AuditSink + Send + Sync>,
) -> AppState {
    let (broadcast_tx, _) = broadcast::channel(32);
    AppState {
        engine: std::sync::Arc::new(Mutex::new(MultiEngine::new_with_instruments(initial))),
        broadcast_tx,
        audit_sink,
        market_state: Arc::new(Mutex::new(MarketState::Open)),
        admin_config: Arc::new(Mutex::new(HashMap::new())),
    }
}

/// Builds the REST/WebSocket router with the given state. Use with [`create_app_state`] when sharing engine with FIX.
/// When auth is enabled (API_KEYS set, DISABLE_AUTH not true), /orders, /orders/cancel, /orders/modify, and
/// /ws/market-data require a valid API key (Authorization: Bearer &lt;key&gt; or X-API-Key). /health is always public.
/// Pass `auth_config` to override env (e.g. tests can pass a fixed config to avoid env races).
pub fn create_router_with_state(state: AppState) -> Router<()> {
    create_router_with_state_and_auth(state, None)
}

/// Like [`create_router_with_state`] but with explicit auth config (when `Some`, used instead of env).
pub fn create_router_with_state_and_auth(state: AppState, auth_config_override: Option<AuthConfig>) -> Router<()> {
    let auth_config = auth_config_override.unwrap_or_else(AuthConfig::from_env);

    let protected = Router::new()
        .route("/orders", post(submit_order))
        .route("/orders/cancel", post(cancel_order))
        .route("/orders/modify", post(modify_order))
        .route("/ws/market-data", get(ws_market_data))
        .route("/admin/status", get(admin_status))
        .route("/admin/instruments", get(admin_instruments_list).post(admin_instruments_post))
        .route("/admin/instruments/:id", delete(admin_instruments_delete))
        .route("/admin/config", get(admin_config_get).patch(admin_config_patch))
        .route("/admin/market-state", get(admin_market_state_get).post(admin_market_state_post))
        .route("/admin/emergency-halt", post(admin_emergency_halt))
        .layer(Extension(state.clone()))
        .route_layer(middleware::from_fn(move |req: Request<Body>, next: Next| {
            let config = auth_config.clone();
            async move { auth::require_api_key_or_anonymous(req, next, config).await }
        }));

    Router::new()
        .route("/health", get(health))
        .layer(Extension(state))
        .merge(protected)
}

/// Builds the REST/WebSocket router with a new state (convenience for tests). Returns `Router<()>` for `axum::serve`.
pub fn create_router(instrument_id: InstrumentId) -> Router<()> {
    create_router_with_state(create_app_state(instrument_id))
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Admin-only: returns 200 with status. Requires Admin or Operator role (403 for Trader).
async fn admin_status(Extension(auth): Extension<AuthUser>) -> Response {
    auth::require_admin_or_operator(&auth)
        .map_err(|r| r)
        .map(|()| (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response())
        .unwrap_or_else(|r| r)
}

// --- Admin API (US-008, US-009, US-011, US-012) ---

async fn admin_instruments_list(
    Extension(auth): Extension<AuthUser>,
    Extension(state): Extension<AppState>,
) -> Response {
    auth::require_admin_or_operator(&auth)
        .map_err(|r| r)
        .and_then(|()| {
            let guard = state.engine.lock().expect("lock");
            let list: Vec<serde_json::Value> = guard
                .list_instruments()
                .into_iter()
                .map(|(id, symbol)| {
                    let mut obj = serde_json::json!({ "instrument_id": id.0 });
                    if let Some(s) = symbol {
                        obj["symbol"] = serde_json::Value::String(s);
                    }
                    obj
                })
                .collect();
            Ok((StatusCode::OK, Json(list)).into_response())
        })
        .unwrap_or_else(|r| r)
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct AdminInstrumentsPostBody {
    instrument_id: u64,
    symbol: Option<String>,
}

async fn admin_instruments_post(
    Extension(auth): Extension<AuthUser>,
    Extension(state): Extension<AppState>,
    Json(body): Json<AdminInstrumentsPostBody>,
) -> Response {
    auth::require_admin_or_operator(&auth)
        .map_err(|r| r)
        .and_then(|()| {
            let mut guard = state.engine.lock().expect("lock");
            match guard.add_instrument(InstrumentId(body.instrument_id), body.symbol) {
                Ok(()) => Ok((StatusCode::CREATED, Json(serde_json::json!({ "instrument_id": body.instrument_id }))).into_response()),
                Err(e) => {
                    let status = if e.contains("already exists") {
                        StatusCode::CONFLICT
                    } else {
                        StatusCode::BAD_REQUEST
                    };
                    Err((status, Json(serde_json::json!({ "error": e }))).into_response())
                }
            }
        })
        .unwrap_or_else(|r| r)
}

async fn admin_instruments_delete(
    Extension(auth): Extension<AuthUser>,
    Extension(state): Extension<AppState>,
    Path(id): Path<u64>,
) -> Response {
    auth::require_admin_or_operator(&auth)
        .map_err(|r| r)
        .and_then(|()| {
            let mut guard = state.engine.lock().expect("lock");
            match guard.remove_instrument(InstrumentId(id)) {
                Ok(()) => Ok((StatusCode::NO_CONTENT, ()).into_response()),
                Err(e) => {
                    let status = if e.contains("not found") {
                        StatusCode::NOT_FOUND
                    } else if e.contains("resting orders") {
                        StatusCode::CONFLICT
                    } else {
                        StatusCode::BAD_REQUEST
                    };
                    Err((status, Json(serde_json::json!({ "error": e }))).into_response())
                }
            }
        })
        .unwrap_or_else(|r| r)
}

async fn admin_config_get(
    Extension(auth): Extension<AuthUser>,
    Extension(state): Extension<AppState>,
) -> Response {
    auth::require_admin_or_operator(&auth)
        .map_err(|r| r)
        .and_then(|()| {
            let guard = state.admin_config.lock().expect("lock");
            let config: serde_json::Map<String, serde_json::Value> = guard.clone().into_iter().collect();
            Ok((StatusCode::OK, Json(serde_json::Value::Object(config))).into_response())
        })
        .unwrap_or_else(|r| r)
}

async fn admin_config_patch(
    Extension(auth): Extension<AuthUser>,
    Extension(state): Extension<AppState>,
    Json(patch): Json<serde_json::Value>,
) -> Response {
    auth::require_admin_or_operator(&auth)
        .map_err(|r| r)
        .and_then(|()| {
            let obj = patch.as_object().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "config must be a JSON object" })),
                )
                    .into_response()
            })?;
            let mut guard = state.admin_config.lock().expect("lock");
            for (k, v) in obj {
                guard.insert(k.clone(), v.clone());
            }
            Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response())
        })
        .unwrap_or_else(|r| r)
}

async fn admin_market_state_get(
    Extension(auth): Extension<AuthUser>,
    Extension(state): Extension<AppState>,
) -> Response {
    auth::require_admin_or_operator(&auth)
        .map_err(|r| r)
        .and_then(|()| {
            let guard = state.market_state.lock().expect("lock");
            let s = guard.as_str();
            Ok((StatusCode::OK, Json(serde_json::json!({ "state": s }))).into_response())
        })
        .unwrap_or_else(|r| r)
}

#[derive(serde::Deserialize)]
struct AdminMarketStatePostBody {
    state: String,
}

async fn admin_market_state_post(
    Extension(auth): Extension<AuthUser>,
    Extension(state): Extension<AppState>,
    Json(body): Json<AdminMarketStatePostBody>,
) -> Response {
    let actor = auth.key_id.as_deref().unwrap_or("anonymous").to_string();
    auth::require_admin_or_operator(&auth)
        .map_err(|r| r)
        .and_then(|()| {
            let new_state = MarketState::from_str(body.state.trim())
                .ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": "state must be Open, Halted, or Closed" })),
                    )
                        .into_response()
                })?;
            *state.market_state.lock().expect("lock") = new_state;
            state.audit_sink.emit(&AuditEvent::now(
                actor,
                "market_state_change",
                Some(serde_json::json!({ "state": new_state.as_str() })),
                "success",
            ));
            Ok((StatusCode::OK, Json(serde_json::json!({ "state": new_state.as_str() }))).into_response())
        })
        .unwrap_or_else(|r| r)
}

async fn admin_emergency_halt(
    Extension(auth): Extension<AuthUser>,
    Extension(state): Extension<AppState>,
) -> Response {
    let actor = auth.key_id.as_deref().unwrap_or("anonymous").to_string();
    auth::require_admin_or_operator(&auth)
        .map_err(|r| r)
        .and_then(|()| {
            *state.market_state.lock().expect("lock") = MarketState::Halted;
            state.audit_sink.emit(&AuditEvent::now(
                actor,
                "emergency_halt",
                Some(serde_json::json!({ "state": "Halted" })),
                "success",
            ));
            Ok((
                StatusCode::OK,
                Json(serde_json::json!({ "state": "Halted", "message": "emergency halt applied" })),
            )
                .into_response())
        })
        .unwrap_or_else(|r| r)
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
    let snapshots: Vec<MarketDataSnapshot> = {
        let guard = state.engine.lock().expect("lock");
        guard
            .instruments()
            .into_iter()
            .filter_map(|id| {
                guard.book_snapshot_for(id).map(|book| MarketDataSnapshot {
                    msg_type: "snapshot",
                    instrument_id: book.instrument_id.0,
                    best_bid: book.best_bid,
                    best_ask: book.best_ask,
                })
            })
            .collect()
    };
    for snapshot in snapshots {
        let json = match serde_json::to_string(&snapshot) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if socket.send(Message::Text(json.into())).await.is_err() {
            return;
        }
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
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CancelRequest>,
) -> Response {
    let actor = auth.key_id.as_deref().unwrap_or("anonymous").to_string();
    let order_id = body.order_id;
    let mut guard = state.engine.lock().expect("lock");
    let removed = guard.cancel_order(OrderId(order_id));
    let update = removed.and_then(|instrument_id| {
        guard.book_snapshot_for(instrument_id).map(|s| BookUpdate {
            instrument_id: s.instrument_id.0,
            best_bid: s.best_bid,
            best_ask: s.best_ask,
        })
    });
    drop(guard);
    if let Some(u) = update {
        let _ = state.broadcast_tx.send(u);
    }
    state.audit_sink.emit(&AuditEvent::now(
        actor,
        "order_cancel",
        Some(serde_json::json!({ "order_id": order_id })),
        if removed.is_some() { "success" } else { "not_found" },
    ));
    #[derive(serde::Serialize)]
    struct Out {
        canceled: bool,
    }
    (StatusCode::OK, Json(Out { canceled: removed.is_some() })).into_response()
}

#[derive(serde::Deserialize)]
struct ModifyRequest {
    order_id: u64,
    replacement: Order,
}

async fn modify_order(
    Extension(state): Extension<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<ModifyRequest>,
) -> Response {
    if *state.market_state.lock().expect("lock") != MarketState::Open {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "market not open" })),
        )
            .into_response();
    }
    let actor = auth.key_id.as_deref().unwrap_or("anonymous").to_string();
    let order_id = body.order_id;
    let mut guard = state.engine.lock().expect("lock");
    let out = match guard.modify_order(OrderId(order_id), &body.replacement) {
        Ok((trades, reports)) => {
            let instrument_id = body.replacement.instrument_id;
            let update = guard
                .book_snapshot_for(instrument_id)
                .map(|s| BookUpdate {
                    instrument_id: s.instrument_id.0,
                    best_bid: s.best_bid,
                    best_ask: s.best_ask,
                });
            drop(guard);
            if let Some(u) = update {
                let _ = state.broadcast_tx.send(u);
            }
            state.audit_sink.emit(&AuditEvent::now(
                actor.clone(),
                "order_modify",
                Some(serde_json::json!({ "order_id": order_id, "replacement_order_id": body.replacement.order_id.0 })),
                "success",
            ));
            #[derive(serde::Serialize)]
            struct Out {
                trades: Vec<crate::Trade>,
                reports: Vec<crate::ExecutionReport>,
            }
            (StatusCode::OK, Json(Out { trades, reports })).into_response()
        }
        Err(e) => {
            state.audit_sink.emit(&AuditEvent::now(
                actor,
                "order_modify",
                Some(serde_json::json!({ "order_id": order_id })),
                "rejected",
            ));
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e })),
            )
                .into_response()
        }
    };
    out
}

async fn submit_order(
    Extension(state): Extension<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(order): Json<Order>,
) -> Response {
    if *state.market_state.lock().expect("lock") != MarketState::Open {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "market not open" })),
        )
            .into_response();
    }
    let actor = auth.key_id.as_deref().unwrap_or("anonymous").to_string();
    let order_id = order.order_id.0;
    let instrument_id = order.instrument_id;
    let mut guard = state.engine.lock().expect("lock");
    match guard.submit_order(order) {
        Ok((trades, reports)) => {
            let update = guard
                .book_snapshot_for(instrument_id)
                .map(|s| BookUpdate {
                    instrument_id: s.instrument_id.0,
                    best_bid: s.best_bid,
                    best_ask: s.best_ask,
                });
            drop(guard);
            if let Some(u) = update {
                let _ = state.broadcast_tx.send(u);
            }
            state.audit_sink.emit(&AuditEvent::now(
                actor,
                "order_submit",
                Some(serde_json::json!({ "order_id": order_id, "instrument_id": instrument_id.0 })),
                "success",
            ));
            #[derive(serde::Serialize)]
            struct Out {
                trades: Vec<crate::Trade>,
                reports: Vec<crate::ExecutionReport>,
            }
            (StatusCode::OK, Json(Out { trades, reports })).into_response()
        }
        Err(e) => {
            state.audit_sink.emit(&AuditEvent::now(
                actor,
                "order_submit",
                Some(serde_json::json!({ "order_id": order_id, "instrument_id": instrument_id.0 })),
                "rejected",
            ));
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e })),
            )
                .into_response()
        }
    }
}
