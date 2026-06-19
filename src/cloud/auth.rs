//! Desktop sign-in to the rekaptr.dev cloud via Clerk as an OAuth Identity
//! Provider: Authorization Code + PKCE with a loopback (127.0.0.1) callback
//! (RFC 8252). Implements the flow designed in `docs/teams-auth-flow.md`.
//!
//! **Threading:** every method here is blocking (blocking `reqwest`, a blocking
//! loopback `TcpListener`, blocking keychain I/O). Call it from a background
//! thread / `cx.background_executor()`, never on the GPUI main thread or inside
//! the tokio runtime (blocking reqwest must not run on an async worker).
//!
//! **Security:** this is a *public* OAuth client — there is no client secret;
//! PKCE protects the code exchange. The `client_id` is safe to ship in the
//! open-source app. No server secrets ever live here.
#![allow(dead_code)] // wired into the Teams UI in a later step (Phase 7)

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Configuration ───────────────────────────────────────────────────
// Fill these from the Clerk OAuth Application (docs/teams-auth-flow.md §2).
// All are public, non-secret values. Each can be overridden at runtime via the
// matching env var, so dev/staging don't need a recompile.
const DEFAULT_ISSUER: &str = "https://clerk.rekaptr.dev"; // confirmed via OIDC discovery
const DEFAULT_CLIENT_ID: &str = "1dyhGfjmGFqRKWBn"; // Clerk OAuth app (public, PKCE)
const DEFAULT_API_BASE: &str = "https://app.rekaptr.dev"; // teams API host

const SCOPES: &str = "profile email offline_access"; // offline_access ⇒ refresh token
const REDIRECT_PATH: &str = "/callback";
/// Refresh once the access token is within this window of expiry.
const REFRESH_SKEW: Duration = Duration::from_secs(30);
/// How long to wait for the user to complete the browser sign-in.
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

// Keychain coordinates (Windows Credential Manager via `keyring`).
const KEYCHAIN_SERVICE: &str = "Rekaptr";
const KEYCHAIN_USER: &str = "cloud-oauth-tokens";

#[derive(Debug, thiserror::Error)]
pub enum CloudAuthError {
    #[error("not signed in")]
    NotSignedIn,
    #[error("sign-in was cancelled or timed out")]
    Cancelled,
    #[error("oauth state mismatch (possible CSRF)")]
    StateMismatch,
    #[error("token endpoint error: {0}")]
    Token(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("token store error: {0}")]
    Store(String),
}

type Result<T> = std::result::Result<T, CloudAuthError>;

/// Cached OAuth token set, persisted in the OS keychain.
#[derive(Clone, Serialize, Deserialize)]
struct TokenSet {
    access_token: String,
    refresh_token: Option<String>,
    /// Unix seconds at which the access token expires.
    expires_at: u64,
}

impl TokenSet {
    fn is_fresh(&self) -> bool {
        let now = now_unix();
        let skew = REFRESH_SKEW.as_secs();
        self.expires_at > now + skew
    }
}

/// Raw `/oauth/token` response.
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

pub struct CloudAuth {
    issuer: String,
    client_id: String,
    api_base: String,
    cached: Mutex<Option<TokenSet>>,
}

/// A fresh blocking HTTP client. **Must** be created (and dropped) on a
/// background, non-async thread: `reqwest::blocking` owns a private Tokio
/// runtime, and creating/dropping one inside the app's `#[tokio::main]` async
/// context panics ("cannot drop a runtime in an async context"). We therefore
/// never store one in `AppState` — each request builds its own.
pub(crate) fn blocking_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::new()
}

impl CloudAuth {
    /// Load any cached tokens from the keychain. Does no network/runtime work,
    /// so it is safe to call from the async startup path.
    pub fn new() -> Self {
        let cached = load_tokens();
        CloudAuth {
            issuer: env_or("REKAPTR_CLERK_ISSUER", DEFAULT_ISSUER),
            client_id: env_or("REKAPTR_OAUTH_CLIENT_ID", DEFAULT_CLIENT_ID),
            api_base: env_or("REKAPTR_API_BASE", DEFAULT_API_BASE),
            cached: Mutex::new(cached),
        }
    }

    /// Base URL of the teams API (e.g. `https://app.rekaptr.dev`); the API
    /// wrapper appends `/api/...` paths.
    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    /// Whether a (possibly stale) token set is cached. Stale tokens are still
    /// "signed in" — they refresh on next use.
    pub fn is_signed_in(&self) -> bool {
        self.cached.lock().is_some()
    }

    /// Run the full browser OAuth flow and cache the resulting tokens. Blocking;
    /// opens the system browser and waits for the loopback callback.
    pub fn sign_in(&self) -> Result<()> {
        let verifier = gen_token_bytes(32); // 256-bit PKCE verifier
        let challenge = b64url(Sha256::digest(verifier.as_bytes()).as_slice());
        let state = gen_token_bytes(16);

        // One-shot loopback server on an OS-chosen port (RFC 8252 §7.3).
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let redirect_uri = format!("http://127.0.0.1:{port}{REDIRECT_PATH}");

        let authorize_url = self.authorize_url(&redirect_uri, &challenge, &state);
        open_in_browser(&authorize_url)?;

        let code = wait_for_code(&listener, &state)?;
        let tokens = self.exchange_code(&code, &verifier, &redirect_uri)?;
        self.store(tokens)?;
        log::info!("[Cloud] signed in");
        Ok(())
    }

    /// Force a token refresh using the stored refresh token, regardless of
    /// expiry. Used to recover from a `401` on an otherwise-fresh token.
    pub fn force_refresh(&self) -> Result<String> {
        let refresh = match self.cached.lock().as_ref().and_then(|t| t.refresh_token.clone()) {
            Some(r) => r,
            None => {
                // Nothing left to refresh with — drop the dead session so
                // `is_signed_in()` stops reporting a session we can't use.
                self.clear();
                return Err(CloudAuthError::NotSignedIn);
            }
        };
        match self.refresh(&refresh) {
            Ok(refreshed) => {
                let token = refreshed.access_token.clone();
                self.store(refreshed)?;
                Ok(token)
            }
            Err(e) => {
                self.clear();
                Err(e)
            }
        }
    }

    /// Return a valid access token, refreshing if it is missing/near expiry.
    pub fn access_token(&self) -> Result<String> {
        if let Some(ts) = self.cached.lock().clone() {
            if ts.is_fresh() {
                return Ok(ts.access_token);
            }
            if let Some(refresh) = ts.refresh_token.clone() {
                return match self.refresh(&refresh) {
                    Ok(refreshed) => {
                        let token = refreshed.access_token.clone();
                        self.store(refreshed)?;
                        Ok(token)
                    }
                    Err(e) => {
                        // The refresh token is dead — drop the cached session so
                        // the UI returns to the sign-in gate instead of looping
                        // on "not signed in" with a stale token still cached.
                        self.clear();
                        Err(e)
                    }
                };
            }
            // Expired and no refresh token: the session is unrecoverable.
            self.clear();
        }
        Err(CloudAuthError::NotSignedIn)
    }

    /// Drop the cached session everywhere (memory + keychain). Called when a
    /// token can no longer be refreshed, so `is_signed_in()` reflects reality.
    fn clear(&self) {
        clear_tokens();
        *self.cached.lock() = None;
    }

    /// Revoke the refresh token (best effort) and clear the cache.
    pub fn sign_out(&self) -> Result<()> {
        if let Some(ts) = self.cached.lock().take() {
            if let Some(refresh) = ts.refresh_token {
                // Endpoint per the issuer's OIDC discovery (revocation_endpoint).
                let _ = blocking_client()
                    .post(format!("{}/oauth/token/revoke", self.issuer))
                    .form(&[("token", refresh.as_str()), ("client_id", &self.client_id)])
                    .send();
            }
        }
        clear_tokens();
        Ok(())
    }

    // ── internals ───────────────────────────────────────────────────
    fn authorize_url(&self, redirect_uri: &str, challenge: &str, state: &str) -> String {
        let mut url = url::Url::parse(&format!("{}/oauth/authorize", self.issuer))
            .expect("issuer is a valid base URL");
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", SCOPES)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", state);
        url.into()
    }

    fn exchange_code(&self, code: &str, verifier: &str, redirect_uri: &str) -> Result<TokenSet> {
        let resp = blocking_client()
            .post(format!("{}/oauth/token", self.issuer))
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("code_verifier", verifier),
                ("client_id", &self.client_id),
                ("redirect_uri", redirect_uri),
            ])
            .send()?;
        self.parse_token_response(resp, None)
    }

    fn refresh(&self, refresh_token: &str) -> Result<TokenSet> {
        let resp = blocking_client()
            .post(format!("{}/oauth/token", self.issuer))
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", &self.client_id),
            ])
            .send()?;
        // Reuse the old refresh token if the server doesn't rotate it.
        self.parse_token_response(resp, Some(refresh_token.to_string()))
    }

    fn parse_token_response(
        &self,
        resp: reqwest::blocking::Response,
        fallback_refresh: Option<String>,
    ) -> Result<TokenSet> {
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(CloudAuthError::Token(format!("{status}: {body}")));
        }
        let body: TokenResponse = resp.json()?;
        let expires_at = now_unix() + body.expires_in.unwrap_or(3600);
        Ok(TokenSet {
            access_token: body.access_token,
            refresh_token: body.refresh_token.or(fallback_refresh),
            expires_at,
        })
    }

    fn store(&self, tokens: TokenSet) -> Result<()> {
        save_tokens(&tokens)?;
        *self.cached.lock() = Some(tokens);
        Ok(())
    }
}

impl Default for CloudAuth {
    fn default() -> Self {
        Self::new()
    }
}

// ── Loopback callback ───────────────────────────────────────────────
/// Block until the browser redirects to the loopback `/callback`, returning the
/// authorization `code` after validating `state`.
fn wait_for_code(listener: &TcpListener, expected_state: &str) -> Result<String> {
    listener.set_nonblocking(false)?;
    // Best-effort overall deadline so a never-completed sign-in doesn't hang.
    let deadline = SystemTime::now() + CALLBACK_TIMEOUT;

    loop {
        if SystemTime::now() >= deadline {
            return Err(CloudAuthError::Cancelled);
        }
        let (mut stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;

        let mut reader = BufReader::new(&stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;

        // "GET /callback?code=...&state=... HTTP/1.1"
        let path = request_line.split_whitespace().nth(1).unwrap_or("");
        let parsed = url::Url::parse(&format!("http://127.0.0.1{path}")).ok();

        let mut code: Option<String> = None;
        let mut state: Option<String> = None;
        let mut error: Option<String> = None;
        if let Some(u) = parsed {
            for (k, v) in u.query_pairs() {
                match k.as_ref() {
                    "code" => code = Some(v.into_owned()),
                    "state" => state = Some(v.into_owned()),
                    "error" => error = Some(v.into_owned()),
                    _ => {}
                }
            }
        }

        // Favicon / unrelated probes can hit the listener first — ignore and wait
        // for the real callback.
        if code.is_none() && error.is_none() {
            write_browser_response(&mut stream, "Waiting for sign-in…");
            continue;
        }

        if let Some(err) = error {
            write_browser_response(&mut stream, "Sign-in failed. You can close this tab.");
            return Err(CloudAuthError::Token(err));
        }
        if state.as_deref() != Some(expected_state) {
            write_browser_response(&mut stream, "Sign-in failed. You can close this tab.");
            return Err(CloudAuthError::StateMismatch);
        }
        write_browser_response(&mut stream, "You're signed in. You can close this tab and return to Rekaptr.");
        return Ok(code.unwrap());
    }
}

fn write_browser_response(stream: &mut std::net::TcpStream, message: &str) {
    let body = format!(
        "<!doctype html><html><body style=\"font-family:system-ui;background:#0a0a0a;color:#e5e5e5;display:flex;align-items:center;justify-content:center;height:100vh;margin:0\"><p>{message}</p></body></html>"
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Open a URL in the default browser without going through a shell (so query
/// `&` separators are not interpreted). `rundll32 url.dll,FileProtocolHandler`
/// is the Windows-native way that takes the URL as a single argument.
fn open_in_browser(url: &str) -> Result<()> {
    std::process::Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .spawn()?;
    Ok(())
}

// ── Token persistence (Windows Credential Manager) ──────────────────
fn keychain() -> std::result::Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER)
}

fn load_tokens() -> Option<TokenSet> {
    let entry = keychain().ok()?;
    let json = entry.get_password().ok()?;
    serde_json::from_str(&json).ok()
}

fn save_tokens(tokens: &TokenSet) -> Result<()> {
    let json = serde_json::to_string(tokens).map_err(|e| CloudAuthError::Store(e.to_string()))?;
    keychain()
        .and_then(|e| e.set_password(&json))
        .map_err(|e| CloudAuthError::Store(e.to_string()))
}

fn clear_tokens() {
    if let Ok(entry) = keychain() {
        let _ = entry.delete_credential();
    }
}

// ── small helpers ───────────────────────────────────────────────────
fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// `n` random bytes, base64url-encoded (used for the PKCE verifier and state).
fn gen_token_bytes(n: usize) -> String {
    use rand::Rng;
    let mut buf = vec![0u8; n];
    rand::rng().fill_bytes(&mut buf);
    b64url(&buf)
}

/// Base64url encoding without padding (RFC 4648 §5).
fn b64url(input: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | b2 as u32;
        out.push(A[((n >> 18) & 63) as usize] as char);
        out.push(A[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(A[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(A[(n & 63) as usize] as char);
        }
    }
    out
}
