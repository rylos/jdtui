//! JDownloader calls on one device, on top of the My.JDownloader transport.
//!
//! Field names follow the JDownloader API; `Option` marks what the device
//! only returns when asked for or when it has a value (a disabled link, for
//! instance, simply omits `enabled`).

use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::myjd::{Error, MyJd, Result};

pub type SharedApi = Arc<Mutex<JdApi>>;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub uuid: i64,
    #[serde(default)]
    pub name: String,
    pub bytes_loaded: Option<i64>,
    pub bytes_total: Option<i64>,
    pub child_count: Option<i64>,
    pub comment: Option<String>,
    pub enabled: Option<bool>,
    pub eta: Option<i64>,
    pub finished: Option<bool>,
    pub priority: Option<String>,
    pub running: Option<bool>,
    pub save_to: Option<String>,
    pub speed: Option<i64>,
    pub status: Option<String>,
    pub hosts: Option<Vec<String>>,
    pub available_online_count: Option<i64>,
    pub available_offline_count: Option<i64>,
    #[serde(skip)]
    pub links: Vec<Link>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub uuid: i64,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "packageUUID")]
    pub package_uuid: i64,
    pub added_date: Option<i64>,
    pub availability: Option<String>,
    pub bytes_loaded: Option<i64>,
    pub bytes_total: Option<i64>,
    pub comment: Option<String>,
    pub enabled: Option<bool>,
    pub eta: Option<i64>,
    pub extraction_status: Option<String>,
    pub finished: Option<bool>,
    pub finished_date: Option<i64>,
    pub host: Option<String>,
    pub priority: Option<String>,
    pub running: Option<bool>,
    pub speed: Option<i64>,
    pub status: Option<String>,
    pub url: Option<String>,
}

impl Package {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(false)
    }
    pub fn is_finished(&self) -> bool {
        self.finished.unwrap_or(false)
    }
    pub fn is_running(&self) -> bool {
        self.running.unwrap_or(false)
    }
    pub fn progress(&self) -> f64 {
        if self.is_finished() {
            return 100.0;
        }
        match (self.bytes_loaded, self.bytes_total) {
            (Some(done), Some(total)) if total > 0 => (done as f64 / total as f64 * 100.0).min(100.0),
            _ => 0.0,
        }
    }
}

impl Link {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(false)
    }
    pub fn is_finished(&self) -> bool {
        self.finished.unwrap_or(false)
    }
    pub fn progress(&self) -> f64 {
        if self.is_finished() {
            return 100.0;
        }
        match (self.bytes_loaded, self.bytes_total) {
            (Some(done), Some(total)) if total > 0 => (done as f64 / total as f64 * 100.0).min(100.0),
            _ => 0.0,
        }
    }
}

/// Everything the interface shows, fetched in one round.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub state: String,
    pub downloads: Vec<Package>,
    pub grabber: Vec<Package>,
}

/// What to do with the files of a package being removed, mirroring the
/// three choices the desktop GUI offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveMode {
    /// Take it off the list, leave the files where they are.
    ListOnly,
    /// Move the files to the system recycle bin.
    Recycle,
    /// Delete the files for good.
    DeleteFiles,
}

impl RemoveMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RemoveMode::ListOnly => "REMOVE_LINKS_ONLY",
            RemoveMode::Recycle => "REMOVE_LINKS_AND_RECYCLE_FILES",
            RemoveMode::DeleteFiles => "REMOVE_LINKS_AND_DELETE_FILES",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RemoveMode::ListOnly => "Remove from the list only",
            RemoveMode::Recycle => "Remove and move files to the recycle bin",
            RemoveMode::DeleteFiles => "Remove and delete files from disk",
        }
    }

    /// Whether it touches data on disk.
    pub fn touches_files(self) -> bool {
        self != RemoveMode::ListOnly
    }
}

/// The GUI's "add links" dialog, field by field.
#[derive(Debug, Clone, Default)]
pub struct AddLinks {
    pub links: String,
    pub package_name: String,
    pub destination: String,
    pub extract_password: String,
    pub download_password: String,
    pub priority: String,
    pub autostart: bool,
}

pub struct JdApi {
    myjd: MyJd,
    device_id: String,
}

fn opt(s: &str) -> Value {
    if s.trim().is_empty() { Value::Null } else { Value::String(s.trim().to_string()) }
}

impl JdApi {
    pub fn new(myjd: MyJd, device_id: String) -> Self {
        Self { myjd, device_id }
    }

    pub fn list_devices(&mut self) -> Result<Vec<crate::myjd::Device>> {
        match self.myjd.list_devices() {
            Err(e) if e.is_session_expired() => {
                self.myjd.reconnect()?;
                self.myjd.list_devices()
            }
            other => other,
        }
    }

    /// Point the same session at another JDownloader of the account.
    pub fn set_device(&mut self, device_id: String) {
        self.device_id = device_id;
    }

    /// One device call, with a single transparent reconnect when the session
    /// has expired in the meantime.
    fn call<T: serde::de::DeserializeOwned>(&mut self, path: &str, params: &[Value]) -> Result<T> {
        match self.myjd.device_call(&self.device_id, path, params) {
            Err(e) if e.is_session_expired() => {
                self.myjd.reconnect()?;
                self.myjd.device_call(&self.device_id, path, params)
            }
            other => other,
        }
    }

    fn call_unit(&mut self, path: &str, params: &[Value]) -> Result<()> {
        let _: Value = self.call(path, params)?;
        Ok(())
    }

    // --- reads ---------------------------------------------------------

    pub fn state(&mut self) -> Result<String> {
        self.call("/downloadcontroller/getCurrentState", &[])
    }

    pub fn downloads(&mut self) -> Result<Vec<Package>> {
        let packages: Vec<Package> = self.call(
            "/downloadsV2/queryPackages",
            &[json!({
                "bytesLoaded": true, "bytesTotal": true, "childCount": true,
                "comment": true, "enabled": true, "eta": true, "finished": true,
                "priority": true, "running": true, "saveTo": true, "speed": true,
                "status": true, "maxResults": -1, "startAt": 0,
            })],
        )?;
        let links: Vec<Link> = self.call(
            "/downloadsV2/queryLinks",
            &[json!({
                "addedDate": true, "bytesLoaded": true, "bytesTotal": true,
                "comment": true, "enabled": true, "eta": true, "extractionStatus": true,
                "finished": true, "finishedDate": true, "host": true, "packageUUIDs": [],
                "priority": true, "running": true, "speed": true, "status": true,
                "url": true, "maxResults": -1, "startAt": 0,
            })],
        )?;
        Ok(attach(packages, links))
    }

    pub fn grabber(&mut self) -> Result<Vec<Package>> {
        let packages: Vec<Package> = self.call(
            "/linkgrabberv2/queryPackages",
            &[json!({
                "availableOnlineCount": true, "availableOfflineCount": true,
                "bytesTotal": true, "childCount": true, "comment": true,
                "enabled": true, "hosts": true, "priority": true, "saveTo": true,
                "status": true, "maxResults": -1, "startAt": 0,
            })],
        )?;
        let links: Vec<Link> = self.call(
            "/linkgrabberv2/queryLinks",
            &[json!({
                "availability": true, "bytesTotal": true, "comment": true,
                "enabled": true, "host": true, "packageUUIDs": [], "priority": true,
                "status": true, "url": true, "maxResults": -1, "startAt": 0,
            })],
        )?;
        Ok(attach(packages, links))
    }

    pub fn snapshot(&mut self) -> Result<Snapshot> {
        Ok(Snapshot { state: self.state()?, downloads: self.downloads()?, grabber: self.grabber()? })
    }

    // --- actions -------------------------------------------------------
    //
    // Every action takes link ids and package ids the way the API does, so
    // a mixed selection goes out in one call.

    pub fn start(&mut self) -> Result<()> {
        self.call_unit("/downloadcontroller/start", &[])
    }

    pub fn stop(&mut self) -> Result<()> {
        self.call_unit("/downloadcontroller/stop", &[])
    }

    pub fn set_enabled(&mut self, enable: bool, links: &[i64], packages: &[i64], grabber: bool) -> Result<()> {
        let path = if grabber { "/linkgrabberv2/setEnabled" } else { "/downloadsV2/setEnabled" };
        self.call_unit(path, &[json!(enable), json!(links), json!(packages)])
    }

    pub fn force_download(&mut self, links: &[i64], packages: &[i64]) -> Result<()> {
        self.call_unit("/downloadsV2/forceDownload", &[json!(links), json!(packages)])
    }

    pub fn reset(&mut self, links: &[i64], packages: &[i64]) -> Result<()> {
        self.call_unit("/downloadsV2/resetLinks", &[json!(links), json!(packages)])
    }

    pub fn remove(&mut self, links: &[i64], packages: &[i64], grabber: bool) -> Result<()> {
        let path = if grabber { "/linkgrabberv2/removeLinks" } else { "/downloadsV2/removeLinks" };
        self.call_unit(path, &[json!(links), json!(packages)])
    }

    /// Remove from the download list, deciding what happens to the files
    /// already on disk. `removeLinks` always keeps them, so this goes through
    /// cleanup, which is what the GUI's delete dialog does.
    pub fn remove_with_files(&mut self, links: &[i64], packages: &[i64], mode: RemoveMode) -> Result<()> {
        self.call_unit(
            "/downloadsV2/cleanup",
            &[json!(links), json!(packages), json!("DELETE_ALL"), json!(mode.as_str()), json!("SELECTED")],
        )
    }

    /// Delete the finished entries of the selection, like the GUI's Cleanup.
    pub fn cleanup_finished(&mut self, links: &[i64], packages: &[i64]) -> Result<()> {
        self.call_unit(
            "/downloadsV2/cleanup",
            &[json!(links), json!(packages), json!("DELETE_FINISHED"), json!("REMOVE_LINKS_ONLY"), json!("SELECTED")],
        )
    }

    pub fn move_to_downloads(&mut self, links: &[i64], packages: &[i64]) -> Result<()> {
        self.call_unit("/linkgrabberv2/moveToDownloadlist", &[json!(links), json!(packages)])
    }

    pub fn add_links(&mut self, req: &AddLinks) -> Result<()> {
        let priority = if req.priority.is_empty() { "DEFAULT" } else { req.priority.as_str() };
        self.call_unit(
            "/linkgrabberv2/addLinks",
            &[json!({
                "links": req.links.split_whitespace().collect::<Vec<_>>().join("\n"),
                "packageName": opt(&req.package_name),
                "destinationFolder": opt(&req.destination),
                "extractPassword": opt(&req.extract_password),
                "downloadPassword": opt(&req.download_password),
                "priority": priority,
                "autostart": req.autostart,
                "overwritePackagizerRules": false,
            })],
        )
    }
}

/// Hang every link under its package, the way the GUI tree does.
fn attach(mut packages: Vec<Package>, links: Vec<Link>) -> Vec<Package> {
    let mut by_package: std::collections::HashMap<i64, Vec<Link>> = std::collections::HashMap::new();
    for link in links {
        by_package.entry(link.package_uuid).or_default().push(link);
    }
    for pkg in &mut packages {
        pkg.links = by_package.remove(&pkg.uuid).unwrap_or_default();
    }
    packages
}

pub fn describe_error(e: &Error) -> String {
    match e {
        Error::Api { kind, .. } if kind == "OFFLINE" => "device is offline".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod live {
    //! Exercises the real service against a throwaway package it creates and
    //! removes itself. Run with `cargo test -- --ignored --nocapture`.
    use super::*;
    use crate::myjd::MyJd;

    /// The device named by `JDTUI_TEST_DEVICE`, or the one in the config, or
    /// the first on the account.
    fn api() -> JdApi {
        let cfg = crate::config::Config::load().expect("config");
        let (email, password) = (cfg.email.expect("email"), cfg.password.expect("password"));
        let mut myjd = MyJd::new(&email, &password);
        myjd.connect().expect("connect");
        let devices = myjd.list_devices().expect("devices");
        assert!(!devices.is_empty(), "no JDownloader on this account");
        let wanted = std::env::var("JDTUI_TEST_DEVICE").ok();
        let device = wanted
            .and_then(|name| devices.iter().find(|d| d.name == name).cloned())
            .or_else(|| cfg.device.and_then(|id| devices.iter().find(|d| d.id == id).cloned()))
            .unwrap_or_else(|| devices[0].clone());
        println!("using device: {}", device.name);
        JdApi::new(myjd, device.id)
    }

    fn wait_for<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
        for _ in 0..30 {
            if let Some(v) = f() {
                return v;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        panic!("timed out waiting for {what}");
    }

    #[test]
    #[ignore]
    fn remove_with_delete_files_is_accepted() {
        const NAME: &str = "jdtui-remove-test";
        let mut api = api();

        api.add_links(&AddLinks {
            links: "http://example.com/jdtui-remove-test.bin".into(),
            package_name: NAME.into(),
            ..Default::default()
        })
        .expect("add links");

        let uuid = wait_for("the package to appear in the grabber", || {
            api.grabber().ok()?.into_iter().find(|p| p.name == NAME).map(|p| p.uuid)
        });
        println!("created in the link grabber: {NAME} ({uuid})");

        api.move_to_downloads(&[], &[uuid]).expect("move to downloads");
        let uuid = wait_for("the package to reach the download list", || {
            api.downloads().ok()?.into_iter().find(|p| p.name == NAME).map(|p| p.uuid)
        });
        println!("moved to the download list: {uuid}");

        api.remove_with_files(&[], &[uuid], RemoveMode::DeleteFiles).expect("remove with delete files");
        wait_for("the package to disappear", || api.downloads().ok()?.iter().all(|p| p.name != NAME).then_some(()));
        println!("removed with REMOVE_LINKS_AND_DELETE_FILES, gone from the list");

        // Nothing of ours must be left behind in either list.
        assert!(api.grabber().unwrap().iter().all(|p| p.name != NAME));
    }
}
