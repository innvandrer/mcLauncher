//! "Join server → build matching instance": decode the Forge/NeoForge mod
//! list from a Server List Ping response, map each mod to a downloadable file
//! (Modrinth first, then CurseForge), and build an instance with the right
//! loader + mods. Best-effort by design — server-only mods are skipped and
//! client-only mods can't be detected from the handshake at all.
//!
//! Modern Forge/NeoForge (1.18+) compresses the mod list into the status
//! JSON's `forgeData.d` property: a byte buffer packed 15 bits per UTF-16
//! code unit (see `encodeOptimized` in FML's ServerStatusPing). The decoder
//! below is implemented from that format spec and verified by round-trip
//! against our own test encoder — it has NOT been validated against captured
//! real-server payloads (no network in the dev sandbox), so treat unusual
//! servers with care.

use crate::error::{Error, Result};
use crate::state::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

// ---------------------------------------------------------------------------
// Mod list model (attached to ServerStatus)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerMod {
    pub id: String,
    pub version: String,
    /// True when the server marked this mod "IGNORESERVERONLY": clients don't
    /// need it, so instance building skips it.
    pub ignore_server_only: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerModList {
    /// "forge" | "neoforge"
    pub loader: String,
    /// The server sent a truncated list (very large packs).
    pub truncated: bool,
    pub mods: Vec<ServerMod>,
}

/// The marker Forge substitutes for a version when a mod doesn't need to be
/// present on the client.
const IGNORE_SERVER_ONLY: &str = "SERVER_ONLY";

// ---------------------------------------------------------------------------
// forgeData.d decoding (packed binary-in-UTF16)
// ---------------------------------------------------------------------------

/// Undo `encodeOptimized`: chars carry 15 bits each (LSB-first), preceded by
/// two chars holding the byte length (15 bits each, little-endian).
fn decode_packed_utf16(s: &str) -> Result<Vec<u8>> {
    let mut chars = s.chars();
    let size0 = chars.next().ok_or_else(|| Error::Other("empty forgeData.d".into()))? as u32;
    let size1 = chars.next().ok_or_else(|| Error::Other("short forgeData.d".into()))? as u32;
    let size = (size0 | (size1 << 15)) as usize;
    if size > 4 * 1024 * 1024 {
        return Err(Error::Other("forgeData.d too large".into()));
    }

    let mut out = Vec::with_capacity(size);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for c in chars {
        buffer |= ((c as u32) & 0x7FFF) << bits;
        bits += 15;
        while bits >= 8 && out.len() < size {
            out.push((buffer & 0xFF) as u8);
            buffer >>= 8;
            bits -= 8;
        }
    }
    // Trailing bits may still hold the last byte(s).
    while bits > 0 && out.len() < size {
        out.push((buffer & 0xFF) as u8);
        buffer >>= 8;
        bits = bits.saturating_sub(8);
    }
    if out.len() < size {
        return Err(Error::Other("forgeData.d ended early".into()));
    }
    Ok(out)
}

/// Minimal reader over the decoded buffer (Minecraft network primitives).
struct ByteReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|e| *e <= self.buf.len())
            .ok_or_else(|| Error::Other("unexpected end of forge payload".into()))?;
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    fn u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn varint(&mut self) -> Result<u32> {
        let mut result: u32 = 0;
        for i in 0..5 {
            let byte = self.u8()?;
            result |= ((byte & 0x7F) as u32) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(result);
            }
        }
        Err(Error::Other("VarInt too long".into()))
    }

    /// Minecraft `readUtf`: VarInt byte length + UTF-8 bytes.
    fn utf(&mut self) -> Result<String> {
        let len = self.varint()? as usize;
        if len > 1024 * 1024 {
            return Err(Error::Other("string too long in forge payload".into()));
        }
        let bytes = self.take(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Parse the decoded `forgeData.d` buffer (FML `deserializeOptimized`):
/// truncated flag, mod count, then per mod a VarInt whose low bit flags
/// "ignore server only" and whose high bits count the mod's channels.
fn parse_forge_payload(bytes: &[u8]) -> Result<(bool, Vec<ServerMod>)> {
    let mut r = ByteReader::new(bytes);
    let truncated = r.bool()?;
    let mod_count = r.u16()? as usize;
    if mod_count > 4096 {
        return Err(Error::Other("implausible mod count in forge payload".into()));
    }

    let mut mods = Vec::with_capacity(mod_count);
    for _ in 0..mod_count {
        let channel_size_and_flag = r.varint()?;
        let channel_count = (channel_size_and_flag >> 1) as usize;
        let ignore_server_only = (channel_size_and_flag & 1) != 0;
        let id = r.utf()?;
        let version = if ignore_server_only {
            IGNORE_SERVER_ONLY.to_string()
        } else {
            r.utf()?
        };
        for _ in 0..channel_count {
            let _channel_name = r.utf()?;
            let _channel_version = r.utf()?;
            let _required = r.bool()?;
        }
        mods.push(ServerMod {
            id,
            version,
            ignore_server_only,
        });
    }
    // Non-mod network channels follow; nothing there affects the mod list.
    Ok((truncated, mods))
}

/// Extract the mod list from a status-response JSON document, handling every
/// known encoding generation:
/// - 1.18+ Forge/NeoForge: `forgeData.d` (packed, decoded above)
/// - 1.13–1.17 FML2: `forgeData.mods` as plain JSON
/// - 1.7–1.12 FML1: `modinfo.modList`
pub fn parse_status_mods(v: &serde_json::Value) -> Option<ServerModList> {
    if let Some(fd) = v.get("forgeData") {
        let (truncated, mods) = if let Some(d) = fd.get("d").and_then(|x| x.as_str()) {
            match decode_packed_utf16(d).and_then(|b| parse_forge_payload(&b)) {
                Ok(ok) => ok,
                Err(_) => return None,
            }
        } else {
            let list = fd.get("mods").and_then(|x| x.as_array())?;
            let mods = list
                .iter()
                .filter_map(|m| {
                    let id = m.get("modId").and_then(|x| x.as_str())?.to_string();
                    let version = m
                        .get("modmarker")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string();
                    // FML2 uses a literal "OHNOES…" marker for server-only mods.
                    let ignore = version.contains("OHNOES");
                    Some(ServerMod {
                        id,
                        version: if ignore { IGNORE_SERVER_ONLY.into() } else { version },
                        ignore_server_only: ignore,
                    })
                })
                .collect::<Vec<_>>();
            let truncated = fd.get("truncated").and_then(|x| x.as_bool()).unwrap_or(false);
            (truncated, mods)
        };

        // NeoForge kept the `forgeData` field; its presence in the mod list is
        // the reliable tell.
        let loader = if mods.iter().any(|m| m.id == "neoforge") {
            "neoforge"
        } else {
            "forge"
        };
        return Some(ServerModList {
            loader: loader.to_string(),
            truncated,
            mods,
        });
    }

    // Legacy 1.7–1.12: {"modinfo": {"type": "FML", "modList": [{modid, version}]}}
    if let Some(mi) = v.get("modinfo") {
        let list = mi.get("modList").and_then(|x| x.as_array())?;
        let mods = list
            .iter()
            .filter_map(|m| {
                let id = m.get("modid").and_then(|x| x.as_str())?.to_string();
                let version = m
                    .get("version")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string();
                Some(ServerMod {
                    id,
                    version,
                    ignore_server_only: false,
                })
            })
            .collect::<Vec<_>>();
        if mods.is_empty() {
            return None;
        }
        return Some(ServerModList {
            loader: "forge".to_string(),
            truncated: false,
            mods,
        });
    }

    None
}

/// Pull a Minecraft version ("1.21.1") out of a status `version.name` string
/// like "Paper 1.21.1", "forge, 1.20.1" or plain "1.20.4".
pub fn extract_mc_version(version_name: &str) -> Option<String> {
    let bytes = version_name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut dots = 0;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                if bytes[i] == b'.' {
                    dots += 1;
                }
                i += 1;
            }
            // Trailing dot ("1.20.") shouldn't count.
            let mut end = i;
            while end > start && bytes[end - 1] == b'.' {
                end -= 1;
                dots -= 1;
            }
            if dots >= 1 {
                return Some(version_name[start..end].to_string());
            }
        } else {
            i += 1;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Mapping (modId, version) → downloadable files
// ---------------------------------------------------------------------------

/// Mod ids that are part of the platform itself, never separate downloads.
const PLATFORM_MOD_IDS: &[&str] = &["minecraft", "forge", "neoforge", "fml", "mcp"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedMod {
    pub mod_id: String,
    pub version: String,
    /// "modrinth" | "curseforge"
    pub provider: String,
    pub project_id: String,
    pub version_id: String,
    pub file_name: String,
    pub url: String,
    pub sha1: Option<String>,
    /// False when no file matched the server's exact mod version and the
    /// newest compatible file was picked instead.
    pub exact: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedMod {
    pub mod_id: String,
    pub version: String,
    /// A search page the user can open to hunt the mod down manually.
    pub search_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedMod {
    pub mod_id: String,
    /// "platform" | "server-only"
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerModPlan {
    pub address: String,
    pub mc_version: Option<String>,
    pub loader: String,
    pub truncated: bool,
    pub resolved: Vec<PlannedMod>,
    pub unresolved: Vec<UnresolvedMod>,
    pub skipped: Vec<SkippedMod>,
}

/// Normalize a mod id / slug for fuzzy comparison ("Iron_Chests" ≈ "iron-chests").
fn normalize_slug(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Rank a project's versions against the server's mod version: exact
/// `version_number` match, then substring containment either way, then the
/// newest compatible version (marked non-exact).
fn pick_version<'a>(
    versions: &'a [crate::modrinth::ProjectVersion],
    want: &str,
) -> Option<(&'a crate::modrinth::ProjectVersion, bool)> {
    if versions.is_empty() {
        return None;
    }
    if !want.is_empty() && want != IGNORE_SERVER_ONLY {
        if let Some(v) = versions.iter().find(|v| v.version_number == want) {
            return Some((v, true));
        }
        if let Some(v) = versions
            .iter()
            .find(|v| v.version_number.contains(want) || want.contains(&v.version_number))
        {
            return Some((v, true));
        }
        if let Some(v) = versions.iter().find(|v| {
            v.files
                .iter()
                .any(|f| f.filename.to_lowercase().contains(&want.to_lowercase()))
        }) {
            return Some((v, true));
        }
    }
    versions.first().map(|v| (v, false))
}

#[derive(serde::Deserialize)]
struct MrProject {
    id: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    client_side: String,
}

enum ModResolution {
    Planned(PlannedMod),
    ServerOnly,
    NotFound,
}

/// Try Modrinth: slug match on the modId first (modIds frequently equal the
/// Modrinth slug), then a search constrained to the loader + MC version whose
/// best hit's slug normalizes to the modId.
async fn resolve_on_modrinth(
    state: &AppState,
    m: &ServerMod,
    loader: &str,
    mc_version: &str,
) -> Result<ModResolution> {
    let project = {
        state.crosssource.throttle_modrinth().await;
        let direct = state
            .http
            .get(format!("https://api.modrinth.com/v2/project/{}", m.id))
            .send()
            .await;
        let mut found: Option<MrProject> = None;
        if let Ok(r) = direct {
            if let Ok(r) = r.error_for_status() {
                found = r.json().await.ok();
            }
        }
        if found.is_none() {
            state.crosssource.throttle_modrinth().await;
            let hits = crate::modrinth::search(
                state,
                &m.id,
                "mod",
                Some(loader),
                Some(mc_version),
                "relevance",
                5,
                0,
            )
            .await
            .map(|r| r.hits)
            .unwrap_or_default();
            let want = normalize_slug(&m.id);
            if let Some(hit) = hits.iter().find(|h| normalize_slug(&h.slug) == want) {
                state.crosssource.throttle_modrinth().await;
                let r = state
                    .http
                    .get(format!(
                        "https://api.modrinth.com/v2/project/{}",
                        hit.project_id
                    ))
                    .send()
                    .await?;
                if let Ok(r) = r.error_for_status() {
                    found = r.json().await.ok();
                }
            }
        }
        found
    };

    let Some(project) = project else {
        return Ok(ModResolution::NotFound);
    };
    if project.client_side == "unsupported" {
        return Ok(ModResolution::ServerOnly);
    }

    state.crosssource.throttle_modrinth().await;
    let versions =
        crate::modrinth::project_versions(state, &project.id, Some(loader), Some(mc_version))
            .await
            .unwrap_or_default();
    let Some((version, exact)) = pick_version(&versions, &m.version) else {
        return Ok(ModResolution::NotFound);
    };
    let Some(file) = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
    else {
        return Ok(ModResolution::NotFound);
    };

    let _ = project.slug; // (kept for debugging clarity in the struct)
    Ok(ModResolution::Planned(PlannedMod {
        mod_id: m.id.clone(),
        version: m.version.clone(),
        provider: "modrinth".into(),
        project_id: project.id.clone(),
        version_id: version.id.clone(),
        file_name: file.filename.clone(),
        url: file.url.clone(),
        sha1: file.hashes.sha1.clone(),
        exact,
    }))
}

/// Try CurseForge: exact slug filter on the search endpoint, then the newest
/// compatible file (preferring one whose name carries the wanted version).
/// Files the API refuses to serve fall back to the Modrinth mirror or are
/// treated as not found.
async fn resolve_on_curseforge(
    state: &AppState,
    m: &ServerMod,
    loader: &str,
    mc_version: &str,
) -> Result<ModResolution> {
    let Ok(_key) = crate::curseforge::api_key(state) else {
        return Ok(ModResolution::NotFound);
    };
    state.crosssource.throttle_curseforge().await;
    let Some(candidate) = crate::curseforge::find_server_mod_file(
        state,
        &m.id,
        &m.version,
        Some(loader),
        mc_version,
    )
    .await?
    else {
        return Ok(ModResolution::NotFound);
    };

    Ok(ModResolution::Planned(PlannedMod {
        mod_id: m.id.clone(),
        version: m.version.clone(),
        provider: "curseforge".into(),
        project_id: candidate.project_id.to_string(),
        version_id: candidate.file_id.to_string(),
        file_name: candidate.file_name,
        url: candidate.url,
        sha1: candidate.sha1,
        exact: candidate.exact,
    }))
}

/// Ping the server, decode its mod list, and map every mod to a downloadable
/// file. Never fails per-mod: unresolvable mods land in `unresolved` with a
/// manual search link.
pub async fn analyze_server(
    app: &AppHandle,
    state: &AppState,
    address: &str,
) -> Result<ServerModPlan> {
    let status = crate::servers::ping_server(address).await;
    if !status.online {
        return Err(Error::Other("Server is offline or unreachable.".into()));
    }
    let Some(mod_list) = status.mod_info else {
        return Err(Error::Other(
            "No mod list in the server's ping response (vanilla, plugin-based, or Fabric server)."
                .into(),
        ));
    };
    let mc_version = status.version.as_deref().and_then(extract_mc_version);

    let mut plan = ServerModPlan {
        address: address.to_string(),
        mc_version: mc_version.clone(),
        loader: mod_list.loader.clone(),
        truncated: mod_list.truncated,
        resolved: Vec::new(),
        unresolved: Vec::new(),
        skipped: Vec::new(),
    };

    let Some(mc_version) = mc_version else {
        return Err(Error::Other(
            "Couldn't tell the server's Minecraft version from its ping response.".into(),
        ));
    };

    let task_id = format!("server-analyze:{address}");
    let total = mod_list.mods.len() as u64;
    let mut done = 0u64;
    for m in &mod_list.mods {
        done += 1;
        crate::net::emit_progress(
            app,
            &task_id,
            "Analyzing server mods",
            "resolve",
            done,
            total,
            false,
            None,
        );

        if PLATFORM_MOD_IDS.contains(&m.id.as_str()) {
            plan.skipped.push(SkippedMod {
                mod_id: m.id.clone(),
                reason: "platform".into(),
            });
            continue;
        }
        if m.ignore_server_only {
            plan.skipped.push(SkippedMod {
                mod_id: m.id.clone(),
                reason: "server-only".into(),
            });
            continue;
        }

        let resolution = match resolve_on_modrinth(state, m, &mod_list.loader, &mc_version).await {
            Ok(ModResolution::NotFound) => {
                resolve_on_curseforge(state, m, &mod_list.loader, &mc_version)
                    .await
                    .unwrap_or(ModResolution::NotFound)
            }
            Ok(other) => other,
            Err(_) => ModResolution::NotFound,
        };

        match resolution {
            ModResolution::Planned(p) => plan.resolved.push(p),
            ModResolution::ServerOnly => plan.skipped.push(SkippedMod {
                mod_id: m.id.clone(),
                reason: "server-only".into(),
            }),
            ModResolution::NotFound => plan.unresolved.push(UnresolvedMod {
                mod_id: m.id.clone(),
                version: m.version.clone(),
                search_url: format!(
                    "https://modrinth.com/mods?q={}",
                    urlencode(&m.id)
                ),
            }),
        }
    }
    crate::net::emit_progress(app, &task_id, "Analyzing server mods", "resolve", total, total, true, None);

    Ok(plan)
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The result of building an instance from a server's mod list.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstanceOutcome {
    pub instance: crate::models::Instance,
    pub plan: ServerModPlan,
}

/// Create an instance matching a modded server: right loader + MC version,
/// all resolvable mods installed, and a report of everything that needs
/// manual attention. Downloads reuse the shared pipeline (task id
/// `modpack:<instance>` so the grid card shows install progress).
pub async fn create_instance_from_server(
    app: &AppHandle,
    state: &AppState,
    name: &str,
    address: &str,
) -> Result<ServerInstanceOutcome> {
    use crate::net::DownloadItem;

    let plan = analyze_server(app, state, address).await?;
    let mc_version = plan
        .mc_version
        .clone()
        .ok_or_else(|| Error::Other("Unknown server Minecraft version.".into()))?;

    let loader = match plan.loader.as_str() {
        "neoforge" => crate::models::Loader::Neoforge,
        _ => crate::models::Loader::Forge,
    };
    // Latest recommended/newest loader build for that MC version. The ping
    // doesn't carry the server's exact loader build, and clients don't need
    // to match it anyway.
    let loader_version = match loader {
        crate::models::Loader::Neoforge => crate::forge::list_neoforge(state, &mc_version).await,
        _ => crate::forge::list_forge(state, &mc_version).await,
    }
    .ok()
    .and_then(|v| v.first().map(|f| f.version.clone()));
    if loader_version.is_none() {
        return Err(Error::Other(format!(
            "No {} builds found for Minecraft {mc_version}.",
            plan.loader
        )));
    }

    let instance = crate::instances::create_instance(
        state,
        name,
        &mc_version,
        loader,
        loader_version,
        None,
    )?;
    let _ = app.emit("instance://created", instance.clone());

    let mods_dir = state.dirs.game_dir(&instance.id).join("mods");
    let items: Vec<DownloadItem> = plan
        .resolved
        .iter()
        .map(|p| DownloadItem::new(p.url.clone(), mods_dir.join(&p.file_name), p.sha1.clone()))
        .collect();

    let settings = crate::instances::load_settings(state);
    let task_id = format!("modpack:{}", instance.id);
    let cancel = state.cancel_flag(&task_id);
    let res = crate::net::download_many_cancellable(
        app,
        &state.http,
        &task_id,
        &format!("Building instance for {address}"),
        items,
        settings.max_concurrent_downloads,
        Some(cancel),
    )
    .await;
    state.clear_cancel(&task_id);

    if let Err(e) = res {
        let _ = crate::instances::delete_instance(state, &instance.id);
        let _ = app.emit("instance://removed", instance.id.clone());
        return Err(e);
    }

    // Track every installed mod's identity for updates/exports.
    for provider in ["modrinth", "curseforge"] {
        let entries: Vec<(String, String)> = plan
            .resolved
            .iter()
            .filter(|p| p.provider == provider)
            .map(|p| (p.file_name.clone(), p.project_id.clone()))
            .collect();
        crate::instances::record_installs(state, &instance.id, &entries, provider);
    }

    Ok(ServerInstanceOutcome { instance, plan })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test-only mirror of FML's `encodeOptimized` + `serializeOptimized`, so
    // the decoder can be validated by round-trip. Fixtures are generated from
    // the format spec — NOT captured from live servers.
    mod encoder {
        pub fn write_varint(buf: &mut Vec<u8>, mut v: u32) {
            loop {
                let mut b = (v & 0x7F) as u8;
                v >>= 7;
                if v != 0 {
                    b |= 0x80;
                }
                buf.push(b);
                if v == 0 {
                    break;
                }
            }
        }

        pub fn write_utf(buf: &mut Vec<u8>, s: &str) {
            write_varint(buf, s.len() as u32);
            buf.extend_from_slice(s.as_bytes());
        }

        pub struct Mod<'a> {
            pub id: &'a str,
            pub version: &'a str,
            pub ignore_server_only: bool,
            pub channels: Vec<(&'a str, &'a str, bool)>,
        }

        pub fn serialize_payload(truncated: bool, mods: &[Mod]) -> Vec<u8> {
            let mut buf = Vec::new();
            buf.push(truncated as u8);
            buf.extend_from_slice(&(mods.len() as u16).to_be_bytes());
            for m in mods {
                let flag = ((m.channels.len() as u32) << 1) | (m.ignore_server_only as u32);
                write_varint(&mut buf, flag);
                write_utf(&mut buf, m.id);
                if !m.ignore_server_only {
                    write_utf(&mut buf, m.version);
                }
                for (name, version, required) in &m.channels {
                    write_utf(&mut buf, name);
                    write_utf(&mut buf, version);
                    buf.push(*required as u8);
                }
            }
            // Non-mod channel list (empty).
            write_varint(&mut buf, 0);
            buf
        }

        pub fn encode_packed_utf16(bytes: &[u8]) -> String {
            let len = bytes.len() as u32;
            let mut s = String::new();
            s.push(char::from_u32(len & 0x7FFF).unwrap());
            s.push(char::from_u32((len >> 15) & 0x7FFF).unwrap());
            let mut buffer: u32 = 0;
            let mut bits: u32 = 0;
            for b in bytes {
                buffer |= (*b as u32) << bits;
                bits += 8;
                if bits >= 15 {
                    s.push(char::from_u32(buffer & 0x7FFF).unwrap());
                    buffer >>= 15;
                    bits -= 15;
                }
            }
            if bits > 0 {
                s.push(char::from_u32(buffer & 0x7FFF).unwrap());
            }
            s
        }
    }

    #[test]
    fn packed_utf16_round_trips() {
        for data in [
            Vec::new(),
            vec![0u8],
            vec![0xFF],
            (0u8..=255).collect::<Vec<u8>>(),
            b"hello forge world".to_vec(),
            vec![0xAB; 1000],
        ] {
            let encoded = encoder::encode_packed_utf16(&data);
            let decoded = decode_packed_utf16(&encoded).unwrap();
            assert_eq!(decoded, data, "len {}", data.len());
        }
    }

    #[test]
    fn decodes_full_forge_handshake_payload() {
        let mods = vec![
            encoder::Mod {
                id: "jei",
                version: "15.2.0.27",
                ignore_server_only: false,
                channels: vec![("jei:networking", "1.0.0", true)],
            },
            encoder::Mod {
                id: "chunkloaders",
                version: "1.2.8a",
                ignore_server_only: false,
                channels: vec![],
            },
            encoder::Mod {
                id: "spark",
                version: "1.10.53",
                ignore_server_only: true, // server-only marker
                channels: vec![],
            },
        ];
        let payload = encoder::serialize_payload(false, &mods);
        let d = encoder::encode_packed_utf16(&payload);

        let status = serde_json::json!({
            "version": { "name": "Paper 1.20.1", "protocol": 763 },
            "forgeData": {
                "channels": [],
                "mods": [],
                "d": d,
                "fmlNetworkVersion": 3,
                "truncated": false
            }
        });
        let list = parse_status_mods(&status).expect("mod list parsed");
        assert_eq!(list.loader, "forge");
        assert!(!list.truncated);
        assert_eq!(list.mods.len(), 3);
        assert_eq!(list.mods[0].id, "jei");
        assert_eq!(list.mods[0].version, "15.2.0.27");
        assert!(!list.mods[0].ignore_server_only);
        assert_eq!(list.mods[1].id, "chunkloaders");
        assert!(list.mods[2].ignore_server_only);
        assert_eq!(list.mods[2].version, IGNORE_SERVER_ONLY);
    }

    #[test]
    fn detects_neoforge_and_truncation() {
        let mods = vec![
            encoder::Mod {
                id: "neoforge",
                version: "21.1.90",
                ignore_server_only: false,
                channels: vec![],
            },
            encoder::Mod {
                id: "sodium",
                version: "0.6.0",
                ignore_server_only: false,
                channels: vec![],
            },
        ];
        let payload = encoder::serialize_payload(true, &mods);
        let d = encoder::encode_packed_utf16(&payload);
        let status = serde_json::json!({ "forgeData": { "d": d, "fmlNetworkVersion": 3 } });
        let list = parse_status_mods(&status).unwrap();
        assert_eq!(list.loader, "neoforge");
        assert!(list.truncated);
    }

    #[test]
    fn parses_fml2_plain_json_mod_list() {
        let status = serde_json::json!({
            "forgeData": {
                "fmlNetworkVersion": 2,
                "mods": [
                    { "modId": "forge", "modmarker": "ANY" },
                    { "modId": "create", "modmarker": "0.5.1f" },
                    { "modId": "serverstuff", "modmarker": "OHNOES\u{1F631}" }
                ]
            }
        });
        let list = parse_status_mods(&status).unwrap();
        assert_eq!(list.loader, "forge");
        assert_eq!(list.mods.len(), 3);
        assert_eq!(list.mods[1].id, "create");
        assert_eq!(list.mods[1].version, "0.5.1f");
        assert!(list.mods[2].ignore_server_only);
    }

    #[test]
    fn parses_legacy_fml1_modinfo() {
        let status = serde_json::json!({
            "modinfo": {
                "type": "FML",
                "modList": [
                    { "modid": "mcp", "version": "9.42" },
                    { "modid": "ic2", "version": "2.8.170" }
                ]
            }
        });
        let list = parse_status_mods(&status).unwrap();
        assert_eq!(list.loader, "forge");
        assert_eq!(list.mods[1].id, "ic2");
        assert_eq!(list.mods[1].version, "2.8.170");
    }

    #[test]
    fn vanilla_status_has_no_mod_list() {
        let status = serde_json::json!({ "version": { "name": "1.21.1" }, "players": {} });
        assert!(parse_status_mods(&status).is_none());
    }

    #[test]
    fn corrupt_packed_data_is_rejected_not_panicking() {
        let status = serde_json::json!({ "forgeData": { "d": "\u{0002}\u{0000}x" } });
        assert!(parse_status_mods(&status).is_none());
    }

    #[test]
    fn mc_version_extraction() {
        assert_eq!(extract_mc_version("Paper 1.21.1").as_deref(), Some("1.21.1"));
        assert_eq!(extract_mc_version("1.20.4").as_deref(), Some("1.20.4"));
        assert_eq!(extract_mc_version("forge, 1.20.1").as_deref(), Some("1.20.1"));
        assert_eq!(extract_mc_version("NeoForge 1.21").as_deref(), Some("1.21"));
        assert_eq!(extract_mc_version("Velocity 3"), None);
        assert_eq!(extract_mc_version(""), None);
    }

    #[test]
    fn version_ranking_prefers_exact_then_fuzzy_then_newest() {
        fn v(number: &str, filename: &str) -> crate::modrinth::ProjectVersion {
            serde_json::from_value(serde_json::json!({
                "id": format!("id-{number}"),
                "project_id": "p",
                "version_number": number,
                "name": number,
                "date_published": "2025-01-01T00:00:00Z",
                "files": [{ "url": "u", "filename": filename, "primary": true, "size": 1,
                            "hashes": { "sha1": null, "sha512": null } }],
                "dependencies": []
            }))
            .unwrap()
        }
        let versions = vec![
            v("16.0.1", "jei-16.0.1.jar"),
            v("15.2.0.27", "jei-1.20.1-15.2.0.27.jar"),
            v("15.0.0", "jei-15.0.0.jar"),
        ];
        // Exact version-number match.
        let (picked, exact) = pick_version(&versions, "15.2.0.27").unwrap();
        assert_eq!(picked.version_number, "15.2.0.27");
        assert!(exact);
        // Containment (server reports a longer string).
        let (picked, exact) = pick_version(&versions, "15.0.0+forge").unwrap();
        assert_eq!(picked.version_number, "15.0.0");
        assert!(exact);
        // No match: newest compatible, flagged approximate.
        let (picked, exact) = pick_version(&versions, "14.9.9").unwrap();
        assert_eq!(picked.version_number, "16.0.1");
        assert!(!exact);
        // Server-only marker never pretends to be exact.
        let (_, exact) = pick_version(&versions, IGNORE_SERVER_ONLY).unwrap();
        assert!(!exact);
    }

    #[test]
    fn slug_normalization() {
        assert_eq!(normalize_slug("Iron_Chests"), "ironchests");
        assert_eq!(normalize_slug("iron-chests"), "ironchests");
        assert_ne!(normalize_slug("ironchest"), normalize_slug("iron-chests"));
    }

    #[test]
    fn urlencoding() {
        assert_eq!(urlencode("simple-id_1.2~x"), "simple-id_1.2~x");
        assert_eq!(urlencode("a b/c"), "a%20b%2Fc");
    }
}
