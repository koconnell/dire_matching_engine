//! HTTP server for the matching engine (Phase 2: REST).
//!
//! Endpoints: health, submit order, cancel order, modify order. OpenAPI spec: see `project_docs/openapi.yaml`.

use dire_matching_engine::api;
use dire_matching_engine::InstrumentId;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let _ = env_logger::try_init();
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

    let app = api::create_router(instrument_id);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await.expect("bind");
    eprintln!("listening on http://{}", addr);
    axum::serve(listener, app.into_make_service())
        .await
        .expect("serve");
}
