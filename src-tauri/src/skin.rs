//! Minecraft skin management via the Minecraft Services API.
//! Only Microsoft accounts support skin changes.

use crate::error::{Error, Result};
use crate::instances;
use crate::state::AppState;
use serde::{Deserialize, Serialize};

const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinInfo {
    pub url: String,
    pub variant: String, // "classic" or "slim"
}

#[derive(Debug, Deserialize)]
struct McProfile {
    skins: Vec<McSkin>,
}

#[derive(Debug, Deserialize)]
struct McSkin {
    state: String,
    url: String,
    #[serde(default = "default_variant")]
    variant: String,
}

fn default_variant() -> String {
    "classic".to_string()
}

fn access_token(state: &AppState) -> Result<String> {
    let account = instances::active_account(state)
        .ok_or_else(|| Error::Auth("No active account".into()))?;
    if account.kind != "microsoft" {
        return Err(Error::Auth(
            "Skin management is only available for Microsoft accounts".into(),
        ));
    }
    Ok(account.access_token.clone())
}

/// Fetch the active skin for the logged-in account.
pub async fn get_skin(state: &AppState) -> Result<SkinInfo> {
    let token = access_token(state)?;
    let profile: McProfile = state
        .http
        .get(PROFILE_URL)
        .bearer_auth(&token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let active = profile
        .skins
        .into_iter()
        .find(|s| s.state == "ACTIVE")
        .ok_or_else(|| Error::Other("No active skin found on this account".into()))?;

    Ok(SkinInfo {
        url: active.url,
        variant: active.variant.to_lowercase(),
    })
}

/// Change skin by providing a URL to an existing skin image.
/// `variant` should be `"classic"` or `"slim"`.
pub async fn set_skin_url(state: &AppState, url: &str, variant: &str) -> Result<()> {
    let token = access_token(state)?;
    let body = serde_json::json!({ "variant": variant, "url": url });
    state
        .http
        .post(format!("{PROFILE_URL}/skins"))
        .bearer_auth(&token)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Upload a local skin file from disk.
/// `file_path` is an absolute path to a PNG file.
/// `variant` should be `"classic"` or `"slim"`.
pub async fn set_skin_file(state: &AppState, file_path: &str, variant: &str) -> Result<()> {
    let token = access_token(state)?;
    let bytes = std::fs::read(file_path)
        .map_err(|e| Error::Other(format!("Could not read skin file: {e}")))?;

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(|e| Error::Other(e.to_string()))?;
    let form = reqwest::multipart::Form::new()
        .text("variant", variant.to_string())
        .part("file", part);

    state
        .http
        .put(format!("{PROFILE_URL}/skins"))
        .bearer_auth(&token)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}
