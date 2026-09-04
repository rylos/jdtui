//! Interface state that is independent of how it is drawn: the flat list of
//! rows the cursor walks, the selection, the context menu and the forms.

use std::collections::HashSet;

use crate::api::{Package, RemoveMode, Snapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Downloads,
    Grabber,
}

impl Tab {
    pub fn other(self) -> Tab {
        match self {
            Tab::Downloads => Tab::Grabber,
            Tab::Grabber => Tab::Downloads,
        }
    }
    pub fn is_grabber(self) -> bool {
        self == Tab::Grabber
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowKind {
    Package,
    Link,
}

/// Identity of a row across refreshes. Uuids repeat across kinds, so the
/// kind is part of it.
pub type RowKey = (RowKind, i64);

/// A visible row: a package, or a link of an expanded package.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub kind: RowKind,
    pub package: usize,
    pub link: Option<usize>,
}

impl Row {
    pub fn is_package(&self) -> bool {
        self.kind == RowKind::Package
    }
}

pub fn packages_of(snapshot: &Snapshot, tab: Tab) -> &[Package] {
    match tab {
        Tab::Downloads => &snapshot.downloads,
        Tab::Grabber => &snapshot.grabber,
    }
}

pub fn build_rows(packages: &[Package], expanded: &HashSet<i64>) -> Vec<Row> {
    let mut rows = Vec::new();
    for (p, pkg) in packages.iter().enumerate() {
        rows.push(Row { kind: RowKind::Package, package: p, link: None });
        if expanded.contains(&pkg.uuid) {
            for l in 0..pkg.links.len() {
                rows.push(Row { kind: RowKind::Link, package: p, link: Some(l) });
            }
        }
    }
    rows
}

pub fn row_key(packages: &[Package], row: &Row) -> RowKey {
    match row.link {
        None => (RowKind::Package, packages[row.package].uuid),
        Some(l) => (RowKind::Link, packages[row.package].links[l].uuid),
    }
}

pub fn row_name<'a>(packages: &'a [Package], row: &Row) -> &'a str {
    match row.link {
        None => &packages[row.package].name,
        Some(l) => &packages[row.package].links[l].name,
    }
}

pub fn row_priority<'a>(packages: &'a [Package], row: &Row) -> Option<&'a str> {
    match row.link {
        None => packages[row.package].priority.as_deref(),
        Some(l) => packages[row.package].links[l].priority.as_deref(),
    }
}

/// The link the stop mark goes on for this row: the link itself, or the
/// last link of a package, which is as close as the API gets to "after
/// this package".
pub fn stop_mark_target(packages: &[Package], row: &Row) -> Option<i64> {
    match row.link {
        None => packages[row.package].links.last().map(|l| l.uuid),
        Some(l) => Some(packages[row.package].links[l].uuid),
    }
}

/// Whether the stop mark sits on this row: on the link, or on any link
/// of the package.
pub fn row_stop_marked(packages: &[Package], row: &Row, stop_mark: Option<i64>) -> bool {
    let Some(mark) = stop_mark else { return false };
    match row.link {
        None => packages[row.package].links.iter().any(|l| l.uuid == mark),
        Some(l) => packages[row.package].links[l].uuid == mark,
    }
}

pub fn row_enabled(packages: &[Package], row: &Row) -> bool {
    match row.link {
        None => packages[row.package].is_enabled(),
        Some(l) => packages[row.package].links[l].is_enabled(),
    }
}

/// (link ids, package ids) for a set of rows, as the API takes them.
pub fn collect_ids(packages: &[Package], rows: &[Row]) -> (Vec<i64>, Vec<i64>) {
    let mut links = Vec::new();
    let mut pkgs = Vec::new();
    for row in rows {
        match row.link {
            None => pkgs.push(packages[row.package].uuid),
            Some(l) => links.push(packages[row.package].links[l].uuid),
        }
    }
    (links, pkgs)
}

pub fn describe(rows: &[Row]) -> String {
    if rows.len() == 1 {
        return if rows[0].is_package() { "Package".into() } else { "Link".into() };
    }
    let pkgs = rows.iter().filter(|r| r.is_package()).count();
    let links = rows.len() - pkgs;
    let mut parts = Vec::new();
    if pkgs > 0 {
        parts.push(format!("{pkgs} package{}", if pkgs > 1 { "s" } else { "" }));
    }
    if links > 0 {
        parts.push(format!("{links} link{}", if links > 1 { "s" } else { "" }));
    }
    parts.join(" + ")
}

// --- context menu -----------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ToggleExpand,
    ToggleEnabled,
    Force,
    /// Continue interrupted links from where they stopped.
    Resume,
    Reset,
    Cleanup,
    /// Ask what to do with the files first, on the downloads tab.
    Remove,
    /// Remove, having decided about the files.
    RemoveWith(RemoveMode),
    MoveToDownloads,
    /// Empty the Link Grabber; ignores the selection.
    ClearGrabber,
    /// Open the priority chooser.
    Priority,
    /// Apply a priority, one of `PRIORITIES`.
    PriorityTo(&'static str),
    /// Set or clear the stop mark on the row; one row only, downloads tab.
    ToggleStopMark,
    /// Open the rename form; one row only.
    Rename,
    /// Open the download folder form; packages only.
    Directory,
    Properties,
}

#[derive(Debug, Clone)]
pub struct MenuEntry {
    pub label: String,
    pub action: Action,
    pub confirm: bool,
}

fn entry(label: impl Into<String>, action: Action, confirm: bool) -> MenuEntry {
    MenuEntry { label: label.into(), action, confirm }
}

/// The actions the API supports for this selection.
pub fn context_menu(tab: Tab, packages: &[Package], rows: &[Row], stop_mark: Option<i64>) -> Vec<MenuEntry> {
    let single = rows.len() == 1;
    let any_enabled = rows.iter().any(|r| row_enabled(packages, r));
    let suffix = if single { "" } else { " all" };
    let toggle =
        entry(format!("{}{suffix}", if any_enabled { "Disable" } else { "Enable" }), Action::ToggleEnabled, false);

    let mut entries = if tab.is_grabber() {
        vec![
            entry(format!("Move to download list{suffix}"), Action::MoveToDownloads, false),
            toggle,
            entry(format!("Remove{suffix}"), Action::Remove, true),
        ]
    } else {
        let mut v = vec![
            entry(format!("Force download{suffix}"), Action::Force, false),
            entry(format!("Resume{suffix}"), Action::Resume, false),
            toggle,
            entry(format!("Reset{suffix}"), Action::Reset, true),
        ];
        if rows.iter().any(|r| r.is_package()) {
            v.push(entry("Delete finished links", Action::Cleanup, true));
        }
        v.push(entry(format!("Remove{suffix}"), Action::Remove, true));
        v
    };
    let mut before_remove = entries.len() - 1;
    let mut insert = |e: MenuEntry| {
        entries.insert(before_remove, e);
        before_remove += 1;
    };
    insert(entry("Set priority…", Action::Priority, false));
    if single {
        insert(entry("Rename…", Action::Rename, false));
    }
    if rows.iter().all(|r| r.is_package()) {
        insert(entry("Set download folder…", Action::Directory, false));
    }
    if single && !tab.is_grabber() {
        let marked = row_stop_marked(packages, &rows[0], stop_mark);
        insert(entry(if marked { "Remove stop mark" } else { "Stop after this" }, Action::ToggleStopMark, false));
    }

    if single && rows[0].is_package() {
        entries.insert(0, entry("Collapse / Expand", Action::ToggleExpand, false));
    }
    if single {
        entries.push(entry("Properties", Action::Properties, false));
    }
    entries
}

// --- forms ------------------------------------------------------------------

pub const PRIORITIES: [&str; 7] = ["HIGHEST", "HIGHER", "HIGH", "DEFAULT", "LOW", "LOWER", "LOWEST"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Secret,
    Choice,
    Flag,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub label: &'static str,
    pub hint: &'static str,
    pub kind: FieldKind,
    pub text: String,
    pub flag: bool,
}

impl Field {
    fn text(label: &'static str, hint: &'static str) -> Self {
        Field { label, hint, kind: FieldKind::Text, text: String::new(), flag: false }
    }
    fn secret(label: &'static str) -> Self {
        Field { label, hint: "", kind: FieldKind::Secret, text: String::new(), flag: false }
    }
}

/// A small vertical form: text fields, one choice field and flags.
#[derive(Debug, Clone)]
pub struct Form {
    pub title: &'static str,
    pub fields: Vec<Field>,
    pub index: usize,
}

impl Form {
    pub fn login(email: &str) -> Self {
        let mut email_field = Field::text("Email", "your My.JDownloader account");
        email_field.text = email.to_string();
        Form {
            title: "Sign in to My.JDownloader",
            fields: vec![email_field, Field::secret("Password")],
            index: if email.is_empty() { 0 } else { 1 },
        }
    }

    pub fn add_links() -> Self {
        Form {
            title: "Add links to the Link Grabber",
            fields: vec![
                Field::text("Links", "one or more urls, separated by spaces"),
                Field::text("Package name", "leave empty for automatic"),
                Field::text("Save to", "leave empty for the default folder"),
                Field::text("Extract password", ""),
                Field::text("Download password", ""),
                Field { label: "Priority", hint: "", kind: FieldKind::Choice, text: "DEFAULT".into(), flag: false },
                Field { label: "Autostart", hint: "", kind: FieldKind::Flag, text: String::new(), flag: false },
            ],
            index: 0,
        }
    }

    pub fn rename(current: &str) -> Self {
        let mut name = Field::text("Name", "");
        name.text = current.to_string();
        Form { title: "Rename", fields: vec![name], index: 0 }
    }

    pub fn directory(current: &str) -> Self {
        let mut dir = Field::text("Save to", "absolute path on the JDownloader machine");
        dir.text = current.to_string();
        Form { title: "Download folder", fields: vec![dir], index: 0 }
    }

    pub fn value(&self, label: &str) -> &str {
        self.fields.iter().find(|f| f.label == label).map(|f| f.text.as_str()).unwrap_or("")
    }

    pub fn flag(&self, label: &str) -> bool {
        self.fields.iter().find(|f| f.label == label).map(|f| f.flag).unwrap_or(false)
    }

    pub fn next(&mut self) {
        self.index = (self.index + 1) % self.fields.len();
    }

    pub fn prev(&mut self) {
        self.index = (self.index + self.fields.len() - 1) % self.fields.len();
    }

    pub fn type_str(&mut self, s: &str) {
        let f = &mut self.fields[self.index];
        if matches!(f.kind, FieldKind::Text | FieldKind::Secret) {
            f.text.push_str(s);
        }
    }

    pub fn backspace(&mut self) {
        let f = &mut self.fields[self.index];
        if matches!(f.kind, FieldKind::Text | FieldKind::Secret) {
            f.text.pop();
        }
    }

    /// Left/right on a choice cycles it; on a flag flips it.
    pub fn cycle(&mut self, delta: i32) {
        let f = &mut self.fields[self.index];
        match f.kind {
            FieldKind::Choice => {
                let i = PRIORITIES.iter().position(|p| *p == f.text).unwrap_or(3) as i32;
                let n = PRIORITIES.len() as i32;
                f.text = PRIORITIES[((i + delta).rem_euclid(n)) as usize].to_string();
            }
            FieldKind::Flag => f.flag = !f.flag,
            _ => {}
        }
    }

    pub fn is_valid(&self) -> bool {
        // The first field is the one that cannot be empty in both forms.
        !self.fields[0].text.trim().is_empty()
    }
}
