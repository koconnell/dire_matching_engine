#!/usr/bin/env bash
# Certification suite: run critical REST path against a running engine.
# Exit 0 on success, non-zero on failure.
#
# Config (env):
#   CERT_BASE_URL  - Base URL for REST API (default: http://127.0.0.1:8080)
#   CERT_API_KEY   - Optional. If set, sent as Bearer token (required when server uses API_KEYS).
#
# Example:
#   ./scripts/cert_suite.sh
#   CERT_BASE_URL=http://sandbox.example.com:8080 CERT_API_KEY=mykey ./scripts/cert_suite.sh

set -e

BASE="${CERT_BASE_URL:-http://127.0.0.1:8080}"
# Strip trailing slash
BASE="${BASE%/}"

CURL_OPTS=(-s -w "\n%{http_code}" --connect-timeout 5 --max-time 30)
if [[ -n "${CERT_API_KEY}" ]]; then
  AUTH_HEADER=(-H "Authorization: Bearer ${CERT_API_KEY}")
else
  AUTH_HEADER=()
fi

err() { echo "cert_suite: $*" >&2; }
fail() { err "$*"; exit 1; }

# Response helper: last line is HTTP code, rest is body
read_code() { echo "$REPLY" | tail -n1; }
read_body() { echo "$REPLY" | sed '$d'; }

# --- 1. Health ---
REPLY=$(curl "${CURL_OPTS[@]}" "${BASE}/health")
CODE=$(read_code)
if [[ "$CODE" != "200" ]]; then
  fail "GET /health failed (HTTP $CODE). Is the engine running at $BASE?"
fi
BODY=$(read_body)
BODY="${BODY%$'\n'}"  # trim trailing newline for health check
if [[ "$BODY" != "ok" ]]; then
  fail "GET /health unexpected body: $BODY"
fi
echo "OK GET /health"

# --- 2. Submit order (1001) ---
ORDER1='{"order_id":1001,"client_order_id":"cert-1001","instrument_id":1,"side":"Sell","order_type":"Limit","quantity":"10","price":"100","time_in_force":"GTC","timestamp":1,"trader_id":1}'
REPLY=$(curl "${CURL_OPTS[@]}" "${AUTH_HEADER[@]}" -X POST -H "Content-Type: application/json" -d "$ORDER1" "${BASE}/orders")
CODE=$(read_code)
if [[ "$CODE" == "401" ]]; then
  fail "POST /orders 401 Unauthorized. Set CERT_API_KEY for authenticated targets (e.g. CERT_API_KEY=yourkey)."
fi
if [[ "$CODE" != "200" ]]; then
  BODY=$(read_body)
  fail "POST /orders failed (HTTP $CODE): $BODY"
fi
BODY=$(read_body)
if ! echo "$BODY" | grep -q '"reports"'; then
  fail "POST /orders response missing reports: $BODY"
fi
echo "OK POST /orders (submit 1001)"

# --- 3. Cancel order 1001 ---
CANCEL1='{"order_id":1001}'
REPLY=$(curl "${CURL_OPTS[@]}" "${AUTH_HEADER[@]}" -X POST -H "Content-Type: application/json" -d "$CANCEL1" "${BASE}/orders/cancel")
CODE=$(read_code)
if [[ "$CODE" == "401" ]]; then
  fail "POST /orders/cancel 401. Set CERT_API_KEY."
fi
if [[ "$CODE" != "200" ]]; then
  BODY=$(read_body)
  fail "POST /orders/cancel failed (HTTP $CODE): $BODY"
fi
BODY=$(read_body)
if ! echo "$BODY" | grep -q '"canceled":true'; then
  fail "POST /orders/cancel expected canceled:true: $BODY"
fi
echo "OK POST /orders/cancel (1001)"

# --- 4. Submit order (1002) then modify ---
ORDER2='{"order_id":1002,"client_order_id":"cert-1002","instrument_id":1,"side":"Sell","order_type":"Limit","quantity":"10","price":"100","time_in_force":"GTC","timestamp":1,"trader_id":1}'
REPLY=$(curl "${CURL_OPTS[@]}" "${AUTH_HEADER[@]}" -X POST -H "Content-Type: application/json" -d "$ORDER2" "${BASE}/orders")
CODE=$(read_code)
if [[ "$CODE" == "401" ]]; then
  fail "POST /orders (1002) 401. Set CERT_API_KEY."
fi
if [[ "$CODE" != "200" ]]; then
  BODY=$(read_body)
  fail "POST /orders (1002) failed (HTTP $CODE): $BODY"
fi
echo "OK POST /orders (submit 1002)"

MODIFY2='{"order_id":1002,"replacement":{"order_id":1002,"client_order_id":"cert-1002","instrument_id":1,"side":"Sell","order_type":"Limit","quantity":"5","price":"100","time_in_force":"GTC","timestamp":2,"trader_id":1}}'
REPLY=$(curl "${CURL_OPTS[@]}" "${AUTH_HEADER[@]}" -X POST -H "Content-Type: application/json" -d "$MODIFY2" "${BASE}/orders/modify")
CODE=$(read_code)
if [[ "$CODE" == "401" ]]; then
  fail "POST /orders/modify 401. Set CERT_API_KEY."
fi
if [[ "$CODE" != "200" ]]; then
  BODY=$(read_body)
  fail "POST /orders/modify failed (HTTP $CODE): $BODY"
fi
BODY=$(read_body)
if ! echo "$BODY" | grep -q '"reports"'; then
  fail "POST /orders/modify response missing reports: $BODY"
fi
echo "OK POST /orders/modify (1002)"

echo "cert_suite: all checks passed (target=$BASE)"
exit 0
