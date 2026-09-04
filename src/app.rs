//! Application state and key handling. Drawing lives in `ui.rs`.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::api::{AddLinks, JdApi, RemoveMode, Snapshot, describe_error};
use crate::config::Config;
use crate::model::{
    Action, Form, MenuEntry, PRIORITIES, Row, RowKey, Tab, build_rows, collect_ids, context_menu, describe,
    packages_of, row_key, row_name, row_priority, row_stop_marked, stop_mark_target,
};
use crate::myjd::{Device, MyJd};
use crate::poller::{Poller, Update};

/// Offered in the order the GUI lists them: safest first.
pub const REMOVE_MODES: [RemoveMode; 3] = [RemoveMode::ListOnly, RemoveMode::Recycle, RemoveMode::DeleteFiles];

pub enum Screen {
    /// Asking for credentials; `error` explains why the last attempt failed.
    Login {
        form: Form,
        error: Option<String>,
    },
    /// More than one JDownloader on the account, and none remembered.
    Devices {
        devices: Vec<Device>,
        index: usize,
    },
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    List,
    Menu,
    Properties,
    Confirm(Action),
    /// Choosing what happens to the files of the packages being removed.
    RemoveChoice,
    /// Choosing a priority for the selection.
    PriorityChoice,
    Add,
    /// Editing the name of the row under the cursor, in `form`.
    Rename,
    /// Editing the download folder of the selected packages, in `form`.
    Directory,
    /// The full key reference; the footer only shows the frequent ones.
    Help,
}

/// Every key of the main screen, grouped for the help panel. The README's
/// key table mirrors this list.
pub const HELP: &[(&str, &[(&str, &str)])] = &[
    (
        "Navigation",
        &[
            ("Tab", "Switch between Downloads and Link Grabber"),
            ("↑ ↓  k j", "Move the cursor"),
            ("→ ←", "Expand / collapse a package"),
        ],
    ),
    (
        "Selection",
        &[
            ("Space", "Mark the row under the cursor"),
            ("a", "Mark every row, or clear the marks"),
            ("Esc", "Clear the selection"),
        ],
    ),
    (
        "Actions",
        &[
            ("Enter", "Context menu on the selection"),
            ("p", "Properties of the selected row"),
            ("n", "Add links to the Link Grabber"),
            ("t", "Stop after this row (again to clear)"),
            ("c", "Move the whole Link Grabber to downloads"),
        ],
    ),
    (
        "JDownloader",
        &[
            ("s", "Start / stop downloads"),
            ("P", "Pause / resume downloads"),
            ("d", "Switch to another JDownloader of the account"),
        ],
    ),
    ("Program", &[("?  h", "This help"), ("q  Ctrl-C", "Quit")]),
];

pub enum Key {
    Char(char),
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    Paste(String),
    CtrlC,
}

pub struct App {
    pub config: Config,
    pub screen: Screen,
    pub mode: Mode,
    pub should_quit: bool,

    myjd: Option<MyJd>,
    api: Option<Arc<Mutex<JdApi>>>,
    poller: Option<Poller>,
    pub device_name: String,

    pub snapshot: Snapshot,
    pub tab: Tab,
    pub rows: Vec<Row>,
    pub cursor: usize,
    pub expanded: HashSet<i64>,
    pub marked: HashSet<RowKey>,

    pub menu: Vec<MenuEntry>,
    pub menu_index: usize,
    /// Highlighted entry of the remove-choice panel.
    pub remove_index: usize,
    /// Highlighted entry of the priority panel, an index into `PRIORITIES`.
    pub priority_index: usize,
    pub form: Option<Form>,

    /// Footer message and whether it is an error.
    pub message: Option<(String, bool)>,
    /// Last refresh failure, shown in the header until a refresh succeeds.
    pub refresh_error: Option<String>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let mut app = App {
            screen: Screen::Login { form: Form::login(config.email.as_deref().unwrap_or("")), error: None },
            config,
            mode: Mode::List,
            should_quit: false,
            myjd: None,
            api: None,
            poller: None,
            device_name: String::new(),
            snapshot: Snapshot::default(),
            tab: Tab::Downloads,
            rows: Vec::new(),
            cursor: 0,
            expanded: HashSet::new(),
            marked: HashSet::new(),
            menu: Vec::new(),
            menu_index: 0,
            remove_index: 0,
            priority_index: 0,
            form: None,
            message: None,
            refresh_error: None,
        };
        if app.config.has_credentials() {
            let email = app.config.email.clone().unwrap_or_default();
            let password = app.config.password.clone().unwrap_or_default();
            app.sign_in(&email, &password);
        }
        app
    }

    /// Build an app sitting on the main screen with a given snapshot and no
    /// session behind it. Rendering never touches the network, so this is what
    /// the interface tests and the screenshot example draw.
    pub fn with_snapshot(snapshot: Snapshot) -> Self {
        let mut app = App {
            config: Config::default(),
            screen: Screen::Main,
            mode: Mode::List,
            should_quit: false,
            myjd: None,
            api: None,
            poller: None,
            device_name: "jd2@test".into(),
            snapshot,
            tab: Tab::Downloads,
            rows: Vec::new(),
            cursor: 0,
            expanded: HashSet::new(),
            marked: HashSet::new(),
            menu: Vec::new(),
            menu_index: 0,
            remove_index: 0,
            priority_index: 0,
            form: None,
            message: None,
            refresh_error: None,
        };
        app.rebuild_rows();
        app
    }

    // --- session -------------------------------------------------------

    fn sign_in(&mut self, email: &str, password: &str) {
        let mut myjd = MyJd::new(email, password);
        if let Err(e) = myjd.connect() {
            let why = if e.is_auth_failure() {
                "Email or password refused".to_string()
            } else {
                format!("Could not sign in: {}", describe_error(&e))
            };
            self.screen = Screen::Login { form: Form::login(email), error: Some(why) };
            return;
        }
        let devices = match myjd.list_devices() {
            Ok(d) => d,
            Err(e) => {
                self.screen = Screen::Login {
                    form: Form::login(email),
                    error: Some(format!("Could not list devices: {}", describe_error(&e))),
                };
                return;
            }
        };
        self.config.email = Some(email.to_string());
        self.config.password = Some(password.to_string());
        self.myjd = Some(myjd);
        self.pick_device(devices);
    }

    fn pick_device(&mut self, devices: Vec<Device>) {
        if devices.is_empty() {
            self.screen = Screen::Login {
                form: Form::login(self.config.email.as_deref().unwrap_or("")),
                error: Some("No JDownloader is connected to this account".into()),
            };
            return;
        }
        if let Some(wanted) = &self.config.device
            && let Some(d) = devices.iter().find(|d| &d.id == wanted)
        {
            let d = d.clone();
            return self.select_device(d);
        }
        if devices.len() == 1 {
            let d = devices[0].clone();
            return self.select_device(d);
        }
        self.screen = Screen::Devices { devices, index: 0 };
    }

    fn select_device(&mut self, device: Device) {
        self.config.device = Some(device.id.clone());
        let _ = self.config.save();
        self.device_name = device.name.clone();

        if let Some(api) = &self.api {
            // Switching from the main screen: keep the session, swap the target.
            if let Ok(mut a) = api.lock() {
                a.set_device(device.id);
            }
            self.snapshot = Snapshot::default();
            self.rows.clear();
            self.cursor = 0;
            self.expanded.clear();
            self.marked.clear();
            self.mode = Mode::List;
            if let Some(p) = &self.poller {
                p.refresh_now();
            }
            self.screen = Screen::Main;
            return;
        }

        let Some(myjd) = self.myjd.take() else { return };
        let api = Arc::new(Mutex::new(JdApi::new(myjd, device.id)));
        self.poller = Some(Poller::start(api.clone(), Duration::from_millis(self.config.refresh_ms())));
        self.api = Some(api);
        self.screen = Screen::Main;
    }

    /// Reopen the device picker from the main screen.
    fn choose_device(&mut self) {
        match self.with_api(|a| a.list_devices()) {
            Ok(devices) if devices.is_empty() => {
                self.message = Some(("No JDownloader is connected to this account".into(), true))
            }
            Ok(devices) => {
                let index = devices.iter().position(|d| Some(&d.id) == self.config.device.as_ref()).unwrap_or(0);
                self.screen = Screen::Devices { devices, index };
            }
            Err(e) => self.message = Some((format!("Could not list devices: {e}"), true)),
        }
    }

    /// Drain what the poller produced since the last frame.
    pub fn tick(&mut self) {
        let Some(poller) = &self.poller else { return };
        let mut latest = None;
        while let Some(update) = poller.try_recv() {
            latest = Some(update);
        }
        match latest {
            Some(Update::Snapshot(s)) => {
                self.snapshot = s;
                self.refresh_error = None;
                self.rebuild_rows();
            }
            Some(Update::Error(e)) => self.refresh_error = Some(e),
            None => {}
        }
    }

    // --- rows ----------------------------------------------------------

    fn packages(&self) -> &[crate::api::Package] {
        packages_of(&self.snapshot, self.tab)
    }

    fn rebuild_rows(&mut self) {
        self.rows = build_rows(self.packages(), &self.expanded);
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
        // Drop marks on rows that no longer exist.
        let live: HashSet<RowKey> = self.rows.iter().map(|r| row_key(self.packages(), r)).collect();
        self.marked.retain(|k| live.contains(k));
    }

    pub fn current_row(&self) -> Option<Row> {
        self.rows.get(self.cursor).copied()
    }

    /// Marked rows if any, otherwise the row under the cursor.
    pub fn target_rows(&self) -> Vec<Row> {
        let marked: Vec<Row> =
            self.rows.iter().copied().filter(|r| self.marked.contains(&row_key(self.packages(), r))).collect();
        if !marked.is_empty() {
            return marked;
        }
        self.current_row().into_iter().collect()
    }

    fn toggle_expand(&mut self, row: Row) {
        if row.is_package() {
            let uuid = self.packages()[row.package].uuid;
            if !self.expanded.remove(&uuid) {
                self.expanded.insert(uuid);
            }
            self.rebuild_rows();
        }
    }

    // --- actions -------------------------------------------------------

    fn with_api<T>(&self, f: impl FnOnce(&mut JdApi) -> crate::myjd::Result<T>) -> Result<T, String> {
        let api = self.api.as_ref().ok_or("not connected")?;
        let mut guard = api.lock().map_err(|_| "api lock poisoned".to_string())?;
        f(&mut guard).map_err(|e| describe_error(&e))
    }

    fn run_action(&mut self, action: Action) {
        let targets = self.target_rows();
        if targets.is_empty() {
            return;
        }
        match action {
            Action::ToggleExpand => {
                if let Some(row) = self.current_row() {
                    self.toggle_expand(row);
                }
                self.mode = Mode::List;
                return;
            }
            Action::Properties => {
                self.mode = Mode::Properties;
                return;
            }
            Action::ToggleStopMark => {
                self.toggle_stop_mark();
                return;
            }
            Action::Rename => {
                self.form = Some(Form::rename(row_name(self.packages(), &targets[0])));
                self.mode = Mode::Rename;
                return;
            }
            Action::Directory => {
                let current = self.packages()[targets[0].package].save_to.clone().unwrap_or_default();
                self.form = Some(Form::directory(&current));
                self.mode = Mode::Directory;
                return;
            }
            Action::Priority => {
                // Start from the current priority when there is one row.
                let current = if targets.len() == 1 { row_priority(self.packages(), &targets[0]) } else { None };
                self.priority_index = PRIORITIES.iter().position(|p| Some(*p) == current).unwrap_or(3);
                self.mode = Mode::PriorityChoice;
                return;
            }
            _ => {}
        }

        let (links, pkgs) = collect_ids(self.packages(), &targets);
        let grabber = self.tab.is_grabber();
        let what = describe(&targets);
        let any_enabled = targets.iter().any(|r| crate::model::row_enabled(self.packages(), r));

        let outcome = match action {
            Action::ToggleEnabled => self
                .with_api(|a| a.set_enabled(!any_enabled, &links, &pkgs, grabber))
                .map(|_| format!("{what} {}", if any_enabled { "disabled" } else { "enabled" })),
            Action::Force => {
                self.with_api(|a| a.force_download(&links, &pkgs)).map(|_| format!("{what} forced to start"))
            }
            Action::Resume => self.with_api(|a| a.resume(&links, &pkgs)).map(|_| format!("{what} resumed")),
            Action::Reset => self.with_api(|a| a.reset(&links, &pkgs)).map(|_| format!("{what} reset")),
            Action::Remove => self.with_api(|a| a.remove(&links, &pkgs, grabber)).map(|_| format!("{what} removed")),
            Action::RemoveWith(mode) => {
                self.with_api(|a| a.remove_with_files(&links, &pkgs, mode)).map(|_| match mode {
                    RemoveMode::ListOnly => format!("{what} removed from the list"),
                    RemoveMode::Recycle => format!("{what} removed, files moved to the recycle bin"),
                    RemoveMode::DeleteFiles => format!("{what} removed, files deleted"),
                })
            }
            Action::Cleanup => {
                self.with_api(|a| a.cleanup_finished(&links, &pkgs)).map(|_| "Finished links deleted".into())
            }
            Action::MoveToDownloads => self
                .with_api(|a| a.move_to_downloads(&links, &pkgs))
                .map(|_| format!("{what} moved to the download list")),
            Action::PriorityTo(priority) => self
                .with_api(|a| a.set_priority(priority, &links, &pkgs, grabber))
                .map(|_| format!("{what} set to priority {}", priority.to_lowercase())),
            Action::ToggleExpand
            | Action::Properties
            | Action::Priority
            | Action::Rename
            | Action::Directory
            | Action::ToggleStopMark => unreachable!(),
        };
        self.finish(outcome);
    }

    fn finish(&mut self, outcome: Result<String, String>) {
        match outcome {
            Ok(msg) => {
                self.message = Some((msg, false));
                self.marked.clear();
                if let Some(p) = &self.poller {
                    p.refresh_now();
                }
            }
            Err(e) => self.message = Some((format!("Failed: {e}"), true)),
        }
        self.mode = Mode::List;
    }

    fn toggle_downloads(&mut self) {
        let outcome = if self.snapshot.is_running() {
            self.with_api(|a| a.stop()).map(|_| "Downloads stopped".to_string())
        } else {
            self.with_api(|a| a.start()).map(|_| "Downloads started".to_string())
        };
        self.finish(outcome);
    }

    fn toggle_pause(&mut self) {
        let outcome = match self.snapshot.state.as_str() {
            "RUNNING" => self.with_api(|a| a.pause(true)).map(|_| "Downloads paused".to_string()),
            "PAUSE" => self.with_api(|a| a.pause(false)).map(|_| "Downloads resumed".to_string()),
            _ => Err("downloads are not running".to_string()),
        };
        self.finish(outcome);
    }

    fn submit_add_form(&mut self) {
        let Some(form) = &self.form else { return };
        if !form.is_valid() {
            self.message = Some(("Paste at least one url".into(), true));
            return;
        }
        let req = AddLinks {
            links: form.value("Links").to_string(),
            package_name: form.value("Package name").to_string(),
            destination: form.value("Save to").to_string(),
            extract_password: form.value("Extract password").to_string(),
            download_password: form.value("Download password").to_string(),
            priority: form.value("Priority").to_string(),
            autostart: form.flag("Autostart"),
        };
        let count = req.links.split_whitespace().count();
        let outcome = self
            .with_api(|a| a.add_links(&req))
            .map(|_| format!("Sent {count} url{} to the Link Grabber", if count == 1 { "" } else { "s" }));
        if outcome.is_ok() {
            self.form = None;
        }
        self.finish(outcome);
    }

    /// Downloads stop after the row under the cursor; again on the same
    /// row clears the mark.
    fn toggle_stop_mark(&mut self) {
        if self.tab.is_grabber() {
            return;
        }
        let Some(row) = self.current_row() else { return };
        let name = truncate(row_name(self.packages(), &row), 40);
        let outcome = if row_stop_marked(self.packages(), &row, self.snapshot.stop_mark) {
            self.with_api(|a| a.remove_stop_mark()).map(|_| "Stop mark removed".to_string())
        } else {
            match stop_mark_target(self.packages(), &row) {
                Some(link) => {
                    self.with_api(|a| a.set_stop_mark(link)).map(|_| format!("Downloads will stop after '{name}'"))
                }
                None => Err("the package has no links".to_string()),
            }
        };
        // The mark is not a selection action: keep the marks.
        let marked = std::mem::take(&mut self.marked);
        self.finish(outcome);
        self.marked = marked;
    }

    fn submit_rename(&mut self) {
        let Some(form) = &self.form else { return };
        let name = form.value("Name").trim().to_string();
        if name.is_empty() {
            self.message = Some(("The name cannot be empty".into(), true));
            return;
        }
        let Some(row) = self.target_rows().into_iter().next() else { return };
        let grabber = self.tab.is_grabber();
        let outcome = match row.link {
            None => {
                let uuid = self.packages()[row.package].uuid;
                self.with_api(|a| a.rename_package(uuid, &name, grabber))
            }
            Some(l) => {
                let uuid = self.packages()[row.package].links[l].uuid;
                self.with_api(|a| a.rename_link(uuid, &name, grabber))
            }
        }
        .map(|_| format!("Renamed to '{}'", truncate(&name, 60)));
        if outcome.is_ok() {
            self.form = None;
        }
        self.finish(outcome);
    }

    fn submit_directory(&mut self) {
        let Some(form) = &self.form else { return };
        let dir = form.value("Save to").trim().to_string();
        if dir.is_empty() {
            self.message = Some(("The folder cannot be empty".into(), true));
            return;
        }
        let targets = self.target_rows();
        let (_, pkgs) = collect_ids(self.packages(), &targets);
        let grabber = self.tab.is_grabber();
        let what = describe(&targets);
        let outcome = self
            .with_api(|a| a.set_download_directory(&dir, &pkgs, grabber))
            .map(|_| format!("{what} will be saved to {}", truncate(&dir, 60)));
        if outcome.is_ok() {
            self.form = None;
        }
        self.finish(outcome);
    }

    // --- keys ----------------------------------------------------------

    pub fn handle_key(&mut self, key: Key) {
        if matches!(key, Key::CtrlC) {
            self.should_quit = true;
            return;
        }
        match &mut self.screen {
            Screen::Login { .. } => self.handle_login_key(key),
            Screen::Devices { .. } => self.handle_devices_key(key),
            Screen::Main => match self.mode {
                Mode::List => self.handle_list_key(key),
                Mode::Menu => self.handle_menu_key(key),
                Mode::Properties => {
                    if matches!(key, Key::Esc | Key::Enter | Key::Char('q' | 'p')) {
                        self.mode = Mode::List;
                    }
                }
                Mode::RemoveChoice => match key {
                    Key::Esc | Key::Left | Key::Char('q') => {
                        self.mode = Mode::List;
                        self.message = Some(("Cancelled".into(), false));
                    }
                    Key::Up | Key::Char('k') => self.remove_index = self.remove_index.saturating_sub(1),
                    Key::Down | Key::Char('j') => {
                        self.remove_index = (self.remove_index + 1).min(REMOVE_MODES.len() - 1)
                    }
                    Key::Enter | Key::Right | Key::Char(' ') => {
                        let mode = REMOVE_MODES[self.remove_index];
                        if mode.touches_files() {
                            // Deleting data gets its own yes/no, like the GUI.
                            let targets = self.target_rows();
                            self.message = Some((format!("{} on {}?  [y/N]", mode.label(), describe(&targets)), false));
                            self.mode = Mode::Confirm(Action::RemoveWith(mode));
                        } else {
                            self.run_action(Action::RemoveWith(mode));
                        }
                    }
                    _ => {}
                },
                Mode::PriorityChoice => match key {
                    Key::Esc | Key::Left | Key::Char('q') => {
                        self.mode = Mode::List;
                        self.message = Some(("Cancelled".into(), false));
                    }
                    Key::Up | Key::Char('k') => self.priority_index = self.priority_index.saturating_sub(1),
                    Key::Down | Key::Char('j') => {
                        self.priority_index = (self.priority_index + 1).min(PRIORITIES.len() - 1)
                    }
                    Key::Enter | Key::Right | Key::Char(' ') => {
                        self.run_action(Action::PriorityTo(PRIORITIES[self.priority_index]))
                    }
                    _ => {}
                },
                Mode::Confirm(action) => match key {
                    Key::Char('y' | 'Y') => self.run_action(action),
                    _ => {
                        self.message = Some(("Cancelled".into(), false));
                        self.mode = Mode::List;
                    }
                },
                Mode::Add | Mode::Rename | Mode::Directory => self.handle_form_key(key),
                Mode::Help => {
                    if matches!(key, Key::Esc | Key::Enter | Key::Char('q' | '?' | 'h')) {
                        self.mode = Mode::List;
                    }
                }
            },
        }
    }

    fn handle_login_key(&mut self, key: Key) {
        let Screen::Login { form, .. } = &mut self.screen else { return };
        match key {
            Key::Esc => self.should_quit = true,
            Key::Enter => {
                if form.index == 0 {
                    form.next();
                    return;
                }
                let email = form.value("Email").trim().to_string();
                let password = form.value("Password").to_string();
                if email.is_empty() || password.is_empty() {
                    return;
                }
                self.sign_in(&email, &password);
            }
            other => form_edit(form, other),
        }
    }

    fn handle_devices_key(&mut self, key: Key) {
        let Screen::Devices { devices, index } = &mut self.screen else { return };
        match key {
            Key::Up | Key::Char('k') => *index = index.saturating_sub(1),
            Key::Down | Key::Char('j') => *index = (*index + 1).min(devices.len() - 1),
            Key::Enter => {
                let d = devices[*index].clone();
                self.select_device(d);
            }
            Key::Esc | Key::Char('q') => {
                // From the main screen this is a cancel, not a quit.
                if self.api.is_some() {
                    self.screen = Screen::Main;
                } else {
                    self.should_quit = true;
                }
            }
            _ => {}
        }
    }

    fn handle_list_key(&mut self, key: Key) {
        self.message = None;
        match key {
            Key::Char('q') => self.should_quit = true,
            Key::Tab | Key::BackTab => {
                self.tab = self.tab.other();
                self.cursor = 0;
                self.marked.clear();
                self.rebuild_rows();
            }
            Key::Up | Key::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            Key::Down | Key::Char('j') => {
                self.cursor = (self.cursor + 1).min(self.rows.len().saturating_sub(1));
            }
            Key::Right => {
                if let Some(row) = self.current_row() {
                    self.toggle_expand(row);
                }
            }
            Key::Left => {
                if let Some(row) = self.current_row() {
                    if row.is_package() {
                        let uuid = self.packages()[row.package].uuid;
                        self.expanded.remove(&uuid);
                        self.rebuild_rows();
                    } else if let Some(i) = self.rows.iter().position(|r| r.is_package() && r.package == row.package) {
                        self.cursor = i;
                    }
                }
            }
            Key::Char(' ') => {
                if let Some(row) = self.current_row() {
                    let k = row_key(self.packages(), &row);
                    if !self.marked.remove(&k) {
                        self.marked.insert(k);
                    }
                }
            }
            Key::Char('a') => {
                if self.marked.is_empty() {
                    self.marked = self.rows.iter().map(|r| row_key(self.packages(), r)).collect();
                } else {
                    self.marked.clear();
                }
            }
            Key::Esc => self.marked.clear(),
            Key::Enter => {
                let targets = self.target_rows();
                if !targets.is_empty() {
                    self.menu = context_menu(self.tab, self.packages(), &targets, self.snapshot.stop_mark);
                    self.menu_index = 0;
                    self.mode = Mode::Menu;
                }
            }
            Key::Char('p') => {
                if self.current_row().is_some() {
                    self.mode = Mode::Properties;
                }
            }
            Key::Char('n') => {
                self.form = Some(Form::add_links());
                self.mode = Mode::Add;
            }
            Key::Char('s') => self.toggle_downloads(),
            Key::Char('P') => self.toggle_pause(),
            Key::Char('t') if !self.tab.is_grabber() => self.toggle_stop_mark(),
            Key::Char('d') => self.choose_device(),
            Key::Char('?' | 'h') => self.mode = Mode::Help,
            Key::Char('c') if self.tab.is_grabber() => {
                let pkgs: Vec<i64> = self.snapshot.grabber.iter().map(|p| p.uuid).collect();
                if pkgs.is_empty() {
                    self.message = Some(("Link Grabber is empty".into(), true));
                } else {
                    let n = pkgs.len();
                    let outcome = self
                        .with_api(|a| a.move_to_downloads(&[], &pkgs))
                        .map(|_| format!("{n} package{} moved to the download list", if n == 1 { "" } else { "s" }));
                    self.finish(outcome);
                }
            }
            _ => {}
        }
    }

    fn handle_menu_key(&mut self, key: Key) {
        match key {
            Key::Esc | Key::Left | Key::Char('q') => self.mode = Mode::List,
            Key::Up | Key::Char('k') => self.menu_index = self.menu_index.saturating_sub(1),
            Key::Down | Key::Char('j') => {
                self.menu_index = (self.menu_index + 1).min(self.menu.len().saturating_sub(1))
            }
            Key::Enter | Key::Right | Key::Char(' ') => {
                let Some(entry) = self.menu.get(self.menu_index).cloned() else { return };
                if entry.action == Action::Remove && !self.tab.is_grabber() {
                    // Files may exist on disk: ask what to do with them,
                    // the way the desktop dialog does.
                    self.remove_index = 0;
                    self.mode = Mode::RemoveChoice;
                } else if entry.confirm {
                    let targets = self.target_rows();
                    let subject = if targets.len() == 1 {
                        let name = row_name(self.packages(), &targets[0]);
                        format!("'{}'", truncate(name, 40))
                    } else {
                        describe(&targets)
                    };
                    self.message = Some((format!("{} on {subject}?  [y/N]", entry.label), false));
                    self.mode = Mode::Confirm(entry.action);
                } else {
                    self.run_action(entry.action);
                }
            }
            _ => {}
        }
    }

    /// Keys of the popup forms: add links, rename, download folder.
    fn handle_form_key(&mut self, key: Key) {
        match key {
            Key::Esc => {
                self.form = None;
                self.mode = Mode::List;
                self.message = Some(("Cancelled".into(), false));
            }
            Key::Enter => match self.mode {
                Mode::Rename => self.submit_rename(),
                Mode::Directory => self.submit_directory(),
                _ => self.submit_add_form(),
            },
            other => {
                if let Some(form) = &mut self.form {
                    form_edit(form, other);
                }
            }
        }
    }
}

/// Editing keys shared by every form.
fn form_edit(form: &mut Form, key: Key) {
    match key {
        Key::Tab | Key::Down => form.next(),
        Key::BackTab | Key::Up => form.prev(),
        Key::Left => form.cycle(-1),
        Key::Right => form.cycle(1),
        Key::Backspace => form.backspace(),
        Key::Char(c) => form.type_str(&c.to_string()),
        Key::Paste(text) => {
            // Newlines in a pasted list of urls become separators.
            let flat: String = text.split(['\r', '\n']).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
            form.type_str(&flat);
        }
        _ => {}
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}
