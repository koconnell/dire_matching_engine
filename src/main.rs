//! HTTP API for the matching engine (health + submit order).
//!
//! Used for deployment: Kubernetes probes hit `/health`; clients submit orders via `POST /orders`.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use dire_matching_engine::{Engine, InstrumentId, Order};
use std::sync::Mutex;
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
    engine: std::sync::Arc<Mutex<Engine>>,
}

#[tokio::main]
async fn main() {
    let instrument_id = InstrumentId(
        std::env::var("INSTRUMENT_ID")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1),
    );
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let state = AppState {
        engine: std::sync::Arc::new(Mutex::new(Engine::new(instrument_id))),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/orders", post(submit_order))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await.expect("bind");
    eprintln!("listening on http://{}", addr);
    axum::serve(listener, app).await.expect("serve");
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn submit_order(
    State(state): State<AppState>,
    Json(order): Json<Order>,
) -> Response {
    let mut guard = state.engine.lock().expect("lock");
    match guard.submit_order(order) {
        Ok((trades, reports)) => {
            #[derive(serde::Serialize)]
            struct Out {
                trades: Vec<dire_matching_engine::Trade>,
                reports: Vec<dire_matching_engine::ExecutionReport>,
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
