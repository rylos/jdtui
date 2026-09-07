# Conventions

- rustfmt: `max_width = 120`, `use_small_heuristics = "Max"`. Always run `cargo fmt`; clippy must be warning-free (`is_multiple_of` etc. are enforced).
- Every module starts with a `//!` doc comment stating its role and boundary. Keep accurate when moving code.
- Doc comments explain *why*/non-obvious behaviour, terse; none on trivial items.
- State/logic in `app.rs`, rendering in `ui.rs` taking `&App`; `model.rs` holds pure functions over `&[Package]` + `&[Row]`.
- New key binding = 4 places: `handle_list_key`, `HELP` const (app.rs), README key table, and the footer only if it is a frequent key. Help panel is drawn over the whole frame minus the footer (21 usable lines at 24 rows, 2 columns at ≥120 cols, key column sized per column): check `docs/help.png` after adding keys, and keep descriptions ≤ ~40 chars.
- New context-menu action = `Action` variant + `context_menu` entry (model.rs) + arm in `run_action` (or early-return branch for modes/forms) + the `unreachable!()` list.
- New API endpoint: method on `JdApi` in api.rs, then a `#[ignore]` live test in `api::live` that creates/uses a throwaway `jdtui-*` grabber package and removes it; run it once against the real device before building UI on it.
- Screenshots: any UI/key change → `cargo run --example screenshots` (regenerates all PNG/SVG; commit them). New panels get a shot + README paragraph.
- Errors: protocol layer returns `myjd::Result`; UI strings via `api::describe_error`; shown in footer `message: Option<(String, bool)>` (bool = is_error) or header `refresh_error`.
- Tests: inline `mod tests` (offline: crypto fixtures, form editing, row filtering) and `mod live` (`#[ignore]`, real service) in api.rs, myjd.rs and poller.rs. Live helpers: `wait_for` (15 s), `wait_for_long` (2 min, for crawls). A failed live test leaves `jdtui-*` packages on the device: remove them before rerunning or the counts are off.
- Commit messages: imperative English sentence, no prefix/scope; body only when a non-obvious decision needs explaining.
- Never commit real credentials/device names; screenshot data is invented on purpose.
- Commits and tags are SSH-signed (repo-local git config, `~/.ssh/id_rsa`, allowed signers in `.github/allowed_signers`). Nothing to do per-commit; just do not turn it off.
- Anything the panel shows must read in English whichever language JDownloader runs in: JD translates enum labels and its `status` sentence, so jdtui carries its own wording (`options.rs` for settings choices) and derives states from booleans/enums.

## Interface decisions (2026-09-06)

- Header colours: red means trouble only (ERROR); IDLE/STOPPED are DarkGray, PAUSED/STOPPING/CONNECTING yellow, RUNNING green. Border follows.
- Keys: `p` pause/resume, `i` properties with `P` as alias (user asked to swap the old p/P; `i` chosen as the common TUI convention). Footer, HELP const, README table and screenshots must stay in sync.
- Disabled rows (package or link) fade: row style DarkGray + DIM in `ui::row_base_style`; JD omits false booleans, so a missing `enabled` is disabled.
- Announced on jlesage/docker-jdownloader-2 discussions #300 (Show and tell), 2026-09-06.

## Wording and shape (2026-09-07)

- One word per concept: the route is "direct"/"relay" everywhere (header `⇄ direct`, About "direct to …", `jdtui status`, config key `direct`). Never introduce a synonym like "straight to".
- Never show JDownloader's localized `status` text as a state, and never colour by it: derive from booleans and enums. JD's sentence stays only as a fallback for cases jdtui does not model.
- CLI output is a contract: the `status` JSON shape has a test. Text output stays terse and greppable.
- Live-testing against the user's JDownloader is fine, but always clean up (remove test packages, empty the folders) in the same turn.
