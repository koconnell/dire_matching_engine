//! REST API integration tests (Phase 2). Spawn the server and call endpoints with reqwest.

use dire_matching_engine::api;
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

#[tokio::test]
async fn health_returns_ok() {
    let (addr, _handle) = spawn_app().await;
    let url = format!("http://{}/health", addr);
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn submit_order_accepts_limit_order_returns_200() {
    let (addr, _handle) = spawn_app().await;
    let url = format!("http://{}/orders", addr);
    let order = serde_json::json!({
        "order_id": 1,
        "client_order_id": "c1",
        "instrument_id": 1,
        "side": "Sell",
        "order_type": "Limit",
        "quantity": "10",
        "price": "100",
        "time_in_force": "GTC",
        "timestamp": 1,
        "trader_id": 1
    });
    let client = reqwest::Client::new();
    let response = client.post(&url).json(&order).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let json: serde_json::Value = response.json().await.unwrap();
    assert!(json.get("reports").and_then(|r| r.as_array()).map(|a| !a.is_empty()).unwrap_or(false));
}

#[tokio::test]
async fn submit_order_then_cancel_returns_canceled_true() {
    let (addr, _handle) = spawn_app().await;
    let url_orders = format!("http://{}/orders", addr);
    let url_cancel = format!("http://{}/orders/cancel", addr);
    let order = serde_json::json!({
        "order_id": 1,
        "client_order_id": "c1",
        "instrument_id": 1,
        "side": "Sell",
        "order_type": "Limit",
        "quantity": "5",
        "price": "100",
        "time_in_force": "GTC",
        "timestamp": 1,
        "trader_id": 1
    });
    let client = reqwest::Client::new();
    let _ = client.post(&url_orders).json(&order).send().await.unwrap();
    let cancel_body = serde_json::json!({ "order_id": 1 });
    let response = client.post(&url_cancel).json(&cancel_body).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json.get("canceled"), Some(&serde_json::json!(true)));
}

#[tokio::test]
async fn cancel_nonexistent_order_returns_canceled_false() {
    let (addr, _handle) = spawn_app().await;
    let url = format!("http://{}/orders/cancel", addr);
    let cancel_body = serde_json::json!({ "order_id": 999 });
    let client = reqwest::Client::new();
    let response = client.post(&url).json(&cancel_body).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json.get("canceled"), Some(&serde_json::json!(false)));
}

#[tokio::test]
async fn modify_order_returns_trades_and_reports() {
    let (addr, _handle) = spawn_app().await;
    let url_orders = format!("http://{}/orders", addr);
    let url_modify = format!("http://{}/orders/modify", addr);
    let sell = serde_json::json!({
        "order_id": 1,
        "client_order_id": "c1",
        "instrument_id": 1,
        "side": "Sell",
        "order_type": "Limit",
        "quantity": "10",
        "price": "100",
        "time_in_force": "GTC",
        "timestamp": 1,
        "trader_id": 1
    });
    let client = reqwest::Client::new();
    let _ = client.post(&url_orders).json(&sell).send().await.unwrap();
    let modify_body = serde_json::json!({
        "order_id": 1,
        "replacement": {
            "order_id": 1,
            "client_order_id": "c1",
            "instrument_id": 1,
            "side": "Sell",
            "order_type": "Limit",
            "quantity": "5",
            "price": "100",
            "time_in_force": "GTC",
            "timestamp": 2,
            "trader_id": 1
        }
    });
    let response = client.post(&url_modify).json(&modify_body).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let json: serde_json::Value = response.json().await.unwrap();
    assert!(json.get("reports").is_some());
    assert!(json.get("trades").is_some());
}

#[tokio::test]
async fn submit_order_invalid_limit_no_price_returns_400() {
    let (addr, _handle) = spawn_app().await;
    let url = format!("http://{}/orders", addr);
    let order = serde_json::json!({
        "order_id": 1,
        "client_order_id": "c1",
        "instrument_id": 1,
        "side": "Buy",
        "order_type": "Limit",
        "quantity": "10",
        "price": null,
        "time_in_force": "GTC",
        "timestamp": 1,
        "trader_id": 1
    });
    let client = reqwest::Client::new();
    let response = client.post(&url).json(&order).send().await.unwrap();
    assert_eq!(response.status(), 400);
    let json: serde_json::Value = response.json().await.unwrap();
    assert!(json.get("error").is_some());
}
