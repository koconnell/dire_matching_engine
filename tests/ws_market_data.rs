//! WebSocket market-data integration tests (Phase 2). Connect to /ws/market-data and assert snapshot.

use dire_matching_engine::api;
use futures_util::StreamExt;
use dire_matching_engine::InstrumentId;
use std::net::SocketAddr;

async fn spawn_app() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = api::create_router(InstrumentId(1));
    let handle = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (addr, handle)
}

#[derive(serde::Deserialize)]
struct MarketDataSnapshot {
    #[serde(rename = "type")]
    msg_type: String,
    instrument_id: u64,
    best_bid: Option<rust_decimal::Decimal>,
    best_ask: Option<rust_decimal::Decimal>,
}

#[tokio::test]
async fn ws_market_data_sends_snapshot_on_connect() {
    let (addr, _handle) = spawn_app().await;
    let url = format!("ws://{}/ws/market-data", addr);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("connect");
    let raw = ws.next().await.expect("one message").expect("ws recv");
    let msg = raw.into_text().expect("text frame");
    let snapshot: MarketDataSnapshot = serde_json::from_str(&msg).expect("json");
    assert_eq!(snapshot.msg_type, "snapshot");
    assert_eq!(snapshot.instrument_id, 1);
    // Empty book at start
    assert!(snapshot.best_bid.is_none());
    assert!(snapshot.best_ask.is_none());
}

#[tokio::test]
async fn ws_market_data_snapshot_reflects_book_after_order() {
    let (addr, _handle) = spawn_app().await;
    // Submit a limit buy so we have a best bid
    let order_url = format!("http://{}/orders", addr);
    let order = serde_json::json!({
        "order_id": 10,
        "client_order_id": "c10",
        "instrument_id": 1,
        "side": "Buy",
        "order_type": "Limit",
        "quantity": "5",
        "price": "99.50",
        "time_in_force": "GTC",
        "timestamp": 1,
        "trader_id": 1
    });
    let client = reqwest::Client::new();
    let _ = client.post(&order_url).json(&order).send().await.unwrap();

    let url = format!("ws://{}/ws/market-data", addr);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("connect");
    let raw = ws.next().await.expect("one message").expect("ws recv");
    let msg = raw.into_text().expect("text frame");
    let snapshot: MarketDataSnapshot = serde_json::from_str(&msg).expect("json");
    assert_eq!(snapshot.msg_type, "snapshot");
    assert_eq!(snapshot.instrument_id, 1);
    assert!(snapshot.best_bid.is_some());
    let expected_bid: rust_decimal::Decimal = "99.5".parse().unwrap();
    assert_eq!(snapshot.best_bid.unwrap(), expected_bid);
}
