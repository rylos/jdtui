# jdtui — core

Rust TUI for JDownloader 2 over the My.JDownloader relay API. Single crate: lib (`src/lib.rs`, all modules `pub`) + bin (`src/main.rs`). ~4k lines, repo github.com/rylos/jdtui.

## Source map (src/)
- `main.rs` — clap `Args` (`--refresh-ms`, `--choose-device`, `--config-path`), terminal setup (ratatui::init, bracketed paste), event loop: `app.tick()` → `ui::draw` → drain `app.clipboard` as OSC 52 → poll key 50ms → `translate(KeyCode)` → `app.handle_key(Key)`.
- `app.rs` — `App` state + all key handling. `Screen` {Login, Devices, Main}. `Mode` {List, Menu, Properties, Confirm(Action), RemoveChoice, PriorityChoice, Add, Rename, Directory, NewPackage, ArchivePassword, Help, Urls, Accounts, DeviceMenu}. `DeviceMenu` reuses `menu`/`menu_index` with `model::device_menu()`; device actions (`Action::is_device`) skip the selection and go through `run_device_action`. `HELP` const = key reference (help panel; README table mirrors it). `App::new(config)` auto-signs-in; `App::with_snapshot(snapshot)` = network-free App on Main screen (UI tests + screenshot example).
- `ui.rs` — pure drawing, `draw(frame, &App)`. Side panels (menu, remove/priority choice, properties) on the right; popups (forms, help, urls, accounts) centered. Help panel is 2 columns ≥120 cols, drops blank lines when height is short.
- `model.rs` — view model: `Tab`, `Row`/`RowKey`, `build_rows`, `Action` enum, `context_menu(tab, packages, rows, stop_mark)`, `Form`/`Field` constructors (login, add_links, rename, directory, new_package, archive_password), `PRIORITIES`, `stop_mark_target`/`row_stop_marked`.
- `api.rs` — `JdApi` device calls; data types `Package`, `Link`, `Snapshot`, `Status`, `RemoveMode`, `AddLinks`, `ArchiveStatus`, `CaptchaJob`, `Account`. `status()` = slow part (stop mark, collecting, extraction queue, captchas); `snapshot(status)` = state, speed, downloads, grabber (4 round trips, ~110 ms each via relay).
- `myjd.rs` — native My.JDownloader protocol + crypto; own `Error` with `is_auth_failure`/`is_session_expired`.
- `poller.rs` — background thread; fetches `status()` every `STATUS_EVERY` (5) refreshes or right after `refresh_now()` (called after every successful action), `snapshot()` every refresh.
- `config.rs` — `~/.config/jdtui/config.toml`, mode 0600.

## Other dirs
- `examples/screenshots.rs` — draws real frames into ratatui `TestBackend` → `docs/*.svg` + PNG (rsvg-convert + pngquant, both installed). Data is invented.
- `docs/` — screenshots + `announcement.txt` (draft community post).

## Invariants
- Network only in `myjd.rs`/`api.rs`/`poller.rs`; `ui.rs` and `model.rs` never touch it. Keep ui a function of `&App`.
- Destructive actions (`MenuEntry.confirm`, `RemoveMode::touches_files`, `Action::ClearGrabber`) go through `Mode::Confirm` / `Mode::RemoveChoice` first.
- Actions: `run_action` → `with_api` → `finish` (message, clears marks, `refresh_now`). Actions that are not selection actions (stop mark, archive password) save/restore `marked` around `finish`.
- Popup forms all share `self.form` + `handle_form_key`, dispatching on `Mode` at Enter. `Field` text fields keep a char-index `cursor` (insert/backspace/delete/clear are `Field` methods; `Form::cycle` moves it on text fields, cycles choices/flips flags otherwise). Prefill with `Field::with_text` so the cursor lands at the end.
- Keys reach the app only through `Key` (app.rs) translated in `main.rs::translate`; a new physical key (Home, Delete, Ctrl-x…) needs a `Key` variant plus a `translate` arm.
- API quirks worth remembering are in `mem:api_quirks`.

See `mem:tech_stack`, `mem:conventions`, `mem:suggested_commands` (incl. live tests), `mem:task_completion`.
