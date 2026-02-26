//! REST API integration tests (Phase 2). Spawn the server and call endpoints with reqwest.

use dire_matching_engine::api;
use dire_matching_engine::audit::InMemoryAuditSink;
use dire_matching_engine::auth::AuthConfig;
use dire_matching_engine::InstrumentId;
use std::net::SocketAddr;
use std::sync::Arc;

/// Spawn app with auth disabled (for tests that don't send API keys).
async fn spawn_app() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    spawn_app_with_auth(None).await
}

/// Spawn app; if `api_keys` is Some("key:role,key2:role2"), auth is enabled with that config (no env).
async fn spawn_app_with_auth(api_keys: Option<&str>) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let state = api::create_app_state(InstrumentId(1));
    let auth_config = match api_keys {
        Some(keys) => Some(AuthConfig::from_keys(keys)),
        None => Some(AuthConfig::disabled()),
    };
    let app = api::create_router_with_state_and_auth(state, auth_config);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (addr, handle)
}

/// Spawn app with in-memory audit sink; returns (addr, handle, sink) so tests can assert on audit events.
async fn spawn_app_with_audit_sink(api_keys: Option<&str>) -> (SocketAddr, tokio::task::JoinHandle<()>, Arc<InMemoryAuditSink>) {
    let audit_sink = Arc::new(InMemoryAuditSink::new());
    let state = api::create_app_state_with_sink(InstrumentId(1), audit_sink.clone());
    let auth_config = match api_keys {
        Some(keys) => Some(AuthConfig::from_keys(keys)),
        None => Some(AuthConfig::disabled()),
    };
    let app = api::create_router_with_state_and_auth(state, auth_config);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (addr, handle, audit_sink)
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

/// When a buy matches a resting sell, the response must include at least one report with non-null avg_price, last_qty, last_px.
#[tokio::test]
async fn submit_sell_then_buy_matching_returns_reports_with_prices() {
    let (addr, _handle) = spawn_app().await;
    let url = format!("http://{}/orders", addr);
    let client = reqwest::Client::new();
    let sell = serde_json::json!({
        "order_id": 50,
        "client_order_id": "c50",
        "instrument_id": 1,
        "side": "Sell",
        "order_type": "Limit",
        "quantity": "10",
        "price": "100",
        "time_in_force": "GTC",
        "timestamp": 1,
        "trader_id": 1
    });
    client.post(&url).json(&sell).send().await.unwrap();
    let buy = serde_json::json!({
        "order_id": 51,
        "client_order_id": "c51",
        "instrument_id": 1,
        "side": "Buy",
        "order_type": "Limit",
        "quantity": "5",
        "price": "100",
        "time_in_force": "GTC",
        "timestamp": 2,
        "trader_id": 2
    });
    let response = client.post(&url).json(&buy).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let json: serde_json::Value = response.json().await.unwrap();
    let trades = json.get("trades").and_then(|t| t.as_array()).unwrap();
    assert_eq!(trades.len(), 1, "expected one trade when buy matches sell");
    let reports = json.get("reports").and_then(|r| r.as_array()).unwrap();
    let has_price_in_report = reports.iter().any(|r| {
        r.get("avg_price").map(|v| !v.is_null()).unwrap_or(false)
            && r.get("last_px").map(|v| !v.is_null()).unwrap_or(false)
    });
    assert!(has_price_in_report, "at least one report must have avg_price and last_px set when there is a fill; reports: {:?}", reports);
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

// --- Phase 3: API key auth ---

#[tokio::test]
async fn auth_required_returns_401_without_key() {
    let (addr, _handle) = spawn_app_with_auth(Some("secret123:trader")).await;
    let url = format!("http://{}/orders", addr);
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "order_id": 1,
            "client_order_id": "c1",
            "instrument_id": 1,
            "side": "Buy",
            "order_type": "Limit",
            "quantity": "10",
            "price": "100",
            "time_in_force": "GTC",
            "timestamp": 1,
            "trader_id": 1
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn auth_with_valid_key_returns_200() {
    let (addr, _handle) = spawn_app_with_auth(Some("testkey:trader")).await;
    let url = format!("http://{}/orders", addr);
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", "Bearer testkey")
        .json(&serde_json::json!({
            "order_id": 99,
            "client_order_id": "c99",
            "instrument_id": 1,
            "side": "Buy",
            "order_type": "Limit",
            "quantity": "5",
            "price": "100",
            "time_in_force": "GTC",
            "timestamp": 1,
            "trader_id": 1
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn auth_accepts_x_api_key_header() {
    let (addr, _handle) = spawn_app_with_auth(Some("mykey:trader")).await;
    let url = format!("http://{}/orders", addr);
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("X-API-Key", "mykey")
        .json(&serde_json::json!({
            "order_id": 98,
            "client_order_id": "c98",
            "instrument_id": 1,
            "side": "Sell",
            "order_type": "Limit",
            "quantity": "1",
            "price": "50",
            "time_in_force": "GTC",
            "timestamp": 1,
            "trader_id": 1
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn rbac_trader_to_admin_returns_403() {
    let (addr, _handle) = spawn_app_with_auth(Some("t:trader,a:admin,o:operator")).await;
    let url = format!("http://{}/admin/status", addr);
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", "Bearer t")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn rbac_admin_to_admin_returns_200() {
    let (addr, _handle) = spawn_app_with_auth(Some("t:trader,a:admin,o:operator")).await;
    let url = format!("http://{}/admin/status", addr);
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", "Bearer a")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("ok"));
}

#[tokio::test]
async fn rbac_operator_to_admin_returns_200() {
    let (addr, _handle) = spawn_app_with_auth(Some("t:trader,a:admin,o:operator")).await;
    let url = format!("http://{}/admin/status", addr);
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", "Bearer o")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

// --- Phase 3 §3: Audit trail ---

#[tokio::test]
async fn audit_order_submit_emits_event() {
    let (addr, _handle, sink) = spawn_app_with_audit_sink(None).await;
    let url = format!("http://{}/orders", addr);
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "order_id": 42,
            "client_order_id": "c42",
            "instrument_id": 1,
            "side": "Buy",
            "order_type": "Limit",
            "quantity": "1",
            "price": "99",
            "time_in_force": "GTC",
            "timestamp": 1,
            "trader_id": 1
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let events = sink.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "order_submit");
    assert_eq!(events[0].outcome, "success");
    assert_eq!(
        events[0].resource.as_ref().and_then(|r| r.get("order_id").and_then(|v| v.as_u64())),
        Some(42)
    );
}

// --- Phase 3 §4: Admin API, market state, order rejection when not Open ---

#[tokio::test]
async fn admin_market_state_halted_rejects_order_then_open_accepts() {
    let (addr, _handle) = spawn_app_with_auth(Some("a:admin")).await;
    let client = reqwest::Client::new();
    let auth_header = "Bearer a";

    // Set market to Halted
    let set_halted = client
        .post(format!("http://{}/admin/market-state", addr))
        .header("Authorization", auth_header)
        .json(&serde_json::json!({ "state": "Halted" }))
        .send()
        .await
        .unwrap();
    assert_eq!(set_halted.status(), 200);

    // Order submit should return 503
    let order = serde_json::json!({
        "order_id": 1,
        "client_order_id": "c1",
        "instrument_id": 1,
        "side": "Buy",
        "order_type": "Limit",
        "quantity": "1",
        "price": "100",
        "time_in_force": "GTC",
        "timestamp": 1,
        "trader_id": 1
    });
    let resp = client
        .post(format!("http://{}/orders", addr))
        .header("Authorization", auth_header)
        .json(&order)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 503);

    // Set market back to Open
    client
        .post(format!("http://{}/admin/market-state", addr))
        .header("Authorization", auth_header)
        .json(&serde_json::json!({ "state": "Open" }))
        .send()
        .await
        .unwrap();

    // Order submit should now succeed
    let resp2 = client
        .post(format!("http://{}/orders", addr))
        .header("Authorization", auth_header)
        .json(&order)
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), 200);
}

#[tokio::test]
async fn admin_emergency_halt_sets_halted() {
    let (addr, _handle) = spawn_app_with_auth(Some("o:operator")).await;
    let client = reqwest::Client::new();

    let halt = client
        .post(format!("http://{}/admin/emergency-halt", addr))
        .header("Authorization", "Bearer o")
        .send()
        .await
        .unwrap();
    assert_eq!(halt.status(), 200);

    let get_state = client
        .get(format!("http://{}/admin/market-state", addr))
        .header("Authorization", "Bearer o")
        .send()
        .await
        .unwrap();
    assert_eq!(get_state.status(), 200);
    let json: serde_json::Value = get_state.json().await.unwrap();
    assert_eq!(json.get("state").and_then(|v| v.as_str()), Some("Halted"));

    // Order rejected when halted
    let order = serde_json::json!({
        "order_id": 2,
        "client_order_id": "c2",
        "instrument_id": 1,
        "side": "Sell",
        "order_type": "Limit",
        "quantity": "1",
        "price": "99",
        "time_in_force": "GTC",
        "timestamp": 1,
        "trader_id": 1
    });
    let resp = client
        .post(format!("http://{}/orders", addr))
        .header("Authorization", "Bearer o")
        .json(&order)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 503);
}

#[tokio::test]
async fn admin_instruments_list_returns_current() {
    let (addr, _handle) = spawn_app_with_auth(Some("a:admin")).await;
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/admin/instruments", addr))
        .header("Authorization", "Bearer a")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let json: serde_json::Value = response.json().await.unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0].get("instrument_id").and_then(|v| v.as_u64()), Some(1));
}

#[tokio::test]
async fn admin_instruments_add_list_delete() {
    let (addr, _handle) = spawn_app_with_auth(Some("a:admin")).await;
    let client = reqwest::Client::new();
    let auth = "Bearer a";

    let list0 = client
        .get(format!("http://{}/admin/instruments", addr))
        .header("Authorization", auth)
        .send()
        .await
        .unwrap();
    assert_eq!(list0.status(), 200);
    let arr0: Vec<serde_json::Value> = list0.json().await.unwrap();
    assert_eq!(arr0.len(), 1);

    let add = client
        .post(format!("http://{}/admin/instruments", addr))
        .header("Authorization", auth)
        .json(&serde_json::json!({ "instrument_id": 2, "symbol": "BAR" }))
        .send()
        .await
        .unwrap();
    assert_eq!(add.status(), 201);

    let list1 = client
        .get(format!("http://{}/admin/instruments", addr))
        .header("Authorization", auth)
        .send()
        .await
        .unwrap();
    assert_eq!(list1.status(), 200);
    let arr1: Vec<serde_json::Value> = list1.json().await.unwrap();
    assert_eq!(arr1.len(), 2);
    let has2 = arr1.iter().any(|o| o.get("instrument_id").and_then(|v| v.as_u64()) == Some(2));
    assert!(has2, "expected instrument 2 in list: {:?}", arr1);
    let sym = arr1.iter().find(|o| o.get("instrument_id").and_then(|v| v.as_u64()) == Some(2)).and_then(|o| o.get("symbol"));
    assert_eq!(sym.and_then(|s| s.as_str()), Some("BAR"));

    let del = client
        .delete(format!("http://{}/admin/instruments/2", addr))
        .header("Authorization", auth)
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);

    let list2 = client
        .get(format!("http://{}/admin/instruments", addr))
        .header("Authorization", auth)
        .send()
        .await
        .unwrap();
    assert_eq!(list2.status(), 200);
    let arr2: Vec<serde_json::Value> = list2.json().await.unwrap();
    assert_eq!(arr2.len(), 1);
    assert_eq!(arr2[0].get("instrument_id").and_then(|v| v.as_u64()), Some(1));
}

#[tokio::test]
async fn admin_config_get_and_patch() {
    let (addr, _handle) = spawn_app_with_auth(Some("a:admin")).await;
    let client = reqwest::Client::new();
    let auth = "Bearer a";

    let get0 = client
        .get(format!("http://{}/admin/config", addr))
        .header("Authorization", auth)
        .send()
        .await
        .unwrap();
    assert_eq!(get0.status(), 200);
    let empty: serde_json::Value = get0.json().await.unwrap();
    assert!(empty.as_object().map(|o| o.is_empty()).unwrap_or(false));

    let patch = client
        .patch(format!("http://{}/admin/config", addr))
        .header("Authorization", auth)
        .json(&serde_json::json!({ "max_order_quantity": 500 }))
        .send()
        .await
        .unwrap();
    assert_eq!(patch.status(), 200);

    let get1 = client
        .get(format!("http://{}/admin/config", addr))
        .header("Authorization", auth)
        .send()
        .await
        .unwrap();
    assert_eq!(get1.status(), 200);
    let config: serde_json::Value = get1.json().await.unwrap();
    assert_eq!(config.get("max_order_quantity").and_then(|v| v.as_u64()), Some(500));
}

/// Trader cannot change market state (RBAC: admin/operator only).
#[tokio::test]
async fn integration_trader_cannot_set_market_state() {
    let (addr, _handle) = spawn_app_with_auth(Some("t:trader,a:admin")).await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/admin/market-state", addr))
        .header("Authorization", "Bearer t")
        .json(&serde_json::json!({ "state": "Halted" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}
