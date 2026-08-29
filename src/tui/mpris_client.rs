use dbus::ffidisp::Connection;
use mpris::{LoopStatus, Metadata, PlaybackStatus, Player, PlayerFinder};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;
use std::time::Duration;

pub type MprisResult<T> = Result<T, String>;

#[derive(Debug, PartialEq, Eq)]
enum SessionBusTarget {
    Environment,
    Explicit(PathBuf),
}

fn fallback_session_bus_path(effective_uid: u32) -> PathBuf {
    PathBuf::from(format!("/run/user/{effective_uid}/bus"))
}

fn select_session_bus_target(
    dbus_session_bus_address: Option<&OsStr>,
    xdg_runtime_dir: Option<&OsStr>,
    effective_uid: u32,
    fallback_is_socket: bool,
) -> SessionBusTarget {
    if dbus_session_bus_address.is_some() || xdg_runtime_dir.is_some() || !fallback_is_socket {
        SessionBusTarget::Environment
    } else {
        SessionBusTarget::Explicit(fallback_session_bus_path(effective_uid))
    }
}

fn current_session_bus_target() -> SessionBusTarget {
    let effective_uid = unsafe { libc::geteuid() };
    let fallback_path = fallback_session_bus_path(effective_uid);
    let fallback_is_socket = fs::metadata(&fallback_path)
        .map(|metadata| metadata.file_type().is_socket())
        .unwrap_or(false);

    select_session_bus_target(
        env::var_os("DBUS_SESSION_BUS_ADDRESS").as_deref(),
        env::var_os("XDG_RUNTIME_DIR").as_deref(),
        effective_uid,
        fallback_is_socket,
    )
}

fn create_player_finder() -> MprisResult<PlayerFinder> {
    match current_session_bus_target() {
        SessionBusTarget::Environment => PlayerFinder::new().map_err(|error| error.to_string()),
        SessionBusTarget::Explicit(path) => {
            let address = format!("unix:path={}", path.display());
            let connection = Connection::open_private(&address).map_err(|error| {
                format!("failed to connect to fallback session bus {address}: {error}")
            })?;
            connection.register().map_err(|error| {
                format!("failed to register with fallback session bus {address}: {error}")
            })?;
            Ok(PlayerFinder::for_connection(connection))
        }
    }
}

/// Current player state
#[derive(Debug, Clone, Default)]
pub struct PlayerState {
    pub connected: bool,
    pub title: String,
    pub artists: String,
    pub album: String,
    pub art_url: Option<String>,
    pub status: Status,
    pub position: Duration,
    pub length: Duration,
    pub volume: f64,
    pub shuffle: bool,
    pub loop_status: RepeatMode,
}

/// Playback status
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Status {
    Playing,
    Paused,
    #[default]
    Stopped,
}

impl From<PlaybackStatus> for Status {
    fn from(s: PlaybackStatus) -> Self {
        match s {
            PlaybackStatus::Playing => Status::Playing,
            PlaybackStatus::Paused => Status::Paused,
            PlaybackStatus::Stopped => Status::Stopped,
        }
    }
}

impl Status {
    pub fn icon(&self) -> &'static str {
        match self {
            Status::Playing => "⏸",
            Status::Paused => "▶",
            Status::Stopped => "⏹",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum RepeatMode {
    #[default]
    None,
    Track,
    Playlist,
}

impl From<LoopStatus> for RepeatMode {
    fn from(s: LoopStatus) -> Self {
        match s {
            LoopStatus::None => RepeatMode::None,
            LoopStatus::Track => RepeatMode::Track,
            LoopStatus::Playlist => RepeatMode::Playlist,
        }
    }
}

impl RepeatMode {
    pub fn icon(&self) -> &'static str {
        match self {
            RepeatMode::None => "",
            RepeatMode::Track => "↻1",
            RepeatMode::Playlist => "↻",
        }
    }
}

/// MPRIS client for controlling media players
pub struct MprisClient {
    player: Option<Player>,
    preferred_players: Vec<String>,
}

impl MprisClient {
    /// Create a new MPRIS client
    pub fn new(preferred_players: Vec<String>) -> Self {
        Self {
            player: None,
            preferred_players,
        }
    }

    /// Try to connect to a media player
    pub fn connect(&mut self) -> MprisResult<bool> {
        let finder = create_player_finder()?;

        // Try preferred players first
        for preferred in &self.preferred_players {
            let preferred_lower = preferred.to_lowercase();
            if let Ok(players) = finder.find_all() {
                for player in players {
                    let identity = player.identity().to_lowercase();
                    if identity.contains(&preferred_lower) {
                        self.player = Some(player);
                        return Ok(true);
                    }
                }
            }
        }

        // Fall back to any active player
        if let Ok(player) = finder.find_active() {
            self.player = Some(player);
            return Ok(true);
        }

        // Try first available player
        if let Ok(players) = finder.find_all() {
            if let Some(player) = players.into_iter().next() {
                self.player = Some(player);
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Check if still connected and reconnect if needed
    pub fn ensure_connected(&mut self) -> bool {
        if let Some(ref player) = self.player {
            if player.is_running() {
                return true;
            }
        }

        self.player = None;
        self.connect().unwrap_or(false)
    }

    /// Get current player state
    pub fn get_state(&mut self) -> PlayerState {
        if !self.ensure_connected() {
            return PlayerState::default();
        }

        let player = match &self.player {
            Some(p) => p,
            None => return PlayerState::default(),
        };

        let metadata = player.get_metadata().ok();
        let status = player
            .get_playback_status()
            .map(Status::from)
            .unwrap_or_default();

        let position = player.get_position().unwrap_or(Duration::ZERO);

        let length = metadata
            .as_ref()
            .and_then(|m| m.length())
            .unwrap_or(Duration::ZERO);

        let volume = player.get_volume().unwrap_or(-1.0);
        let shuffle = player.get_shuffle().unwrap_or(false);
        let loop_status = player
            .get_loop_status()
            .map(RepeatMode::from)
            .unwrap_or_default();

        PlayerState {
            connected: true,
            title: extract_title(&metadata),
            artists: extract_artists(&metadata),
            album: extract_album(&metadata),
            art_url: metadata
                .as_ref()
                .and_then(|m| m.art_url().map(String::from)),
            status,
            position,
            length,
            volume,
            shuffle,
            loop_status,
        }
    }

    /// Toggle play/pause
    pub fn toggle(&mut self) -> MprisResult<()> {
        if let Some(ref player) = self.player {
            player.play_pause().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Next track
    pub fn next(&mut self) -> MprisResult<()> {
        if let Some(ref player) = self.player {
            player.next().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Previous track
    pub fn prev(&mut self) -> MprisResult<()> {
        if let Some(ref player) = self.player {
            player.previous().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Seek forward by duration
    pub fn seek_forward(&mut self, duration: Duration) -> MprisResult<()> {
        if let Some(ref player) = self.player {
            let micros = duration.as_micros();
            let offset = if micros > i64::MAX as u128 {
                i64::MAX
            } else {
                micros as i64
            };
            player.seek(offset).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Seek backward by duration
    pub fn seek_backward(&mut self, duration: Duration) -> MprisResult<()> {
        if let Some(ref player) = self.player {
            let micros = duration.as_micros();
            let offset = if micros > i64::MAX as u128 {
                i64::MIN
            } else {
                -(micros as i64)
            };
            player.seek(offset).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Set position
    pub fn set_position(&mut self, position: Duration) -> MprisResult<()> {
        if let Some(ref player) = self.player {
            let metadata = player.get_metadata().map_err(|e| e.to_string())?;
            let track_id = metadata
                .track_id()
                .ok_or_else(|| "No track ID available - cannot set position".to_string())?;
            player
                .set_position(track_id, &position)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Adjust volume by delta
    pub fn adjust_volume(&mut self, delta: f64) -> MprisResult<()> {
        if let Some(ref player) = self.player {
            let current = player.get_volume().unwrap_or(1.0);
            let new_volume = (current + delta).clamp(0.0, 1.0);
            player.set_volume(new_volume).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

fn extract_title(metadata: &Option<Metadata>) -> String {
    metadata
        .as_ref()
        .and_then(|m| m.title().map(String::from))
        .unwrap_or_else(|| "Unknown".into())
}

fn extract_artists(metadata: &Option<Metadata>) -> String {
    metadata
        .as_ref()
        .and_then(|m| m.artists())
        .map(|a| a.join(", "))
        .unwrap_or_else(|| "Unknown Artist".into())
}

fn extract_album(metadata: &Option<Metadata>) -> String {
    metadata
        .as_ref()
        .and_then(|m| m.album_name().map(String::from))
        .unwrap_or_else(|| "Unknown Album".into())
}

/// Format duration as MM:SS
pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let mins = secs / 60;
    let secs = secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[test]
    fn session_bus_selection_uses_fallback_socket_when_environment_is_absent() {
        assert_eq!(
            select_session_bus_target(None, None, 1000, true),
            SessionBusTarget::Explicit(PathBuf::from("/run/user/1000/bus"))
        );
    }

    #[test]
    fn session_bus_selection_keeps_explicit_dbus_address() {
        assert_eq!(
            select_session_bus_target(
                Some(OsStr::new("unix:path=/custom/session-bus")),
                None,
                1000,
                true,
            ),
            SessionBusTarget::Environment
        );
    }

    #[test]
    fn session_bus_selection_keeps_explicit_runtime_directory() {
        assert_eq!(
            select_session_bus_target(None, Some(OsStr::new("/custom/runtime")), 1000, true),
            SessionBusTarget::Environment
        );
    }

    #[test]
    fn session_bus_selection_treats_empty_environment_values_as_present() {
        assert_eq!(
            select_session_bus_target(Some(OsStr::new("")), None, 1000, true),
            SessionBusTarget::Environment
        );
        assert_eq!(
            select_session_bus_target(None, Some(OsStr::new("")), 1000, true),
            SessionBusTarget::Environment
        );
    }

    #[test]
    fn session_bus_selection_does_not_use_non_socket_fallback() {
        assert_eq!(
            select_session_bus_target(None, None, 1000, false),
            SessionBusTarget::Environment
        );
    }

    #[test]
    fn session_bus_selection_uses_effective_user_id_in_fallback_path() {
        assert_eq!(
            select_session_bus_target(None, None, 4242, true),
            SessionBusTarget::Explicit(PathBuf::from("/run/user/4242/bus"))
        );
    }

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(Duration::from_secs(0)), "00:00");
    }

    #[test]
    fn test_format_duration_seconds_only() {
        assert_eq!(format_duration(Duration::from_secs(5)), "00:05");
        assert_eq!(format_duration(Duration::from_secs(59)), "00:59");
    }

    #[test]
    fn test_format_duration_minutes_and_seconds() {
        assert_eq!(format_duration(Duration::from_secs(60)), "01:00");
        assert_eq!(format_duration(Duration::from_secs(65)), "01:05");
        assert_eq!(format_duration(Duration::from_secs(125)), "02:05");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(Duration::from_secs(3600)), "60:00");
        assert_eq!(format_duration(Duration::from_secs(3661)), "61:01");
        assert_eq!(format_duration(Duration::from_secs(7384)), "123:04");
    }

    #[test]
    fn test_format_duration_ignores_subseconds() {
        assert_eq!(format_duration(Duration::from_millis(1500)), "00:01");
        assert_eq!(format_duration(Duration::from_millis(59999)), "00:59");
    }

    #[test]
    fn test_status_icon() {
        assert_eq!(Status::Playing.icon(), "⏸");
        assert_eq!(Status::Paused.icon(), "▶");
        assert_eq!(Status::Stopped.icon(), "⏹");
    }

    #[test]
    fn test_status_default() {
        assert_eq!(Status::default(), Status::Stopped);
    }
}
