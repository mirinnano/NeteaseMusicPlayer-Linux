//
// discord_rpc.rs
// Discord Rich Presence integration
// Distributed under terms of the GPL-3.0 license.
//
use discord_rich_presence::{
    DiscordIpc, DiscordIpcClient,
    activity::{Activity, ActivityType, Assets, Timestamps},
};
use log::*;
use ncm_api::SongInfo;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Discord Application Client ID.
const DISCORD_CLIENT_ID: &str = "1523317273966809309";

/// Stores the current activity state for partial updates.
#[derive(Clone)]
struct ActivityState {
    song_name: String,
    artist: String,
    album: String,
    pic_url: String,
    duration_secs: u64,
    playing: bool,
    start_ts: i64,
}

/// Commands sent from the main thread to the Discord IPC thread.
enum DiscordCommand {
    /// Update the rich presence with a new song.
    UpdateActivity {
        song_name: String,
        artist: String,
        album: String,
        pic_url: String,
        duration_secs: u64,
        position_secs: u64,
        playing: bool,
        current_lyric: Option<String>,
    },
    /// Update only the lyric text (no throttle, for frequent lyric changes).
    UpdateLyric {
        lyric: String,
    },

    /// Update play/pause state (preserves song info, updates timestamps).
    UpdatePlayState {
        playing: bool,
        position_secs: u64,
    },

    /// Set lyrics for the current song.
    SetLyrics {
        lyrics: Vec<(u64, String)>,
    },
    /// Clear lyrics for the current song.
    ClearLyrics,
    /// Clear the current activity (e.g. on pause with no song).
    ClearActivity,
    /// Shut down the IPC thread.
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct DiscordRpcController {
    sender: Arc<Mutex<mpsc::Sender<DiscordCommand>>>,

    lyrics: Arc<Mutex<Vec<(u64, String)>>>,
    last_update: Arc<AtomicU64>,
}

// SAFETY: The inner fields are always accessed behind a Mutex.
unsafe impl Send for DiscordRpcController {}
unsafe impl Sync for DiscordRpcController {}

impl DiscordRpcController {
    /// Spawns the Discord IPC background thread and attempts to connect.
    /// Returns `None` if the thread fails to start.
    pub fn new() -> Option<Self> {
        let (tx, rx) = mpsc::channel::<DiscordCommand>();
        let sender = Arc::new(Mutex::new(tx));
        let connected = Arc::new(Mutex::new(false));
        let lyrics = Arc::new(Mutex::new(Vec::new()));
        let last_update = Arc::new(AtomicU64::new(0));
        let connected_clone = connected.clone();
        let lyrics_clone = lyrics.clone();

        let result = thread::Builder::new()
            .name("discord-rpc".into())
            .spawn(move || {
                Self::ipc_thread_main(rx, connected_clone, lyrics_clone);
            });

        match result {
            Ok(_handle) => {
                info!("[DiscordRPC] IPC thread started");
                Some(Self {
                    sender,

                    lyrics,
                    last_update,
                })
            }
            Err(e) => {
                warn!("[DiscordRPC] Failed to spawn IPC thread: {e}");
                None
            }
        }
    }

    /// The main loop of the Discord IPC thread.
    /// Owns the `DiscordIpcClient` and processes commands sequentially.
    fn ipc_thread_main(
        rx: mpsc::Receiver<DiscordCommand>,
        connected: Arc<Mutex<bool>>,
        lyrics: Arc<Mutex<Vec<(u64, String)>>>,
    ) {
        let mut client = DiscordIpcClient::new(DISCORD_CLIENT_ID);

        // Try initial connection
        let mut is_connected = false;
        if let Err(e) = client.connect() {
            warn!("[DiscordRPC] Initial connection failed (Discord may not be running): {e}");
        } else {
            info!("[DiscordRPC] Connected to Discord");
            is_connected = true;
        }
        {
            let mut c = connected.lock().unwrap();
            *c = is_connected;
        }

        // Activity state for partial updates
        let mut activity_state: Option<ActivityState> = None;


        // Process commands
        for cmd in rx {
            match cmd {
                DiscordCommand::UpdateActivity {
                    song_name,
                    artist,
                    album,
                    pic_url,
                    duration_secs,
                    position_secs,
                    playing,
                    current_lyric,
                } => {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;

                    let start_ts = now - position_secs as i64;
                    let end_ts = start_ts + duration_secs as i64;

                    // Store activity state for partial updates
                    activity_state = Some(ActivityState {
                        song_name: song_name.clone(),
                        artist: artist.clone(),
                        album: album.clone(),
                        pic_url: pic_url.clone(),
                        duration_secs,
                        playing,
                        start_ts,
                    });


                    // If not connected, try to reconnect
                    if !is_connected {
                        match client.reconnect() {
                            Ok(()) => {
                                info!("[DiscordRPC] Reconnected to Discord");
                                is_connected = true;
                            }
                            Err(e) => {
                                debug!("[DiscordRPC] Reconnect failed: {e}");
                                continue;
                            }
                        }
                    }

                    // Use current lyric if available, otherwise show artist
                    let state = match &current_lyric {
                        Some(lyric) if !lyric.is_empty() => {
                            // Truncate lyric to fit Discord's 128 char limit
                            let truncated = if lyric.len() > 120 {
                                format!("{}...", &lyric[..117])
                            } else {
                                lyric.clone()
                            };
                            format!("♪ {truncated}")
                        }
                        _ => format!("by {artist}"),
                    };

                    let mut activity = Activity::new()
                        .activity_type(ActivityType::Listening)
                        .name(&song_name)
                        .details(&song_name)
                        .state(state);

                    if playing && duration_secs > 0 {
                        activity = activity.timestamps(
                            Timestamps::new().start(start_ts).end(end_ts),
                        );
                    }

                    if !album.is_empty() {
                        activity = activity.assets(
                            Assets::new()
                                .large_image(&pic_url)
                                .large_text(&album),
                        );
                    } else if !pic_url.is_empty() {
                        activity = activity.assets(
                            Assets::new().large_image(&pic_url),
                        );
                    }

                    if let Err(e) = client.set_activity(activity) {
                        warn!("[DiscordRPC] Failed to set activity: {e}");
                        is_connected = false;
                        {
                            let mut c = connected.lock().unwrap();
                            *c = false;
                        }
                    }
                }
                DiscordCommand::UpdateLyric { lyric: lyric_text } => {
                    // Update lyric text without changing timestamps
                    if !is_connected {
                        match client.reconnect() {
                            Ok(()) => {
                                info!("[DiscordRPC] Reconnected to Discord");
                                is_connected = true;
                            }
                            Err(e) => {
                                debug!("[DiscordRPC] Reconnect failed: {e}");
                                continue;
                            }
                        }
                    }

                    // Build activity with stored state — include timestamps to keep progress bar visible
                    if let Some(state) = &activity_state {
                        let state_text = if !lyric_text.is_empty() {
                            let truncated = if lyric_text.len() > 120 {
                                format!("{}...", &lyric_text[..117])
                            } else {
                                lyric_text.clone()
                            };
                            format!("♪ {truncated}")
                        } else {
                            format!("by {}", state.artist)
                        };

                        let end_ts = state.start_ts + state.duration_secs as i64;

                        let mut activity = Activity::new()
                            .activity_type(ActivityType::Listening)
                            .name(&state.song_name)
                            .details(&state.song_name)
                            .state(&state_text)
                            .timestamps(
                                Timestamps::new().start(state.start_ts).end(end_ts),
                            );

                        if !state.album.is_empty() {
                            activity = activity.assets(
                                Assets::new()
                                    .large_image(&state.pic_url)
                                    .large_text(&state.album),
                            );
                        } else if !state.pic_url.is_empty() {
                            activity = activity.assets(
                                Assets::new().large_image(&state.pic_url),
                            );
                        }

                        if let Err(e) = client.set_activity(activity) {
                            warn!("[DiscordRPC] Failed to set activity: {e}");
                            is_connected = false;
                            {
                                let mut c = connected.lock().unwrap();
                                *c = false;
                            }
                        }
                    }
                }
                DiscordCommand::UpdatePlayState { playing, position_secs: _ } => {
                    // Update play/pause state without changing song info
                    if let Some(state) = &mut activity_state {
                        state.playing = playing;

                        let mut activity = Activity::new()
                            .activity_type(ActivityType::Listening)
                            .name(&state.song_name)
                            .details(&state.song_name)
                            .state(if playing {
                                format!("♪ playing")
                            } else {
                                format!("⏸ paused")
                            });

                        // Only show timestamps when playing (shows progress bar)
                        if playing && state.duration_secs > 0 {
                            let end_ts = state.start_ts + state.duration_secs as i64;
                            activity = activity.timestamps(
                                Timestamps::new().start(state.start_ts).end(end_ts),
                            );
                        }
                        // When paused, omit timestamps so Discord doesn't show the progress bar

                        if !state.album.is_empty() {
                            activity = activity.assets(
                                Assets::new()
                                    .large_image(&state.pic_url)
                                    .large_text(&state.album),
                            );
                        } else if !state.pic_url.is_empty() {
                            activity = activity.assets(
                                Assets::new().large_image(&state.pic_url),
                            );
                        }

                        if let Err(e) = client.set_activity(activity) {
                            warn!("[DiscordRPC] Failed to set activity: {e}");
                            is_connected = false;
                            {
                                let mut c = connected.lock().unwrap();
                                *c = false;
                            }
                        }
                    }
                }

                DiscordCommand::SetLyrics { lyrics: new_lyrics } => {
                    // Store lyrics in the shared state
                    if let Ok(mut l) = lyrics.lock() {
                        *l = new_lyrics.clone();
                        debug!("[DiscordRPC] Stored {} lyric lines", new_lyrics.len());
                    }
                }
                DiscordCommand::ClearLyrics => {
                    // Clear lyrics from the shared state
                    if let Ok(mut l) = lyrics.lock() {
                        l.clear();
                        debug!("[DiscordRPC] Cleared lyrics");
                    }

                }
                DiscordCommand::ClearActivity => {
                    if is_connected {
                        if let Err(e) = client.clear_activity() {
                            warn!("[DiscordRPC] Failed to clear activity: {e}");
                            is_connected = false;
                            {
                                let mut c = connected.lock().unwrap();
                                *c = false;
                            }
                        }
                    }
                    activity_state = None;

                }
                DiscordCommand::Shutdown => {
                    if is_connected {
                        let _ = client.close();
                    }
                    info!("[DiscordRPC] IPC thread shutting down");
                    return;
                }
            }
            {
                let mut c = connected.lock().unwrap();
                *c = is_connected;
            }
        }

        // Channel closed, shut down
        if is_connected {
            let _ = client.close();
        }
        info!("[DiscordRPC] IPC thread exiting (channel closed)");
    }

    /// Find the current lyric line based on playback position (in milliseconds).
    pub fn find_current_lyric(&self, position_ms: u64) -> Option<String> {
        if let Ok(lyrics) = self.lyrics.lock() {
            if lyrics.is_empty() {
                debug!("[DiscordRPC] find_current_lyric: lyrics empty at {}ms", position_ms);
                return None;
            }

            // Find the last lyric with timestamp <= position_ms
            let mut current: Option<&str> = None;
            for (ts, text) in lyrics.iter() {
                if *ts <= position_ms {
                    current = Some(text);
                } else {
                    break;
                }
            }

            let result = current.map(|s| s.to_string());
            if let Some(ref r) = result {
                debug!("[DiscordRPC] find_current_lyric at {}ms: Some(\"{}\")", position_ms, r);
            } else {
                debug!("[DiscordRPC] find_current_lyric: no lyric at {}ms", position_ms);
            }
            result
        } else {
            None
        }
    }

    /// Update the rich presence activity.
    /// Called when song changes, play/pause, or periodically for position sync.
    pub fn update_activity(
        &self,
        si: &SongInfo,
        duration_secs: u64,
        position_secs: u64,
        playing: bool,
    ) {
        // Throttle updates to avoid Discord rendering glitches (min 1s between updates).
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last = self.last_update.load(Ordering::Relaxed);
        if now_ms.saturating_sub(last) < 1000 && position_secs > 0 {
            return;
        }
        self.last_update.store(now_ms, Ordering::Relaxed);

        debug!("[DiscordRPC] update_activity: {} - pos={}s dur={}s playing={}", si.name, position_secs, duration_secs, playing);
        let song_name = si.name.clone();
        let artist = si.singer.clone();
        let album = si.album.clone();
        let pic_url = si.pic_url.clone();
        let position_ms = position_secs * 1000;
        let current_lyric = if playing {
            self.find_current_lyric(position_ms)
        } else {
            None
        };

        let cmd = DiscordCommand::UpdateActivity {
            song_name,
            artist,
            album,
            pic_url,
            duration_secs,
            position_secs,
            playing,
            current_lyric,
        };

        if let Ok(tx) = self.sender.lock() {
            if let Err(e) = tx.send(cmd) {
                warn!("[DiscordRPC] Failed to send update command: {e}");
            }
        }
    }
    /// Update only the lyric text (no throttle).
    /// Called frequently as lyrics change during playback.
    pub fn update_lyric(&self, lyric: String) {
        debug!("[DiscordRPC] update_lyric: {:?}", lyric);
        let cmd = DiscordCommand::UpdateLyric { lyric };
        if let Ok(tx) = self.sender.lock() {
            if let Err(e) = tx.send(cmd) {
                warn!("[DiscordRPC] Failed to send lyric update command: {e}");
            }
        }
    }
    /// Set lyrics for the current song.
    pub fn set_lyrics(&self, lyrics: Vec<(u64, String)>) {
        debug!("[DiscordRPC] set_lyrics called with {} lines", lyrics.len());
        if let Ok(mut l) = self.lyrics.lock() {
            *l = lyrics.clone();
        }
        if let Ok(tx) = self.sender.lock() {
            let _ = tx.send(DiscordCommand::SetLyrics { lyrics });
        }
    }

    /// Clear lyrics (e.g. when song changes).
    pub fn clear_lyrics(&self) {
        if let Ok(tx) = self.sender.lock() {
            let _ = tx.send(DiscordCommand::ClearLyrics);
        }
    }

    /// Clear the current activity (e.g. when paused with no song).
    pub fn clear_activity(&self) {
        if let Ok(tx) = self.sender.lock() {
            let _ = tx.send(DiscordCommand::ClearActivity);
        }
    }

    /// Update play/pause state without changing song info.
    /// Called on play/pause toggle. Updates timestamps to keep progress bar in sync.
    pub fn set_playing(&self, playing: bool, position_secs: u64) {
        debug!("[DiscordRPC] set_playing: playing={}, position={}s", playing, position_secs);
        if let Ok(tx) = self.sender.lock() {
            let _ = tx.send(DiscordCommand::UpdatePlayState { playing, position_secs });
        }
    }

    /// Shut down the Discord IPC thread.
    pub fn shutdown(&self) {
        if let Ok(tx) = self.sender.lock() {
            let _ = tx.send(DiscordCommand::Shutdown);
        }
    }
}
