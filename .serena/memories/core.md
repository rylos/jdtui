# jdtui — core

Rust TUI for JDownloader 2 over the My.JDownloader relay API. Single crate: lib (`src/lib.rs`, all modules `pub`) + bin (`src/main.rs`). ~2.9k lines, v1.0.0, repo github.com/rylos/jdtui.

## Source map (src/)
- `main.rs` — clap `Args` (`--refresh-ms`, `--choose-device`, `--config-path`), terminal setup (ratatui::init, bracketed paste), event loop: `app.tick()` → `ui::draw` → poll key 50ms → `translate(KeyCode)` → `app.handle_key(Key)`.
- `app.rs` — `App` state + all key handling. `Screen` {Login, Devices, Main}, `Mode` {List, Menu, Properties, Confirm(Action), RemoveChoice, Add}. `App::new(config)` auto-signs-in when config has credentials; `App::with_snapshot(snapshot)` = network-free App on Main screen (used by UI tests and screenshot example).
- `ui.rs` — pure drawing, `draw(frame, &App)`; no state mutation. Colours adapt to truecolor via `rgb(r,g,b,indexed)` fallback.
- `model.rs` — view model: `Tab`, `Row`/`RowKind`/`RowKey`, `build_rows` (packages + expanded set → flat rows), `Action` enum, `context_menu(tab, packages, rows)`, `Form`/`Field` (login + add-links forms), `PRIORITIES`.
- `api.rs` — `JdApi` high-level device calls (downloads/grabber queries, start/stop, enable, force, reset, remove, move_to_downloads, add_links); data types `Package`, `Link`, `Snapshot`, `RemoveMode`, `AddLinks`. `SharedApi = Arc<Mutex<JdApi>>`.
- `myjd.rs` — native My.JDownloader protocol: `MyJd` (connect/reconnect/disconnect/list_devices/device_call), crypto helpers (`secret`, `hmac_hex`, `encrypt`/`decrypt` AES-128-CBC, hex). Own `Error` type with `is_auth_failure`/`is_session_expired`.
- `poller.rs` — background thread refreshing `Snapshot` every `refresh_ms`; `refresh_now()` wakes it early (called after every successful action). Sends `Update::{Snapshot,Error}` via mpsc; `Drop` stops thread.
- `config.rs` — `~/.config/jdtui/config.toml` (email, password, device, refresh_ms); saved with mode 0600; `refresh_ms()` clamps to >=200.

## Other dirs
- `examples/screenshots.rs` — renders real frames into ratatui `TestBackend`, writes `docs/*.svg` (PNG converted separately for README). Data is invented.
- `docs/` — screenshots + `announcement.txt` (draft community post, not code).

## Invariants
- Network only in `myjd.rs`/`api.rs`/`poller.rs`; `ui.rs` and `model.rs` never touch it.
- Layering: main → app → {model, api, poller, config} ; api → myjd. Keep ui a function of `&App`.
- Destructive actions (`Action` with confirm=true, `RemoveMode::touches_files`) go through `Mode::Confirm` / `Mode::RemoveChoice` before running.
- Actions run through `App::with_api` then `finish` → `poller.refresh_now()`.

See `mem:tech_stack` (deps/toolchain), `mem:conventions` (style), `mem:suggested_commands` (run/test incl. live tests), `mem:task_completion` (checks before done).
