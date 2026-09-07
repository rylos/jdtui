//! The settings the Options panel offers.
//!
//! JDownloader's advanced configuration is over two thousand entries across
//! more than two hundred interfaces, and most of them belong to a single
//! hoster plugin. Mirroring that in a terminal would be unusable, so this
//! module names the handful that get touched while downloads are running,
//! in the order they make sense to read. Everything else stays where it
//! belongs, in JDownloader's own settings.
//!
//! Each entry here is only a pointer: the label, the description, the type
//! and the value all come from the device at `/config/list`, so a setting
//! that a JDownloader does not have simply does not appear.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::api::{ConfigEntry, EnumOption};

/// How a number is to be read. JDownloader stores plain integers; the unit
/// is knowledge about the key, not something the API reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    None,
    /// Bytes per second, as the speed limit is stored.
    Speed,
    /// Whole megabytes, as the free space reserve is stored.
    Megabytes,
}

pub struct Spec {
    pub section: &'static str,
    pub interface: &'static str,
    pub key: &'static str,
    pub label: &'static str,
    /// What the setting does, in one line. JDownloader ships descriptions
    /// for some of these but not all, and the ones it ships are aimed at
    /// developers, so the panel prefers this.
    pub note: &'static str,
    pub unit: Unit,
}

const GENERAL: &str = "org.jdownloader.settings.GeneralSettings";
const GRABBER: &str = "org.jdownloader.gui.views.linkgrabber.addlinksdialog.LinkgrabberSettings";
const EXTRACTION: &str = "org.jdownloader.extensions.extraction.ExtractionConfig";

/// The interfaces the panel reads, each with one call.
pub const INTERFACES: [&str; 3] = [GENERAL, GRABBER, EXTRACTION];

macro_rules! spec {
    ($section:expr, $interface:expr, $key:expr, $label:expr, $note:expr) => {
        Spec { section: $section, interface: $interface, key: $key, label: $label, note: $note, unit: Unit::None }
    };
    ($section:expr, $interface:expr, $key:expr, $label:expr, $note:expr, $unit:expr) => {
        Spec { section: $section, interface: $interface, key: $key, label: $label, note: $note, unit: $unit }
    };
}

/// The curated list, in the order the panel shows it.
pub const CURATED: &[Spec] = &[
    spec!(
        "Downloads",
        GENERAL,
        "MaxSimultaneDownloads",
        "Simultaneous downloads",
        "How many files download at the same time."
    ),
    spec!(
        "Downloads",
        GENERAL,
        "MaxDownloadsPerHostEnabled",
        "Limit downloads per host",
        "Cap how many of those may come from one hoster."
    ),
    spec!(
        "Downloads",
        GENERAL,
        "MaxSimultaneDownloadsPerHost",
        "Downloads per host",
        "The cap itself; it does nothing while the limit above is off."
    ),
    spec!(
        "Downloads",
        GENERAL,
        "MaxChunksPerFile",
        "Chunks per file",
        "Connections one file is split over. Some hosters refuse more than one."
    ),
    spec!(
        "Downloads",
        GENERAL,
        "DownloadSpeedLimitEnabled",
        "Speed limit",
        "Whether the limit below applies. `p` pauses instead, which limits harder and temporarily."
    ),
    spec!(
        "Downloads",
        GENERAL,
        "DownloadSpeedLimit",
        "Speed limit value",
        "The ceiling on the total download speed.",
        Unit::Speed
    ),
    spec!(
        "Downloads",
        GENERAL,
        "MaxPluginRetries",
        "Retries per link",
        "How often a failing download is tried again before it is given up on."
    ),
    spec!(
        "Downloads",
        GENERAL,
        "ForcedFreeSpaceOnDisk",
        "Keep free on disk",
        "Downloads stop rather than fill the disk below this.",
        Unit::Megabytes
    ),
    spec!(
        "Downloads",
        GENERAL,
        "IfFileExistsAction",
        "If the file exists",
        "What happens when the file being written is already there."
    ),
    spec!(
        "Downloads",
        GENERAL,
        "AutoStartDownloadOption",
        "Start on launch",
        "Whether JDownloader picks the downloads up again when it starts."
    ),
    spec!(
        "Downloads",
        GENERAL,
        "DefaultDownloadFolder",
        "Download folder",
        "Where packages land when nothing else says otherwise. This path is on the JDownloader machine."
    ),
    spec!(
        "Link Grabber",
        GRABBER,
        "LinkgrabberAutoStartEnabled",
        "Start added links",
        "Begin downloading as soon as added links are confirmed."
    ),
    spec!(
        "Link Grabber",
        GRABBER,
        "AutoConfirmManagerAutoStart",
        "Confirm added links",
        "Whether crawled links move to the downloads on their own."
    ),
    spec!(
        "Link Grabber",
        GRABBER,
        "AutoExtractionEnabled",
        "Extract archives",
        "Unpack archives once every volume has finished."
    ),
    spec!(
        "Extraction",
        EXTRACTION,
        "DeepExtractionEnabled",
        "Extract nested archives",
        "Unpack archives found inside archives, as far down as they go."
    ),
    spec!(
        "Extraction",
        EXTRACTION,
        "AskForUnknownPasswordsEnabled",
        "Ask for passwords",
        "Prompt when an archive is encrypted and no known password fits. Headless, this waits for a client."
    ),
    spec!(
        "Extraction",
        EXTRACTION,
        "DeleteArchiveFilesAfterExtractionAction",
        "After extraction",
        "What happens to the archive volumes once they are unpacked."
    ),
    spec!(
        "Extraction",
        EXTRACTION,
        "DeleteArchiveDownloadlinksAfterExtraction",
        "Remove links after extraction",
        "Take the archive's links out of the download list once it is unpacked."
    ),
    spec!(
        "Extraction",
        EXTRACTION,
        "CustomExtractionPathEnabled",
        "Extract elsewhere",
        "Unpack into the folder below instead of beside the archive."
    ),
    spec!(
        "Extraction",
        EXTRACTION,
        "CustomExtractionPath",
        "Extraction folder",
        "Where extracted files go. This path is on the JDownloader machine."
    ),
];

/// How the panel lets a setting be changed, decided by the `abstractType`
/// the device reports. Anything else is shown but not offered for editing:
/// lists of objects and free-form maps need a dialog a terminal row cannot
/// give, and JDownloader's own settings do it better.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    Toggle,
    Number,
    Text,
    Choice,
    ReadOnly,
}

/// One curated setting as the device reports it.
pub struct Setting {
    pub spec: &'static Spec,
    pub entry: ConfigEntry,
}

impl Setting {
    pub fn edit(&self) -> Edit {
        match self.entry.abstract_type.as_deref() {
            Some("BOOLEAN") => Edit::Toggle,
            Some("INT" | "LONG") => Edit::Number,
            Some("STRING") => Edit::Text,
            Some("ENUM") => Edit::Choice,
            _ => Edit::ReadOnly,
        }
    }

    pub fn is_default(&self) -> bool {
        match (&self.entry.value, &self.entry.default_value) {
            (Some(v), Some(d)) => v == d,
            _ => true,
        }
    }

    /// The value as the panel shows it, with enum choices translated where
    /// `enums` has them.
    pub fn shown(&self, enums: &BTreeMap<String, Vec<EnumOption>>) -> String {
        self.render(self.entry.value.as_ref(), enums)
    }

    /// The value JDownloader ships, shown the same way.
    pub fn shown_default(&self, enums: &BTreeMap<String, Vec<EnumOption>>) -> String {
        self.render(self.entry.default_value.as_ref(), enums)
    }

    fn render(&self, value: Option<&Value>, enums: &BTreeMap<String, Vec<EnumOption>>) -> String {
        let Some(value) = value else { return "-".into() };
        match self.edit() {
            Edit::Toggle => if value.as_bool().unwrap_or(false) { "on" } else { "off" }.into(),
            Edit::Choice => self.choice_label(value.as_str().unwrap_or_default(), enums),
            Edit::Number => match value.as_i64() {
                Some(n) => number(n, self.spec.unit),
                None => value.to_string(),
            },
            Edit::Text => match value.as_str() {
                Some("") | None => "not set".into(),
                Some(s) => s.into(),
            },
            Edit::ReadOnly => value.to_string(),
        }
    }

    /// The translated label of one choice, falling back to the raw name so
    /// a setting is never shown as blank.
    pub fn choice_label(&self, name: &str, enums: &BTreeMap<String, Vec<EnumOption>>) -> String {
        self.entry
            .kind
            .as_deref()
            .and_then(|kind| enums.get(kind))
            .and_then(|options| options.iter().find(|o| o.name == name))
            .map(|o| o.shown())
            .unwrap_or_else(|| EnumOption { name: name.to_string(), label: None }.shown())
    }
}

/// A number with the unit its key is stored in. The speed limit is bytes
/// per second and the disk reserve is megabytes; JDownloader reports
/// neither, so both are knowledge held here.
pub fn number(n: i64, unit: Unit) -> String {
    match unit {
        Unit::None => n.to_string(),
        Unit::Speed => format!("{}/s", crate::ui::human_size(n)),
        Unit::Megabytes => format!("{} MB", n),
    }
}

/// Keep the curated settings the device reported, in the curated order.
/// `entries` is everything the `INTERFACES` calls brought back.
pub fn collect(entries: Vec<ConfigEntry>) -> Vec<Setting> {
    CURATED
        .iter()
        .filter_map(|spec| {
            let entry = entries.iter().find(|e| e.interface_name == spec.interface && e.key == spec.key)?.clone();
            Some(Setting { spec, entry })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(interface: &str, key: &str, kind: &str, value: serde_json::Value) -> ConfigEntry {
        ConfigEntry {
            interface_name: interface.into(),
            key: key.into(),
            abstract_type: Some(kind.into()),
            value: Some(value.clone()),
            default_value: Some(value),
            ..Default::default()
        }
    }

    #[test]
    fn every_curated_interface_is_one_we_ask_for() {
        for spec in CURATED {
            assert!(INTERFACES.contains(&spec.interface), "{} is never fetched", spec.interface);
        }
    }

    #[test]
    fn collect_keeps_the_curated_order_and_drops_the_rest() {
        let entries = vec![
            entry(EXTRACTION, "DeepExtractionEnabled", "BOOLEAN", json!(true)),
            entry(GENERAL, "MaxChunksPerFile", "INT", json!(5)),
            entry(GENERAL, "SomethingElse", "INT", json!(1)),
        ];
        let settings = collect(entries);
        let keys: Vec<&str> = settings.iter().map(|s| s.spec.key).collect();
        assert_eq!(keys, ["MaxChunksPerFile", "DeepExtractionEnabled"]);
    }

    #[test]
    fn a_setting_the_device_does_not_have_is_left_out() {
        assert!(collect(Vec::new()).is_empty());
    }

    #[test]
    fn values_are_shown_by_type() {
        let enums = BTreeMap::new();
        let toggle = Setting { spec: &CURATED[1], entry: entry(GENERAL, "k", "BOOLEAN", json!(false)) };
        assert_eq!(toggle.shown(&enums), "off");
        let text = Setting { spec: &CURATED[10], entry: entry(GENERAL, "k", "STRING", json!("")) };
        assert_eq!(text.shown(&enums), "not set");
    }

    #[test]
    fn a_number_carries_the_unit_of_its_key() {
        assert_eq!(number(10240, Unit::Speed), "10.00 KB/s");
        assert_eq!(number(128, Unit::Megabytes), "128 MB");
        assert_eq!(number(5, Unit::None), "5");
    }

    #[test]
    fn an_untranslated_choice_falls_back_to_its_name() {
        let mut enums = BTreeMap::new();
        enums.insert(
            "Action".to_string(),
            vec![EnumOption { name: "SKIP_FILE".into(), label: Some("Salta file".into()) }],
        );
        let mut e = entry(GENERAL, "IfFileExistsAction", "ENUM", json!("SKIP_FILE"));
        e.kind = Some("Action".into());
        let setting = Setting { spec: &CURATED[8], entry: e };
        assert_eq!(setting.shown(&enums), "Salta file");
        assert_eq!(setting.choice_label("OVERWRITE_FILE", &enums), "Overwrite file");
    }
}

#[cfg(test)]
mod live {
    //! Check the curated keys against a real JDownloader. Run with
    //! `cargo test -- --ignored --nocapture`.
    use super::*;
    use crate::api::JdApi;
    use crate::myjd::MyJd;

    #[test]
    #[ignore]
    fn every_curated_setting_exists_on_the_device() {
        let cfg = crate::config::Config::load().expect("config");
        let (email, password) = (cfg.email.expect("email"), cfg.password.expect("password"));
        let mut myjd = MyJd::new(&email, &password);
        myjd.connect().expect("connect");
        let devices = myjd.list_devices().expect("devices");
        let device = cfg
            .device
            .and_then(|id| devices.iter().find(|d| d.id == id).cloned())
            .unwrap_or_else(|| devices[0].clone());
        println!("using device: {}", device.name);
        let mut api = JdApi::new(myjd, device.id);
        let mut entries = Vec::new();
        for interface in INTERFACES {
            entries.extend(api.config_list(&format!(".*{interface}.*")).expect("list"));
        }
        let settings = collect(entries);
        let mut enums: BTreeMap<String, Vec<EnumOption>> = BTreeMap::new();
        for s in &settings {
            if s.edit() == Edit::Choice
                && let Some(kind) = s.entry.kind.clone()
            {
                enums.insert(kind.clone(), api.config_enum(&kind).expect("enum"));
            }
        }
        for s in &settings {
            println!("{:<34} {:<10} {}", s.spec.label, format!("{:?}", s.edit()), s.shown(&enums));
        }
        let missing: Vec<&str> = CURATED
            .iter()
            .filter(|spec| !settings.iter().any(|s| s.spec.key == spec.key))
            .map(|spec| spec.key)
            .collect();
        assert!(missing.is_empty(), "not reported by the device: {missing:?}");
        assert!(settings.iter().all(|s| s.edit() != Edit::ReadOnly), "a curated setting cannot be edited");
    }

    /// Open the panel the way a user does and flip a harmless setting, then
    /// put it back exactly as it was.
    #[test]
    #[ignore]
    fn the_panel_opens_and_writes() {
        use crate::app::{App, Key, Mode};
        let mut app = App::new(crate::config::Config::load().expect("config"));
        app.handle_key(Key::Char('o'));
        assert_eq!(app.mode, Mode::Options, "message: {:?}", app.message);
        assert!(!app.options.is_empty());

        // "Limit downloads per host" is off on a default JDownloader and
        // changes nothing on its own, so it is safe to flip.
        let at = app.options.iter().position(|s| s.spec.key == "MaxDownloadsPerHostEnabled").expect("the setting");
        app.option_index = at;
        let before = app.options[at].entry.value.clone();
        app.handle_key(Key::Enter);
        let after = app.options[at].entry.value.clone();
        println!("{before:?} -> {after:?}  ({:?})", app.message);
        assert_ne!(before, after, "the device did not take the change");
        app.handle_key(Key::Enter);
        assert_eq!(app.options[at].entry.value, before, "the setting was not put back");
    }
}
