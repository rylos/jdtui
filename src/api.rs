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
    /// The link offers several variants (video qualities, audio only…).
    pub variants: Option<bool>,
    /// The variant currently chosen, when the link has any.
    pub variant: Option<LinkVariant>,
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

/// What changes rarely, refreshed less often than the lists.
#[derive(Debug, Clone, Default)]
pub struct Status {
    pub stop_mark: Option<i64>,
    pub collecting: bool,
    pub extracting: Vec<ArchiveStatus>,
    pub captchas: Vec<CaptchaJob>,
}

/// Everything the interface shows, fetched in one round.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub state: String,
    /// Bytes per second, as the controller reports it: includes traffic
    /// the per-link figures miss (rar extraction aside).
    pub speed: i64,
    /// Uuid of the link or package downloads stop after, if set.
    pub stop_mark: Option<i64>,
    /// The Link Grabber is still crawling what was added.
    pub collecting: bool,
    /// Archives being extracted or queued for it.
    pub extracting: Vec<ArchiveStatus>,
    /// Captchas JDownloader is waiting on; nothing downloads from those
    /// hosters until someone solves them.
    pub captchas: Vec<CaptchaJob>,
    pub downloads: Vec<Package>,
    pub grabber: Vec<Package>,
}

impl Snapshot {
    /// The download controller is active, paused or not. States are those
    /// of JDownloader's DownloadWatchDog: IDLE, RUNNING, PAUSE, STOPPING,
    /// STOPPED_STATE.
    pub fn is_running(&self) -> bool {
        matches!(self.state.as_str(), "RUNNING" | "PAUSE")
    }

    pub fn is_paused(&self) -> bool {
        self.state == "PAUSE"
    }
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

/// A mount point on the JDownloader machine.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StorageInfo {
    pub path: Option<String>,
    pub free: Option<i64>,
    pub size: Option<i64>,
}

/// One notification from the JDownloader event channel.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Event {
    pub eventid: String,
    pub publisher: String,
    #[serde(rename = "eventData")]
    pub event_data: Option<Value>,
}

/// One of the forms a link can be downloaded in, such as a video quality.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LinkVariant {
    pub id: Option<String>,
    pub name: Option<String>,
}

/// One archive in the extraction queue.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveStatus {
    pub archive_id: Option<String>,
    pub archive_name: Option<String>,
    pub controller_id: Option<i64>,
    /// `RUNNING` or `QUEUED`.
    pub controller_status: Option<String>,
}

/// A captcha JDownloader is waiting on.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptchaJob {
    pub id: i64,
    pub hoster: Option<String>,
    pub link: Option<i64>,
    pub created: Option<i64>,
    pub timeout: Option<i64>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub explain: Option<String>,
}

/// A premium (or free) account JDownloader knows about.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub uuid: i64,
    pub hostname: Option<String>,
    pub username: Option<String>,
    pub enabled: Option<bool>,
    pub valid: Option<bool>,
    pub valid_until: Option<i64>,
    pub traffic_left: Option<i64>,
    pub traffic_max: Option<i64>,
    pub error_type: Option<String>,
    pub error_string: Option<String>,
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

    /// Like `call`, for calls that block on the device side.
    fn call_long<T: serde::de::DeserializeOwned>(
        &mut self,
        path: &str,
        params: &[Value],
        timeout: std::time::Duration,
    ) -> Result<T> {
        match self.myjd.device_call_with_timeout(&self.device_id, path, params, Some(timeout)) {
            Err(e) if e.is_session_expired() => {
                self.myjd.reconnect()?;
                self.myjd.device_call_with_timeout(&self.device_id, path, params, Some(timeout))
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

    pub fn speed(&mut self) -> Result<i64> {
        self.call("/downloadcontroller/getSpeedInBps", &[])
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
                "status": true, "url": true, "variants": true, "variantID": true,
                "variantName": true, "maxResults": -1, "startAt": 0,
            })],
        )?;
        Ok(attach(packages, links))
    }

    /// The slow-changing part of a snapshot: four round trips that need
    /// not run on every refresh.
    pub fn status(&mut self) -> Result<Status> {
        Ok(Status {
            stop_mark: self.stop_mark()?,
            collecting: self.is_collecting()?,
            extracting: self.extraction_queue()?,
            captchas: self.captchas()?,
        })
    }

    /// The lists and the controller state, four round trips, around a
    /// `status` fetched earlier.
    pub fn snapshot(&mut self, status: Status) -> Result<Snapshot> {
        Ok(Snapshot {
            state: self.state()?,
            speed: self.speed()?,
            stop_mark: status.stop_mark,
            collecting: status.collecting,
            extracting: status.extracting,
            captchas: status.captchas,
            downloads: self.downloads()?,
            grabber: self.grabber()?,
        })
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

    /// Pause (`true`) or resume (`false`). JDownloader only pauses from
    /// RUNNING; anywhere else the call is silently ignored.
    pub fn pause(&mut self, value: bool) -> Result<()> {
        self.call_unit("/downloadcontroller/pause", &[json!(value)])
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

    pub fn set_priority(&mut self, priority: &str, links: &[i64], packages: &[i64], grabber: bool) -> Result<()> {
        let path = if grabber { "/linkgrabberv2/setPriority" } else { "/downloadsV2/setPriority" };
        self.call_unit(path, &[json!(priority), json!(links), json!(packages)])
    }

    pub fn rename_package(&mut self, package: i64, name: &str, grabber: bool) -> Result<()> {
        let path = if grabber { "/linkgrabberv2/renamePackage" } else { "/downloadsV2/renamePackage" };
        self.call_unit(path, &[json!(package), json!(name)])
    }

    pub fn rename_link(&mut self, link: i64, name: &str, grabber: bool) -> Result<()> {
        let path = if grabber { "/linkgrabberv2/renameLink" } else { "/downloadsV2/renameLink" };
        self.call_unit(path, &[json!(link), json!(name)])
    }

    /// Packages only: the API has no per-link download folder.
    pub fn set_download_directory(&mut self, directory: &str, packages: &[i64], grabber: bool) -> Result<()> {
        let path = if grabber { "/linkgrabberv2/setDownloadDirectory" } else { "/downloadsV2/setDownloadDirectory" };
        self.call_unit(path, &[json!(directory), json!(packages)])
    }

    /// Continue interrupted links from where they stopped, unlike `reset`.
    pub fn resume(&mut self, links: &[i64], packages: &[i64]) -> Result<()> {
        self.call_unit("/downloadsV2/resumeLinks", &[json!(links), json!(packages)])
    }

    /// Uuid of the link or package downloads stop after, `None` when unset.
    pub fn stop_mark(&mut self) -> Result<Option<i64>> {
        let id: i64 = self.call("/downloadsV2/getStopMark", &[])?;
        Ok((id > 0).then_some(id))
    }

    /// The API marks links only: given a package id it marks the first
    /// link of it, so callers pick the link themselves.
    pub fn set_stop_mark(&mut self, link: i64) -> Result<()> {
        self.call_unit("/downloadsV2/setStopMark", &[json!(link), json!(-1)])
    }

    pub fn remove_stop_mark(&mut self) -> Result<()> {
        self.call_unit("/downloadsV2/removeStopMark", &[])
    }

    /// Move the selection into a new package. Empty `directory` keeps the
    /// current folder.
    pub fn move_to_new_package(
        &mut self,
        links: &[i64],
        packages: &[i64],
        name: &str,
        directory: &str,
        grabber: bool,
    ) -> Result<()> {
        let path = if grabber { "/linkgrabberv2/movetoNewPackage" } else { "/downloadsV2/movetoNewPackage" };
        self.call_unit(path, &[json!(links), json!(packages), json!(name), opt(directory)])
    }

    pub fn split_by_hoster(&mut self, links: &[i64], packages: &[i64], grabber: bool) -> Result<()> {
        let path = if grabber { "/linkgrabberv2/splitPackageByHoster" } else { "/downloadsV2/splitPackageByHoster" };
        self.call_unit(path, &[json!(links), json!(packages)])
    }

    /// The content urls of the selection, one per distinct url.
    pub fn download_urls(&mut self, links: &[i64], packages: &[i64], grabber: bool) -> Result<Vec<String>> {
        let path = if grabber { "/linkgrabberv2/getDownloadUrls" } else { "/downloadsV2/getDownloadUrls" };
        let map: std::collections::BTreeMap<String, Vec<i64>> =
            self.call(path, &[json!(links), json!(packages), json!(["CONTENT"])])?;
        Ok(map.into_keys().collect())
    }

    /// Clear the skip reason of the selection, whatever it was.
    pub fn unskip(&mut self, links: &[i64], packages: &[i64]) -> Result<()> {
        // Package ids first: this one takes them the other way round.
        self.call_unit("/downloadsV2/unskip", &[json!(packages), json!(links), Value::Null])
    }

    pub fn check_online_status(&mut self, links: &[i64], packages: &[i64], grabber: bool) -> Result<()> {
        let path =
            if grabber { "/linkgrabberv2/startOnlineStatusCheck" } else { "/downloadsV2/startOnlineStatusCheck" };
        self.call_unit(path, &[json!(links), json!(packages)])
    }

    // --- extraction, captchas, accounts ---------------------------------

    pub fn extraction_queue(&mut self) -> Result<Vec<ArchiveStatus>> {
        self.call("/extraction/getQueue", &[])
    }

    /// Queue the complete archives of the selection for extraction now.
    pub fn extract_now(&mut self, links: &[i64], packages: &[i64]) -> Result<()> {
        self.call_unit("/extraction/startExtractionNow", &[json!(links), json!(packages)])
    }

    pub fn add_archive_password(&mut self, password: &str) -> Result<()> {
        self.call_unit("/extraction/addArchivePassword", &[json!(password)])
    }

    pub fn captchas(&mut self) -> Result<Vec<CaptchaJob>> {
        self.call("/captcha/list", &[])
    }

    /// Give up on one captcha; the link it blocks is skipped.
    pub fn skip_captcha(&mut self, id: i64) -> Result<()> {
        self.call_unit("/captcha/skip", &[json!(id), json!("SINGLE")])
    }

    pub fn accounts(&mut self) -> Result<Vec<Account>> {
        self.call(
            "/accountsV2/listAccounts",
            &[json!({
                "enabled": true, "error": true, "trafficLeft": true, "trafficMax": true,
                "userName": true, "valid": true, "validUntil": true, "maxResults": -1, "startAt": 0,
            })],
        )
    }

    pub fn set_accounts_enabled(&mut self, enable: bool, ids: &[i64]) -> Result<()> {
        let path = if enable { "/accountsV2/enableAccounts" } else { "/accountsV2/disableAccounts" };
        self.call_unit(path, &[json!(ids)])
    }

    pub fn refresh_accounts(&mut self, ids: &[i64]) -> Result<()> {
        self.call_unit("/accountsV2/refreshAccounts", &[json!(ids)])
    }

    // --- the JDownloader itself -----------------------------------------

    pub fn update_available(&mut self) -> Result<bool> {
        self.call("/update/isUpdateAvailable", &[])
    }

    pub fn run_update_check(&mut self) -> Result<()> {
        self.call_unit("/update/runUpdateCheck", &[])
    }

    /// Restart into the updater, which applies what was downloaded.
    pub fn update_and_restart(&mut self) -> Result<()> {
        self.call_unit("/update/restartAndUpdate", &[])
    }

    pub fn restart_jd(&mut self) -> Result<()> {
        self.call_unit("/system/restartJD", &[])
    }

    pub fn exit_jd(&mut self) -> Result<()> {
        self.call_unit("/system/exitJD", &[])
    }

    /// Ask the router for a new IP, as configured in JDownloader.
    pub fn reconnect(&mut self) -> Result<()> {
        self.call_unit("/reconnect/doReconnect", &[])
    }

    /// The variants a grabber link offers; empty when it has none.
    pub fn variants(&mut self, link: i64) -> Result<Vec<LinkVariant>> {
        self.call("/linkgrabberv2/getVariants", &[json!(link)])
    }

    pub fn set_variant(&mut self, link: i64, variant_id: &str) -> Result<()> {
        self.call_unit("/linkgrabberv2/setVariant", &[json!(link), json!(variant_id)])
    }

    // --- events -----------------------------------------------------------
    //
    // Everything that changes what the interface shows, minus the per-second
    // progress noise, which the periodic refresh covers while downloading.
    // Patterns are Java regexes searched in "publisher.eventid".

    pub const EVENT_SUBSCRIPTIONS: &'static [&'static str] = &[
        "^downloadwatchdog",
        "^downloads\\.(REFRESH_STRUCTURE|REMOVE_|ADD_|REFRESH_CONTENT)",
        "^downloads\\.(LINK|PACKAGE)_UPDATE\\.(enabled|finished|priority|saveTo|skipped|extractionStatus)$",
        "^linkcollector",
        "^linkcrawler",
        "^captchas",
        "^extraction",
    ];

    /// Poll timeout JDownloader applies to `listen`; its default.
    pub const EVENT_POLL: std::time::Duration = std::time::Duration::from_secs(25);

    /// Open an event subscription; returns its id.
    pub fn subscribe_events(&mut self) -> Result<i64> {
        #[derive(Deserialize)]
        struct Sub {
            subscriptionid: i64,
        }
        let sub: Sub = self.call("/events/subscribe", &[json!(Self::EVENT_SUBSCRIPTIONS), json!([])])?;
        Ok(sub.subscriptionid)
    }

    /// Block until events arrive or the poll timeout passes (then empty).
    pub fn listen_events(&mut self, subscription: i64) -> Result<Vec<Event>> {
        self.call_long("/events/listen", &[json!(subscription)], Self::EVENT_POLL + std::time::Duration::from_secs(15))
    }

    pub fn unsubscribe_events(&mut self, subscription: i64) -> Result<()> {
        self.call_unit("/events/unsubscribe", &[json!(subscription)])
    }

    /// The download folders used lately, as the GUI's folder combo lists
    /// them, placeholders such as `<jd:packagename>` included.
    pub fn folder_history(&mut self) -> Result<Vec<String>> {
        self.call("/linkgrabberv2/getDownloadFolderHistorySelectionBase", &[])
    }

    /// The mount points JDownloader sees, with their free space.
    pub fn storage_roots(&mut self) -> Result<Vec<StorageInfo>> {
        self.call("/system/getStorageInfos", &[Value::Null])
    }

    pub fn is_collecting(&mut self) -> Result<bool> {
        self.call("/linkgrabberv2/isCollecting", &[])
    }

    /// Stop every running crawl job.
    pub fn abort_collecting(&mut self) -> Result<()> {
        self.call_unit("/linkgrabberv2/abort", &[])
    }

    pub fn clear_grabber(&mut self) -> Result<()> {
        self.call_unit("/linkgrabberv2/clearList", &[])
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

    fn wait_for<T>(what: &str, f: impl FnMut() -> Option<T>) -> T {
        wait_up_to(what, 30, f)
    }

    /// Two minutes, for crawls that go out to the hoster.
    fn wait_for_long<T>(what: &str, f: impl FnMut() -> Option<T>) -> T {
        wait_up_to(what, 240, f)
    }

    fn wait_up_to<T>(what: &str, tries: usize, mut f: impl FnMut() -> Option<T>) -> T {
        for _ in 0..tries {
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

        // The stop mark, while there is a link of ours to put it on.
        let link = wait_for("the link to be listed", || {
            api.downloads().ok()?.into_iter().find(|p| p.uuid == uuid)?.links.first().map(|l| l.uuid)
        });
        api.set_stop_mark(link).expect("set stop mark");
        wait_for("the stop mark to read back", || (api.stop_mark().ok()? == Some(link)).then_some(()));
        api.remove_stop_mark().expect("remove stop mark");
        wait_for("the stop mark to clear", || api.stop_mark().ok()?.is_none().then_some(()));
        println!("stop mark set and cleared");

        api.remove_with_files(&[], &[uuid], RemoveMode::DeleteFiles).expect("remove with delete files");
        wait_for("the package to disappear", || api.downloads().ok()?.iter().all(|p| p.name != NAME).then_some(()));
        println!("removed with REMOVE_LINKS_AND_DELETE_FILES, gone from the list");

        // Nothing of ours must be left behind in either list.
        assert!(api.grabber().unwrap().iter().all(|p| p.name != NAME));
    }

    /// Pause and resume, leaving the controller as it was found. From
    /// anything but RUNNING the device ignores the pause, so the test only
    /// asserts a transition when there was something to pause.
    #[test]
    #[ignore]
    fn pause_is_accepted_and_reversible() {
        let mut api = api();
        let before = api.state().expect("state");
        println!("state before: {before}");

        api.pause(true).expect("pause");
        let paused = wait_for("the state to settle", || {
            let s = api.state().ok()?;
            (before != "RUNNING" || s == "PAUSE").then_some(s)
        });
        println!("state after pause: {paused}");

        api.pause(false).expect("resume");
        let after = wait_for("the state to come back", || {
            let s = api.state().ok()?;
            (s != "PAUSE").then_some(s)
        });
        println!("state after resume: {after}");
        assert_eq!(after, before);
    }

    /// The update check is the only device-level call safe to run here.
    #[test]
    #[ignore]
    fn update_check_answers() {
        let mut api = api();
        api.run_update_check().expect("run update check");
        let available = api.update_available().expect("update available");
        println!("update available: {available}");
    }

    /// Priority, rename and download folder on a throwaway grabber package,
    /// checked by reading them back; then removed.
    #[test]
    #[ignore]
    fn edits_are_applied_to_a_grabber_package() {
        const NAME: &str = "jdtui-edit-test";
        let mut api = api();

        api.add_links(&AddLinks {
            links: "http://example.com/jdtui-edit-test.bin".into(),
            package_name: NAME.into(),
            ..Default::default()
        })
        .expect("add links");
        let pkg = wait_for("the package to appear in the grabber", || {
            api.grabber().ok()?.into_iter().find(|p| p.name == NAME)
        });
        println!("created in the link grabber: {NAME} ({})", pkg.uuid);

        api.set_priority("HIGH", &[], &[pkg.uuid], true).expect("set priority");
        wait_for("the priority to read back", || {
            let p = api.grabber().ok()?.into_iter().find(|p| p.uuid == pkg.uuid)?;
            (p.priority.as_deref() == Some("HIGH")).then_some(())
        });
        println!("priority HIGH read back");

        let renamed = format!("{NAME}-renamed");
        api.rename_package(pkg.uuid, &renamed, true).expect("rename package");
        wait_for("the new name to read back", || {
            api.grabber().ok()?.into_iter().find(|p| p.uuid == pkg.uuid && p.name == renamed).map(|_| ())
        });
        println!("renamed to {renamed}");

        let link = pkg.links.first().expect("a link").uuid;
        api.rename_link(link, "jdtui-edit-test-renamed.bin", true).expect("rename link");
        wait_for("the link name to read back", || {
            let p = api.grabber().ok()?.into_iter().find(|p| p.uuid == pkg.uuid)?;
            p.links.iter().any(|l| l.uuid == link && l.name == "jdtui-edit-test-renamed.bin").then_some(())
        });
        println!("link renamed");

        let dir = format!("{}/jdtui-edit-test", pkg.save_to.clone().unwrap_or_default().trim_end_matches('/'));
        api.set_download_directory(&dir, &[pkg.uuid], true).expect("set download directory");
        wait_for("the folder to read back", || {
            let p = api.grabber().ok()?.into_iter().find(|p| p.uuid == pkg.uuid)?;
            (p.save_to.as_deref().map(|s| s.trim_end_matches(['/', '\\'])) == Some(dir.as_str())).then_some(())
        });
        println!("download folder set to {dir}");

        api.remove(&[], &[pkg.uuid], true).expect("remove");
        wait_for("the package to disappear", || api.grabber().ok()?.iter().all(|p| p.uuid != pkg.uuid).then_some(()));
    }

    /// A YouTube link crawls into several variants; pick another one and
    /// read it back. Needs the YouTube plugin, which every JDownloader has.
    #[test]
    #[ignore]
    fn variants_can_be_listed_and_chosen() {
        let mut api = api();
        api.add_links(&AddLinks {
            links: "https://www.youtube.com/watch?v=jNQXAC9IVRw".into(),
            package_name: "jdtui-variant-test".into(),
            ..Default::default()
        })
        .expect("add links");
        // The YouTube plugin names the package itself, so look for the
        // host; crawling a video takes a while.
        let (pkg, link) = wait_for_long("a YouTube link with variants to appear", || {
            let pkg = api.grabber().ok()?.into_iter().find(|p| {
                p.links.iter().any(|l| l.host.as_deref() == Some("youtube.com") && l.variants == Some(true))
            })?;
            let link = pkg.links.iter().find(|l| l.variants == Some(true))?.clone();
            Some((pkg, link))
        });
        println!("link {} ({}), current variant {:?}", link.name, link.uuid, link.variant);

        let variants = api.variants(link.uuid).expect("variants");
        println!("{} variants:", variants.len());
        for v in &variants {
            println!("  {:?} {:?}", v.id, v.name);
        }
        assert!(variants.len() > 1);

        let other = variants.iter().find(|v| v.id != link.variant.as_ref().and_then(|c| c.id.clone())).unwrap();
        let wanted = other.id.clone().unwrap();
        api.set_variant(link.uuid, &wanted).expect("set variant");
        wait_for("the variant to read back", || {
            let p = api.grabber().ok()?.into_iter().find(|p| p.uuid == pkg.uuid)?;
            let l = p.links.iter().find(|l| l.uuid == link.uuid)?;
            (l.variant.as_ref().and_then(|v| v.id.clone()) == Some(wanted.clone())).then_some(())
        });
        println!("variant switched to {:?}", other.name);

        api.remove(&[], &[pkg.uuid], true).expect("remove");
        wait_for("the package to disappear", || api.grabber().ok()?.iter().all(|p| p.uuid != pkg.uuid).then_some(()));
    }

    /// The read-only endpoints answer, and the grabber ones that reshape
    /// packages accept a throwaway package.
    #[test]
    #[ignore]
    fn queries_and_package_reshaping_are_accepted() {
        const NAME: &str = "jdtui-reshape-test";
        let mut api = api();

        println!("extraction queue: {:?}", api.extraction_queue().expect("extraction queue"));
        println!("captchas: {:?}", api.captchas().expect("captchas"));
        let accounts = api.accounts().expect("accounts");
        println!("accounts: {}", accounts.len());
        for a in &accounts {
            println!("  {:?} {:?} enabled={:?} valid={:?}", a.hostname, a.username, a.enabled, a.valid);
        }

        api.add_links(&AddLinks {
            links: "http://example.com/jdtui-reshape-a.bin http://example.org/jdtui-reshape-b.bin".into(),
            package_name: NAME.into(),
            ..Default::default()
        })
        .expect("add links");
        let pkg = wait_for("the package to appear with both links", || {
            api.grabber().ok()?.into_iter().find(|p| p.name == NAME && p.links.len() == 2)
        });
        println!("created: {} ({})", pkg.name, pkg.uuid);

        let urls = api.download_urls(&[], &[pkg.uuid], true).expect("download urls");
        println!("urls: {urls:?}");
        assert_eq!(urls.len(), 2);

        api.check_online_status(&[], &[pkg.uuid], true).expect("online status check");

        let first = pkg.links[0].uuid;
        api.move_to_new_package(&[first], &[], "jdtui-reshape-new", "", true).expect("move to new package");
        wait_for("the new package to appear", || {
            api.grabber().ok()?.into_iter().find(|p| p.name == "jdtui-reshape-new").map(|_| ())
        });
        println!("moved one link to a new package");

        api.split_by_hoster(&[], &[pkg.uuid], true).expect("split by hoster");

        // Everything of ours goes, whatever shape it ended up in.
        let ours: Vec<i64> =
            api.grabber().unwrap().iter().filter(|p| p.name.starts_with("jdtui-reshape")).map(|p| p.uuid).collect();
        println!("removing {} package(s)", ours.len());
        api.remove(&[], &ours, true).expect("remove");
        wait_for("our packages to disappear", || {
            api.grabber().ok()?.iter().all(|p| !p.name.starts_with("jdtui-reshape")).then_some(())
        });
    }
}
