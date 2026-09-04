//! Interface state that is independent of how it is drawn: the flat list of
//! rows the cursor walks, the selection, the context menu and the forms.

use std::collections::HashSet;

use crate::api::{Link, Package, RemoveMode, Snapshot};

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

/// The visible rows. With a filter, a package stays if it matches by
/// itself (then all its links show when expanded) or through some of its
/// links (then only those show).
pub fn build_rows(packages: &[Package], expanded: &HashSet<i64>, filter: &str) -> Vec<Row> {
    let needle = filter.trim().to_lowercase();
    let has = |s: Option<&str>| s.is_some_and(|s| s.to_lowercase().contains(&needle));
    let link_matches = |l: &Link| has(Some(&l.name)) || has(l.host.as_deref()) || has(l.status.as_deref());
    let mut rows = Vec::new();
    for (p, pkg) in packages.iter().enumerate() {
        let pkg_matches = needle.is_empty()
            || has(Some(&pkg.name))
            || has(pkg.status.as_deref())
            || has(pkg.save_to.as_deref())
            || pkg.hosts.iter().flatten().any(|h| has(Some(h)));
        let links: Vec<usize> = (0..pkg.links.len()).filter(|&l| pkg_matches || link_matches(&pkg.links[l])).collect();
        if !pkg_matches && links.is_empty() {
            continue;
        }
        rows.push(Row { kind: RowKind::Package, package: p, link: None });
        if expanded.contains(&pkg.uuid) {
            rows.extend(links.into_iter().map(|l| Row { kind: RowKind::Link, package: p, link: Some(l) }));
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
    /// Open the new-package form for the selection.
    NewPackage,
    SplitByHoster,
    /// Show the urls of the selection and copy them to the clipboard.
    Urls,
    /// Clear the skip reason of skipped links; downloads only.
    Unskip,
    /// Re-check whether the links are still online.
    CheckOnline,
    /// Queue the complete archives of the selection for extraction.
    ExtractNow,
    /// Open the variant chooser; one grabber link with variants.
    Variant,
    // The JDownloader itself, from the device menu; no selection involved.
    CheckUpdate,
    /// Give up on every captcha waiting; the blocked links are skipped.
    SkipCaptchas,
    UpdateAndRestart,
    RestartJd,
    ExitJd,
    Reconnect,
    Properties,
}

impl Action {
    /// Acts on the JDownloader, not on rows.
    pub fn is_device(self) -> bool {
        matches!(
            self,
            Action::CheckUpdate
                | Action::SkipCaptchas
                | Action::UpdateAndRestart
                | Action::RestartJd
                | Action::ExitJd
                | Action::Reconnect
        )
    }
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
            entry(format!("Unskip{suffix}"), Action::Unskip, false),
            toggle,
            entry(format!("Reset{suffix}"), Action::Reset, true),
        ];
        v.push(entry("Extract now", Action::ExtractNow, false));
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
    insert(entry("Move to new package…", Action::NewPackage, false));
    insert(entry("Split by hoster", Action::SplitByHoster, false));
    insert(entry("Copy urls", Action::Urls, false));
    insert(entry("Check availability", Action::CheckOnline, false));
    if single
        && tab.is_grabber()
        && let Some(l) = rows[0].link
        && packages[rows[0].package].links[l].variants == Some(true)
    {
        insert(entry("Choose variant…", Action::Variant, false));
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

/// What can be done to the JDownloader itself. Nothing that touches the
/// host machine (shutdown, standby) is offered, and updating only when
/// JDownloader says there is one.
pub fn device_menu(update_available: bool, captchas: usize) -> Vec<MenuEntry> {
    let mut v = Vec::new();
    if captchas > 0 {
        let label = format!("Skip {captchas} waiting captcha{}", if captchas == 1 { "" } else { "s" });
        v.push(entry(label, Action::SkipCaptchas, true));
    }
    v.push(entry("Check for updates", Action::CheckUpdate, false));
    if update_available {
        v.push(entry("Update and restart", Action::UpdateAndRestart, true));
    }
    v.extend([
        entry("Restart JDownloader", Action::RestartJd, true),
        entry("Reconnect (new IP)", Action::Reconnect, true),
        entry("Exit JDownloader", Action::ExitJd, true),
    ]);
    v
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
    /// Insertion point in `text`, in chars; text fields only.
    pub cursor: usize,
    pub flag: bool,
}

impl Field {
    fn text(label: &'static str, hint: &'static str) -> Self {
        Field { label, hint, kind: FieldKind::Text, text: String::new(), cursor: 0, flag: false }
    }
    fn secret(label: &'static str) -> Self {
        Field { label, hint: "", kind: FieldKind::Secret, text: String::new(), cursor: 0, flag: false }
    }
    /// Prefilled, with the cursor at the end.
    fn with_text(mut self, text: &str) -> Self {
        self.text = text.to_string();
        self.cursor = self.text.chars().count();
        self
    }

    fn editable(&self) -> bool {
        matches!(self.kind, FieldKind::Text | FieldKind::Secret)
    }

    fn byte_at(&self, chars: usize) -> usize {
        self.text.char_indices().nth(chars).map(|(i, _)| i).unwrap_or(self.text.len())
    }

    fn insert(&mut self, s: &str) {
        let at = self.byte_at(self.cursor);
        self.text.insert_str(at, s);
        self.cursor += s.chars().count();
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            let (from, to) = (self.byte_at(self.cursor - 1), self.byte_at(self.cursor));
            self.text.replace_range(from..to, "");
            self.cursor -= 1;
        }
    }

    fn delete(&mut self) {
        if self.cursor < self.text.chars().count() {
            let (from, to) = (self.byte_at(self.cursor), self.byte_at(self.cursor + 1));
            self.text.replace_range(from..to, "");
        }
    }

    fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
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
        let email_field = Field::text("Email", "your My.JDownloader account").with_text(email);
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
                Field {
                    label: "Priority",
                    hint: "",
                    kind: FieldKind::Choice,
                    text: "DEFAULT".into(),
                    cursor: 0,
                    flag: false,
                },
                Field {
                    label: "Autostart",
                    hint: "",
                    kind: FieldKind::Flag,
                    text: String::new(),
                    cursor: 0,
                    flag: false,
                },
            ],
            index: 0,
        }
    }

    pub fn rename(current: &str) -> Self {
        let name = Field::text("Name", "").with_text(current);
        Form { title: "Rename", fields: vec![name], index: 0 }
    }

    pub fn directory(current: &str) -> Self {
        let dir = Field::text("Save to", "absolute path on the JDownloader machine").with_text(current);
        Form { title: "Download folder", fields: vec![dir], index: 0 }
    }

    pub fn archive_password() -> Self {
        Form {
            title: "Add an archive password",
            fields: vec![Field::text("Password", "added to the list JDownloader tries on every archive")],
            index: 0,
        }
    }

    pub fn filter(current: &str) -> Self {
        let field = Field::text("Filter", "name, hoster or status; case does not matter").with_text(current);
        Form { title: "Filter the list", fields: vec![field], index: 0 }
    }

    pub fn new_package() -> Self {
        Form {
            title: "Move to a new package",
            fields: vec![
                Field::text("Package name", ""),
                Field::text("Save to", "leave empty to keep the current folder"),
            ],
            index: 0,
        }
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

    /// Insert at the cursor of the active text field.
    pub fn type_str(&mut self, s: &str) {
        let f = &mut self.fields[self.index];
        if f.editable() {
            f.insert(s);
        }
    }

    pub fn backspace(&mut self) {
        let f = &mut self.fields[self.index];
        if f.editable() {
            f.backspace();
        }
    }

    pub fn delete(&mut self) {
        let f = &mut self.fields[self.index];
        if f.editable() {
            f.delete();
        }
    }

    /// Ctrl-U: empty the active text field.
    pub fn clear(&mut self) {
        let f = &mut self.fields[self.index];
        if f.editable() {
            f.clear();
        }
    }

    /// Left/right: move the cursor of a text field; cycle a choice; flip
    /// a flag.
    pub fn cycle(&mut self, delta: i32) {
        let f = &mut self.fields[self.index];
        match f.kind {
            FieldKind::Choice => {
                let i = PRIORITIES.iter().position(|p| *p == f.text).unwrap_or(3) as i32;
                let n = PRIORITIES.len() as i32;
                f.text = PRIORITIES[((i + delta).rem_euclid(n)) as usize].to_string();
            }
            FieldKind::Flag => f.flag = !f.flag,
            FieldKind::Text | FieldKind::Secret => {
                let len = f.text.chars().count();
                f.cursor = if delta < 0 { f.cursor.saturating_sub(1) } else { (f.cursor + 1).min(len) };
            }
        }
    }

    pub fn home(&mut self) {
        self.fields[self.index].cursor = 0;
    }

    pub fn end(&mut self) {
        let f = &mut self.fields[self.index];
        f.cursor = f.text.chars().count();
    }

    pub fn is_valid(&self) -> bool {
        // The first field is the one that cannot be empty in both forms.
        !self.fields[0].text.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_fields_edit_at_the_cursor() {
        let mut form = Form::rename("città");
        assert_eq!(form.fields[0].cursor, 5);
        form.cycle(-1);
        form.cycle(-1);
        form.type_str("XY");
        assert_eq!(form.value("Name"), "citXYtà");
        assert_eq!(form.fields[0].cursor, 5);
        form.backspace();
        assert_eq!(form.value("Name"), "citXtà");
        form.delete();
        assert_eq!(form.value("Name"), "citXà");
        form.home();
        form.delete();
        assert_eq!(form.value("Name"), "itXà");
        form.end();
        form.type_str("!");
        assert_eq!(form.value("Name"), "itXà!");
        form.cycle(1);
        assert_eq!(form.fields[0].cursor, 5, "cannot move past the end");
        form.clear();
        assert_eq!(form.value("Name"), "");
        assert_eq!(form.fields[0].cursor, 0);
    }

    #[test]
    fn filter_keeps_matching_packages_and_links() {
        let pkg = |uuid: i64, name: &str, links: &[(&str, &str)]| Package {
            uuid,
            name: name.into(),
            links: links
                .iter()
                .enumerate()
                .map(|(i, (n, h))| Link {
                    uuid: uuid * 100 + i as i64,
                    name: (*n).into(),
                    host: Some((*h).into()),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let packages = vec![
            pkg(1, "Debian images", &[("debian.iso", "mirror.org"), ("sha256", "mirror.org")]),
            pkg(2, "Ubuntu", &[("ubuntu.iso", "cdn.net")]),
        ];
        let expanded: HashSet<i64> = [1, 2].into_iter().collect();

        assert_eq!(build_rows(&packages, &expanded, "").len(), 5);
        // The package matches: all its links stay.
        let rows = build_rows(&packages, &expanded, "DEBIAN");
        assert_eq!(rows.len(), 3);
        // Only a link matches: the package stays with that link alone.
        let rows = build_rows(&packages, &expanded, "sha");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].link, Some(1));
        // Host matches too.
        assert_eq!(build_rows(&packages, &expanded, "cdn").len(), 2);
        assert!(build_rows(&packages, &expanded, "nothing").is_empty());
    }

    #[test]
    fn choices_and_flags_ignore_typing() {
        let mut form = Form::add_links();
        form.index = form.fields.iter().position(|f| f.label == "Priority").unwrap();
        form.type_str("x");
        form.backspace();
        assert_eq!(form.value("Priority"), "DEFAULT");
        form.cycle(-1);
        assert_eq!(form.value("Priority"), "HIGH");
        form.index = form.fields.iter().position(|f| f.label == "Autostart").unwrap();
        form.cycle(1);
        assert!(form.flag("Autostart"));
    }
}
