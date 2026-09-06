//! `~/.config/jdtui/config.toml`.
//!
//! Credentials are written here after the first successful login so they
//! are not asked again; the file is created with mode 0600.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub email: Option<String>,
    pub password: Option<String>,
    /// Device id chosen last time; skipped when it no longer exists.
    pub device: Option<String>,
    /// Interface refresh period in milliseconds.
    pub refresh_ms: Option<u64>,
    /// Listen to JDownloader's event channel (default true): changes show
    /// up at once, and the refresh slows down while nothing downloads.
    pub events: Option<bool>,
    /// Talk to JDownloader directly when one of the addresses it reports
    /// answers (default true), as the web interface does; the relay is the
    /// fallback either way.
    pub direct: Option<bool>,
    /// Addresses to try besides those JDownloader reports, `host:port`,
    /// for a JDownloader that does not know how it is reached (a Docker
    /// container sees its own address only).
    pub direct_addresses: Option<Vec<String>>,
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("jdtui").join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        restrict_permissions(&path);
        Ok(())
    }

    pub fn has_credentials(&self) -> bool {
        matches!((&self.email, &self.password), (Some(e), Some(p)) if !e.is_empty() && !p.is_empty())
    }

    pub fn refresh_ms(&self) -> u64 {
        self.refresh_ms.unwrap_or(1000).max(200)
    }

    pub fn events(&self) -> bool {
        self.events.unwrap_or(true)
    }

    /// The extra direct addresses to try, or none when direct connections
    /// are off.
    pub fn direct(&self) -> Option<Vec<String>> {
        self.direct.unwrap_or(true).then(|| self.direct_addresses.clone().unwrap_or_default())
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) {}
