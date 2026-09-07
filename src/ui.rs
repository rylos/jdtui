//! Drawing. Reads `App`, never mutates it.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Paragraph, Row as TRow, Table, TableState, Tabs};

use crate::api::{Link, Package};
use crate::app::{App, HELP, Mode, Screen, truncate};
use crate::model::{
    Extraction, FieldKind, Form, PRIORITIES, Row, Tab, describe, extraction_of, row_enabled, row_key, row_stop_marked,
};

// --- palette ----------------------------------------------------------------
//
// Named colours are whatever the terminal theme decides they are, and a light
// "blue" under white text is unreadable. Both ends of each pair are pinned;
// terminals without truecolor get the closest indexed colour instead.

fn truecolor() -> bool {
    std::env::var("COLORTERM").map(|v| v.contains("truecolor") || v.contains("24bit")).unwrap_or(false)
}

fn rgb(r: u8, g: u8, b: u8, indexed: u8) -> Color {
    if truecolor() { Color::Rgb(r, g, b) } else { Color::Indexed(indexed) }
}

fn selected_style() -> Style {
    Style::new().fg(Color::White).bg(rgb(0x1f, 0x4f, 0x82, 24)).add_modifier(Modifier::BOLD)
}

fn marked_style() -> Style {
    Style::new().fg(Color::White).bg(rgb(0x1a, 0x33, 0x50, 23))
}

fn tab_active_style() -> Style {
    Style::new().fg(rgb(0x10, 0x16, 0x1f, 16)).bg(rgb(0x7c, 0xc7, 0xff, 117)).add_modifier(Modifier::BOLD)
}

fn accent() -> Color {
    Color::Cyan
}

// --- formatting -------------------------------------------------------------

pub fn human_size(bytes: i64) -> String {
    if bytes <= 0 {
        return "0 B".into();
    }
    let mut v = bytes as f64;
    for unit in ["B", "KB", "MB", "GB"] {
        if v < 1024.0 {
            return format!("{v:.2} {unit}");
        }
        v /= 1024.0;
    }
    format!("{v:.2} TB")
}

pub fn human_eta(seconds: i64) -> String {
    if seconds <= 0 {
        return "-".into();
    }
    let (h, m, s) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// Epoch milliseconds as a local-looking timestamp without pulling in a
/// timezone database: UTC, marked as such.
pub fn human_time(epoch_ms: i64) -> String {
    let secs = epoch_ms / 1000;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant), valid for the range we care about.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rem / 3600, (rem % 3600) / 60)
}

fn progress_bar(pct: f64, width: usize) -> Span<'static> {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let bar: String = "━".repeat(filled.min(width)) + &"╌".repeat(width.saturating_sub(filled));
    let color = if pct >= 100.0 { Color::Green } else { accent() };
    Span::styled(bar, Style::new().fg(color))
}

fn package_status(p: &Package) -> String {
    if let Some(s) = &p.status {
        return s.clone();
    }
    if p.is_finished() {
        "Finished".into()
    } else if p.is_running() {
        "Downloading".into()
    } else if !p.is_enabled() {
        "Disabled".into()
    } else {
        "Queued".into()
    }
}

// --- entry point ------------------------------------------------------------

pub fn draw(frame: &mut Frame, app: &App) {
    match &app.screen {
        Screen::Login { form, error } => draw_login(frame, form, error.as_deref()),
        Screen::Devices { devices, index } => draw_devices(frame, devices, *index),
        Screen::Main => draw_main(frame, app),
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

/// The inside of a panel, one cell short on the right. Text that reaches
/// the border reads as if it had been cut off, even when it has not.
fn padded(inner: Rect) -> Rect {
    Rect { width: inner.width.saturating_sub(1), ..inner }
}

fn panel(title: &str, subtitle: Option<&str>) -> Block<'static> {
    let mut b = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(accent()))
        .title(Line::from(format!(" {title} ")).bold());
    if let Some(s) = subtitle {
        b = b.title_bottom(Line::from(format!(" {s} ")).dim().right_aligned());
    }
    b
}

// --- login & devices --------------------------------------------------------

fn draw_login(frame: &mut Frame, form: &Form, error: Option<&str>) {
    let area = centered(frame.area(), 64, 9 + error.is_some() as u16);
    frame.render_widget(Clear, area);
    let block = panel(&form.title, Some("Enter sign in · Tab next field · Esc quit"));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![Line::raw("")];
    lines.extend(form_lines(form));
    if let Some(e) = error {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(format!("  {e}"), Style::new().fg(Color::Red))));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from("  Credentials are saved to the config file after the first successful sign in.").dim());
    frame.render_widget(Paragraph::new(lines), padded(inner));
}

fn draw_devices(frame: &mut Frame, devices: &[crate::myjd::Device], index: usize) {
    let area = centered(frame.area(), 60, (devices.len() as u16 + 4).min(frame.area().height));
    frame.render_widget(Clear, area);
    let block = panel("Choose a JDownloader", Some("Enter select · Esc back"));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows: Vec<TRow> = devices
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let style = if i == index { selected_style() } else { Style::new() };
            TRow::new(vec![
                Cell::from(format!(" {} {}", if i == index { "›" } else { " " }, d.name)),
                Cell::from(Span::styled(d.kind.clone(), Style::new().dim())),
            ])
            .style(style)
        })
        .collect();
    let table = Table::new(rows, [Constraint::Fill(1), Constraint::Length(10)]);
    frame.render_widget(table, inner);
}

/// One line per field: label, then the value, the hint or the masked secret.
fn form_lines(form: &Form) -> Vec<Line<'static>> {
    let label_width = form.fields.iter().map(|f| f.label.len()).max().unwrap_or(10) + 2;
    form.fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let active = i == form.index;
            let label = Span::styled(
                format!("{:>w$}  ", f.label, w = label_width),
                if active { Style::new().bold() } else { Style::new().dim() },
            );
            let mut value = match f.kind {
                FieldKind::Flag => Span::raw(if f.flag { "[x] yes" } else { "[ ] no" }),
                FieldKind::Choice => Span::raw(format!("‹ {} ›", f.text)),
                FieldKind::Secret => Span::raw("•".repeat(f.text.chars().count())),
                FieldKind::Text => {
                    if f.text.is_empty() && !active && !f.hint.is_empty() {
                        Span::styled(f.hint.to_string(), Style::new().dim().italic())
                    } else {
                        Span::raw(f.text.clone())
                    }
                }
            };
            if active && matches!(f.kind, FieldKind::Text | FieldKind::Secret) {
                // The cursor is the char at `f.cursor` drawn in reverse, or
                // a reversed blank past the end: no extra cell, so the text
                // does not shift.
                let shown: Vec<char> = value.content.chars().collect();
                let at = f.cursor.min(shown.len());
                let before: String = shown[..at].iter().collect();
                let under: String = shown.get(at).map(|c| c.to_string()).unwrap_or_else(|| " ".into());
                let after: String = shown.get(at + 1..).map(|s| s.iter().collect()).unwrap_or_default();
                let style = selected_style();
                return Line::from(vec![
                    label,
                    Span::styled(before, style),
                    Span::styled(under, style.add_modifier(Modifier::REVERSED)),
                    Span::styled(after, style),
                ]);
            }
            if active {
                value = value.style(selected_style());
            }
            Line::from(vec![label, value])
        })
        .collect()
}

// --- main screen ------------------------------------------------------------

fn draw_main(frame: &mut Frame, app: &App) {
    let chunks =
        Layout::vertical([Constraint::Length(4), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)])
            .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_tabs(frame, app, chunks[1]);
    draw_body(frame, app, chunks[2]);
    draw_footer(frame, app, chunks[3]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let dl = &app.snapshot.downloads;
    let speed = app.snapshot.speed;
    let total: i64 = dl.iter().filter_map(|p| p.bytes_total).sum();
    let loaded: i64 = dl.iter().filter_map(|p| p.bytes_loaded).sum();
    let running = dl.iter().filter(|p| p.is_running()).count();
    let done = dl.iter().filter(|p| p.is_finished()).count();

    let state = app.snapshot.state.as_str();
    let (state_text, state_color) = match (&app.refresh_error, state) {
        // Red is for trouble only: an idle or stopped list is the normal
        // state of affairs, not something to fix.
        (Some(e), _) => (format!("ERROR: {e}"), Color::Red),
        (None, "") => ("CONNECTING…".to_string(), Color::Yellow),
        (None, "RUNNING") => (state.to_string(), Color::Green),
        (None, "PAUSE") => ("PAUSED".to_string(), Color::Yellow),
        (None, "STOPPED_STATE") => ("STOPPED".to_string(), Color::DarkGray),
        (None, "IDLE") => (state.to_string(), Color::DarkGray),
        (None, other) => (other.to_string(), Color::Yellow),
    };

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(state_color))
        .title(Line::from(format!(" jdtui · {} ", app.device_name)).bold());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // The state line carries the most: the controller, the event channel,
    // how the calls travel and anything waiting on someone.
    let cols = Layout::horizontal([Constraint::Fill(3), Constraint::Fill(2), Constraint::Fill(2)]).split(inner);
    let mut state_line = vec![Span::raw("State: "), Span::styled(state_text, Style::new().fg(state_color).bold())];
    if app.events_live {
        state_line.push(Span::styled(" · live", Style::new().dim()));
    }
    // How the calls reach JDownloader: straight to it, or through the
    // My.JDownloader relay. Shown once the first snapshot is in.
    if !app.snapshot.state.is_empty() {
        match &app.snapshot.direct {
            Some(_) => state_line.push(Span::styled(" · ⇄ direct", Style::new().fg(Color::Green).dim())),
            None => state_line.push(Span::styled(" · ☁ relay", Style::new().dim())),
        }
    }
    // What is waiting on someone or something, next to the state.
    let captchas = app.snapshot.captchas.len();
    if captchas > 0 {
        state_line.push(Span::raw("  |  "));
        state_line.push(Span::styled(
            format!("{captchas} captcha{} waiting", if captchas == 1 { "" } else { "s" }),
            Style::new().fg(Color::Red).bold(),
        ));
    }
    let extracting = app.snapshot.extracting.len();
    if extracting > 0 {
        state_line.push(Span::raw("  |  "));
        state_line.push(Span::styled(format!("Extracting: {extracting}"), Style::new().fg(Color::Yellow).bold()));
    }
    let left = vec![
        Line::from(state_line),
        Line::from(vec![
            Span::raw("Packages: "),
            Span::styled(dl.len().to_string(), Style::new().bold()),
            Span::raw("  |  Running: "),
            Span::styled(running.to_string(), Style::new().fg(Color::Green).bold()),
            Span::raw("  |  Done: "),
            Span::styled(done.to_string(), Style::new().dim()),
        ]),
    ];
    let mid = vec![
        Line::from(vec![
            Span::raw("Speed: "),
            Span::styled(format!("{}/s", human_size(speed)), Style::new().fg(accent()).bold()),
        ]),
        Line::from(vec![Span::styled("Loaded: ", Style::new().dim()), Span::raw(human_size(loaded))]),
    ];
    let right = vec![
        Line::from(vec![Span::styled("Total: ", Style::new().dim()), Span::raw(human_size(total))]),
        Line::from(vec![
            Span::styled("Left:  ", Style::new().dim()),
            Span::styled(human_size((total - loaded).max(0)), Style::new().fg(Color::Yellow)),
        ]),
    ];
    frame.render_widget(Paragraph::new(left), cols[0]);
    frame.render_widget(Paragraph::new(mid), cols[1]);
    frame.render_widget(Paragraph::new(right), cols[2]);
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles = vec![
        Line::from(format!(" Downloads ({}) ", app.snapshot.downloads.len())),
        Line::from(format!(
            " Link Grabber ({}){} ",
            app.snapshot.grabber.len(),
            if app.snapshot.collecting { " · crawling…" } else { "" }
        )),
    ];
    let tabs = Tabs::new(titles)
        .select(if app.tab == Tab::Downloads { 0 } else { 1 })
        .style(Style::new().dim())
        .highlight_style(tab_active_style())
        .divider(" ");
    frame.render_widget(tabs, area);
}

fn draw_body(frame: &mut Frame, app: &App, area: Rect) {
    let side_width = match app.mode {
        Mode::Menu | Mode::DeviceMenu => 34,
        Mode::RemoveChoice | Mode::Confirm(crate::model::Action::RemoveWith(_)) => 46,
        Mode::PriorityChoice => 34,
        Mode::VariantChoice => 44,
        Mode::Properties => 62,
        _ => 0,
    };
    let (list_area, side_area) = if side_width > 0 && area.width > side_width + 40 {
        let cols = Layout::horizontal([Constraint::Fill(1), Constraint::Length(side_width)]).split(area);
        (cols[0], Some(cols[1]))
    } else {
        (area, None)
    };

    if matches!(app.mode, Mode::Options | Mode::OptionChoice | Mode::OptionEdit) {
        draw_list(frame, app, list_area);
        draw_options(frame, app, area);
        if app.mode == Mode::OptionChoice {
            draw_option_choice(frame, app, area);
        }
        if app.mode == Mode::OptionEdit
            && let Some(form) = &app.form
        {
            let popup = centered(area, 70, 6);
            frame.render_widget(Clear, popup);
            let block = panel(&form.title, Some("Enter apply · Ctrl-U clear · Esc cancel"));
            let inner = block.inner(popup);
            frame.render_widget(block, popup);
            let mut lines = vec![Line::raw("")];
            lines.extend(form_lines(form));
            frame.render_widget(Paragraph::new(lines), padded(inner));
        }
        return;
    }

    if matches!(
        app.mode,
        Mode::Add | Mode::Rename | Mode::Directory | Mode::NewPackage | Mode::ArchivePassword | Mode::Filter
    ) && let Some(form) = &app.form
    {
        draw_list(frame, app, list_area);
        // The add form gets a line saying where the files will go.
        let preview = (app.mode == Mode::Add).then(|| match &app.folder_policy {
            Some(policy) => {
                let path = crate::model::resolve_folder(form.value("Save to"), form.value("Package name"), policy);
                let why = if policy.subfolder_by_package && !form.value("Save to").contains("<jd:packagename>") {
                    "  (JDownloader adds the subfolder)"
                } else {
                    ""
                };
                Line::from(vec![
                    Span::styled("  Files will go to  ", Style::new().dim()),
                    Span::styled(path, Style::new().fg(Color::Yellow)),
                    Span::styled(why, Style::new().dim()),
                ])
            }
            None => {
                Line::from(Span::styled("  Could not read where JDownloader will put the files", Style::new().dim()))
            }
        });
        let popup = centered(area, 90, form.fields.len() as u16 + 6 + if preview.is_some() { 2 } else { 0 });
        frame.render_widget(Clear, popup);
        let hint = match app.mode {
            _ if form.active_label() == "Save to" => "Enter apply · Ctrl-F folders · Ctrl-U clear · Esc cancel",
            Mode::Add if form.is_valid() => "Enter add · Tab/↑↓ field · ←→ move / change · Esc cancel",
            Mode::Add => "Paste at least one url · Tab/↑↓ field · Esc cancel",
            Mode::Filter => "Enter keep · Esc clear",
            _ => "Enter apply · ←→ Home End move · Ctrl-U clear · Esc cancel",
        };
        let block = panel(&form.title, Some(hint));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        let mut lines = vec![Line::raw("")];
        lines.extend(form_lines(form));
        if let Some(preview) = preview {
            lines.push(Line::raw(""));
            lines.push(preview);
        }
        frame.render_widget(Paragraph::new(lines), padded(inner));
        return;
    }

    if app.mode == Mode::Help {
        draw_list(frame, app, list_area);
        // Everything but the footer, so 24 rows hold the whole list.
        let whole = frame.area();
        draw_help(frame, Rect::new(whole.x, whole.y, whole.width, whole.height.saturating_sub(1)));
        return;
    }
    if app.mode == Mode::Urls {
        draw_list(frame, app, list_area);
        draw_urls(frame, app, area);
        return;
    }
    if app.mode == Mode::Accounts {
        draw_list(frame, app, list_area);
        draw_accounts(frame, app, area);
        return;
    }
    if app.mode == Mode::FolderChoice {
        draw_list(frame, app, list_area);
        draw_folders(frame, app, area);
        return;
    }
    if app.mode == Mode::About {
        draw_list(frame, app, list_area);
        draw_about(frame, app, area);
        return;
    }

    draw_list(frame, app, list_area);
    if let Some(side) = side_area {
        match app.mode {
            Mode::Menu | Mode::DeviceMenu => draw_menu(frame, app, side),
            Mode::RemoveChoice | Mode::Confirm(crate::model::Action::RemoveWith(_)) => {
                draw_remove_choice(frame, app, side)
            }
            Mode::PriorityChoice => draw_priority_choice(frame, app, side),
            Mode::VariantChoice => draw_variant_choice(frame, app, side),
            Mode::Properties => draw_properties(frame, app, side),
            _ => {}
        }
    }
}

fn draw_list(frame: &mut Frame, app: &App, area: Rect) {
    let packages = crate::model::packages_of(&app.snapshot, app.tab);
    let title = match app.tab {
        Tab::Downloads => "Downloads",
        Tab::Grabber => "Link Grabber",
    };
    let mut title_line = Line::from(vec![Span::raw(format!(" {title} "))]);
    if !app.marked.is_empty() {
        title_line.push_span(Span::styled(format!("({} selected) ", app.marked.len()), Style::new().fg(accent())));
    }
    if !app.filter.is_empty() {
        title_line.push_span(Span::styled(
            format!("(filter: {}) ", crate::app::truncate(&app.filter, 30)),
            Style::new().fg(Color::Yellow),
        ));
    }
    if app.tab == Tab::Downloads
        && let Some(uuid) = app.snapshot.stop_mark
    {
        let name = packages
            .iter()
            .find_map(|p| {
                if p.uuid == uuid {
                    Some(p.name.as_str())
                } else {
                    p.links.iter().find(|l| l.uuid == uuid).map(|l| l.name.as_str())
                }
            })
            .unwrap_or("a hidden entry");
        title_line.push_span(Span::styled(
            format!("(stops after '{}') ", crate::app::truncate(name, 30)),
            Style::new().fg(Color::Red),
        ));
    }
    let border_color = if app.refresh_error.is_some() { Color::Yellow } else { Color::DarkGray };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border_color))
        .title(title_line);

    if app.rows.is_empty() {
        let text = if app.snapshot.state.is_empty() {
            "Loading…"
        } else if app.tab == Tab::Downloads {
            "No packages in the download list"
        } else {
            "Link Grabber is empty"
        };
        frame.render_widget(Paragraph::new(Line::from(text).dim().italic()).block(block), area);
        return;
    }

    let (header, widths, rows) = match app.tab {
        Tab::Downloads => downloads_rows(app, packages),
        Tab::Grabber => grabber_rows(app, packages),
    };
    let table = Table::new(rows, widths)
        .header(TRow::new(header).style(Style::new().bold()))
        .block(block)
        .column_spacing(1)
        .row_highlight_style(selected_style());
    // Inner height minus the header row: what a page is.
    app.page.set(area.height.saturating_sub(3).max(1) as usize);
    let mut state = TableState::default().with_selected(Some(app.cursor));
    frame.render_stateful_widget(table, area, &mut state);
}

fn row_base_style(app: &App, packages: &[Package], row: &Row) -> Style {
    let base = if app.marked.contains(&row_key(packages, row)) { marked_style() } else { Style::new() };
    // A disabled row fades, as in the web interface: grey where the cells
    // set no colour of their own, dimmed where they do.
    if row_enabled(packages, row) { base } else { base.fg(Color::DarkGray).add_modifier(Modifier::DIM) }
}

fn mark(app: &App, packages: &[Package], row: &Row) -> &'static str {
    if app.marked.contains(&row_key(packages, row)) { "✓" } else { " " }
}

/// The "downloads stop after this" badge, empty on every other row.
fn stop_mark(app: &App, packages: &[Package], row: &Row) -> Span<'static> {
    if row_stop_marked(packages, row, app.snapshot.stop_mark) {
        Span::styled("  ■ stop", Style::new().fg(Color::Red).bold())
    } else {
        Span::raw("")
    }
}

/// The Status cell of a row. An archive jdtui can name itself wins over
/// JDownloader's own sentence, which is written in its language and is
/// usually too long for the column.
fn status_span(packages: &[Package], row: &Row, extraction: Option<Extraction>) -> Span<'static> {
    if let Some(state) = extraction {
        let style = match state {
            Extraction::Running => Style::new().fg(Color::Yellow).bold(),
            Extraction::Queued => Style::new().fg(Color::Yellow).dim(),
            Extraction::Done => Style::new().fg(Color::Green),
            Extraction::Failed => Style::new().fg(Color::Red).bold(),
        };
        return Span::styled(state.label(), style);
    }
    let package = &packages[row.package];
    match row.link {
        None => Span::styled(
            package_status(package),
            status_style(package.is_finished(), package.is_running(), package.is_enabled(), false),
        ),
        Some(l) => {
            let link = &package.links[l];
            let text = link.status.clone().unwrap_or_else(|| {
                if !link.is_enabled() {
                    "Disabled".into()
                } else if link.is_finished() {
                    "Finished".into()
                } else if link.running.unwrap_or(false) {
                    "Downloading".into()
                } else {
                    "-".into()
                }
            });
            let running = link.running.unwrap_or(false);
            Span::styled(text, status_style(link.is_finished(), running, link.is_enabled(), true))
        }
    }
}

/// How long the row still has to wait, in seconds.
///
/// JDownloader counts that wait in seconds while a link is downloading,
/// but in milliseconds while its archive is being unpacked, and it stops
/// reporting the package's own wait then, putting one on each of its
/// links instead. Both were measured against the clock on a live
/// extraction: the figure fell by about a thousand a second.
fn eta_seconds(raw: i64, extracting: bool, running: bool) -> i64 {
    if extracting {
        return raw / 1000;
    }
    // A row that is neither downloading nor being unpacked has no wait to
    // report, and the figure it still carries would be read in the wrong
    // unit: better nothing than "118h" for an archive with a minute to go.
    if running { raw } else { 0 }
}

fn package_eta(package: &Package, extraction: Option<Extraction>) -> i64 {
    if extraction == Some(Extraction::Running) {
        let longest = package.links.iter().filter_map(|l| l.eta).max().unwrap_or(0);
        return eta_seconds(longest, true, false);
    }
    package.eta.unwrap_or(0)
}

/// Colour by state, never by the words: JDownloader writes those in its
/// own language, so "Completato" and "Download" would look alike. Green is
/// done with, cyan is moving, and a disabled row keeps the grey the whole
/// row is drawn in.
fn status_style(finished: bool, running: bool, enabled: bool, link: bool) -> Style {
    let base = if link { Style::new().dim() } else { Style::new() };
    if !enabled {
        return base;
    }
    if finished {
        return base.fg(Color::Green);
    }
    if running {
        return base.fg(accent());
    }
    if link { base } else { base.fg(Color::Yellow) }
}

fn downloads_rows<'a>(app: &'a App, packages: &'a [Package]) -> (Vec<&'static str>, Vec<Constraint>, Vec<TRow<'a>>) {
    let header = vec!["Name", "Links", "Size", "Status", "Progress", "%", "Speed", "ETA"];
    // Status carries sentences JDownloader wrote, so it gets a share of
    // its own rather than what is left over.
    let widths = vec![
        Constraint::Fill(3),
        Constraint::Length(6),
        Constraint::Length(20),
        Constraint::Fill(2),
        Constraint::Length(18),
        Constraint::Length(5),
        Constraint::Length(11),
        Constraint::Length(8),
    ];
    let rows = app
        .rows
        .iter()
        .map(|row| {
            let pkg = &packages[row.package];
            let style = row_base_style(app, packages, row);
            let extraction = extraction_of(packages, row, &app.snapshot.extracting);
            match row.link {
                None => {
                    let marker = if app.expanded.contains(&pkg.uuid) { "▼" } else { "▶" };
                    let pct = pkg.progress();
                    TRow::new(vec![
                        Cell::from(Line::from(vec![
                            Span::raw(format!("{}{marker} {}", mark(app, packages, row), pkg.name)),
                            stop_mark(app, packages, row),
                        ])),
                        Cell::from(Span::styled(format!("[{}]", pkg.child_count.unwrap_or(0)), Style::new().dim())),
                        Cell::from(Span::styled(
                            format!(
                                "{}/{}",
                                human_size(pkg.bytes_loaded.unwrap_or(0)),
                                human_size(pkg.bytes_total.unwrap_or(0))
                            ),
                            Style::new().dim(),
                        )),
                        Cell::from(status_span(packages, row, extraction)),
                        Cell::from(progress_bar(pct, 18)),
                        Cell::from(format!("{pct:.0}%")),
                        Cell::from(Span::styled(
                            pkg.speed
                                .filter(|s| *s > 0)
                                .map(|s| format!("{}/s", human_size(s)))
                                .unwrap_or_else(|| "-".into()),
                            Style::new().fg(accent()),
                        )),
                        Cell::from(Span::styled(
                            human_eta(package_eta(pkg, extraction)),
                            Style::new().fg(Color::Green),
                        )),
                    ])
                    .style(style)
                }
                Some(l) => {
                    let link: &Link = &pkg.links[l];
                    let pct = link.progress();
                    TRow::new(vec![
                        Cell::from(Line::from(vec![
                            Span::styled(format!(" {}  └ {}", mark(app, packages, row), link.name), Style::new().dim()),
                            stop_mark(app, packages, row),
                        ])),
                        Cell::from(""),
                        Cell::from(Span::styled(
                            format!(
                                "{}/{}",
                                human_size(link.bytes_loaded.unwrap_or(0)),
                                human_size(link.bytes_total.unwrap_or(0))
                            ),
                            Style::new().dim(),
                        )),
                        Cell::from(status_span(packages, row, extraction)),
                        Cell::from(""),
                        Cell::from(Span::styled(format!("{pct:.0}%"), Style::new().dim())),
                        Cell::from(
                            link.speed.filter(|s| *s > 0).map(|s| format!("{}/s", human_size(s))).unwrap_or_default(),
                        ),
                        Cell::from(Span::styled(
                            human_eta(eta_seconds(
                                link.eta.unwrap_or(0),
                                extraction == Some(Extraction::Running),
                                link.running.unwrap_or(false),
                            )),
                            Style::new().dim(),
                        )),
                    ])
                    .style(style)
                }
            }
        })
        .collect();
    (header, widths, rows)
}

fn grabber_rows<'a>(app: &'a App, packages: &'a [Package]) -> (Vec<&'static str>, Vec<Constraint>, Vec<TRow<'a>>) {
    let header = vec!["Name", "Links", "Size", "Online", "Offline", "Hoster", "Save to"];
    let widths = vec![
        Constraint::Fill(3),
        Constraint::Length(6),
        Constraint::Length(12),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Fill(1),
        Constraint::Fill(2),
    ];
    let rows = app
        .rows
        .iter()
        .map(|row| {
            let pkg = &packages[row.package];
            let style = row_base_style(app, packages, row);
            match row.link {
                None => {
                    let marker = if app.expanded.contains(&pkg.uuid) { "▼" } else { "▶" };
                    TRow::new(vec![
                        Cell::from(format!("{}{marker} {}", mark(app, packages, row), pkg.name)),
                        Cell::from(Span::styled(format!("[{}]", pkg.child_count.unwrap_or(0)), Style::new().dim())),
                        Cell::from(Span::styled(human_size(pkg.bytes_total.unwrap_or(0)), Style::new().dim())),
                        Cell::from(Span::styled(
                            pkg.available_online_count.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
                            Style::new().fg(Color::Green),
                        )),
                        Cell::from(Span::styled(
                            pkg.available_offline_count.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
                            Style::new().fg(Color::Red),
                        )),
                        Cell::from(Span::styled(
                            pkg.hosts.as_ref().map(|h| h.join(", ")).unwrap_or_else(|| "-".into()),
                            Style::new().fg(Color::Magenta),
                        )),
                        Cell::from(Span::styled(pkg.save_to.clone().unwrap_or_else(|| "-".into()), Style::new().dim())),
                    ])
                    .style(style)
                }
                Some(l) => {
                    let link = &pkg.links[l];
                    let availability = link.availability.clone().unwrap_or_else(|| "-".into());
                    let online = availability == "ONLINE";
                    let mut name = vec![Span::styled(
                        format!(" {}  └ {}", mark(app, packages, row), link.name),
                        Style::new().dim(),
                    )];
                    if let Some(v) = link.variant.as_ref().and_then(|v| v.name.clone()) {
                        name.push(Span::styled(format!("  [{v}]"), Style::new().fg(Color::Magenta)));
                    }
                    TRow::new(vec![
                        Cell::from(Line::from(name)),
                        Cell::from(""),
                        Cell::from(Span::styled(human_size(link.bytes_total.unwrap_or(0)), Style::new().dim())),
                        Cell::from(Span::styled(
                            if online { availability.clone() } else { String::new() },
                            Style::new().fg(Color::Green),
                        )),
                        Cell::from(Span::styled(
                            if online { String::new() } else { availability },
                            Style::new().fg(Color::Red),
                        )),
                        Cell::from(link.host.clone().unwrap_or_else(|| "-".into())),
                        Cell::from(Span::styled(link.url.clone().unwrap_or_default(), Style::new().dim())),
                    ])
                    .style(style)
                }
            }
        })
        .collect();
    (header, widths, rows)
}

fn draw_remove_choice(frame: &mut Frame, app: &App, area: Rect) {
    let targets = app.target_rows();
    let block = panel(&format!("Remove {}", describe(&targets)), Some("Enter choose · Esc cancel"));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![Line::from(Span::styled(" What about the files already on disk?", Style::new().dim()))];
    lines.push(Line::raw(""));
    for (i, mode) in crate::app::REMOVE_MODES.iter().enumerate() {
        let selected = i == app.remove_index;
        let style = if selected {
            selected_style()
        } else if mode.touches_files() {
            Style::new().fg(Color::Red)
        } else {
            Style::new()
        };
        lines.push(Line::from(Span::styled(format!(" {} {}", if selected { "›" } else { " " }, mode.label()), style)));
    }
    frame.render_widget(Paragraph::new(lines), padded(inner));
}

fn draw_priority_choice(frame: &mut Frame, app: &App, area: Rect) {
    let targets = app.target_rows();
    let block = panel(&format!("Priority of {}", describe(&targets)), Some("Enter choose · Esc cancel"));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = PRIORITIES
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let selected = i == app.priority_index;
            let style = if selected { selected_style() } else { Style::new() };
            let label = format!("{}{}", &p[..1], p[1..].to_lowercase());
            Line::from(Span::styled(format!(" {} {label}", if selected { "›" } else { " " }), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), padded(inner));
}

fn draw_variant_choice(frame: &mut Frame, app: &App, area: Rect) {
    let block = panel("Variant", Some("Enter choose · Esc cancel"));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines: Vec<Line> = app
        .variants
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let selected = i == app.variant_index;
            let style = if selected { selected_style() } else { Style::new() };
            let name = v.name.clone().or_else(|| v.id.clone()).unwrap_or_default();
            Line::from(Span::styled(format!(" {} {name}", if selected { "›" } else { " " }), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), padded(inner));
}

fn draw_menu(frame: &mut Frame, app: &App, area: Rect) {
    let title = if app.mode == Mode::DeviceMenu { app.device_name.clone() } else { describe(&app.target_rows()) };
    let block = panel(&title, Some("Enter run · Esc close"));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines: Vec<Line> = app
        .menu
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let selected = i == app.menu_index;
            let style = if selected {
                selected_style()
            } else if e.confirm {
                Style::new().fg(Color::Red)
            } else {
                Style::new()
            };
            Line::from(Span::styled(format!(" {} {}", if selected { "›" } else { " " }, e.label), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), padded(inner));
}

fn draw_properties(frame: &mut Frame, app: &App, area: Rect) {
    let Some(row) = app.current_row() else { return };
    let packages = crate::model::packages_of(&app.snapshot, app.tab);
    let pkg = &packages[row.package];

    let mut fields: Vec<(&str, String)> = Vec::new();
    let mut push = |k: &'static str, v: Option<String>| {
        if let Some(v) = v.filter(|s| !s.is_empty()) {
            fields.push((k, v));
        }
    };
    match row.link {
        None => {
            push("Name", Some(pkg.name.clone()));
            push("UUID", Some(pkg.uuid.to_string()));
            push("Type", Some("Package".into()));
            push("Status", pkg.status.clone());
            push("Size", pkg.bytes_total.map(human_size));
            push("Loaded", pkg.bytes_loaded.map(human_size));
            push("Links", pkg.child_count.map(|n| n.to_string()));
            push(
                "Online / Offline",
                pkg.available_online_count.map(|n| format!("{n} / {}", pkg.available_offline_count.unwrap_or(0))),
            );
            push("Hosts", pkg.hosts.as_ref().map(|h| h.join(", ")));
            push("Enabled", Some(if pkg.is_enabled() { "yes" } else { "no" }.into()));
            push("Priority", pkg.priority.clone());
            push("Speed", pkg.speed.filter(|s| *s > 0).map(|s| format!("{}/s", human_size(s))));
            push("ETA", pkg.eta.filter(|e| *e > 0).map(human_eta));
            push("Save to", pkg.save_to.clone());
            push("Comment", pkg.comment.clone());
        }
        Some(l) => {
            let link = &pkg.links[l];
            push("Name", Some(link.name.clone()));
            push("UUID", Some(link.uuid.to_string()));
            push("Type", Some("Link".into()));
            push("Status", link.status.clone());
            push("Extraction", link.extraction_status.clone());
            push("Size", link.bytes_total.map(human_size));
            push("Loaded", link.bytes_loaded.map(human_size));
            push("Availability", link.availability.clone());
            push("Variant", link.variant.as_ref().and_then(|v| v.name.clone()));
            push("Host", link.host.clone());
            push("Enabled", Some(if link.is_enabled() { "yes" } else { "no" }.into()));
            push("Priority", link.priority.clone());
            push("Speed", link.speed.filter(|s| *s > 0).map(|s| format!("{}/s", human_size(s))));
            push("ETA", link.eta.filter(|e| *e > 0).map(human_eta));
            push("Save to", pkg.save_to.clone());
            push("URL", link.url.clone());
            push("Added", link.added_date.filter(|d| *d > 0).map(human_time));
            push("Finished", link.finished_date.filter(|d| *d > 0).map(human_time));
            push("Comment", link.comment.clone());
        }
    }

    let block = panel("Properties", Some("Esc close"));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows: Vec<TRow> = fields
        .into_iter()
        .map(|(k, v)| TRow::new(vec![Cell::from(Span::styled(k, Style::new().dim())), Cell::from(Text::from(v))]))
        .collect();
    let table = Table::new(rows, [Constraint::Length(16), Constraint::Fill(1)]).column_spacing(2);
    frame.render_widget(table, inner);
}

/// The key reference, over the whole frame but the footer: the sections
/// go in two columns when the terminal is wide enough, the key column of
/// each sized to its own keys, and blank lines between sections only when
/// the height allows.
fn draw_help(frame: &mut Frame, area: Rect) {
    let two_columns = area.width >= 120;
    let available = area.height.saturating_sub(4) as usize;
    // Which sections go to which column: split where the line count halves.
    let split = |gap: usize| -> Vec<Vec<usize>> {
        let mut columns: Vec<Vec<usize>> = vec![Vec::new(), Vec::new()];
        let total: usize = HELP.iter().map(|(_, k)| k.len() + 1 + gap).sum();
        let mut filled = 0;
        for (i, (_, keys)) in HELP.iter().enumerate() {
            let col = if two_columns && filled >= total.div_ceil(2) { 1 } else { 0 };
            columns[col].push(i);
            filled += keys.len() + 1 + gap;
        }
        columns
    };
    let layout = |gap: usize| -> Vec<Vec<Line<'static>>> {
        split(gap)
            .into_iter()
            .map(|sections| {
                let key_width =
                    sections.iter().flat_map(|&i| HELP[i].1.iter()).map(|(k, _)| k.chars().count()).max().unwrap_or(8);
                let mut lines: Vec<Line> = Vec::new();
                for &i in &sections {
                    let (section, keys) = HELP[i];
                    if !lines.is_empty() {
                        lines.extend(std::iter::repeat_n(Line::raw(""), gap));
                    }
                    lines.push(Line::from(Span::styled(format!(" {section}"), Style::new().fg(accent()).bold())));
                    for (key, what) in keys {
                        lines.push(Line::from(vec![
                            Span::styled(format!("   {key:<w$}  ", w = key_width), Style::new().bold()),
                            Span::raw(what.to_string()),
                        ]));
                    }
                }
                lines
            })
            .collect()
    };
    let tallest = |columns: &[Vec<Line>]| columns.iter().map(|c| c.len()).max().unwrap_or(0);
    let mut columns = layout(1);
    // Tight: no blank lines between sections, and none above them either.
    let mut top_blank = 1;
    if tallest(&columns) > available {
        columns = layout(0);
        top_blank = 0;
    }

    let column_width = 58u16;
    let width = if two_columns { column_width * 2 + 2 } else { column_width + 2 };
    let height = ((tallest(&columns) + top_blank) as u16 + 2).min(area.height);
    let popup = centered(area, width, height);
    frame.render_widget(Clear, popup);
    let block = panel("Keys", Some("Esc close"));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let cols = Layout::horizontal([Constraint::Length(column_width), Constraint::Fill(1)]).split(inner);
    for (i, lines) in columns.into_iter().enumerate() {
        let mut text = vec![Line::raw(""); top_blank];
        text.extend(lines);
        // Each column stops one cell short too, so nothing leans against
        // the column beside it either.
        frame.render_widget(Paragraph::new(text), padded(cols[i]));
    }
}

fn draw_urls(frame: &mut Frame, app: &App, area: Rect) {
    let n = app.urls.len();
    let height = (n as u16 + 4).min(area.height);
    let popup = centered(area, area.width.saturating_sub(8).min(120), height);
    frame.render_widget(Clear, popup);
    let block = panel(&format!("{n} url{}, copied to the clipboard", if n == 1 { "" } else { "s" }), Some("Esc close"));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let mut lines = vec![Line::raw("")];
    lines.extend(app.urls.iter().map(|u| Line::from(format!(" {u}"))));
    frame.render_widget(Paragraph::new(lines), padded(inner));
}

fn draw_folders(frame: &mut Frame, app: &App, area: Rect) {
    let n = app.folders.len();
    let height = (n as u16 + 4).min(area.height);
    let popup = centered(area, area.width.saturating_sub(8).min(90), height);
    frame.render_widget(Clear, popup);
    let block = panel("Download folders", Some("Enter pick · Esc back"));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let mut lines = vec![Line::raw("")];
    lines.extend(app.folders.iter().enumerate().map(|(i, f)| {
        let selected = i == app.folder_index;
        let style = if selected { selected_style() } else { Style::new() };
        Line::from(Span::styled(format!(" {} {f}", if selected { "›" } else { " " }), style))
    }));
    frame.render_widget(Paragraph::new(lines), padded(inner));
}

/// How long JDownloader has been up, from milliseconds.
fn human_uptime(ms: i64) -> String {
    let (d, h, m) = (ms / 86_400_000, (ms / 3_600_000) % 24, (ms / 60_000) % 60);
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

/// The JDownloader and the machine under it. Everything here is read once,
/// when the panel opens.
fn draw_about(frame: &mut Frame, app: &App, area: Rect) {
    let Some(about) = &app.about else { return };
    let label = |text: &str| Span::styled(format!("  {text:<11}"), Style::new().dim());
    let mut lines: Vec<Line> = Vec::new();
    let section = |lines: &mut Vec<Line>, name: &str| {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(format!(" {name}"), Style::new().bold())));
    };

    section(&mut lines, "JDownloader");
    lines.push(Line::from(vec![label("Device"), Span::raw(app.device_name.clone())]));
    lines.push(Line::from(vec![
        label("Version"),
        Span::raw(format!("{}  ·  core revision {}", about.version, about.core_revision)),
    ]));
    lines.push(Line::from(vec![label("Uptime"), Span::raw(human_uptime(about.uptime))]));
    lines.push(Line::from(vec![
        label("Updates"),
        if about.update_available {
            Span::styled("an update is waiting to be installed", Style::new().fg(Color::Yellow))
        } else {
            Span::styled("up to date", Style::new().fg(Color::Green))
        },
    ]));
    lines.push(Line::from(vec![
        label("Calls go"),
        match &about.direct {
            Some(address) => Span::styled(format!("direct to {address}"), Style::new().fg(Color::Green)),
            None => Span::raw("through the My.JDownloader relay"),
        },
    ]));

    let sys = &about.system;
    section(&mut lines, "Machine");
    let mut system = sys.os_string.clone().unwrap_or_else(|| "unknown".into());
    if let Some(name) = &sys.operating_system {
        system.push_str(&format!("  ·  {name}"));
    }
    if let Some(arch) = &sys.arch_string {
        system.push_str(&format!("  ·  {arch}"));
    }
    let mut how = Vec::new();
    if sys.docker.unwrap_or(false) {
        how.push("in a container");
    }
    if sys.snap.unwrap_or(false) {
        how.push("from a snap");
    }
    if sys.headless.unwrap_or(false) {
        how.push("headless");
    }
    if !how.is_empty() {
        system.push_str(&format!("  ·  {}", how.join("  ·  ")));
    }
    lines.push(Line::from(vec![label("System"), Span::raw(system)]));
    let java = match (&sys.java_version_string, &sys.java_name) {
        (Some(v), Some(n)) => format!("{v}  ·  {n}"),
        (Some(v), None) => v.clone(),
        _ => "unknown".into(),
    };
    lines.push(Line::from(vec![label("Java"), Span::raw(java)]));
    if let (Some(used), Some(max)) = (sys.heap_used, sys.heap_max) {
        lines.push(Line::from(vec![
            label("Memory"),
            Span::raw(format!("{} of {} available to it", human_size(used), human_size(max))),
        ]));
    }

    if !about.storage.is_empty() {
        section(&mut lines, "Storage");
        for disk in &about.storage {
            let path = disk.path.clone().unwrap_or_default();
            let text = match (disk.free, disk.size) {
                (Some(free), Some(size)) if size > 0 => {
                    format!("{} free of {}", human_size(free), human_size(size))
                }
                _ => "unknown".into(),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {path:<11}"), Style::new().fg(accent())),
                Span::raw(text),
            ]));
        }
    }

    let height = (lines.len() as u16 + 3).min(area.height);
    let popup = centered(area, area.width.saturating_sub(8).min(78), height);
    frame.render_widget(Clear, popup);
    let block = panel("About", Some("Esc close"));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    // On a short terminal, say what did not fit rather than cutting a
    // line off mid-sentence.
    let room = inner.height as usize;
    if lines.len() > room && room > 0 {
        let hidden = lines.len() - room + 1;
        lines.truncate(room.saturating_sub(1));
        lines.push(Line::from(Span::styled(format!("  … {hidden} more line(s)"), Style::new().dim().italic())));
    }
    frame.render_widget(Paragraph::new(lines), padded(inner));
}

fn draw_accounts(frame: &mut Frame, app: &App, area: Rect) {
    let n = app.accounts.len();
    let height = (n as u16 + 5).min(area.height);
    let popup = centered(area, area.width.saturating_sub(8).min(110), height);
    frame.render_widget(Clear, popup);
    let block = panel("Accounts", Some("Enter enable/disable · r refresh · R refresh all · Esc close"));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if n == 0 {
        frame.render_widget(
            Paragraph::new(Line::from(" No accounts on this JDownloader").dim().italic()),
            padded(inner),
        );
        return;
    }
    let rows: Vec<TRow> = app
        .accounts
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let selected = i == app.account_index;
            let enabled = a.enabled.unwrap_or(false);
            let (status, color) = match (enabled, a.valid, &a.error_string) {
                (false, _, _) => ("disabled".to_string(), Color::DarkGray),
                (true, _, Some(e)) if !e.is_empty() => (e.clone(), Color::Red),
                (true, Some(false), _) => ("invalid".to_string(), Color::Red),
                (true, Some(true), _) => ("ok".to_string(), Color::Green),
                (true, None, _) => ("unchecked".to_string(), Color::Yellow),
            };
            let traffic = match (a.traffic_left, a.traffic_max) {
                (Some(left), Some(max)) if max > 0 => format!("{} / {}", human_size(left), human_size(max)),
                (Some(left), _) if left >= 0 => human_size(left),
                _ => "unlimited".to_string(),
            };
            let until = a.valid_until.filter(|d| *d > 0).map(human_time).unwrap_or_else(|| "-".into());
            let style = if selected { selected_style() } else { Style::new() };
            TRow::new(vec![
                Cell::from(format!(" {} {}", if selected { "›" } else { " " }, a.hostname.clone().unwrap_or_default())),
                Cell::from(a.username.clone().unwrap_or_default()),
                Cell::from(Span::styled(status, Style::new().fg(color))),
                Cell::from(traffic),
                Cell::from(Span::styled(until, Style::new().dim())),
            ])
            .style(style)
        })
        .collect();
    let widths =
        [Constraint::Fill(2), Constraint::Fill(2), Constraint::Fill(2), Constraint::Length(22), Constraint::Length(17)];
    let table = Table::new(rows, widths)
        .header(TRow::new(vec!["  Hoster", "User", "Status", "Traffic left", "Valid until"]).style(Style::new().bold()))
        .column_spacing(1);
    frame.render_widget(table, inner);
}

/// The curated settings of the JDownloader, in the order `options.rs`
/// lists them. Sections are headings rather than rows, so the cursor only
/// ever lands on something that can be changed.
fn draw_options(frame: &mut Frame, app: &App, area: Rect) {
    let popup = centered(area, area.width.saturating_sub(6).min(96), area.height);
    frame.render_widget(Clear, popup);
    let hint = "Enter change · r default · ↑↓ move · Esc close";
    let block = panel("Settings", Some(hint));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    if app.options.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(" This JDownloader reported none of these settings").dim().italic()),
            padded(inner),
        );
        return;
    }

    // The note of the highlighted setting sits at the foot of the panel, so
    // the list above it is one line shorter.
    let note_height = 2;
    let list_height = inner.height.saturating_sub(note_height) as usize;

    let mut lines: Vec<Line> = Vec::new();
    // Which setting each line belongs to; headings belong to none.
    let mut owner: Vec<Option<usize>> = Vec::new();
    let label_width = 34usize;
    let mut section = "";
    for (i, setting) in app.options.iter().enumerate() {
        if setting.spec.section != section {
            section = setting.spec.section;
            if !lines.is_empty() {
                lines.push(Line::raw(""));
                owner.push(None);
            }
            lines.push(Line::from(Span::styled(format!(" {section}"), Style::new().bold().fg(accent()))));
            owner.push(None);
        }
        let selected = i == app.option_index;
        let value = setting.shown(&app.option_enums);
        let value_style = match setting.edit() {
            crate::options::Edit::Toggle if value == "on" => Style::new().fg(Color::Green),
            crate::options::Edit::Toggle => Style::new().fg(Color::DarkGray),
            crate::options::Edit::ReadOnly => Style::new().dim(),
            _ => Style::new().fg(Color::Yellow),
        };
        let mut spans = vec![
            Span::raw(format!("  {} ", if selected { "›" } else { " " })),
            Span::raw(format!("{:<label_width$}", truncate(setting.spec.label, label_width))),
            Span::styled(truncate(&value, 40), value_style),
        ];
        if !setting.is_default() {
            spans.push(Span::styled("  ·", Style::new().dim()));
        }
        let mut line = Line::from(spans);
        if selected {
            line = line.style(selected_style());
        }
        lines.push(line);
        owner.push(Some(i));
    }

    // Scroll so the highlighted row stays on screen, keeping its heading
    // visible when it can.
    let cursor_line = owner.iter().position(|o| *o == Some(app.option_index)).unwrap_or(0);
    let offset = if lines.len() <= list_height || cursor_line < list_height {
        0
    } else {
        (cursor_line + 1 + list_height / 4).saturating_sub(list_height).min(lines.len() - list_height)
    };
    let shown: Vec<Line> = lines.into_iter().skip(offset).take(list_height).collect();
    let list_area = Rect { height: list_height as u16, ..inner };
    frame.render_widget(Paragraph::new(shown), padded(list_area));

    // The note of the highlighted setting, and what JDownloader ships when
    // it has been changed: that is what the `·` beside the value means.
    let mut note = vec![Span::raw("  ")];
    if let Some(setting) = app.options.get(app.option_index) {
        note.push(Span::styled(setting.spec.note, Style::new().dim()));
        if !setting.is_default() {
            note.push(Span::styled(
                format!("  ·  default {}", setting.shown_default(&app.option_enums)),
                Style::new().dim().italic(),
            ));
        }
    }
    let note_area = Rect { y: inner.y + list_height as u16, height: note_height, ..inner };
    frame.render_widget(Paragraph::new(vec![Line::raw(""), Line::from(note)]), padded(note_area));
}

/// The choices of the ENUM setting under the cursor, in JDownloader's own
/// wording: it translates these, so they read as they do in its settings.
fn draw_option_choice(frame: &mut Frame, app: &App, area: Rect) {
    let n = app.option_choices.len();
    let popup = centered(area, 56, (n as u16 + 4).min(area.height));
    frame.render_widget(Clear, popup);
    let title = app.options.get(app.option_index).map(|s| s.spec.label).unwrap_or("Choose");
    let block = panel(title, Some("Enter apply · Esc cancel"));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let lines: Vec<Line> = app
        .option_choices
        .iter()
        .enumerate()
        .map(|(i, choice)| {
            let selected = i == app.option_choice_index;
            let line = Line::from(format!("  {} {}", if selected { "›" } else { " " }, choice.shown()));
            if selected { line.style(selected_style()) } else { line }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), padded(inner));
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let line = match (&app.mode, &app.message) {
        (Mode::Confirm(_), Some((m, _))) => Line::from(Span::styled(m.clone(), Style::new().fg(Color::Yellow).bold())),
        (_, Some((m, is_error))) => Line::from(Span::styled(
            m.clone(),
            Style::new().fg(if *is_error { Color::Red } else { Color::Green }).bold(),
        )),
        _ => {
            let key = |k: &'static str| Span::styled(k, Style::new().bold());
            let sep = || Span::styled("  |  ", Style::new().dim());
            let label = |t: &'static str| Span::styled(t, Style::new().dim());
            // The frequent keys only; `?` lists them all.
            Line::from(vec![
                key("Tab"),
                label(" switch"),
                sep(),
                key("↑↓"),
                label(" move"),
                sep(),
                key("Space"),
                label(" mark"),
                sep(),
                key("Enter"),
                label(" menu"),
                sep(),
                key("n"),
                label(" add links"),
                sep(),
                key("s"),
                label(" start/stop"),
                sep(),
                key("p"),
                label(if app.snapshot.is_paused() { " resume" } else { " pause" }),
                sep(),
                key("?"),
                label(" help"),
                sep(),
                key("q"),
                label(" quit"),
            ])
        }
    };
    frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
}

#[cfg(test)]
mod tests {
    //! Render whole frames into a test buffer and assert on what a user would
    //! actually see. Scraping a pty cannot do this reliably: ratatui moves the
    //! cursor instead of emitting lines.
    use super::*;
    use crate::api::{Link, Package, Snapshot};
    use crate::app::App;
    use crate::model::Action;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn sample() -> Snapshot {
        let link = Link {
            uuid: 20,
            name: "Show.S01E01.mkv".into(),
            package_uuid: 10,
            bytes_loaded: Some(512),
            bytes_total: Some(1024),
            host: Some("example.org".into()),
            url: Some("https://example.org/one".into()),
            ..Default::default()
        };
        let package = Package {
            uuid: 10,
            name: "Show S01".into(),
            bytes_loaded: Some(512),
            bytes_total: Some(1024),
            child_count: Some(1),
            enabled: Some(true),
            finished: Some(true),
            save_to: Some("/output/Show S01".into()),
            status: Some("Extraction OK".into()),
            links: vec![link],
            ..Default::default()
        };
        Snapshot { state: "IDLE".into(), downloads: vec![package], ..Default::default() }
    }

    /// The whole frame as text, one string per row.
    fn render(app: &App) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(170, 30)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let width = buffer.area().width as usize;
        buffer.content().chunks(width).map(|row| row.iter().map(|c| c.symbol()).collect::<String>()).collect()
    }

    fn shows(app: &App, needle: &str) -> bool {
        render(app).iter().any(|line| line.contains(needle))
    }

    #[test]
    fn main_screen_lists_packages() {
        let app = App::with_snapshot(sample());
        assert!(shows(&app, "jdtui · jd2@test"));
        assert!(shows(&app, "Show S01"));
        assert!(shows(&app, "Extraction OK"));
        assert!(shows(&app, "Downloads (1)"));
    }

    /// A few curated settings as a device would report them.
    fn options() -> Vec<crate::options::Setting> {
        use crate::api::ConfigEntry;
        use serde_json::json;
        let entry = |interface: &str, key: &str, kind: &str, value, default| ConfigEntry {
            interface_name: interface.into(),
            key: key.into(),
            abstract_type: Some(kind.into()),
            value: Some(value),
            default_value: Some(default),
            ..Default::default()
        };
        let general = "org.jdownloader.settings.GeneralSettings";
        crate::options::collect(vec![
            entry(general, "MaxSimultaneDownloads", "INT", json!(5), json!(3)),
            entry(general, "DownloadSpeedLimitEnabled", "BOOLEAN", json!(false), json!(false)),
            entry(general, "DownloadSpeedLimit", "INT", json!(10240), json!(51200)),
            entry(general, "DefaultDownloadFolder", "STRING", json!("/output"), json!("/config/Downloads")),
        ])
    }

    #[test]
    fn the_settings_panel_shows_labels_and_values() {
        let mut app = App::with_snapshot(sample());
        app.options = options();
        app.mode = crate::app::Mode::Options;
        assert!(shows(&app, "Settings"));
        assert!(shows(&app, "Downloads"), "the section heading");
        assert!(shows(&app, "Simultaneous downloads"));
        assert!(shows(&app, "10.00 KB/s"), "a speed is shown in its unit, not in bytes");
        assert!(shows(&app, "/output"));
        assert!(shows(&app, "How many files download at the same time"), "the note of the highlighted setting");
    }

    #[test]
    fn a_setting_left_at_its_default_carries_no_mark() {
        let mut app = App::with_snapshot(sample());
        app.options = options();
        app.mode = crate::app::Mode::Options;
        let rows = render(&app);
        let limit = rows.iter().find(|r| r.contains("Speed limit ")).expect("the row");
        assert!(!limit.contains('·'), "unchanged settings are not marked: {limit}");
        let simultaneous = rows.iter().find(|r| r.contains("Simultaneous downloads")).expect("the row");
        assert!(simultaneous.contains('·'), "a changed setting is marked: {simultaneous}");
    }

    #[test]
    fn choosing_a_value_shows_jdownloaders_own_wording() {
        let mut app = App::with_snapshot(sample());
        app.options = options();
        app.mode = crate::app::Mode::OptionChoice;
        app.option_choices = vec![
            crate::api::EnumOption { name: "SKIP_FILE".into(), label: Some("Salta file".into()) },
            crate::api::EnumOption { name: "OVERWRITE_FILE".into(), label: None },
        ];
        assert!(shows(&app, "Salta file"), "a translated choice");
        assert!(shows(&app, "Overwrite file"), "an untranslated one is made readable");
    }

    #[test]
    fn removing_asks_what_happens_to_the_files() {
        let mut app = App::with_snapshot(sample());
        app.mode = crate::app::Mode::RemoveChoice;
        assert!(shows(&app, "What about the files"), "the question must be visible");
        for mode in crate::app::REMOVE_MODES {
            assert!(shows(&app, mode.label()), "missing choice: {}", mode.label());
        }
    }

    #[test]
    fn the_choice_stays_visible_while_confirming() {
        let mut app = App::with_snapshot(sample());
        app.mode = crate::app::Mode::Confirm(Action::RemoveWith(crate::api::RemoveMode::DeleteFiles));
        app.message = Some(("Remove and delete files from disk on Package?  [y/N]".into(), false));
        assert!(shows(&app, "What about the files"));
        assert!(shows(&app, "[y/N]"));
    }

    #[test]
    fn properties_show_the_link_details() {
        let mut app = App::with_snapshot(sample());
        app.expanded.insert(10);
        app.rows = crate::model::build_rows(&app.snapshot.downloads, &app.expanded, "");
        app.cursor = 1; // the link under the package
        app.mode = crate::app::Mode::Properties;
        assert!(shows(&app, "Properties"));
        assert!(shows(&app, "example.org"));
        assert!(shows(&app, "/output/Show S01"));
    }

    #[test]
    fn add_form_shows_every_field() {
        let mut app = App::with_snapshot(sample());
        app.form = Some(crate::model::Form::add_links());
        app.mode = crate::app::Mode::Add;
        for label in ["Links", "Package name", "Save to", "Priority", "Autostart"] {
            assert!(shows(&app, label), "missing field: {label}");
        }
    }

    #[test]
    fn the_about_panel_says_where_the_calls_go() {
        use crate::api::{About, StorageInfo, SystemInfo};
        let mut app = App::with_snapshot(sample());
        app.about = Some(About {
            version: 48637,
            core_revision: 50639,
            uptime: 2 * 86_400_000,
            system: SystemInfo {
                docker: Some(true),
                java_version_string: Some("1.8.0_492-b09".into()),
                os_string: Some("Linux".into()),
                ..Default::default()
            },
            storage: vec![StorageInfo {
                path: Some("/downloads".into()),
                free: Some(1024 * 1024 * 1024),
                size: Some(4 * 1024 * 1024 * 1024),
            }],
            update_available: true,
            direct: Some("http://192.168.1.30:3129".into()),
        });
        app.mode = crate::app::Mode::About;
        assert!(shows(&app, "core revision 50639"));
        assert!(shows(&app, "2d 0h 0m"));
        assert!(shows(&app, "in a container"));
        assert!(shows(&app, "direct to http://192.168.1.30:3129"));
        assert!(shows(&app, "an update is waiting"));
        assert!(shows(&app, "1.00 GB free of 4.00 GB"));
    }
}
