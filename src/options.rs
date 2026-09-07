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

use serde_json::Value;

use crate::api::ConfigEntry;

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

/// The wording of every choice of one ENUM setting, `constant` to English.
/// JDownloader translates its own labels into the language it runs in, and
/// leaves them out for some settings entirely, so a panel that used them
/// would be part English and part whatever the JDownloader speaks. These
/// keep it in one language, and say what the constants mean: the choice
/// that deletes an archive for good is called `NULL`.
type Choices = &'static [(&'static str, &'static str)];

const IF_FILE_EXISTS: Choices = &[
    ("OVERWRITE_FILE", "Overwrite the file"),
    ("SKIP_FILE", "Skip the file"),
    ("AUTO_RENAME", "Rename the new file"),
    ("ASK_FOR_EACH_FILE", "Ask every time"),
];

const START_ON_LAUNCH: Choices = &[
    ("ALWAYS", "Always"),
    ("ONLY_IF_EXIT_WITH_RUNNING_DOWNLOADS", "Only if downloads were running at exit"),
    ("NEVER", "Never"),
];

const CONFIRM_ADDED: Choices = &[
    ("AUTO", "Follow JDownloader's quick settings"),
    ("ENABLED", "Always start them"),
    ("DISABLED", "Never start them"),
];

const AFTER_EXTRACTION: Choices = &[
    ("NO_DELETE", "Keep the archive files"),
    ("RECYCLE", "Move them to the recycle bin"),
    ("NULL", "Delete them permanently"),
];

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
    /// Empty unless the setting is an ENUM this panel has wording for.
    pub choices: Choices,
}

const GENERAL: &str = "org.jdownloader.settings.GeneralSettings";
const GRABBER: &str = "org.jdownloader.gui.views.linkgrabber.addlinksdialog.LinkgrabberSettings";
const EXTRACTION: &str = "org.jdownloader.extensions.extraction.ExtractionConfig";

/// The interfaces the panel reads, each with one call.
pub const INTERFACES: [&str; 3] = [GENERAL, GRABBER, EXTRACTION];

macro_rules! spec {
    ($section:expr, $interface:expr, $key:expr, $label:expr, $note:expr) => {
        spec!($section, $interface, $key, $label, $note, Unit::None, &[])
    };
    ($section:expr, $interface:expr, $key:expr, $label:expr, $note:expr, unit = $unit:expr) => {
        spec!($section, $interface, $key, $label, $note, $unit, &[])
    };
    ($section:expr, $interface:expr, $key:expr, $label:expr, $note:expr, choices = $choices:expr) => {
        spec!($section, $interface, $key, $label, $note, Unit::None, $choices)
    };
    ($section:expr, $interface:expr, $key:expr, $label:expr, $note:expr, $unit:expr, $choices:expr) => {
        Spec {
            section: $section,
            interface: $interface,
            key: $key,
            label: $label,
            note: $note,
            unit: $unit,
            choices: $choices,
        }
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
        unit = Unit::Speed
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
        unit = Unit::Megabytes
    ),
    spec!(
        "Downloads",
        GENERAL,
        "IfFileExistsAction",
        "If the file exists",
        "What happens when the file being written is already there.",
        choices = IF_FILE_EXISTS
    ),
    spec!(
        "Downloads",
        GENERAL,
        "AutoStartDownloadOption",
        "Start on launch",
        "Whether JDownloader picks the downloads up again when it starts.",
        choices = START_ON_LAUNCH
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
        "Whether crawled links move to the downloads on their own.",
        choices = CONFIRM_ADDED
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
        "What happens to the archive volumes once they are unpacked.",
        choices = AFTER_EXTRACTION
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

    /// The value as the panel shows it.
    pub fn shown(&self) -> String {
        self.render(self.entry.value.as_ref())
    }

    /// The value JDownloader ships, shown the same way.
    pub fn shown_default(&self) -> String {
        self.render(self.entry.default_value.as_ref())
    }

    fn render(&self, value: Option<&Value>) -> String {
        let Some(value) = value else { return "-".into() };
        match self.edit() {
            Edit::Toggle => if value.as_bool().unwrap_or(false) { "on" } else { "off" }.into(),
            Edit::Choice => self.choice_label(value.as_str().unwrap_or_default()),
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

    /// This panel's wording for one choice. A constant it has no wording
    /// for is made readable rather than shown as
    /// `ONLY_IF_EXIT_WITH_RUNNING_DOWNLOADS`, so a JDownloader that grows a
    /// choice still says something.
    pub fn choice_label(&self, name: &str) -> String {
        self.spec
            .choices
            .iter()
            .find(|(constant, _)| *constant == name)
            .map(|(_, wording)| (*wording).to_string())
            .unwrap_or_else(|| humanize(name))
    }
}

/// `ONLY_IF_EXIT_WITH_RUNNING_DOWNLOADS` reads as
/// `Only if exit with running downloads`.
fn humanize(name: &str) -> String {
    let words = name.split('_').map(str::to_lowercase).collect::<Vec<_>>().join(" ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => words,
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
        let toggle = Setting { spec: &CURATED[1], entry: entry(GENERAL, "k", "BOOLEAN", json!(false)) };
        assert_eq!(toggle.shown(), "off");
        let text = Setting { spec: &CURATED[10], entry: entry(GENERAL, "k", "STRING", json!("")) };
        assert_eq!(text.shown(), "not set");
    }

    #[test]
    fn a_number_carries_the_unit_of_its_key() {
        assert_eq!(number(10240, Unit::Speed), "10.00 KB/s");
        assert_eq!(number(128, Unit::Megabytes), "128 MB");
        assert_eq!(number(5, Unit::None), "5");
    }

    #[test]
    fn choices_read_in_this_panels_own_words() {
        let mut e = entry(GENERAL, "IfFileExistsAction", "ENUM", json!("SKIP_FILE"));
        e.kind = Some("org.jdownloader.settings.IfFileExistsAction".into());
        let setting = Setting { spec: &CURATED[8], entry: e };
        assert_eq!(setting.shown(), "Skip the file");
        // A constant this panel has no wording for is still readable.
        assert_eq!(setting.choice_label("SOMETHING_NEW"), "Something new");
    }

    #[test]
    fn every_choice_the_device_offers_has_wording() {
        // The lists were read from a real JDownloader; a constant missing
        // here would be shown humanized, which reads oddly for `NULL`.
        assert_eq!(CURATED.iter().filter(|s| !s.choices.is_empty()).count(), 4);
        for spec in CURATED {
            for (constant, wording) in spec.choices {
                assert!(!wording.is_empty(), "{constant} of {} has no wording", spec.key);
            }
        }
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
        for s in &settings {
            println!("{:<34} {:<10} {}", s.spec.label, format!("{:?}", s.edit()), s.shown());
            if s.edit() == Edit::Choice
                && let Some(kind) = s.entry.kind.clone()
            {
                // Every choice the device offers must have wording here.
                for choice in api.config_enum(&kind).expect("enum") {
                    assert!(
                        s.spec.choices.iter().any(|(constant, _)| *constant == choice.name),
                        "{} offers {}, which {} has no wording for",
                        s.spec.key,
                        choice.name,
                        s.spec.label
                    );
                }
            }
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
