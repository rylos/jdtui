//! Asking GitHub whether a newer jdtui has been released.
//!
//! This is the only thing jdtui says to anyone but My.JDownloader and the
//! JDownloader itself, so it is easy to turn off (`update_check = false`),
//! it asks at most once a day, it never blocks anything, and a failure is
//! silent: not being able to reach GitHub is not a problem worth a message.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const LATEST: &str = "https://api.github.com/repos/rylos/jdtui/releases/latest";
const TIMEOUT: Duration = Duration::from_secs(10);
/// Least time between one look and the next.
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// What was found last time, so a run that happens minutes after another
/// asks nobody.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Cache {
    /// Seconds since the epoch.
    checked: u64,
    /// The newest version GitHub reported, without the leading `v`.
    latest: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

fn cache_path() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".")).join("jdtui").join("update.json")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// What a look for a newer jdtui found, and when the looking happened.
#[derive(Debug, Clone)]
pub struct Check {
    /// The newer version, or `None` for "this one is the newest known".
    pub newer: Option<String>,
    /// Seconds since the epoch. An answer from the cache carries the time
    /// the cache was filled, not now: "nothing newer" is only ever true as
    /// of when someone last asked, and the About panel says so.
    pub checked: u64,
}

/// Look for a newer release, from the cache when it was filled less than a
/// day ago. `None` means nobody has managed to ask yet.
pub fn look(current: &str) -> Option<Check> {
    let cached = fs::read_to_string(cache_path()).ok().and_then(|t| serde_json::from_str::<Cache>(&t).ok());
    let cache = match cached {
        Some(cache) if now().saturating_sub(cache.checked) < EVERY.as_secs() => cache,
        _ => {
            let fresh = Cache { checked: now(), latest: ask()? };
            let path = cache_path();
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            if let Ok(text) = serde_json::to_string(&fresh) {
                let _ = fs::write(&path, text);
            }
            fresh
        }
    };
    let newer = (parse(&cache.latest) > parse(current)).then(|| cache.latest.clone());
    Some(Check { newer, checked: cache.checked })
}

fn ask() -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).build().into();
    let text = agent
        .get(LATEST)
        // GitHub refuses a request with no user agent.
        .header("User-Agent", concat!("jdtui/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .into_body()
        .read_to_string()
        .ok()?;
    let release: Release = serde_json::from_str(&text).ok()?;
    Some(release.tag_name.trim_start_matches('v').to_string())
}

/// `1.7.0` as `(1, 7, 0)`. Anything unparseable sorts lowest, so a tag in a
/// shape this does not know never announces itself as an update.
fn parse(version: &str) -> (u64, u64, u64) {
    let mut parts = version.split(['.', '-', '+']).map(|p| p.parse::<u64>().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert!(parse("1.10.0") > parse("1.9.0"), "text order would put 1.10 first");
        assert!(parse("2.0.0") > parse("1.99.99"));
        assert_eq!(parse("1.7.0"), parse("1.7.0"));
    }

    #[test]
    fn a_version_in_an_unknown_shape_is_not_an_update() {
        assert!(parse("nightly") < parse("0.0.1"));
        assert!(parse("1.7.0-rc1") < parse("1.7.1"));
    }

    #[test]
    #[ignore]
    fn ask_github() {
        println!("latest: {:?}", ask());
        println!("looking as 0.0.1: {:?}", look("0.0.1"));
    }
}
