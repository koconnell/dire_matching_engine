//! FIX 4.4 TCP acceptor: one listener, one engine; per-connection session with ClOrdID→OrderId mapping.

use crate::fix::message::{
    execution_report_to_fix_with_side, order_from_cancel_replace, order_from_new_order_single,
    parse_fix_message, FixWriter,
};
use crate::types::{OrderId, Side};
use crate::{Engine, InstrumentId};
use log::warn;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;
use std::time::Duration;
const SENDER_COMP_ID: &str = "DIRED";
const TARGET_COMP_ID: &str = "CLIENT";

/// Run the FIX acceptor on `listener`. Each connection gets a session that shares `engine`.
pub fn run_fix_acceptor(
    listener: std::net::TcpListener,
    engine: std::sync::Arc<Mutex<Engine>>,
    instrument_id: InstrumentId,
) {
    for stream in listener.incoming().flatten() {
        let engine = std::sync::Arc::clone(&engine);
        std::thread::spawn(move || {
            if let Err(e) = handle_fix_connection(stream, engine, instrument_id) {
                warn!("FIX connection error: {}", e);
            }
        });
    }
}

struct Session {
    cl_ord_to_order_id: HashMap<String, OrderId>,
    cl_ord_to_side: HashMap<String, Side>,
    next_order_id: u64,
    out_seq: u32,
}

impl Session {
    fn new() -> Self {
        Self {
            cl_ord_to_order_id: HashMap::new(),
            cl_ord_to_side: HashMap::new(),
            next_order_id: 1,
            out_seq: 1,
        }
    }
    fn next_seq(&mut self) -> u32 {
        let s = self.out_seq;
        self.out_seq += 1;
        s
    }
}

fn handle_fix_connection(
    mut stream: std::net::TcpStream,
    engine: std::sync::Arc<Mutex<Engine>>,
    instrument_id: InstrumentId,
) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;

    let mut session = Session::new();
    let mut buf = vec![0u8; 4096];
    let mut read_pos = 0;

    loop {
        if read_pos >= buf.len() {
            buf.resize(buf.len() * 2, 0);
        }
        let n = stream.read(&mut buf[read_pos..]).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        read_pos += n;

        let (msg, consumed) = match parse_fix_message(&buf[..read_pos]) {
            Some(m) => m,
            None => continue,
        };
        read_pos -= consumed;
        buf.copy_within(consumed.., 0);

        let msg_type = msg.get(&35).ok_or_else(|| "missing MsgType 35".to_string())?.as_str();
        match msg_type {
            "A" => {
                send_logon(&mut stream, session.next_seq())?;
            }
            "5" => {
                send_logout(&mut stream, session.next_seq())?;
                break;
            }
            "0" => {
                send_heartbeat(&mut stream, session.next_seq())?;
            }
            "D" => {
                handle_new_order_single(&mut stream, &msg, &mut session, &engine, instrument_id)?;
            }
            "F" => {
                handle_order_cancel_request(&mut stream, &msg, &mut session, &engine)?;
            }
            "G" => {
                handle_order_cancel_replace_request(&mut stream, &msg, &mut session, &engine, instrument_id)?;
            }
            _ => {
                warn!("FIX unknown MsgType: {}", msg_type);
            }
        }
    }
    Ok(())
}

fn send_logon(stream: &mut std::net::TcpStream, seq: u32) -> Result<(), String> {
    let mut w = FixWriter::new();
    w.set(35, "A");
    w.set(34, seq.to_string());
    w.set(49, SENDER_COMP_ID);
    w.set(52, fix_timestamp_now());
    w.set(56, TARGET_COMP_ID);
    let mut out = Vec::new();
    w.write(&mut out).map_err(|e| e.to_string())?;
    stream.write_all(&out).map_err(|e| e.to_string())?;
    Ok(())
}

fn send_logout(stream: &mut std::net::TcpStream, seq: u32) -> Result<(), String> {
    let mut w = FixWriter::new();
    w.set(35, "5");
    w.set(34, seq.to_string());
    w.set(49, SENDER_COMP_ID);
    w.set(52, fix_timestamp_now());
    w.set(56, TARGET_COMP_ID);
    let mut out = Vec::new();
    w.write(&mut out).map_err(|e| e.to_string())?;
    stream.write_all(&out).map_err(|e| e.to_string())?;
    Ok(())
}

fn send_heartbeat(stream: &mut std::net::TcpStream, seq: u32) -> Result<(), String> {
    let mut w = FixWriter::new();
    w.set(35, "0");
    w.set(34, seq.to_string());
    w.set(49, SENDER_COMP_ID);
    w.set(52, fix_timestamp_now());
    w.set(56, TARGET_COMP_ID);
    let mut out = Vec::new();
    w.write(&mut out).map_err(|e| e.to_string())?;
    stream.write_all(&out).map_err(|e| e.to_string())?;
    Ok(())
}

fn fix_timestamp_now() -> String {
    let now = std::time::SystemTime::now();
    let d = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    let (y, m, d) = message::days_to_ymd((secs / 86400) as i64);
    let t = secs % 86400;
    let h = t / 3600;
    let min = (t % 3600) / 60;
    let s = t % 60;
    format!("{:04}{:02}{:02}-{:02}:{:02}:{:02}", y, m, d, h, min, s)
}

mod message {
    pub fn days_to_ymd(days: i64) -> (u32, u32, u32) {
        let z = days + 719468;
        let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = (yoe + era * 400) as u32 + 1;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        (y, m, d)
    }
}

fn handle_new_order_single(
    stream: &mut std::net::TcpStream,
    fix: &crate::fix::message::FixMessage,
    session: &mut Session,
    engine: &std::sync::Arc<Mutex<Engine>>,
    instrument_id: InstrumentId,
) -> Result<(), String> {
    let order = order_from_new_order_single(fix)?;
    let cl_ord_id = order.client_order_id.clone();
    let side = order.side;
    if order.instrument_id != instrument_id {
        send_rejection(stream, &cl_ord_id, "wrong instrument", session.next_seq())?;
        return Ok(());
    }
    session.cl_ord_to_order_id.insert(cl_ord_id.clone(), order.order_id);
    session.cl_ord_to_side.insert(cl_ord_id.clone(), side);

    let mut guard = engine.lock().expect("lock");
    match guard.submit_order(order) {
        Ok((_trades, reports)) => {
            drop(guard);
            for report in &reports {
                let out = execution_report_to_fix_with_side(
                    report,
                    side,
                    &cl_ord_id,
                    session.next_seq(),
                    SENDER_COMP_ID,
                    TARGET_COMP_ID,
                );
                stream.write_all(&out).map_err(|e| e.to_string())?;
            }
        }
        Err(e) => {
            drop(guard);
            send_rejection(stream, &cl_ord_id, &e, session.next_seq())?;
        }
    }
    Ok(())
}

fn send_rejection(
    stream: &mut std::net::TcpStream,
    cl_ord_id: &str,
    reason: &str,
    seq: u32,
) -> Result<(), String> {
    let mut w = FixWriter::new();
    w.set(35, "8");
    w.set(34, seq.to_string());
    w.set(49, SENDER_COMP_ID);
    w.set(52, fix_timestamp_now());
    w.set(56, TARGET_COMP_ID);
    w.set(11, cl_ord_id);
    w.set(37, "0");
    w.set(17, "0");
    w.set(38, "0");
    w.set(39, "8");
    w.set(40, "2");
    w.set(54, "1");
    w.set(14, "0");
    w.set(151, "0");
    w.set(150, "8");
    w.set(58, reason);
    let mut out = Vec::new();
    w.write(&mut out).map_err(|e| e.to_string())?;
    stream.write_all(&out).map_err(|e| e.to_string())?;
    Ok(())
}

fn handle_order_cancel_request(
    stream: &mut std::net::TcpStream,
    fix: &crate::fix::message::FixMessage,
    session: &mut Session,
    engine: &std::sync::Arc<Mutex<Engine>>,
) -> Result<(), String> {
    let orig_cl_ord_id = fix.get(&41).ok_or_else(|| "missing OrigClOrdID (41)".to_string())?.clone();
    let order_id = *session.cl_ord_to_order_id.get(&orig_cl_ord_id).ok_or_else(|| "OrigClOrdID not found".to_string())?;
    let side = session.cl_ord_to_side.get(&orig_cl_ord_id).copied().unwrap_or(Side::Buy);
    let mut guard = engine.lock().expect("lock");
    let removed = guard.cancel_order(order_id);
    drop(guard);
    if !removed {
        send_rejection(stream, &orig_cl_ord_id, "order not found", session.next_seq())?;
        return Ok(());
    }
    let mut w = FixWriter::new();
    w.set(35, "8");
    w.set(34, session.next_seq().to_string());
    w.set(49, SENDER_COMP_ID);
    w.set(52, fix_timestamp_now());
    w.set(56, TARGET_COMP_ID);
    w.set(11, &orig_cl_ord_id);
    w.set(17, "0");
    w.set(37, order_id.0.to_string());
    w.set(38, "0");
    w.set(39, "4");
    w.set(40, "2");
    w.set(54, match side { Side::Buy => "1", Side::Sell => "2" });
    w.set(14, "0");
    w.set(151, "0");
    w.set(150, "4");
    let mut out = Vec::new();
    w.write(&mut out).map_err(|e| e.to_string())?;
    stream.write_all(&out).map_err(|e| e.to_string())?;
    Ok(())
}

fn handle_order_cancel_replace_request(
    stream: &mut std::net::TcpStream,
    fix: &crate::fix::message::FixMessage,
    session: &mut Session,
    engine: &std::sync::Arc<Mutex<Engine>>,
    _instrument_id: InstrumentId,
) -> Result<(), String> {
    let orig_cl_ord_id = fix.get(&41).ok_or_else(|| "missing OrigClOrdID (41)".to_string())?.clone();
    let order_id = *session.cl_ord_to_order_id.get(&orig_cl_ord_id).ok_or_else(|| "OrigClOrdID not found".to_string())?;
    let new_order_id = session.next_order_id;
    session.next_order_id += 1;
    let replacement = order_from_cancel_replace(fix, new_order_id)?;
    let cl_ord_id = replacement.client_order_id.clone();
    let side = replacement.side;
    session.cl_ord_to_order_id.insert(cl_ord_id.clone(), replacement.order_id);
    session.cl_ord_to_side.insert(cl_ord_id.clone(), side);

    let mut guard = engine.lock().expect("lock");
    match guard.modify_order(order_id, &replacement) {
        Ok((_trades, reports)) => {
            drop(guard);
            for report in &reports {
                let out = execution_report_to_fix_with_side(
                    report,
                    side,
                    &cl_ord_id,
                    session.next_seq(),
                    SENDER_COMP_ID,
                    TARGET_COMP_ID,
                );
                stream.write_all(&out).map_err(|e| e.to_string())?;
            }
        }
        Err(e) => {
            drop(guard);
            send_rejection(stream, &cl_ord_id, &e, session.next_seq())?;
        }
    }
    Ok(())
}