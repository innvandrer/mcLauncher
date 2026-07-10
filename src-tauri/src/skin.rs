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
    let account = instances::active_account(state)?;
    if account.kind != "microsoft" {
        return Err(Error::Auth(
            "Skin management is only available for Microsoft accounts".into(),
        ));
    }
    if account.access_token.is_empty() {
        return Err(Error::Auth(
            "Session token missing. Please sign in again.".into(),
        ));
    }
    Ok(account.access_token)
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

// ---------------------------------------------------------------------------
// Skin wardrobe — a local library of saved skins to re-apply with one click.
// ---------------------------------------------------------------------------

fn kind_url() -> String {
    "url".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSkin {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub variant: String,
    /// "url" for a remote texture, "file" for one imported from disk.
    #[serde(default = "kind_url")]
    pub kind: String,
    /// Remote texture URL (empty for file skins).
    #[serde(default)]
    pub url: String,
    /// Absolute path to the imported PNG copy (file skins only).
    #[serde(default)]
    pub path: Option<String>,
    /// Data-URI preview the webview can render (file skins only; URL skins
    /// preview straight from `url`).
    #[serde(default)]
    pub image: Option<String>,
}

fn skins_file(state: &AppState) -> std::path::PathBuf {
    state.dirs.root.join("skins.json")
}

fn skins_dir(state: &AppState) -> std::path::PathBuf {
    state.dirs.root.join("skins")
}

pub fn list_saved_skins(state: &AppState) -> Vec<SavedSkin> {
    std::fs::read(skins_file(state))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_saved_skins(state: &AppState, skins: &[SavedSkin]) -> Result<()> {
    let data = serde_json::to_vec_pretty(skins)?;
    std::fs::write(skins_file(state), data)?;
    Ok(())
}

/// Standard base64 (no line breaks). Used to inline an imported skin's preview
/// so the webview can render it without an asset-protocol round trip.
fn b64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// Save a remote-URL skin to the wardrobe (de-duplicated by URL).
pub fn save_skin(state: &AppState, name: &str, url: &str, variant: &str) -> Result<Vec<SavedSkin>> {
    let mut skins = list_saved_skins(state);
    if url.is_empty() || !skins.iter().any(|s| s.url == url) {
        let name = name.trim();
        skins.push(SavedSkin {
            id: uuid::Uuid::new_v4().to_string(),
            name: if name.is_empty() { "Skin".to_string() } else { name.to_string() },
            variant: if variant.is_empty() { "classic".to_string() } else { variant.to_string() },
            kind: "url".to_string(),
            url: url.to_string(),
            path: None,
            image: None,
        });
        write_saved_skins(state, &skins)?;
    }
    Ok(skins)
}

/// Import a local PNG into the wardrobe so it can be re-applied without a URL.
/// The file is copied into the launcher's data dir and a preview is inlined.
pub fn save_skin_file(
    state: &AppState,
    name: &str,
    file_path: &str,
    variant: &str,
) -> Result<Vec<SavedSkin>> {
    let bytes = std::fs::read(file_path)
        .map_err(|e| Error::Other(format!("Could not read skin file: {e}")))?;
    let id = uuid::Uuid::new_v4().to_string();
    let dir = skins_dir(state);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{id}.png"));
    std::fs::write(&dest, &bytes)?;

    let name = name.trim();
    let mut skins = list_saved_skins(state);
    skins.push(SavedSkin {
        id,
        name: if name.is_empty() { "Skin".to_string() } else { name.to_string() },
        variant: if variant.is_empty() { "classic".to_string() } else { variant.to_string() },
        kind: "file".to_string(),
        url: String::new(),
        path: Some(dest.to_string_lossy().to_string()),
        image: Some(format!("data:image/png;base64,{}", b64_encode(&bytes))),
    });
    write_saved_skins(state, &skins)?;
    Ok(skins)
}

pub fn delete_saved_skin(state: &AppState, id: &str) -> Result<Vec<SavedSkin>> {
    let mut skins = list_saved_skins(state);
    if let Some(s) = skins.iter().find(|s| s.id == id) {
        if let Some(p) = &s.path {
            let _ = std::fs::remove_file(p);
        }
    }
    skins.retain(|s| s.id != id);
    write_saved_skins(state, &skins)?;
    Ok(skins)
}

// ---------------------------------------------------------------------------
// Player skin lookup — fetch any player's skin by username / UUID (or a pasted
// NameMC profile link) so it can be previewed and applied without a URL.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSkin {
    pub username: String,
    pub uuid: String,
    pub url: String,
    pub variant: String,
}

/// Standard base64 decode (ignores padding/whitespace).
fn b64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    }
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        buf = (buf << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// Pull a bare username/UUID out of free-form input — handles pasted
/// `namemc.com/profile/<x>` links and other URLs as well as plain text.
fn extract_player_query(input: &str) -> String {
    let s = input.trim();
    if let Some(idx) = s.to_lowercase().find("/profile/") {
        let rest = &s[idx + "/profile/".len()..];
        let seg = rest.split(['/', '?', '#']).next().unwrap_or(rest);
        return seg.trim().to_string();
    }
    if s.contains("://") {
        if let Some(seg) = s.split('/').rfind(|p| !p.is_empty()) {
            return seg.split(['?', '#']).next().unwrap_or(seg).to_string();
        }
    }
    s.to_string()
}

fn is_uuid_like(s: &str) -> bool {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// Resolve a username / UUID / NameMC link to that player's active skin.
pub async fn fetch_player_skin(state: &AppState, query: &str) -> Result<PlayerSkin> {
    let q = extract_player_query(query);
    if q.is_empty() {
        return Err(Error::Other("Enter a username or UUID.".into()));
    }

    // Resolve to an undashed UUID (+ canonical username when known).
    let (uuid, mut username) = if is_uuid_like(&q) {
        (q.chars().filter(|c| *c != '-').collect::<String>(), String::new())
    } else {
        #[derive(Deserialize)]
        struct NameToId {
            id: String,
            #[allow(dead_code)]
            name: String,
        }
        let resp = state
            .http
            .get(format!("https://api.mojang.com/users/profiles/minecraft/{q}"))
            .send()
            .await?;
        if resp.status() != reqwest::StatusCode::OK {
            return Err(Error::NotFound(format!("player “{q}”")));
        }
        let n: NameToId = resp.json().await?;
        (n.id, n.name)
    };

    // Fetch the profile and decode its base64 `textures` property.
    #[derive(Deserialize)]
    struct Session {
        name: String,
        properties: Vec<Prop>,
    }
    #[derive(Deserialize)]
    struct Prop {
        name: String,
        value: String,
    }
    let session: Session = state
        .http
        .get(format!(
            "https://sessionserver.mojang.com/session/minecraft/profile/{uuid}"
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if username.is_empty() {
        username = session.name.clone();
    }

    let textures_b64 = session
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .map(|p| p.value.clone())
        .ok_or_else(|| Error::Other("No textures on this profile.".into()))?;
    let decoded =
        b64_decode(&textures_b64).ok_or_else(|| Error::Other("Could not decode profile textures.".into()))?;
    let v: serde_json::Value = serde_json::from_slice(&decoded)?;

    let skin = v.get("textures").and_then(|t| t.get("SKIN"));
    let url = skin
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .ok_or_else(|| Error::Other(format!("{username} has no custom skin.")))?
        .to_string();
    let variant = skin
        .and_then(|s| s.get("metadata"))
        .and_then(|m| m.get("model"))
        .and_then(|m| m.as_str())
        .map(|m| if m == "slim" { "slim" } else { "classic" })
        .unwrap_or("classic")
        .to_string();

    Ok(PlayerSkin { username, uuid, url, variant })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        let cases: &[&[u8]] = &[b"", b"a", b"ab", b"abc", b"hello world", &[0, 255, 128, 1, 2, 3]];
        for data in cases {
            let enc = b64_encode(data);
            assert_eq!(b64_decode(&enc).unwrap(), *data, "roundtrip for {data:?}");
        }
    }

    #[test]
    fn base64_known() {
        assert_eq!(b64_encode(b"Man"), "TWFu");
        assert_eq!(b64_encode(b"Ma"), "TWE=");
        assert_eq!(b64_encode(b"M"), "TQ==");
    }

    #[test]
    fn player_query_extraction() {
        assert_eq!(extract_player_query("Notch"), "Notch");
        assert_eq!(extract_player_query("https://namemc.com/profile/Notch"), "Notch");
        assert_eq!(
            extract_player_query(" https://namemc.com/profile/jeb_/something "),
            "jeb_",
        );
    }

    #[test]
    fn uuid_detection() {
        assert!(is_uuid_like("069a79f444e94726a5befca90e38aaf5"));
        assert!(is_uuid_like("069a79f4-44e9-4726-a5be-fca90e38aaf5"));
        assert!(!is_uuid_like("Notch"));
    }
}
