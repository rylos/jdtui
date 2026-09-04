//! jdtui — a terminal UI for JDownloader 2, over the My.JDownloader API.

use std::io::{Write, stdout};
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use ratatui::crossterm::execute;

use jdtui::app::{App, Key};
use jdtui::config::Config;
use jdtui::ui;

#[derive(Parser, Debug)]
#[command(name = "jdtui", version, about = "A terminal UI for JDownloader 2")]
struct Args {
    /// Refresh period in milliseconds (overrides the config file).
    #[arg(long)]
    refresh_ms: Option<u64>,

    /// Forget the saved device and choose again.
    #[arg(long)]
    choose_device: bool,

    /// Do not listen to JDownloader's event channel; poll only.
    #[arg(long)]
    no_events: bool,

    /// Print the config file location and exit.
    #[arg(long)]
    config_path: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.config_path {
        println!("{}", Config::path().display());
        return Ok(());
    }

    let mut config = Config::load()?;
    if let Some(ms) = args.refresh_ms {
        config.refresh_ms = Some(ms);
    }
    if args.choose_device {
        config.device = None;
    }
    if args.no_events {
        config.events = Some(false);
    }

    let mut terminal = ratatui::init();
    let _ = execute!(stdout(), EnableBracketedPaste);
    let result = run(&mut terminal, App::new(config));
    let _ = execute!(stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, mut app: App) -> Result<()> {
    while !app.should_quit {
        app.tick();
        terminal.draw(|frame| ui::draw(frame, &app))?;
        if let Some(text) = app.clipboard.take() {
            copy_to_clipboard(&text);
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    if let Some(key) = translate(k.code, k.modifiers) {
                        app.handle_key(key);
                    }
                }
                Event::Paste(text) => app.handle_key(Key::Paste(text)),
                _ => {}
            }
        }
    }
    Ok(())
}

/// OSC 52 first: the terminal puts the text on the clipboard wherever it
/// runs, including over SSH; terminals without it ignore the sequence. Then
/// a local clipboard tool when a display is at hand, for those terminals.
fn copy_to_clipboard(text: &str) {
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut out = stdout();
    let _ = write!(out, "\x1b]52;c;{encoded}\x07");
    let _ = out.flush();

    let tools: &[&[&str]] = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        &[&["wl-copy"], &["xclip", "-selection", "clipboard"], &["xsel", "--clipboard", "--input"]]
    } else if std::env::var_os("DISPLAY").is_some() {
        &[&["xclip", "-selection", "clipboard"], &["xsel", "--clipboard", "--input"]]
    } else {
        &[]
    };
    for tool in tools {
        let spawned = std::process::Command::new(tool[0])
            .args(&tool[1..])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        if let Ok(mut child) = spawned {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
            break;
        }
    }
}

fn translate(code: KeyCode, modifiers: KeyModifiers) -> Option<Key> {
    Some(match code {
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Key::CtrlC,
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => Key::CtrlU,
        KeyCode::Char('f') if modifiers.contains(KeyModifiers::CONTROL) => Key::CtrlF,
        KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => Key::Home,
        KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => Key::End,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Delete => Key::Delete,
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        _ => return None,
    })
}
