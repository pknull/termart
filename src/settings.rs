use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub fah: FahSettings,
    #[serde(default)]
    pub globe: GlobeSettings,
    #[serde(default)]
    pub sunlight: SunlightSettings,
    #[serde(default)]
    pub tui: TuiSettings,
}

#[derive(Debug, Default, Deserialize)]
pub struct FahSettings {
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub fah_secret: Option<String>,  // Base64-encoded PKCS#8 RSA private key from browser localStorage
    pub fah_sid: Option<String>,     // Session ID from browser localStorage (fah-sid)
}

#[derive(Debug, Default, Deserialize)]
pub struct GlobeSettings {
    pub geoip_db: Option<PathBuf>,   // Path to GeoLite2-City.mmdb database
}

#[derive(Debug, Default, Deserialize)]
pub struct SunlightSettings {
    pub latitude: Option<f64>,       // Latitude in degrees (-90 to 90)
    pub longitude: Option<f64>,      // Longitude in degrees (-180 to 180)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TuiSettings {
    pub players: Vec<String>,
    pub keybinds: TuiKeybinds,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            players: vec!["spotify".into(), "vlc".into(), "mpd".into()],
            keybinds: TuiKeybinds::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TuiKeybinds {
    pub quit: Vec<String>,
    pub toggle: Vec<String>,
    pub next: Vec<String>,
    pub prev: Vec<String>,
    pub seek_forward: Vec<String>,
    pub seek_backward: Vec<String>,
    pub volume_up: Vec<String>,
    pub volume_down: Vec<String>,
}

impl Default for TuiKeybinds {
    fn default() -> Self {
        Self {
            quit: vec!["q".into(), "Escape".into()],
            toggle: vec![" ".into()],
            next: vec!["n".into(), "Right".into()],
            prev: vec!["p".into(), "Left".into()],
            seek_forward: vec!["l".into(), "Shift+Right".into()],
            seek_backward: vec!["h".into(), "Shift+Left".into()],
            volume_up: vec!["k".into(), "Up".into()],
            volume_down: vec!["j".into(), "Down".into()],
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(settings) => settings,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse {}: {}\nUsing defaults.",
                        path.display(),
                        e
                    );
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: Could not read {}: {}\nUsing defaults.",
                    path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("termart")
            .join("config.toml")
    }
}
