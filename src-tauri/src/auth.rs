//! Microsoft account authentication using the Live Connect OAuth flow inside an
//! embedded login window, then the Xbox Live → XSTS → Minecraft token exchange.
//!
//! A dedicated Tauri webview window is opened to the Microsoft login page. When
//! login completes, Microsoft redirects to
//! `https://login.live.com/oauth20_desktop.srf?code=...`; the window's
//! navigation handler captures that `code`, and the window is closed.
//!
//! This uses the official Minecraft launcher client id, which is already
//! approved for the Minecraft services API (`login_with_xbox`). Custom Azure
//! apps are rejected by that API with "Invalid app registration" until
//! Microsoft manually approves them, so the bundled id is used by default.

use crate::error::{Error, Result};
use crate::models::Account;
use crate::state::AppState;
use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

/// Official Minecraft launcher client id (approved for the Minecraft API).
const BUNDLED_CLIENT_ID: &str = "00000000402b5328";
const AUTHORIZE_URL: &str = "https://login.live.com/oauth20_authorize.srf";
const TOKEN_URL: &str = "https://login.live.com/oauth20_token.srf";
const REDIRECT_URI: &str = "https://login.live.com/oauth20_desktop.srf";
const SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";

fn client_id() -> String {
    ["EZMAPA_CLIENT_ID", "BEACON_CLIENT_ID"]
        .into_iter()
        .find_map(|var| std::env::var(var).ok().filter(|k| !k.trim().is_empty()))
        .unwrap_or_else(|| BUNDLED_CLIENT_ID.to_string())
}

// ── URL helpers ──────────────────────────────────────────────────────────────

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Decode a percent-encoded query value. `+` is treated as a space (the
/// standard for `application/x-www-form-urlencoded` query strings).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let hex = |b: u8| -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push((hi << 4) | lo);
                    i += 3;
                }
                _ => {
                    out.push(b'%');
                    i += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Extract `code` (or an `error`) from a redirect URL's query string.
fn parse_redirect_query(query: &str) -> (Option<String>, Option<String>) {
    let mut code = None;
    let mut oauth_error = None;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let k = kv.next().unwrap_or("");
        let v = kv.next().unwrap_or("");
        match k {
            "code" => code = Some(percent_decode(v)),
            "error" | "error_description" => oauth_error = Some(percent_decode(v)),
            _ => {}
        }
    }
    (code, oauth_error)
}

// ── Token types ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct TokenResp {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    expires_in: i64,
}

// ── Main login entry point ────────────────────────────────────────────────────

/// Opens an embedded Microsoft login window and waits for the OAuth redirect,
/// then completes the Xbox/Minecraft token exchange.
pub async fn login_redirect(app: &AppHandle, state: &AppState) -> Result<Account> {
    let cid = client_id();

    let auth_url = format!(
        "{AUTHORIZE_URL}\
         ?client_id={cid}\
         &response_type=code\
         &redirect_uri={}\
         &scope={}\
         &prompt=select_account",
        percent_encode(REDIRECT_URI),
        percent_encode(SCOPE),
    );

    const LABEL: &str = "ms-login";

    // Close any stale login window from a previous attempt.
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }

    // The navigation handler reports the captured code (or error) here.
    let (tx, mut rx) =
        tokio::sync::mpsc::unbounded_channel::<std::result::Result<String, String>>();

    let parsed = auth_url
        .parse::<tauri::Url>()
        .map_err(|e| Error::Auth(format!("Bad auth URL: {e}")))?;

    let tx_nav = tx.clone();
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(parsed))
        .title("Sign in with Microsoft")
        .inner_size(520.0, 720.0)
        .center()
        .focused(true)
        .on_navigation(move |url| {
            // The final redirect lands on oauth20_desktop.srf?code=...
            if url.as_str().starts_with(REDIRECT_URI) {
                let (code, err) = parse_redirect_query(url.query().unwrap_or(""));
                if let Some(c) = code {
                    let _ = tx_nav.send(Ok(c));
                    return false; // no need to load the blank redirect page
                }
                if let Some(e) = err {
                    let _ = tx_nav.send(Err(e));
                    return false;
                }
            }
            true
        })
        .build()
        .map_err(|e| Error::Auth(format!("Could not open login window: {e}")))?;

    // If the user closes the window before finishing, treat it as a cancel.
    let tx_close = tx.clone();
    window.on_window_event(move |ev| {
        if let WindowEvent::CloseRequested { .. } = ev {
            let _ = tx_close.send(Err("Login window closed.".into()));
        }
    });

    // Let the frontend show its "waiting" state.
    let _ = app.emit("auth://browser-opened", ());

    // Wait up to 5 minutes for a result.
    let received = tokio::time::timeout(std::time::Duration::from_secs(300), rx.recv())
        .await
        .map_err(|_| Error::Auth("Login timed out after 5 minutes.".into()))?;

    // Close the login window regardless of outcome.
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }

    let code = match received {
        Some(Ok(c)) => c,
        Some(Err(e)) => return Err(Error::Auth(e)),
        None => return Err(Error::Auth("Login was cancelled.".into())),
    };

    // Exchange the authorization code for tokens (Live Connect flow, no PKCE).
    let resp = state
        .http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", cid.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", REDIRECT_URI),
            ("scope", SCOPE),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // Microsoft returns JSON like {"error":"...","error_description":"..."}
        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| {
                v.get("error_description")
                    .or_else(|| v.get("error"))
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or(body);
        return Err(Error::Auth(format!(
            "Token exchange failed ({status}): {detail}"
        )));
    }

    let tr: TokenResp = resp.json().await?;

    if tr.access_token.is_empty() {
        return Err(Error::Auth(
            "Microsoft did not return an access token.".into(),
        ));
    }

    complete_xbox_chain(state, &tr.access_token, &tr.refresh_token, tr.expires_in).await
}

// ── Refresh ───────────────────────────────────────────────────────────────────

/// Refresh an expired Microsoft token and re-run the Minecraft exchange.
pub async fn refresh(state: &AppState, refresh_token: &str) -> Result<Account> {
    let cid = client_id();
    let tr: TokenResp = state
        .http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", cid.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if tr.access_token.is_empty() {
        return Err(Error::Auth(
            "Could not refresh session. Please sign in again.".into(),
        ));
    }
    complete_xbox_chain(state, &tr.access_token, &tr.refresh_token, tr.expires_in).await
}

// ── Xbox/Minecraft chain ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct XblResp {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Deserialize)]
struct Xui {
    uhs: String,
}

#[derive(Deserialize)]
struct McResp {
    access_token: String,
    #[serde(default)]
    expires_in: i64,
}

#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}

async fn complete_xbox_chain(
    state: &AppState,
    ms_access_token: &str,
    refresh_token: &str,
    _ms_expires: i64,
) -> Result<Account> {
    // 1. Xbox Live
    let xbl: XblResp = state
        .http
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                // Live Connect (MBI_SSL) tokens are used raw — no "d=" prefix
                // (that prefix is only for Azure AD v2 "XboxLive.signin" tokens).
                "RpsTicket": ms_access_token
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let uhs = xbl
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .ok_or_else(|| Error::Auth("Xbox Live did not return a user hash.".into()))?;

    // 2. XSTS
    let resp = state
        .http
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl.token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;
    if resp.status().as_u16() == 401 {
        let v: serde_json::Value = resp.json().await.unwrap_or_default();
        let xerr = v.get("XErr").and_then(|x| x.as_i64()).unwrap_or(0);
        let msg = match xerr {
            2148916233 => "This Microsoft account has no Xbox profile. Create one at xbox.com first.",
            2148916235 => "Xbox Live is not available in this account's region.",
            2148916236 | 2148916237 => "This account needs adult verification on xbox.com.",
            2148916238 => "This account is a minor and must be added to a Family by an adult.",
            _ => "Xbox authorization failed.",
        };
        return Err(Error::Auth(msg.into()));
    }
    let xsts: XblResp = resp.error_for_status()?.json().await?;

    // 3. Minecraft services
    let mc_resp = state
        .http
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({ "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token) }))
        .send()
        .await?;
    if !mc_resp.status().is_success() {
        let status = mc_resp.status();
        let body = mc_resp.text().await.unwrap_or_default();
        return Err(Error::Auth(format!("Minecraft login failed ({status}): {body}")));
    }
    let mc: McResp = mc_resp.json().await?;

    // 4. Minecraft profile
    let profile: Profile = state
        .http
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc.access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let expires_at = chrono::Utc::now().timestamp() + mc.expires_in.max(0);
    Ok(Account {
        id: profile.id,
        username: profile.name,
        access_token: mc.access_token,
        refresh_token: Some(refresh_token.to_string()),
        expires_at,
        xuid: None,
        kind: "microsoft".into(),
    })
}

// ── Offline UUID ──────────────────────────────────────────────────────────────

/// Offline-mode UUID matching vanilla's `UUID.nameUUIDFromBytes("OfflinePlayer:<name>")`.
pub fn offline_uuid(name: &str) -> String {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(format!("OfflinePlayer:{name}").as_bytes());
    let mut b = h.finalize();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex = hex::encode(b);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}
