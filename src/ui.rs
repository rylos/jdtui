//! Drawing. Reads `App`, never mutates it.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Paragraph, Row as TRow, Table, TableState, Tabs};

use crate::api::{Link, Package};
use crate::app::{App, HELP, Mode, Screen};
use crate::model::{FieldKind, Form, PRIORITIES, Row, Tab, describe, row_key, row_stop_marked};

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
    let block = panel(form.title, Some("Enter sign in · Tab next field · Esc quit"));
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
    frame.render_widget(Paragraph::new(lines), inner);
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
            if active {
                if matches!(f.kind, FieldKind::Text | FieldKind::Secret) {
                    value.content = format!("{}▏", value.content).into();
                }
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
        (Some(e), _) => (format!("ERROR: {e}"), Color::Yellow),
        (None, "") => ("CONNECTING…".to_string(), Color::Yellow),
        (None, "RUNNING") => (state.to_string(), Color::Green),
        (None, "PAUSE") => ("PAUSED".to_string(), Color::Yellow),
        (None, "STOPPED_STATE") => ("STOPPED".to_string(), Color::Red),
        (None, "IDLE") => (state.to_string(), Color::Red),
        (None, other) => (other.to_string(), Color::Yellow),
    };

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(state_color))
        .title(Line::from(format!(" jdtui · {} ", app.device_name)).bold());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let cols = Layout::horizontal([Constraint::Ratio(1, 3); 3]).split(inner);
    let left = vec![
        Line::from(vec![Span::raw("State: "), Span::styled(state_text, Style::new().fg(state_color).bold())]),
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
        Line::from(format!(" Link Grabber ({}) ", app.snapshot.grabber.len())),
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
        Mode::Menu => 34,
        Mode::RemoveChoice | Mode::Confirm(crate::model::Action::RemoveWith(_)) => 46,
        Mode::PriorityChoice => 34,
        Mode::Properties => 62,
        _ => 0,
    };
    let (list_area, side_area) = if side_width > 0 && area.width > side_width + 40 {
        let cols = Layout::horizontal([Constraint::Fill(1), Constraint::Length(side_width)]).split(area);
        (cols[0], Some(cols[1]))
    } else {
        (area, None)
    };

    if matches!(app.mode, Mode::Add | Mode::Rename | Mode::Directory)
        && let Some(form) = &app.form
    {
        draw_list(frame, app, list_area);
        let popup = centered(area, 90, form.fields.len() as u16 + 6);
        frame.render_widget(Clear, popup);
        let hint = match app.mode {
            Mode::Add if form.is_valid() => "Enter add · Tab/↑↓ field · ←→ change · Esc cancel",
            Mode::Add => "Paste at least one url · Tab/↑↓ field · Esc cancel",
            _ => "Enter apply · Esc cancel",
        };
        let block = panel(form.title, Some(hint));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        let mut lines = vec![Line::raw("")];
        lines.extend(form_lines(form));
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }

    if app.mode == Mode::Help {
        draw_list(frame, app, list_area);
        draw_help(frame, area);
        return;
    }

    draw_list(frame, app, list_area);
    if let Some(side) = side_area {
        match app.mode {
            Mode::Menu => draw_menu(frame, app, side),
            Mode::RemoveChoice | Mode::Confirm(crate::model::Action::RemoveWith(_)) => {
                draw_remove_choice(frame, app, side)
            }
            Mode::PriorityChoice => draw_priority_choice(frame, app, side),
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
    let mut state = TableState::default().with_selected(Some(app.cursor));
    frame.render_stateful_widget(table, area, &mut state);
}

fn row_base_style(app: &App, packages: &[Package], row: &Row) -> Style {
    if app.marked.contains(&row_key(packages, row)) { marked_style() } else { Style::new() }
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

fn downloads_rows<'a>(app: &'a App, packages: &'a [Package]) -> (Vec<&'static str>, Vec<Constraint>, Vec<TRow<'a>>) {
    let header = vec!["Name", "Links", "Size", "Status", "Progress", "%", "Speed", "ETA"];
    let widths = vec![
        Constraint::Fill(3),
        Constraint::Length(6),
        Constraint::Length(20),
        Constraint::Fill(1),
        Constraint::Length(18),
        Constraint::Length(5),
        Constraint::Length(12),
        Constraint::Length(9),
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
                        Cell::from(Span::styled(package_status(pkg), Style::new().fg(Color::Yellow))),
                        Cell::from(progress_bar(pct, 18)),
                        Cell::from(format!("{pct:.0}%")),
                        Cell::from(Span::styled(
                            pkg.speed
                                .filter(|s| *s > 0)
                                .map(|s| format!("{}/s", human_size(s)))
                                .unwrap_or_else(|| "-".into()),
                            Style::new().fg(accent()),
                        )),
                        Cell::from(Span::styled(human_eta(pkg.eta.unwrap_or(0)), Style::new().fg(Color::Green))),
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
                        Cell::from(Span::styled(
                            link.status.clone().unwrap_or_else(|| {
                                if link.is_finished() {
                                    "Finished".into()
                                } else if link.running.unwrap_or(false) {
                                    "Downloading".into()
                                } else {
                                    "-".into()
                                }
                            }),
                            Style::new().dim(),
                        )),
                        Cell::from(""),
                        Cell::from(Span::styled(format!("{pct:.0}%"), Style::new().dim())),
                        Cell::from(
                            link.speed.filter(|s| *s > 0).map(|s| format!("{}/s", human_size(s))).unwrap_or_default(),
                        ),
                        Cell::from(""),
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
                    TRow::new(vec![
                        Cell::from(Span::styled(
                            format!(" {}  └ {}", mark(app, packages, row), link.name),
                            Style::new().dim(),
                        )),
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
    frame.render_widget(Paragraph::new(lines), inner);
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
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_menu(frame: &mut Frame, app: &App, area: Rect) {
    let targets = app.target_rows();
    let block = panel(&describe(&targets), Some("Enter run · Esc close"));
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
    frame.render_widget(Paragraph::new(lines), inner);
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

fn draw_help(frame: &mut Frame, area: Rect) {
    let key_width = HELP.iter().flat_map(|(_, keys)| keys.iter()).map(|(k, _)| k.chars().count()).max().unwrap_or(8);
    let section_lines = |section: &str, keys: &[(&str, &str)]| -> Vec<Line<'static>> {
        let mut lines = vec![Line::from(Span::styled(format!(" {section}"), Style::new().fg(accent()).bold()))];
        for (key, what) in keys {
            lines.push(Line::from(vec![
                Span::styled(format!("   {key:<w$}  ", w = key_width), Style::new().bold()),
                Span::raw(what.to_string()),
            ]));
        }
        lines
    };

    // Two columns when the terminal is wide enough, so 24 rows are enough
    // to show everything; one otherwise.
    let two_columns = area.width >= 120;
    let mut columns: Vec<Vec<Line>> = vec![Vec::new(), Vec::new()];
    let total: usize = HELP.iter().map(|(_, k)| k.len() + 2).sum();
    let mut filled = 0;
    for (section, keys) in HELP {
        let col = if two_columns && filled + keys.len() + 2 > total / 2 { 1 } else { 0 };
        if !columns[col].is_empty() {
            columns[col].push(Line::raw(""));
        }
        columns[col].extend(section_lines(section, keys));
        filled += keys.len() + 2;
    }

    let column_width = 58u16;
    let width = if two_columns { column_width * 2 + 2 } else { column_width + 2 };
    let height = (columns.iter().map(|c| c.len()).max().unwrap_or(0) as u16 + 3).min(area.height);
    let popup = centered(area, width, height);
    frame.render_widget(Clear, popup);
    let block = panel("Keys", Some("Esc close"));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let cols = Layout::horizontal([Constraint::Length(column_width), Constraint::Fill(1)]).split(inner);
    for (i, lines) in columns.into_iter().enumerate() {
        let mut text = vec![Line::raw("")];
        text.extend(lines);
        frame.render_widget(Paragraph::new(text), cols[i]);
    }
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
                key("P"),
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
        app.rows = crate::model::build_rows(&app.snapshot.downloads, &app.expanded);
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
}
