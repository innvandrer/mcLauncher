//! Discord Rich Presence: shows what instance is playing in Discord.
//! Reconnects on demand, so it works even if Discord is opened after EZMapa.
//! Silently no-ops if Discord isn't running or the IPC pipe isn't available.

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const CLIENT_ID: &str = "1382889592032276520";
const LARGE_IMAGE: &str = "beacon_logo";
const LARGE_TEXT: &str = "EZMapa Launcher";

/// Shared presence handle. Wraps the IPC client so launch/stop threads can
/// update it without holding the whole AppState lock.
#[derive(Clone)]
pub struct DiscordPresence(Arc<Mutex<Option<DiscordIpcClient>>>);

impl DiscordPresence {
    pub fn new() -> Self {
        let me = Self(Arc::new(Mutex::new(None)));
        // Show "in the launcher" right away, but do the initial IPC connect on
        // a background thread — `new()` runs during app setup, which shouldn't
        // wait on Discord's pipe.
        let bg = me.clone();
        std::thread::spawn(move || bg.set_idle());
        me
    }

    /// Run `f` against a connected client, lazily (re)connecting if needed.
    /// If the call fails (e.g. Discord was closed), the client is dropped so the
    /// next call transparently reconnects.
    fn with_client<F>(&self, f: F)
    where
        F: FnOnce(&mut DiscordIpcClient) -> Result<(), discord_rich_presence::error::Error>,
    {
        let mut lock = match self.0.lock() {
            Ok(l) => l,
            Err(_) => return,
        };
        // (Re)connect if we don't currently hold a live client.
        if lock.is_none() {
            let mut client = DiscordIpcClient::new(CLIENT_ID);
            match client.connect() {
                Ok(()) => *lock = Some(client),
                Err(_) => return, // Discord not running — stay silent.
            }
        }
        let failed = match lock.as_mut() {
            Some(client) => f(client).is_err(),
            None => false,
        };
        if failed {
            *lock = None;
        }
    }

    /// Call when Minecraft launches.
    pub fn set_playing(&self, instance_name: &str, mc_version: &str, loader: &str) {
        let details = format!("Playing {instance_name}");
        let state = if loader == "vanilla" {
            mc_version.to_string()
        } else {
            format!("{mc_version} · {loader}")
        };
        let started = unix_now();
        self.with_client(move |client| {
            let activity = activity::Activity::new()
                .details(&details)
                .state(&state)
                .timestamps(activity::Timestamps::new().start(started))
                .assets(
                    activity::Assets::new()
                        .large_image(LARGE_IMAGE)
                        .large_text(LARGE_TEXT),
                );
            client.set_activity(activity)
        });
    }

    /// Idle/menu presence shown while not in-game.
    pub fn set_idle(&self) {
        self.with_client(|client| {
            let activity = activity::Activity::new()
                .details("In the launcher")
                .state("Browsing instances")
                .assets(
                    activity::Assets::new()
                        .large_image(LARGE_IMAGE)
                        .large_text(LARGE_TEXT),
                );
            client.set_activity(activity)
        });
    }

}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
