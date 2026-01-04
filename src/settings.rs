use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub fah: FahSettings,
    #[serde(default)]
    pub globe: GlobeSettings,
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

impl Settings {
    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("termart")
            .join("config.toml")
    }
}
