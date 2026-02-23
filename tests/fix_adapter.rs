//! FIX 4.4 adapter integration tests. Connect to the FIX acceptor, send NewOrderSingle, assert ExecutionReport(s).

use dire_matching_engine::api;
use dire_matching_engine::fix::message::{parse_fix_message, FixWriter};
use dire_matching_engine::fix::run_fix_acceptor;
use dire_matching_engine::InstrumentId;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn spawn_fix_acceptor() -> (u16, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = api::create_app_state(InstrumentId(1));
    let engine = state.engine.clone();
    let handle = std::thread::spawn(move || {
        run_fix_acceptor(listener, engine, InstrumentId(1));
    });
    std::thread::sleep(Duration::from_millis(50));
    (port, handle)
}

fn build_fix_message(fields: &[(u32, &str)]) -> Vec<u8> {
    let mut w = FixWriter::new();
    for (tag, value) in fields {
        w.set(*tag, *value);
    }
    let mut out = Vec::new();
    w.write(&mut out).unwrap();
    out
}

#[test]
fn fix_logon_returns_logon() {
    let (port, _handle) = spawn_fix_acceptor();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();

    let logon = build_fix_message(&[
        (35, "A"),
        (34, "1"),
        (49, "CLIENT"),
        (52, "20250101-12:00:00"),
        (56, "DIRED"),
    ]);
    stream.write_all(&logon).unwrap();
    stream.flush().unwrap();

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap();
    let (msg, _) = parse_fix_message(&buf[..n]).expect("parse response");
    assert_eq!(msg.get(&35).map(|s| s.as_str()), Some("A"));
}

#[test]
fn fix_new_order_single_returns_execution_report() {
    let (port, _handle) = spawn_fix_acceptor();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();

    let logon = build_fix_message(&[
        (35, "A"),
        (34, "1"),
        (49, "CLIENT"),
        (52, "20250101-12:00:00"),
        (56, "DIRED"),
    ]);
    stream.write_all(&logon).unwrap();
    stream.flush().unwrap();
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).unwrap();

    let new_order = build_fix_message(&[
        (35, "D"),
        (11, "100"),
        (55, "1"),
        (54, "1"),
        (38, "5"),
        (40, "2"),
        (44, "99.50"),
        (59, "0"),
    ]);
    stream.write_all(&new_order).unwrap();
    stream.flush().unwrap();

    let n = stream.read(&mut buf).unwrap();
    let (msg, _) = parse_fix_message(&buf[..n]).expect("parse ExecutionReport");
    assert_eq!(msg.get(&35).map(|s| s.as_str()), Some("8"));
    assert_eq!(msg.get(&39).map(|s| s.as_str()), Some("0")); // OrdStatus New
    assert_eq!(msg.get(&150).map(|s| s.as_str()), Some("0")); // ExecType New
}
