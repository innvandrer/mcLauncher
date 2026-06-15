//! Microsoft account authentication using the OAuth 2.0 device-code flow, then
//! the Xbox Live -> XSTS -> Minecraft token exchange.
//!
//! Requires an Azure "public client" application id with the
//! `XboxLive.signin offline_access` scope. Provide it via the `BEACON_CLIENT_ID`
//! environment variable (see README). This keeps the launcher compliant: every
//! user authenticates with their own valid Microsoft/Minecraft account.

use crate::error::{Error, Result};
use crate::models::Account;
use crate::state::AppState;
use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};

const PLACEHOLDER_CLIENT_ID: &str = "REPLACE_WITH_AZURE_CLIENT_ID";
const BUNDLED_CLIENT_ID: &str = "d1685745-eb57-46a1-99ca-ae061033b189";
const DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "XboxLive.signin offline_access";

fn client_id() -> String {
    std::env::var("BEACON_CLIENT_ID")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| BUNDLED_CLIENT_ID.to_string())
}

#[derive(Deserialize)]
struct DeviceCodeResp {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: i64,
    interval: u64,
    message: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AuthPrompt {
    user_code: String,
    verification_uri: String,
    message: String,
    expires_in: i64,
}

#[derive(Deserialize, Default)]
struct TokenResp {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    expires_in: i64,
    #[serde(default)]
    error: String,
}

/// Run the full device-code login. Emits `auth://prompt` with the code for the
/// user, polls until they finish, then completes the Xbox/Minecraft exchange.
pub async fn login_device_code(app: &AppHandle, state: &AppState) -> Result<Account> {
    let cid = client_id();
    if cid == PLACEHOLDER_CLIENT_ID {
        return Err(Error::Auth(
            "No Azure client ID configured. Set the BEACON_CLIENT_ID environment variable to \
             your Azure application (public client) id with the 'XboxLive.signin offline_access' \
             scope. See the README for a 2-minute setup. (You can use an Offline account to test \
             the launcher without signing in.)"
                .into(),
        ));
    }

    let dc: DeviceCodeResp = state
        .http
        .post(DEVICE_CODE_URL)
        .form(&[("client_id", cid.as_str()), ("scope", SCOPE)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let _ = app.emit(
        "auth://prompt",
        AuthPrompt {
            user_code: dc.user_code.clone(),
            verification_uri: dc.verification_uri.clone(),
            message: dc.message.clone(),
            expires_in: dc.expires_in,
        },
    );

    let interval = dc.interval.max(1);
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs(dc.expires_in.max(60) as u64);

    let ms = loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        if std::time::Instant::now() > deadline {
            return Err(Error::Auth("Login timed out. Please try again.".into()));
        }
        let resp = state
            .http
            .post(TOKEN_URL)
            .form(&[
                ("client_id", cid.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", dc.device_code.as_str()),
            ])
            .send()
            .await?;
        let success = resp.status().is_success();
        let tr: TokenResp = resp.json().await.unwrap_or_default();
        if success && !tr.access_token.is_empty() {
            break tr;
        }
        match tr.error.as_str() {
            "authorization_pending" | "" => continue,
            "slow_down" => {
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                continue;
            }
            "authorization_declined" => return Err(Error::Auth("Login was declined.".into())),
            "expired_token" => {
                return Err(Error::Auth("The login code expired. Please try again.".into()))
            }
            other => return Err(Error::Auth(format!("Login error: {other}"))),
        }
    };

    complete_xbox_chain(state, &ms.access_token, &ms.refresh_token, ms.expires_in).await
}

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
        return Err(Error::Auth("Could not refresh session. Please sign in again.".into()));
    }
    complete_xbox_chain(state, &tr.access_token, &tr.refresh_token, tr.expires_in).await
}

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
                "RpsTicket": format!("d={ms_access_token}")
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
            2148916233 => "This Microsoft account has no Xbox profile. Create one at xbox.com, then try again.",
            2148916235 => "Xbox Live is not available in this account's region.",
            2148916236 | 2148916237 => "This account needs adult verification.",
            2148916238 => "This account is a minor and must be added to a Family by an adult.",
            _ => "Xbox authorization failed.",
        };
        return Err(Error::Auth(msg.into()));
    }
    let xsts: XblResp = resp.error_for_status()?.json().await?;

    // 3. Minecraft services login
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

/// Compute the offline-mode UUID exactly like vanilla
/// (`UUID.nameUUIDFromBytes("OfflinePlayer:<name>")`, an MD5-based v3 UUID).
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
