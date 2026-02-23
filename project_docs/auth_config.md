# API key authentication (Phase 3 §1)

REST order endpoints and the WebSocket market-data endpoint can require an API key. `/health` is always public.

## Enabling auth

Set **`API_KEYS`** to a comma-separated list of `key:role` pairs. Roles: `trader`, `admin`, `operator` (case-insensitive).

```bash
export API_KEYS="secret1:trader,secret2:admin"
cargo run
```

If `API_KEYS` is unset or empty, auth is **disabled** and all requests are accepted with a default trader role.

## Disabling auth (dev/local)

Set **`DISABLE_AUTH=true`** (or `1`) to turn off auth even when `API_KEYS` is set:

```bash
export DISABLE_AUTH=true
cargo run
```

Use this for local development or when running behind a gateway that already authenticates.

## Sending the key

- **`Authorization: Bearer <key>`**  
  Example: `Authorization: Bearer secret1`
- **`X-API-Key: <key>`**  
  Example: `X-API-Key: secret1`

If auth is enabled and the key is missing or invalid, the server returns **401 Unauthorized**.

## Protected routes

When auth is enabled, these require a valid API key:

- `POST /orders`
- `POST /orders/cancel`
- `POST /orders/modify`
- `GET /ws/market-data` (WebSocket upgrade)

`GET /health` is never protected.

## RBAC (Phase 3 §2)

Admin-only routes (e.g. `/admin/*`) require role **admin** or **operator**. A request authenticated with a **trader** key to such a route returns **403 Forbidden**. Use `API_KEYS` to assign roles: `key:trader`, `key:admin`, `key:operator`.

## FIX / WebSocket

- **FIX:** Auth is not applied to the FIX acceptor in this slice; it can be added later (e.g. by SenderCompID or a FIX-specific credential).
- **WebSocket:** The same API key can be sent at upgrade time (e.g. query param or first message); the current implementation protects the HTTP upgrade, so clients can pass the key in a header when opening the WebSocket URL.

## Tests

Integration tests use explicit `AuthConfig` (no env) so they are safe to run in parallel. See `api::create_router_with_state_and_auth` and `AuthConfig::disabled()` / `AuthConfig::from_keys()`.
