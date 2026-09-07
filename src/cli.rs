//! What jdtui does when it is not drawing: one question, one answer, and
//! it exits.
//!
//! This is the half of jdtui that belongs in a script. Every command
//! prints a line a person can read, or the same thing as JSON on stdout
//! with `--json`, so `jq` can take it from there. Nothing is asked
//! interactively: the account has to be in the config file already, which
//! it is after the first run of the interface.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use serde::Serialize;

use crate::api::{AddLinks, JdApi, Package, Snapshot};
use crate::config::Config;
use crate::myjd::{Device, MyJd};
use crate::watch;

#[derive(Subcommand, Debug)]
pub enum Command {
    /// What the download controller is doing.
    Status,
    /// The download list, packages and their links.
    Downloads,
    /// The Link Grabber list, packages and their links.
    Grabber,
    /// The JDownloaders connected to the account.
    Devices,
    /// Start the downloads.
    Start,
    /// Stop the downloads.
    Stop,
    /// Pause the downloads.
    Pause,
    /// Resume paused downloads.
    Resume,
    /// Add links to the Link Grabber.
    Add {
        /// The urls. Reads them from stdin, one per line, when given none.
        urls: Vec<String>,
        /// Name of the package they go into.
        #[arg(long)]
        package: Option<String>,
        /// Where the files should be written.
        #[arg(long)]
        folder: Option<String>,
        /// Password to try when unpacking.
        #[arg(long)]
        extract_password: Option<String>,
        /// Password the hoster asks for.
        #[arg(long)]
        download_password: Option<String>,
        /// Start downloading straight away.
        #[arg(long)]
        autostart: bool,
    },
    /// Watch a folder on this machine and send what is dropped into it:
    /// `.crawljob` files and `.dlc`, `.ccf`, `.rsdf` containers. Each one
    /// is moved to `processed` beside it once JDownloader has taken it,
    /// or to `failed` if it would not.
    Watch {
        /// The folder to watch. Falls back to `watch_folder` in the
        /// config file when left out.
        folder: Option<PathBuf>,
        /// Seconds between one look and the next.
        #[arg(long, default_value_t = 5)]
        interval: u64,
        /// Send what is there now and exit, for cron and the like.
        #[arg(long)]
        once: bool,
    },
}

/// The summary `status` prints. Flat and boring on purpose: it is meant to
/// be read by `jq` and by people in equal measure.
#[derive(Serialize)]
struct Status {
    /// Which JDownloader answered, since an account can have several.
    device: String,
    device_id: String,
    /// As JDownloader names it: IDLE, RUNNING, PAUSE, STOPPING,
    /// STOPPED_STATE.
    state: String,
    /// Whether anything is being downloaded, paused included.
    running: bool,
    paused: bool,
    /// Bytes per second.
    speed: i64,
    packages: usize,
    packages_running: usize,
    packages_finished: usize,
    bytes_loaded: i64,
    bytes_total: i64,
    /// Archives being unpacked or waiting to be.
    extracting: usize,
    /// Captchas nothing will download past until someone solves them.
    captchas: usize,
    grabber_packages: usize,
    /// The Link Grabber is still working out what was added to it.
    collecting: bool,
    /// The address calls went to, or null through the My.JDownloader relay.
    direct: Option<String>,
}

impl Status {
    fn of(device: &Device, snapshot: &Snapshot, direct: Option<String>) -> Self {
        Status {
            device: device.name.clone(),
            device_id: device.id.clone(),
            state: snapshot.state.clone(),
            running: snapshot.is_running(),
            paused: snapshot.is_paused(),
            speed: snapshot.speed,
            packages: snapshot.downloads.len(),
            packages_running: snapshot.downloads.iter().filter(|p| p.is_running()).count(),
            packages_finished: snapshot.downloads.iter().filter(|p| p.is_finished()).count(),
            bytes_loaded: snapshot.downloads.iter().filter_map(|p| p.bytes_loaded).sum(),
            bytes_total: snapshot.downloads.iter().filter_map(|p| p.bytes_total).sum(),
            extracting: snapshot.extracting.len(),
            captchas: snapshot.captchas.len(),
            grabber_packages: snapshot.grabber.len(),
            collecting: snapshot.collecting,
            direct,
        }
    }

    fn print(&self) {
        println!("Device     {}", self.device);
        println!("State      {}", self.state);
        println!("Speed      {}/s", crate::ui::human_size(self.speed));
        println!("Packages   {} ({} running, {} done)", self.packages, self.packages_running, self.packages_finished);
        println!(
            "Loaded     {} of {}",
            crate::ui::human_size(self.bytes_loaded),
            crate::ui::human_size(self.bytes_total)
        );
        if self.grabber_packages > 0 || self.collecting {
            let crawling = if self.collecting { ", still crawling" } else { "" };
            println!("Grabber    {} package(s){crawling}", self.grabber_packages);
        }
        if self.extracting > 0 {
            println!("Extracting {}", self.extracting);
        }
        if self.captchas > 0 {
            println!("Captchas   {} waiting", self.captchas);
        }
        match &self.direct {
            Some(address) => println!("Calls go   direct to {address}"),
            None => println!("Calls go   through the My.JDownloader relay"),
        }
    }
}

/// A JDownloader of the account, for `devices`.
#[derive(Serialize)]
struct DeviceOut {
    name: String,
    id: String,
    #[serde(rename = "type")]
    kind: String,
    /// The one jdtui would talk to without being told otherwise.
    current: bool,
}

pub fn run(command: Command, config: &Config, json: bool, device: Option<&str>) -> Result<()> {
    // The device list is a server call, so it works before picking one.
    if let Command::Devices = command {
        let mut jd = sign_in(config)?;
        let devices = jd.list_devices().context("listing the JDownloaders of the account")?;
        let out: Vec<DeviceOut> = devices
            .into_iter()
            .map(|d| DeviceOut { current: Some(&d.id) == config.device.as_ref(), name: d.name, id: d.id, kind: d.kind })
            .collect();
        if json {
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else if out.is_empty() {
            println!("No JDownloader is connected to this account");
        } else {
            for d in &out {
                println!("{} {:<24} {}", if d.current { "*" } else { " " }, d.name, d.id);
            }
        }
        return Ok(());
    }

    let (mut api, device) = connect(config, device)?;
    match command {
        Command::Devices => unreachable!("handled above"),
        Command::Status => {
            let status = api.status().context("reading what JDownloader is busy with")?;
            let snapshot = api.snapshot(status).context("reading the lists")?;
            let status = Status::of(&device, &snapshot, api.direct().map(str::to_string));
            if json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                status.print();
            }
        }
        Command::Downloads => {
            let packages = api.downloads().context("reading the download list")?;
            print_packages(&packages, json)?;
        }
        Command::Grabber => {
            let packages = api.grabber().context("reading the Link Grabber")?;
            print_packages(&packages, json)?;
        }
        Command::Start => {
            api.start().context("starting the downloads")?;
            done(json, "started");
        }
        Command::Stop => {
            api.stop().context("stopping the downloads")?;
            done(json, "stopped");
        }
        Command::Pause => {
            api.pause(true).context("pausing the downloads")?;
            done(json, "paused");
        }
        Command::Resume => {
            api.pause(false).context("resuming the downloads")?;
            done(json, "resumed");
        }
        Command::Watch { folder, interval, once } => {
            let folder = match folder.or_else(|| config.watch_folder()) {
                Some(f) => f,
                None if config.watch_folder.is_some() => bail!(
                    "watching is switched off in {} (watch = false); name a folder to watch it anyway",
                    Config::path().display()
                ),
                None => bail!("no folder given and no watch_folder in {}", Config::path().display()),
            };
            if once {
                // Two looks with a pause between them: a file is only
                // taken once it has stopped growing, and one look cannot
                // tell that.
                let mut seen = HashMap::new();
                let mut sent = watch::report(&watch::sweep(&mut api, &folder, &mut seen)?);
                std::thread::sleep(watch::SETTLE);
                sent += watch::report(&watch::sweep(&mut api, &folder, &mut seen)?);
                if json {
                    println!("{}", serde_json::json!({ "sent": sent }));
                } else {
                    println!("{sent} file(s) sent");
                }
            } else {
                watch::watch(&mut api, &folder, Duration::from_secs(interval.max(1)))?;
            }
        }
        Command::Add { urls, package, folder, extract_password, download_password, autostart } => {
            let links = if urls.is_empty() { read_stdin()? } else { urls.join("\n") };
            if links.trim().is_empty() {
                bail!("no urls given, on the command line or on stdin");
            }
            let count = links.lines().filter(|l| !l.trim().is_empty()).count();
            api.add_links(&AddLinks {
                links,
                package_name: package.unwrap_or_default(),
                destination: folder.unwrap_or_default(),
                extract_password: extract_password.unwrap_or_default(),
                download_password: download_password.unwrap_or_default(),
                priority: String::new(),
                autostart,
            })
            .context("adding the links")?;
            if json {
                println!("{}", serde_json::json!({ "added": count }));
            } else {
                println!("Added {count} url(s) to the Link Grabber");
            }
        }
    }
    Ok(())
}

fn done(json: bool, what: &str) {
    if json {
        println!("{}", serde_json::json!({ "ok": true, "action": what }));
    } else {
        println!("Downloads {what}");
    }
}

fn print_packages(packages: &[Package], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(packages)?);
        return Ok(());
    }
    for p in packages {
        println!(
            "{:<52} {:>10} {:>5.0}%  {}",
            p.name.chars().take(52).collect::<String>(),
            crate::ui::human_size(p.bytes_total.unwrap_or(0)),
            p.progress(),
            p.status.clone().unwrap_or_default()
        );
    }
    Ok(())
}

fn read_stdin() -> Result<String> {
    use std::io::Read;
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).context("reading urls from stdin")?;
    Ok(text)
}

/// Open a session with the account in the config file. Nothing is asked
/// here: a script has nobody to ask.
fn sign_in(config: &Config) -> Result<MyJd> {
    let (Some(email), Some(password)) = (&config.email, &config.password) else {
        bail!(
            "no My.JDownloader account in {}. Run jdtui once without a command and sign in.",
            Config::path().display()
        );
    };
    let mut jd = MyJd::new(email, password);
    jd.connect().context("signing in to My.JDownloader")?;
    Ok(jd)
}

/// Sign in and pick the JDownloader to talk to: the one named on the
/// command line, else the one saved in the config, else the only one there
/// is.
fn connect(config: &Config, wanted: Option<&str>) -> Result<(JdApi, Device)> {
    let mut jd = sign_in(config)?;
    let devices = jd.list_devices().context("listing the JDownloaders of the account")?;
    if devices.is_empty() {
        bail!("no JDownloader is connected to this account");
    }
    let chosen = match wanted {
        Some(name) => devices
            .iter()
            .find(|d| d.name == name || d.id == name)
            .with_context(|| {
                let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
                format!("no JDownloader called {name}; the account has {}", names.join(", "))
            })?
            .clone(),
        None => match config.device.as_ref().and_then(|id| devices.iter().find(|d| &d.id == id)) {
            Some(d) => d.clone(),
            None if devices.len() == 1 => devices[0].clone(),
            None => {
                let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
                bail!("no JDownloader picked yet; pass --device with one of {}", names.join(", "));
            }
        },
    };
    let direct = config.direct_for(&chosen.name);
    let mut api = JdApi::new(jd, chosen.id.clone());
    api.set_direct_config(direct.is_some(), direct.unwrap_or_default());
    api.ensure_direct();
    Ok((api, chosen))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ArchiveStatus, Link};

    fn package(name: &str, loaded: i64, total: i64, finished: bool, running: bool) -> Package {
        Package {
            uuid: 1,
            name: name.into(),
            bytes_loaded: Some(loaded),
            bytes_total: Some(total),
            finished: Some(finished),
            running: Some(running),
            links: vec![Link::default()],
            ..Default::default()
        }
    }

    #[test]
    fn status_counts_what_a_script_would_branch_on() {
        let snapshot = Snapshot {
            state: "RUNNING".into(),
            speed: 1024,
            downloads: vec![package("done", 100, 100, true, false), package("busy", 50, 200, false, true)],
            grabber: vec![package("waiting", 0, 10, false, false)],
            extracting: vec![ArchiveStatus::default()],
            ..Default::default()
        };
        let device = Device { id: "abc".into(), name: "jd2@test".into(), kind: "jd".into() };
        let status = Status::of(&device, &snapshot, Some("http://192.168.1.30:3129".into()));
        assert!(status.running);
        assert!(!status.paused);
        assert_eq!(status.packages, 2);
        assert_eq!(status.packages_running, 1);
        assert_eq!(status.packages_finished, 1);
        assert_eq!(status.bytes_loaded, 150);
        assert_eq!(status.bytes_total, 300);
        assert_eq!(status.extracting, 1);
        assert_eq!(status.grabber_packages, 1);

        // The shape scripts read must not drift.
        let json = serde_json::to_value(&status).expect("json");
        assert_eq!(json["device"], "jd2@test");
        assert_eq!(json["state"], "RUNNING");
        assert_eq!(json["running"], true);
        assert_eq!(json["direct"], "http://192.168.1.30:3129");
    }

    #[test]
    fn a_paused_controller_still_counts_as_running() {
        let snapshot = Snapshot { state: "PAUSE".into(), ..Default::default() };
        let device = Device { id: "abc".into(), name: "jd2@test".into(), kind: "jd".into() };
        let status = Status::of(&device, &snapshot, None);
        assert!(status.running, "JDownloader keeps the queue while paused");
        assert!(status.paused);
        assert!(status.direct.is_none());
    }
}
