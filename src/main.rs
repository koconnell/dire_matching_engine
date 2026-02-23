//! HTTP server and FIX 4.4 acceptor for the matching engine (Phase 2).
//!
//! REST: health, submit order, cancel order, modify order. WebSocket: /ws/market-data.
//! FIX: TCP acceptor on FIX_PORT (default 9876). Same engine backs all protocols.

use dire_matching_engine::api;
use dire_matching_engine::fix;
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
    let fix_port: u16 = std::env::var("FIX_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9876);

    let state = api::create_app_state(instrument_id);
    let app = api::create_router_with_state(state.clone());

    let fix_addr = format!("0.0.0.0:{}", fix_port);
    let fix_listener = std::net::TcpListener::bind(&fix_addr).expect("FIX bind");
    let engine = state.engine.clone();
    std::thread::spawn(move || {
        fix::run_fix_acceptor(fix_listener, engine, instrument_id);
    });
    eprintln!("FIX acceptor on {}", fix_addr);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await.expect("bind");
    eprintln!("listening on http://{}", addr);
    axum::serve(listener, app.into_make_service())
        .await
        .expect("serve");
}
