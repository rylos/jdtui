//! Renders the screenshots used in the README.
//!
//! The frames are the real interface drawn into a test buffer, so they cannot
//! drift from the code, and the data is invented: no account, device or file
//! of anyone's shows up here.
//!
//! `cargo run --example screenshots` writes SVG files into `docs/`.

use std::fmt::Write as _;
use std::fs;

use jdtui::api::{Link, Package, RemoveMode, Snapshot};
use jdtui::app::{App, Mode};
use jdtui::model::{Action, Form, Tab, build_rows, context_menu};
use jdtui::ui;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

const WIDTH: u16 = 150;
const HEIGHT: u16 = 24;

// Cell metrics and a dark palette for the named colours, so the images look
// like a terminal rather than a spreadsheet.
const CELL_W: f32 = 8.4;
const CELL_H: f32 = 18.0;
const FONT: &str = "'JetBrains Mono','Fira Code','DejaVu Sans Mono','Cascadia Mono',monospace";
const BG: &str = "#14161a";
const FG: &str = "#c8ccd4";

fn palette(color: Color, foreground: bool) -> Option<String> {
    let named = match color {
        Color::Reset => return if foreground { Some(FG.into()) } else { None },
        Color::Black => "#14161a",
        Color::Red => "#e06c75",
        Color::Green => "#98c379",
        Color::Yellow => "#e5c07b",
        Color::Blue => "#61afef",
        Color::Magenta => "#c678dd",
        Color::Cyan => "#56b6c2",
        Color::Gray => "#9aa0aa",
        Color::DarkGray => "#5c6370",
        Color::LightRed => "#ff7b86",
        Color::LightGreen => "#b5e890",
        Color::LightYellow => "#f0d399",
        Color::LightBlue => "#7cc7ff",
        Color::LightMagenta => "#d7a3ea",
        Color::LightCyan => "#6fd3de",
        Color::White => "#f0f2f5",
        Color::Rgb(r, g, b) => return Some(format!("#{r:02x}{g:02x}{b:02x}")),
        Color::Indexed(_) => "#7f8794",
    };
    Some(named.to_string())
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// One `<rect>` per run of equal background, one `<text>` per run of equal
/// style: far fewer nodes than a span per cell, and it stays readable.
fn to_svg(buffer: &Buffer) -> String {
    let (w, h) = (buffer.area().width, buffer.area().height);
    let (pad_x, pad_y) = (14.0_f32, 12.0_f32);
    let width = w as f32 * CELL_W + pad_x * 2.0;
    let height = h as f32 * CELL_H + pad_y * 2.0;

    let mut svg = String::new();
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" font-family="{FONT}" font-size="13">
<rect width="100%" height="100%" rx="8" fill="{BG}"/>
"#
    );

    for y in 0..h {
        // backgrounds
        let mut x = 0;
        while x < w {
            let cell = &buffer[(x, y)];
            let bg = palette(cell.bg, false);
            let mut run = 1;
            while x + run < w && palette(buffer[(x + run, y)].bg, false) == bg {
                run += 1;
            }
            if let Some(bg) = bg.filter(|c| c != BG) {
                let _ = writeln!(
                    svg,
                    r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{bg}"/>"#,
                    pad_x + x as f32 * CELL_W,
                    pad_y + y as f32 * CELL_H,
                    run as f32 * CELL_W,
                    CELL_H
                );
            }
            x += run;
        }

        // text
        //
        // One element per glyph, each with an absolute x. A single element per
        // run would be smaller, but renderers fall back to the font's natural
        // advance after the first coordinate, and the drift accumulated into
        // visibly broken borders.
        for x in 0..w {
            let cell = &buffer[(x, y)];
            let symbol = cell.symbol();
            if symbol.trim().is_empty() {
                continue;
            }
            let mut style = String::new();
            if cell.modifier.contains(Modifier::BOLD) {
                style.push_str(r#" font-weight="bold""#);
            }
            if cell.modifier.contains(Modifier::ITALIC) {
                style.push_str(r#" font-style="italic""#);
            }
            if cell.modifier.contains(Modifier::DIM) {
                style.push_str(r#" opacity="0.65""#);
            }
            let _ = writeln!(
                svg,
                r#"<text x="{:.1}" y="{:.1}" fill="{}"{style}>{}</text>"#,
                pad_x + x as f32 * CELL_W,
                pad_y + (y as f32 + 0.75) * CELL_H,
                palette(cell.fg, true).unwrap_or_else(|| FG.into()),
                escape(symbol)
            );
        }
    }
    svg.push_str("</svg>\n");
    svg
}

fn shot(name: &str, app: &App) {
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    let svg = to_svg(terminal.backend().buffer());
    let path = format!("docs/{name}.svg");
    fs::write(&path, svg).unwrap();
    println!("wrote {path}");

    // The README shows PNGs: they render everywhere, including on hosts that
    // sanitise SVG or lack the fonts. Skipped when the converter is missing.
    let png = format!("docs/{name}.png");
    match std::process::Command::new("rsvg-convert").args(["-z", "2", &path, "-o", &png]).status() {
        Ok(status) if status.success() => {
            let _ = std::process::Command::new("pngquant")
                .args(["--force", "--skip-if-larger", "--quality", "70-95", "--output", &png, &png])
                .status();
            println!("wrote {png}");
        }
        _ => println!("rsvg-convert not available, skipped {png}"),
    }
}

fn link(uuid: i64, package: i64, name: &str, loaded: i64, total: i64, done: bool) -> Link {
    Link {
        uuid,
        name: name.into(),
        package_uuid: package,
        bytes_loaded: Some(loaded),
        bytes_total: Some(total),
        enabled: Some(true),
        finished: Some(done),
        host: Some("mirror.example.org".into()),
        url: Some(format!("https://mirror.example.org/{name}")),
        status: Some(if done { "Extraction OK".into() } else { "Downloading".into() }),
        extraction_status: done.then(|| "SUCCESSFUL".to_string()),
        ..Default::default()
    }
}

const GB: i64 = 1024 * 1024 * 1024;

fn demo() -> Snapshot {
    let finished = Package {
        uuid: 1,
        name: "Blender 4.2 demo files".into(),
        bytes_loaded: Some(12 * GB),
        bytes_total: Some(12 * GB),
        child_count: Some(24),
        enabled: Some(true),
        finished: Some(true),
        save_to: Some("/downloads/Blender 4.2 demo files".into()),
        status: Some("Extraction OK".into()),
        links: (0..24)
            .map(|i| {
                link(
                    100 + i,
                    1,
                    &format!("blender-demo.part{:02}.rar", i + 1),
                    512 * 1024 * 1024,
                    512 * 1024 * 1024,
                    true,
                )
            })
            .collect(),
        ..Default::default()
    };
    let running = Package {
        uuid: 2,
        name: "Ubuntu 24.04.1 Desktop amd64".into(),
        bytes_loaded: Some(3 * GB),
        bytes_total: Some(6 * GB),
        child_count: Some(8),
        enabled: Some(true),
        running: Some(true),
        speed: Some(11 * 1024 * 1024),
        eta: Some(287),
        save_to: Some("/downloads/Ubuntu 24.04.1".into()),
        links: (0..8)
            .map(|i| {
                link(
                    200 + i,
                    2,
                    &format!("ubuntu-24.04.1.part{}.rar", i + 1),
                    if i < 4 { 768 * 1024 * 1024 } else { 0 },
                    768 * 1024 * 1024,
                    i < 4,
                )
            })
            .collect(),
        ..Default::default()
    };
    let queued = Package {
        uuid: 3,
        name: "Debian 13 netinst images".into(),
        bytes_loaded: Some(0),
        bytes_total: Some(2 * GB),
        child_count: Some(4),
        enabled: Some(true),
        save_to: Some("/downloads/Debian 13".into()),
        links: (0..4)
            .map(|i| link(300 + i, 3, &format!("debian-13.part{}.rar", i + 1), 0, 512 * 1024 * 1024, false))
            .collect(),
        ..Default::default()
    };
    let grabbed = Package {
        uuid: 4,
        name: "LibreOffice 25.2 sources".into(),
        bytes_total: Some(1024 * 1024 * 1024),
        child_count: Some(3),
        enabled: Some(true),
        available_online_count: Some(3),
        available_offline_count: Some(0),
        hosts: Some(vec!["mirror.example.org".into()]),
        save_to: Some("/downloads/LibreOffice 25.2".into()),
        links: (0..3)
            .map(|i| {
                let mut l =
                    link(400 + i, 4, &format!("libreoffice-25.2.part{}.rar", i + 1), 0, 350 * 1024 * 1024, false);
                l.availability = Some("ONLINE".into());
                l.status = None;
                l
            })
            .collect(),
        ..Default::default()
    };

    Snapshot {
        state: "RUNNING".into(),
        speed: 11 * 1024 * 1024,
        downloads: vec![finished, running, queued],
        grabber: vec![grabbed],
    }
}

fn base() -> App {
    let mut app = App::with_snapshot(demo());
    app.device_name = "jd2@homeserver".into();
    app
}

fn main() {
    // The interface picks indexed colours when the terminal does not claim
    // truecolor, which is not what we want to show in an image.
    unsafe { std::env::set_var("COLORTERM", "truecolor") };
    fs::create_dir_all("docs").unwrap();

    // The tree, with one package opened.
    let mut app = base();
    app.expanded.insert(2);
    app.rows = build_rows(&app.snapshot.downloads, &app.expanded);
    app.cursor = 1;
    shot("downloads", &app);

    // A selection of links, with the menu open on all of them.
    let mut app = base();
    app.expanded.insert(2);
    app.rows = build_rows(&app.snapshot.downloads, &app.expanded);
    app.cursor = 5;
    for row in [2usize, 3, 4] {
        app.marked.insert(jdtui::model::row_key(&app.snapshot.downloads, &app.rows[row]));
    }
    let targets = app.target_rows();
    app.menu = context_menu(Tab::Downloads, &app.snapshot.downloads, &targets);
    app.menu_index = 0;
    app.mode = Mode::Menu;
    shot("context-menu", &app);

    // Removing asks about the files on disk.
    let mut app = base();
    app.cursor = 0;
    app.mode = Mode::RemoveChoice;
    app.remove_index = 2;
    shot("remove", &app);

    // Adding links.
    let mut app = base();
    let mut form = Form::add_links();
    form.type_str("https://mirror.example.org/debian-13.iso");
    form.next();
    form.type_str("Debian 13");
    app.form = Some(form);
    app.mode = Mode::Add;
    shot("add-links", &app);

    // The Link Grabber tab.
    let mut app = base();
    app.tab = Tab::Grabber;
    app.expanded.insert(4);
    app.rows = build_rows(&app.snapshot.grabber, &app.expanded);
    shot("link-grabber", &app);

    // The key reference.
    let mut app = base();
    app.mode = Mode::Help;
    shot("help", &app);

    // Keep the unused import honest when the example grows.
    let _ = (Action::Properties, RemoveMode::ListOnly);
}
