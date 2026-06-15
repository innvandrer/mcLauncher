//! Discord Rich Presence: shows what instance is playing in Discord.
//! Uses the official Beacon app client ID. Silently no-ops if Discord isn't
//! running or the IPC pipe isn't available.

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const CLIENT_ID: &str = "1382889592032276520";

/// Shared presence handle. Wraps the IPC client so launch/stop threads can
/// update it without holding the whole AppState lock.
#[derive(Clone)]
pub struct DiscordPresence(Arc<Mutex<Option<DiscordIpcClient>>>);

impl DiscordPresence {
    pub fn new() -> Self {
        let mut client = DiscordIpcClient::new(CLIENT_ID);
        let client = match client.connect() {
            Ok(()) => Some(client),
            Err(_) => None, // Discord not running — stay silent.
        };
        Self(Arc::new(Mutex::new(client)))
    }

    /// Call when Minecraft launches.
    pub fn set_playing(&self, instance_name: &str, mc_version: &str, loader: &str) {
        let details = format!("Playing {instance_name}");
        let state = if loader == "vanilla" {
            mc_version.to_string()
        } else {
            format!("{mc_version} · {loader}")
        };
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let activity = activity::Activity::new()
            .details(&details)
            .state(&state)
            .timestamps(activity::Timestamps::new().start(started))
            .assets(
                activity::Assets::new()
                    .large_image("beacon_logo")
                    .large_text("Beacon Launcher"),
            );

        if let Ok(mut lock) = self.0.lock() {
            if let Some(client) = lock.as_mut() {
                let _ = client.set_activity(activity);
            }
        }
    }

    /// Call when the instance stops.
    pub fn clear(&self) {
        if let Ok(mut lock) = self.0.lock() {
            if let Some(client) = lock.as_mut() {
                let _ = client.clear_activity();
            }
        }
    }
}
