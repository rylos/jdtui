//! `~/.config/jdtui/config.toml`.
//!
//! Credentials are written here after the first successful login so they
//! are not asked again; the file is created with mode 0600.

use std::collections::BTreeMap;
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
    pub direct_addresses: Option<DirectAddresses>,
}

/// Extra direct addresses: one list for the whole account, or a list per
/// JDownloader, keyed by the device name shown in the picker. An address
/// belongs to one machine, so with several JDownloaders the keyed form is
/// the right one: `192.168.1.20:3129` must not be tried for a device that
/// lives elsewhere.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DirectAddresses {
    /// `direct_addresses = ["host:port"]`, tried for every device.
    All(Vec<String>),
    /// `[direct_addresses]` with `"device name" = ["host:port"]`.
    ByDevice(BTreeMap<String, Vec<String>>),
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

    /// The extra addresses to try for `device` (its name), or `None` when
    /// direct connections are off altogether.
    pub fn direct_for(&self, device: &str) -> Option<Vec<String>> {
        if !self.direct.unwrap_or(true) {
            return None;
        }
        Some(match &self.direct_addresses {
            None => Vec::new(),
            Some(DirectAddresses::All(list)) => list.clone(),
            Some(DirectAddresses::ByDevice(by_device)) => by_device.get(device).cloned().unwrap_or_default(),
        })
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml: &str) -> Config {
        toml::from_str(toml).expect("config")
    }

    #[test]
    fn a_plain_list_of_addresses_serves_every_device() {
        let cfg = parse("direct_addresses = [\"192.168.1.20:3129\"]");
        assert_eq!(cfg.direct_for("jd2@home").as_deref(), Some(["192.168.1.20:3129".to_string()].as_slice()));
        assert_eq!(cfg.direct_for("jd2@work").as_deref(), Some(["192.168.1.20:3129".to_string()].as_slice()));
    }

    #[test]
    fn addresses_keyed_by_device_serve_that_device_only() {
        let cfg =
            parse("[direct_addresses]\n\"jd2@home\" = [\"192.168.1.20:3129\"]\n\"jd2@work\" = [\"10.0.0.5:3129\"]\n");
        assert_eq!(cfg.direct_for("jd2@home").as_deref(), Some(["192.168.1.20:3129".to_string()].as_slice()));
        assert_eq!(cfg.direct_for("jd2@work").as_deref(), Some(["10.0.0.5:3129".to_string()].as_slice()));
        // A device nobody named still probes what JDownloader reports.
        assert_eq!(cfg.direct_for("jd2@nas"), Some(Vec::new()));
    }

    #[test]
    fn the_example_config_still_matches_the_code() {
        // config.example.toml is what the README sends people to; it must
        // never drift from the keys jdtui actually reads.
        let cfg: Config = toml::from_str(include_str!("../config.example.toml")).expect("example config");
        assert_eq!(cfg.email.as_deref(), Some("you@example.com"));
        assert_eq!(cfg.refresh_ms(), 1000);
        assert!(cfg.events());
        assert_eq!(cfg.direct_for("jd2@nas").as_deref(), Some(["192.168.1.20:3129".to_string()].as_slice()));
        assert_eq!(cfg.direct_for("jd2@docker").map(|a| a.len()), Some(2));
        assert_eq!(cfg.direct_for("jd2@elsewhere"), Some(Vec::new()));
    }

    #[test]
    fn keyed_addresses_survive_a_save() {
        // The config is rewritten whenever a device is picked; a table
        // must come back as a table.
        let cfg = parse("device = \"abc\"\n\n[direct_addresses]\n\"jd2@home\" = [\"192.168.1.20:3129\"]\n");
        let written = toml::to_string_pretty(&cfg).expect("serialise");
        let back = parse(&written);
        assert_eq!(back.direct_for("jd2@home").as_deref(), Some(["192.168.1.20:3129".to_string()].as_slice()));
        assert_eq!(back.device.as_deref(), Some("abc"));
    }

    #[test]
    fn direct_off_means_the_relay_whatever_the_addresses_say() {
        let cfg = parse("direct = false\ndirect_addresses = [\"192.168.1.20:3129\"]");
        assert_eq!(cfg.direct_for("jd2@home"), None);
    }

    #[test]
    fn without_addresses_the_reported_ones_are_still_probed() {
        let cfg = parse("email = \"you@example.com\"");
        assert_eq!(cfg.direct_for("jd2@home"), Some(Vec::new()));
    }
}
