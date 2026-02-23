//! Phase 3 authentication: API keys, config from env, dev bypass.
//!
//! When `DISABLE_AUTH=true` or `API_KEYS` is unset, all requests are accepted with a default
//! trader role. Otherwise, validate `Authorization: Bearer <key>` or `X-API-Key: <key>` and
//! look up the key in `API_KEYS` (format: `key1:role1,key2:role2`; roles: trader, admin, operator).

use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::sync::Arc;

/// Role for RBAC (Phase 3 §2). Used by auth and later by permission checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Trader,
    Admin,
    Operator,
}

impl Role {
    pub fn from_str(s: &str) -> Option<Self> {
        if s.eq_ignore_ascii_case("trader") {
            Some(Role::Trader)
        } else if s.eq_ignore_ascii_case("admin") {
            Some(Role::Admin)
        } else if s.eq_ignore_ascii_case("operator") {
            Some(Role::Operator)
        } else {
            None
        }
    }
}

/// Authenticated user (key id + role). Injected by auth middleware when auth succeeds or is disabled.
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub key_id: Option<String>,
    pub role: Role,
}

impl Default for AuthUser {
    fn default() -> Self {
        Self {
            key_id: None,
            role: Role::Trader,
        }
    }
}

/// Returns `Ok(())` if `user.role` is Admin or Operator; otherwise returns a 403 Response.
/// Use in admin-only handlers: `require_admin_or_operator(&auth)?`.
pub fn require_admin_or_operator(user: &AuthUser) -> Result<(), Response> {
    match user.role {
        Role::Admin | Role::Operator => Ok(()),
        Role::Trader => Err((StatusCode::FORBIDDEN, "admin or operator role required").into_response()),
    }
}

/// Auth configuration: disable flag and key → role map. Built from env.
#[derive(Clone)]
pub struct AuthConfig {
    pub disable: bool,
    keys: Arc<HashMap<String, Role>>,
}

impl AuthConfig {
    /// Auth disabled: all requests accepted with default trader role.
    pub fn disabled() -> Self {
        Self {
            disable: true,
            keys: Arc::new(HashMap::new()),
        }
    }

    /// Build from key:role string (e.g. "key1:trader,key2:admin"). For tests.
    pub fn from_keys(keys: &str) -> Self {
        let map: HashMap<String, Role> = keys
            .split(',')
            .filter_map(|part| {
                let part = part.trim();
                let mut split = part.splitn(2, ':');
                let key = split.next()?.trim().to_string();
                let role_str = split.next()?.trim();
                let role = Role::from_str(role_str)?;
                if key.is_empty() {
                    return None;
                }
                Some((key, role))
            })
            .collect();
        Self {
            disable: map.is_empty(),
            keys: Arc::new(map),
        }
    }

    /// Load from env: `DISABLE_AUTH=true` or unset `API_KEYS` => auth disabled.
    /// `API_KEYS=secret1:trader,secret2:admin` => comma-separated key:role pairs.
    pub fn from_env() -> Self {
        let disable = std::env::var("DISABLE_AUTH")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let keys = std::env::var("API_KEYS").ok().map(|s| {
            let map: HashMap<String, Role> = s
                .split(',')
                .filter_map(|part| {
                    let part = part.trim();
                    let mut split = part.splitn(2, ':');
                    let key = split.next()?.trim().to_string();
                    let role_str = split.next()?.trim();
                    let role = Role::from_str(role_str)?;
                    if key.is_empty() {
                        return None;
                    }
                    Some((key, role))
                })
                .collect();
            Arc::new(map)
        });

        let keys = keys.unwrap_or_else(|| Arc::new(HashMap::new()));

        let disable = disable || keys.is_empty();

        Self { disable, keys }
    }

    pub fn lookup(&self, key: &str) -> Option<Role> {
        self.keys.get(key).copied()
    }
}

/// Returns the API key from `Authorization: Bearer <key>` or `X-API-Key: <key>`.
fn get_api_key_from_request(req: &Request) -> Option<String> {
    if let Some(v) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(s) = v.to_str() {
            let s = s.trim();
            if s.len() >= 7 && s.get(..7).map(|p| p.eq_ignore_ascii_case("bearer ")).unwrap_or(false) {
                return Some(s.get(7..).unwrap_or("").trim().to_string());
            }
        }
    }
    if let Some(v) = req.headers().get("X-API-Key") {
        if let Ok(s) = v.to_str() {
            return Some(s.trim().to_string());
        }
    }
    None
}

/// Auth middleware: when auth is disabled, injects `AuthUser { role: Trader }` and continues.
/// Otherwise, requires a valid API key and injects `AuthUser { key_id, role }`; returns 401 if missing/invalid.
pub async fn require_api_key_or_anonymous(
    mut req: Request<Body>,
    next: Next,
    config: AuthConfig,
) -> Response {
    if config.disable {
        req.extensions_mut().insert(AuthUser::default());
        return next.run(req).await;
    }

    let key = match get_api_key_from_request(&req) {
        Some(k) if !k.is_empty() => k,
        _ => {
            return (StatusCode::UNAUTHORIZED, "missing or invalid Authorization or X-API-Key")
                .into_response();
        }
    };

    match config.lookup(&key) {
        Some(role) => {
            req.extensions_mut().insert(AuthUser {
                key_id: Some(key),
                role,
            });
            next.run(req).await
        }
        None => (StatusCode::UNAUTHORIZED, "invalid API key").into_response(),
    }
}
