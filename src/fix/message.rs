//! FIX 4.4 message parse/build and mapping to engine types.

use crate::execution::ExecutionReport;
use crate::types::{ExecType, InstrumentId, Order, OrderId, OrderStatus, OrderType, Side, TimeInForce, TraderId};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::io::{self, Write};

pub const FIX_SOH: u8 = 0x01;
/// FIX message as tag → value. Tag 8, 9, 10 are treated specially for framing.
pub type FixMessage = HashMap<u32, String>;

/// Parse one FIX message from the start of `buf`. Returns the message and number of bytes consumed.
/// Message must start with 8=FIX.4.4 and use 9=BodyLength, 10=CheckSum.
pub fn parse_fix_message(buf: &[u8]) -> Option<(FixMessage, usize)> {
    if buf.len() < 14 || &buf[0..10] != b"8=FIX.4.4\x01" {
        return None;
    }
    if buf[10..12] != b"9="[..] {
        return None;
    }
    let mut i = 12;
    while i < buf.len() && buf[i] != FIX_SOH {
        i += 1;
    }
    let body_len_str = std::str::from_utf8(&buf[12..i]).ok()?;
    let body_len: usize = body_len_str.parse().ok()?;
    i += 1; // SOH after 9's value
    let body_end = i + body_len;
    if body_end + 7 > buf.len() || &buf[body_end..body_end + 3] != b"10=" {
        return None;
    }
    let msg_end = body_end + 3 + 3 + 1; // 10= + 3-digit checksum + SOH
    if msg_end > buf.len() {
        return None;
    }
    let mut msg = FixMessage::new();
    let mut pos = 0;
    while pos < msg_end {
        let eq = buf[pos..].iter().position(|&b| b == b'=').map(|p| p + pos);
        let eq = match eq {
            Some(e) if e < msg_end => e,
            _ => break,
        };
        let tag_str = std::str::from_utf8(&buf[pos..eq]).ok()?;
        let tag: u32 = tag_str.parse().ok()?;
        pos = eq + 1;
        let soh = buf[pos..].iter().position(|&b| b == FIX_SOH).map(|p| p + pos);
        let soh = soh.unwrap_or(msg_end);
        let value = std::str::from_utf8(&buf[pos..soh]).ok()?.to_string();
        msg.insert(tag, value);
        pos = soh + 1;
        if tag == 10 {
            break;
        }
    }
    Some((msg, msg_end))
}

/// Build a FIX message and write to `w`. Sets 8, 9, 10 automatically.
pub struct FixWriter {
    fields: Vec<(u32, String)>,
}

impl FixWriter {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }
    pub fn set(&mut self, tag: u32, value: impl Into<String>) {
        self.fields.push((tag, value.into()));
    }
    /// Build message: 8=FIX.4.4, 9=body_len, body (all fields except 8,9,10), 10=checksum. Checksum = sum(bytes 8..10) % 256.
    pub fn write(&self, w: &mut impl io::Write) -> io::Result<()> {
        let mut body = Vec::new();
        for (tag, value) in &self.fields {
            if *tag == 8 || *tag == 9 || *tag == 10 {
                continue;
            }
            write!(body, "{}={}\x01", tag, value)?;
        }
        let body_len = body.len();
        let header = format!("8=FIX.4.4\x019={}\x01", body_len);
        let body_slice: &[u8] = &body;
        let sum: u32 = header.bytes().chain(body_slice.iter().copied()).map(|b| b as u32).sum();
        let checksum = sum % 256;
        write!(w, "{}", header)?;
        w.write_all(&body)?;
        write!(w, "10={:03}\x01", checksum)?;
        Ok(())
    }
}

/// NewOrderSingle (35=D) → Order. Uses ClOrdID (11) as order_id if numeric; instrument from 55/48 (default 1); TraderId default 1.
pub fn order_from_new_order_single(fix: &FixMessage) -> Result<Order, String> {
    let cl_ord_id = fix.get(&11).ok_or("missing ClOrdID (11)")?.clone();
    let order_id = cl_ord_id.parse::<u64>().map_err(|_| "ClOrdID must be numeric")?;
    let instrument_id = fix
        .get(&55)
        .or_else(|| fix.get(&48))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
    let side = match fix.get(&54).map(|s| s.as_str()).unwrap_or("1") {
        "1" => Side::Buy,
        "2" => Side::Sell,
        _ => return Err("invalid Side (54)".into()),
    };
    let qty_str = fix.get(&38).ok_or("missing OrderQty (38)")?;
    let quantity: Decimal = qty_str.parse().map_err(|_| "invalid OrderQty (38)")?;
    let ord_type = match fix.get(&40).map(|s| s.as_str()).unwrap_or("2") {
        "1" => OrderType::Market,
        "2" => OrderType::Limit,
        _ => return Err("invalid OrdType (40)".into()),
    };
    let price = if ord_type == OrderType::Limit {
        let p = fix.get(&44).ok_or("missing Price (44) for limit order")?;
        Some(p.parse().map_err(|_| "invalid Price (44)")?)
    } else {
        None
    };
    let tif = match fix.get(&59).map(|s| s.as_str()).unwrap_or("0") {
        "0" | "1" => TimeInForce::GTC,
        "3" => TimeInForce::IOC,
        "4" => TimeInForce::FOK,
        _ => TimeInForce::GTC,
    };
    let timestamp = fix.get(&52).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let trader_id = fix.get(&1).and_then(|s| s.parse::<u64>().ok()).unwrap_or(1);

    Ok(Order {
        order_id: OrderId(order_id),
        client_order_id: cl_ord_id,
        instrument_id: InstrumentId(instrument_id),
        side,
        order_type: ord_type,
        quantity,
        price,
        time_in_force: tif,
        timestamp,
        trader_id: TraderId(trader_id),
    })
}

/// OrderCancelReplaceRequest (35=G) → replacement Order. Uses ClOrdID (11) as new client order id; new_order_id is assigned by session.
pub fn order_from_cancel_replace(fix: &FixMessage, new_order_id: u64) -> Result<Order, String> {
    let cl_ord_id = fix.get(&11).ok_or("missing ClOrdID (11)")?.clone();
    let instrument_id = fix
        .get(&55)
        .or_else(|| fix.get(&48))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
    let side = match fix.get(&54).map(|s| s.as_str()).unwrap_or("1") {
        "1" => Side::Buy,
        "2" => Side::Sell,
        _ => return Err("invalid Side (54)".into()),
    };
    let qty_str = fix.get(&38).ok_or("missing OrderQty (38)")?;
    let quantity: Decimal = qty_str.parse().map_err(|_| "invalid OrderQty (38)")?;
    let ord_type = match fix.get(&40).map(|s| s.as_str()).unwrap_or("2") {
        "1" => OrderType::Market,
        "2" => OrderType::Limit,
        _ => return Err("invalid OrdType (40)".into()),
    };
    let price = if ord_type == OrderType::Limit {
        fix.get(&44).map(|s| s.parse().ok()).flatten()
    } else {
        None
    };
    let tif = match fix.get(&59).map(|s| s.as_str()).unwrap_or("0") {
        "0" | "1" => TimeInForce::GTC,
        "3" => TimeInForce::IOC,
        "4" => TimeInForce::FOK,
        _ => TimeInForce::GTC,
    };
    let timestamp = fix.get(&52).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let trader_id = fix.get(&1).and_then(|s| s.parse::<u64>().ok()).unwrap_or(1);

    Ok(Order {
        order_id: OrderId(new_order_id),
        client_order_id: cl_ord_id,
        instrument_id: InstrumentId(instrument_id),
        side,
        order_type: ord_type,
        quantity,
        price,
        time_in_force: tif,
        timestamp,
        trader_id: TraderId(trader_id),
    })
}

fn exec_type_to_fix(e: ExecType) -> &'static str {
    match e {
        ExecType::New => "0",
        ExecType::PartialFill => "F",
        ExecType::Fill => "F",
        ExecType::Canceled => "4",
        ExecType::Rejected => "8",
    }
}

fn ord_status_to_fix(s: OrderStatus) -> &'static str {
    match s {
        OrderStatus::New => "0",
        OrderStatus::PartiallyFilled => "1",
        OrderStatus::Filled => "2",
        OrderStatus::Canceled => "4",
        OrderStatus::Rejected => "8",
    }
}

/// ExecutionReport doesn't carry side; pass side so we can set tag 54 correctly.
pub fn execution_report_to_fix_with_side(
    report: &ExecutionReport,
    side: Side,
    cl_ord_id: &str,
    seq: u32,
    sender: &str,
    target: &str,
) -> Vec<u8> {
    let mut w = FixWriter::new();
    w.set(35, "8");
    w.set(34, seq.to_string());
    w.set(49, sender);
    w.set(52, format_utc_timestamp(report.timestamp));
    w.set(56, target);
    w.set(11, cl_ord_id);
    w.set(17, report.exec_id.0.to_string());
    w.set(37, report.order_id.0.to_string());
    w.set(38, (report.filled_quantity + report.remaining_quantity).to_string());
    w.set(39, ord_status_to_fix(report.order_status));
    w.set(40, "2");
    w.set(54, match side {
        Side::Buy => "1",
        Side::Sell => "2",
    });
    w.set(14, report.filled_quantity.to_string());
    w.set(151, report.remaining_quantity.to_string());
    if let Some(avg) = report.avg_price {
        w.set(6, avg.to_string());
    }
    if let Some(lq) = report.last_qty {
        w.set(32, lq.to_string());
    }
    if let Some(lp) = report.last_px {
        w.set(31, lp.to_string());
    }
    w.set(150, exec_type_to_fix(report.exec_type));
    let mut out = Vec::new();
    let _ = w.write(&mut out);
    out
}

pub fn execution_report_to_fix(
    report: &ExecutionReport,
    cl_ord_id: &str,
    seq: u32,
    sender: &str,
    target: &str,
) -> Vec<u8> {
    execution_report_to_fix_with_side(report, Side::Buy, cl_ord_id, seq, sender, target)
}

fn format_utc_timestamp(ts: u64) -> String {
    let secs = if ts == 0 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    } else {
        ts
    };
    const SECS_PER_DAY: u64 = 86400;
    let days = (secs / SECS_PER_DAY) as i64;
    let t = secs % SECS_PER_DAY;
    let h = t / 3600;
    let m = (t % 3600) / 60;
    let s = t % 60;
    let (y, mth, d) = days_to_ymd(days);
    format!("{:04}{:02}{:02}-{:02}:{:02}:{:02}", y, mth, d, h, m, s)
}

fn days_to_ymd(days: i64) -> (u32, u32, u32) {
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
