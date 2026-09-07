//! Watching a folder on *this* machine and handing what lands in it to a
//! JDownloader somewhere else.
//!
//! JDownloader has a Folder Watch of its own, and where the folder is on
//! the machine JDownloader runs on, that one is the right tool: it needs
//! no client running at all. What it cannot do is watch a folder here,
//! which is the gap this fills. Anything a script, a browser or a torrent
//! client drops here is sent over the My.JDownloader API and the file is
//! filed away.
//!
//! Two kinds of file are understood, the same two Folder Watch takes:
//!
//! - `.crawljob`, a list of jobs in Java properties or JSON form, turned
//!   into ordinary add-links calls;
//! - `.dlc`, `.ccf`, `.rsdf`, containers handed to JDownloader whole,
//!   which decrypts them itself.
//!
//! The folder is read on whatever this machine is, Linux, Windows or
//! macOS, so the small differences are all handled here: files written
//! with CRLF, a byte order mark in front, or saved as UTF-16 by a Windows
//! editor; the metadata files macOS scatters about; and a file another
//! program still holds open, which Windows will not let us move. Note
//! that a `downloadFolder` inside a job is a path on the machine
//! JDownloader runs on, not on this one.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

use crate::api::{AddLinks, JdApi};

/// Containers JDownloader opens itself; anything else is left alone.
const CONTAINERS: [&str; 3] = ["dlc", "ccf", "rsdf"];
/// A file must have been still for this long before it is read: whoever
/// is writing it may not have finished.
pub const SETTLE: Duration = Duration::from_secs(2);
/// Where a file goes once JDownloader has taken it, and where it goes
/// when JDownloader refuses it.
const DONE: &str = "processed";
const FAILED: &str = "failed";

/// One job read out of a `.crawljob`. Only the keys that map onto what the
/// API can express are kept; the rest of the format is ignored rather than
/// half-honoured.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Job {
    pub links: String,
    pub package_name: String,
    pub destination: String,
    pub extract_password: String,
    pub download_password: String,
    pub priority: String,
    pub autostart: bool,
}

impl From<Job> for AddLinks {
    fn from(job: Job) -> Self {
        AddLinks {
            links: job.links,
            package_name: job.package_name,
            destination: job.destination,
            extract_password: job.extract_password,
            download_password: job.download_password,
            priority: job.priority,
            autostart: job.autostart,
        }
    }
}

fn set(job: &mut Job, key: &str, value: &str) {
    let value = value.trim().trim_matches('"');
    match key.trim().to_ascii_lowercase().as_str() {
        "text" => job.links = value.to_string(),
        "packagename" => job.package_name = value.to_string(),
        "downloadfolder" => job.destination = value.to_string(),
        // A list of candidates; the API takes one, so the first is used.
        "extractpasswords" | "extractpassword" => {
            job.extract_password = value.split(',').next().unwrap_or(value).trim().trim_matches('"').to_string();
        }
        "downloadpassword" => job.download_password = value.to_string(),
        "priority" => job.priority = value.to_ascii_uppercase(),
        "autostart" | "forcedstart" => job.autostart = value.eq_ignore_ascii_case("true"),
        _ => {}
    }
}

/// Read a `.crawljob`, in either of the two shapes Folder Watch accepts:
/// JSON, or key=value lines with `->NEW ENTRY<-` between jobs and `#` for
/// comments. Jobs without a url are dropped.
pub fn parse_crawljob(text: &str) -> Vec<Job> {
    let trimmed = text.trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        return parse_json(trimmed);
    }
    let mut jobs = Vec::new();
    let mut current = Job::default();
    for line in text.lines() {
        let line = line.trim();
        if line.contains("->NEW ENTRY<-") {
            jobs.push(std::mem::take(&mut current));
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            set(&mut current, key, value);
        }
    }
    jobs.push(current);
    jobs.retain(|j| !j.links.trim().is_empty());
    jobs
}

fn parse_json(text: &str) -> Vec<Job> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let entries = match &value {
        serde_json::Value::Array(items) => items.clone(),
        other => vec![other.clone()],
    };
    let mut jobs = Vec::new();
    for entry in entries {
        let Some(object) = entry.as_object() else { continue };
        let mut job = Job::default();
        for (key, raw) in object {
            let value = match raw {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Array(items) => {
                    items.iter().filter_map(|i| i.as_str()).collect::<Vec<_>>().join(",")
                }
                other => other.to_string(),
            };
            set(&mut job, key, &value);
        }
        if !job.links.trim().is_empty() {
            jobs.push(job);
        }
    }
    jobs
}

/// Containers, the extensions JDownloader opens itself.
pub fn is_container(extension: &str) -> bool {
    CONTAINERS.contains(&extension.to_ascii_lowercase().as_str())
}

/// Read a text file whoever wrote it. Windows editors like to add a byte
/// order mark, and the older ones save UTF-16.
pub fn read_text(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let text = match bytes.as_slice() {
        [0xFF, 0xFE, rest @ ..] => decode_utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => decode_utf16(rest, u16::from_be_bytes),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        other => String::from_utf8_lossy(other).into_owned(),
    };
    Ok(text)
}

fn decode_utf16(bytes: &[u8], order: fn([u8; 2]) -> u16) -> String {
    let units: Vec<u16> = bytes.chunks_exact(2).map(|pair| order([pair[0], pair[1]])).collect();
    String::from_utf16_lossy(&units)
}

/// What `folder` holds that jdtui knows how to send, oldest first.
fn candidates(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(folder).with_context(|| format!("reading {}", folder.display()))? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        // Hidden files, and the metadata twins macOS leaves next to a
        // file it copied: never the real thing.
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        if extension == "crawljob" || CONTAINERS.contains(&extension.as_str()) {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

/// Whether the file has stopped changing. A container being copied into
/// the folder is not a container yet.
fn settled(path: &Path, seen: &mut HashMap<PathBuf, u64>) -> bool {
    let Ok(meta) = path.metadata() else { return false };
    let size = meta.len();
    let previous = seen.insert(path.to_path_buf(), size);
    let old_enough =
        meta.modified().ok().and_then(|m| SystemTime::now().duration_since(m).ok()).is_some_and(|age| age >= SETTLE);
    old_enough || previous == Some(size)
}

/// Move the file out of the way so it is not sent twice, into
/// `processed` or `failed` beside it. A name already taken gets a counter.
fn file_away(path: &Path, folder: &Path, ok: bool) -> Result<PathBuf> {
    let target_dir = folder.join(if ok { DONE } else { FAILED });
    fs::create_dir_all(&target_dir).with_context(|| format!("creating {}", target_dir.display()))?;
    let name = path.file_name().unwrap_or_default();
    let mut target = target_dir.join(name);
    let mut n = 1;
    while target.exists() {
        let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        target = target_dir.join(format!("{stem}.{n}.{extension}"));
        n += 1;
    }
    fs::rename(path, &target).with_context(|| format!("moving {} aside", path.display()))?;
    Ok(target)
}

/// Send one file. Returns what to tell the user about it.
fn send(api: &mut JdApi, path: &Path) -> Result<String> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    if CONTAINERS.contains(&extension.as_str()) {
        let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        api.add_container(&extension, &bytes).context("handing the container to JDownloader")?;
        return Ok(format!("container sent ({} bytes)", bytes.len()));
    }
    let text = read_text(path)?;
    let jobs = parse_crawljob(&text);
    if jobs.is_empty() {
        anyhow::bail!("no job with a url in it");
    }
    let count = jobs.len();
    for job in jobs {
        api.add_links(&job.into()).context("adding the links of the job")?;
    }
    Ok(format!("{count} job(s) added"))
}

/// What became of one file the sweep picked up. Nothing is printed here:
/// the interface would be drawn over.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The file as it was named when it arrived.
    pub file: String,
    /// What JDownloader did with it, or why it would not.
    pub result: Result<String, String>,
    /// Where the file was filed away.
    pub moved_to: PathBuf,
}

/// Walk the folder once, sending whatever has settled.
pub fn sweep(api: &mut JdApi, folder: &Path, seen: &mut HashMap<PathBuf, u64>) -> Result<Vec<Outcome>> {
    let mut done = Vec::new();
    for path in candidates(folder)? {
        if !settled(&path, seen) {
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        // Claim the file by moving it before reading it. Sending first
        // and moving after would send it twice if the move failed, and
        // on Windows a move fails for exactly the file another program
        // still holds open, which is the one we must not touch yet.
        let Ok(claimed) = file_away(&path, folder, true) else {
            // Someone still holds it: leave it for the next sweep.
            continue;
        };
        let outcome = match send(api, &claimed) {
            Ok(what) => Outcome { file: name, result: Ok(what), moved_to: claimed },
            Err(e) => {
                let target = file_away(&claimed, folder, false).unwrap_or(claimed);
                Outcome { file: name, result: Err(format!("{e:#}")), moved_to: target }
            }
        };
        done.push(outcome);
        seen.remove(&path);
    }
    Ok(done)
}

/// Say out loud what a sweep did, for the command line.
pub fn report(outcomes: &[Outcome]) -> usize {
    let mut sent = 0;
    for outcome in outcomes {
        match &outcome.result {
            Ok(what) => {
                println!("{}: {what}, moved to {}", outcome.file, outcome.moved_to.display());
                sent += 1;
            }
            Err(why) => eprintln!("{}: {why}, moved to {}", outcome.file, outcome.moved_to.display()),
        }
    }
    sent
}

/// Watch until interrupted, sweeping every `interval`.
pub fn watch(api: &mut JdApi, folder: &Path, interval: Duration) -> Result<()> {
    if !folder.is_dir() {
        anyhow::bail!("{} is not a folder", folder.display());
    }
    println!("Watching {} every {}s. Ctrl-C to stop.", folder.display(), interval.as_secs());
    let mut seen = HashMap::new();
    loop {
        match sweep(api, folder, &mut seen) {
            Ok(outcomes) => {
                report(&outcomes);
            }
            Err(e) => eprintln!("sweep failed: {e:#}"),
        }
        std::thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_properties_crawljob_becomes_one_job() {
        let jobs = parse_crawljob(
            "# a comment\ntext=https://example.com/file\npackageName=Ubuntu\ndownloadFolder=/data\nautoStart=TRUE\n",
        );
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].links, "https://example.com/file");
        assert_eq!(jobs[0].package_name, "Ubuntu");
        assert_eq!(jobs[0].destination, "/data");
        assert!(jobs[0].autostart);
    }

    #[test]
    fn entries_are_split_where_folder_watch_splits_them() {
        let jobs =
            parse_crawljob("text=https://a.example\n->NEW ENTRY<-\ntext=https://b.example\npackageName=Second\n");
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].links, "https://a.example");
        assert_eq!(jobs[1].package_name, "Second");
    }

    #[test]
    fn a_job_without_a_url_is_dropped() {
        assert!(parse_crawljob("packageName=Nothing\nautoStart=TRUE\n").is_empty());
        assert!(parse_crawljob("").is_empty());
    }

    #[test]
    fn the_json_shape_is_understood_too() {
        let jobs = parse_crawljob(
            r#"[{"text": "https://example.com/a", "packageName": "P", "extractPasswords": ["one", "two"]}]"#,
        );
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].links, "https://example.com/a");
        // The API takes a single password, so the first is used.
        assert_eq!(jobs[0].extract_password, "one");
    }

    #[test]
    fn quotes_and_case_do_not_matter() {
        let jobs = parse_crawljob("TEXT=\"https://example.com/x\"\nPRIORITY=high\n");
        assert_eq!(jobs[0].links, "https://example.com/x");
        assert_eq!(jobs[0].priority, "HIGH");
    }

    #[test]
    fn only_containers_and_crawljobs_are_picked_up() {
        let dir = std::env::temp_dir().join(format!("jdtui-watch-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for name in ["a.crawljob", "b.dlc", "c.txt", "d.CCF", "notes.md"] {
            fs::write(dir.join(name), "x").unwrap();
        }
        let found: Vec<String> =
            candidates(&dir).unwrap().iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(found, vec!["a.crawljob", "b.dlc", "d.CCF"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_windows_written_job_is_read_all_the_same() {
        let dir = std::env::temp_dir().join(format!("jdtui-encoding-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // UTF-8 with a byte order mark and CRLF line endings.
        let bom = dir.join("bom.crawljob");
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"text=https://example.com/a\r\npackageName=P\r\n");
        fs::write(&bom, bytes).unwrap();
        let jobs = parse_crawljob(&read_text(&bom).unwrap());
        assert_eq!(jobs.len(), 1, "a byte order mark must not hide the first key");
        assert_eq!(jobs[0].links, "https://example.com/a");
        assert_eq!(jobs[0].package_name, "P");

        // UTF-16 little endian, as an older Notepad saves it.
        let utf16 = dir.join("utf16.crawljob");
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "text=https://example.com/b\r\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        fs::write(&utf16, bytes).unwrap();
        let jobs = parse_crawljob(&read_text(&utf16).unwrap());
        assert_eq!(jobs[0].links, "https://example.com/b");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn macos_metadata_and_hidden_files_are_left_alone() {
        let dir = std::env::temp_dir().join(format!("jdtui-hidden-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for name in ["real.crawljob", "._real.crawljob", ".hidden.dlc"] {
            fs::write(dir.join(name), "text=https://example.com/x").unwrap();
        }
        let found: Vec<String> =
            candidates(&dir).unwrap().iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(found, vec!["real.crawljob"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_still_being_written_waits_for_the_next_sweep() {
        let dir = std::env::temp_dir().join(format!("jdtui-settle-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("growing.crawljob");
        fs::write(&path, "text=https://a.example").unwrap();
        let mut seen = HashMap::new();
        // Just written, and bigger than last time: not yet.
        assert!(!settled(&path, &mut seen));
        fs::write(&path, "text=https://a.example/longer").unwrap();
        assert!(!settled(&path, &mut seen));
        // Unchanged since the last look: good to go.
        assert!(settled(&path, &mut seen));
        fs::remove_dir_all(&dir).unwrap();
    }
}
