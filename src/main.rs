//! HTTP server and FIX 4.4 acceptor for the matching engine (Phase 2).
//!
//! REST: health, submit order, cancel order, modify order. WebSocket: /ws/market-data.
//! FIX: TCP acceptor on FIX_PORT (default 9876). Same engine backs all protocols.
//!
//! Startup: use INSTRUMENT_ID (single u64, default 1) for one instrument, or INSTRUMENT_IDS
//! for multiple (e.g. "1,2,3" or "1:AAPL,2:GOOG" for id:symbol). When INSTRUMENT_IDS is set
//! it takes precedence over INSTRUMENT_ID.
//! Set PERSISTENCE_PATH to a file path to save/load state (instruments, resting orders, market state) across restarts.

use dire_matching_engine::api;
use dire_matching_engine::fix;
use dire_matching_engine::InstrumentId;
use tokio::net::TcpListener;

fn parse_instruments() -> Vec<(InstrumentId, Option<String>)> {
    if let Ok(s) = std::env::var("INSTRUMENT_IDS") {
        let mut out = Vec::new();
        for part in s.split_terminator(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (id, symbol) = match part.find(':') {
                Some(i) => {
                    let id_str = part[..i].trim();
                    let sym = part[i + 1..].trim();
                    (id_str, if sym.is_empty() { None } else { Some(sym.to_string()) })
                }
                None => (part, None),
            };
            if let Ok(n) = id.parse::<u64>() {
                out.push((InstrumentId(n), symbol));
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
    let id = std::env::var("INSTRUMENT_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    vec![(InstrumentId(id), None)]
}

#[tokio::main]
async fn main() {
    let _ = env_logger::try_init();
    let instruments = parse_instruments();
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);
    let fix_port: u16 = std::env::var("FIX_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9876);

    let state = if let Ok(path) = std::env::var("PERSISTENCE_PATH") {
        eprintln!("Persistence enabled: {}", path);
        api::create_app_state_with_persistence(instruments, path)
    } else if instruments.len() == 1 && instruments[0].1.is_none() {
        api::create_app_state(instruments[0].0)
    } else {
        api::create_app_state_with_instruments(instruments)
    };
    let app = api::create_router_with_state(state.clone());

    let fix_addr = format!("0.0.0.0:{}", fix_port);
    let fix_listener = std::net::TcpListener::bind(&fix_addr).expect("FIX bind");
    let engine = state.engine.clone();
    let market_state = state.market_state.clone();
    std::thread::spawn(move || {
        fix::run_fix_acceptor(fix_listener, engine, market_state);
    });
    eprintln!("FIX acceptor on {}", fix_addr);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await.expect("bind");
    eprintln!("listening on http://{}", addr);
    axum::serve(listener, app.into_make_service())
        .await
        .expect("serve");
}
