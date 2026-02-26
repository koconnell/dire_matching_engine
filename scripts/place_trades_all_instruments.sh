#!/usr/bin/env bash
# Place one matching trade per instrument: for each instrument, submit a Sell then a Buy
# at the same price/quantity so they match. Uses admin (or trader) API key.
#
# Env:
#   BASE_URL     - REST base (default http://localhost:8080)
#   API_KEY     - Bearer key with admin or trader role (default: a)
#   MAX_INSTR   - Max instrument id to use if not fetching list (default: 13)
#
# Usage:
#   ./scripts/place_trades_all_instruments.sh
#   API_KEY=myadmin BASE_URL=http://localhost:8080 ./scripts/place_trades_all_instruments.sh

set -e

BASE="${BASE_URL:-http://localhost:8080}"
BASE="${BASE%/}"
KEY="${API_KEY:-a}"
MAX_INSTR="${MAX_INSTR:-13}"

CURL_OPTS=(-s -w "\n%{http_code}" --connect-timeout 5 --max-time 15)
AUTH_HEADER=(-H "Authorization: Bearer ${KEY}")

read_code() { echo "$REPLY" | tail -n1; }
read_body() { echo "$REPLY" | sed '$d'; }

# Ensure market is Open
REPLY=$(curl "${CURL_OPTS[@]}" "${AUTH_HEADER[@]}" -X POST -H "Content-Type: application/json" \
  -d '{"state":"Open"}' "${BASE}/admin/market-state")
CODE=$(read_code)
if [[ "$CODE" != "200" ]]; then
  echo "Warning: POST /admin/market-state returned $CODE (need admin key?). Continuing anyway." >&2
fi

# Get instrument list; fallback to 1..MAX_INSTR
INSTRUMENTS=()
REPLY=$(curl "${CURL_OPTS[@]}" "${AUTH_HEADER[@]}" "${BASE}/admin/instruments")
CODE=$(read_code)
if [[ "$CODE" == "200" ]]; then
  BODY=$(read_body)
  # Parse JSON array of { "instrument_id": n, ... } - simple extraction of numbers
  while read -r id; do
    [[ -n "$id" ]] && INSTRUMENTS+=( "$id" )
  done < <(echo "$BODY" | grep -o '"instrument_id"[[:space:]]*:[[:space:]]*[0-9]*' | grep -o '[0-9]*')
fi
if [[ ${#INSTRUMENTS[@]} -eq 0 ]]; then
  for i in $(seq 1 "$MAX_INSTR"); do INSTRUMENTS+=( "$i" ); done
  echo "Using instrument ids 1..$MAX_INSTR (GET /admin/instruments failed or returned empty)."
fi

echo "Placing matching trades on ${#INSTRUMENTS[@]} instruments (sell then buy at 100)..."

for inst in "${INSTRUMENTS[@]}"; do
  # Unique order ids: 1000*inst + 1 (sell), 1000*inst + 2 (buy)
  oid_sell=$(( inst * 1000 + 1 ))
  oid_buy=$(( inst * 1000 + 2 ))
  # Sell 10 @ 100 (rests) — trader 1
  ORDER_SELL="{\"order_id\":${oid_sell},\"client_order_id\":\"script-sell-${inst}\",\"instrument_id\":${inst},\"side\":\"Sell\",\"order_type\":\"Limit\",\"quantity\":\"10\",\"price\":\"100\",\"time_in_force\":\"GTC\",\"timestamp\":$(date +%s),\"trader_id\":1}"
  REPLY=$(curl "${CURL_OPTS[@]}" "${AUTH_HEADER[@]}" -X POST -H "Content-Type: application/json" -d "$ORDER_SELL" "${BASE}/orders")
  CODE=$(read_code)
  if [[ "$CODE" != "200" ]]; then
    echo "  inst $inst: sell failed HTTP $CODE"
    continue
  fi
  # Buy 10 @ 100 (matches the sell) — trader 2 so it matches (self-trade prevention)
  ORDER_BUY="{\"order_id\":${oid_buy},\"client_order_id\":\"script-buy-${inst}\",\"instrument_id\":${inst},\"side\":\"Buy\",\"order_type\":\"Limit\",\"quantity\":\"10\",\"price\":\"100\",\"time_in_force\":\"GTC\",\"timestamp\":$(date +%s),\"trader_id\":2}"
  REPLY=$(curl "${CURL_OPTS[@]}" "${AUTH_HEADER[@]}" -X POST -H "Content-Type: application/json" -d "$ORDER_BUY" "${BASE}/orders")
  CODE=$(read_code)
  if [[ "$CODE" != "200" ]]; then
    echo "  inst $inst: buy failed HTTP $CODE"
  else
    echo "  inst $inst: sell then buy @ 100 -> matched"
  fi
done

echo "Done."
