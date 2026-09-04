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
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) {}
