# Conventions

- rustfmt: `max_width = 120`, `use_small_heuristics = "Max"`. Always run `cargo fmt`; clippy must be warning-free (`is_multiple_of` etc. are enforced).
- Every module starts with a `//!` doc comment stating its role and boundary. Keep accurate when moving code.
- Doc comments explain *why*/non-obvious behaviour, terse; none on trivial items.
- State/logic in `app.rs`, rendering in `ui.rs` taking `&App`; `model.rs` holds pure functions over `&[Package]` + `&[Row]`.
- New key binding = 4 places: `handle_list_key`, `HELP` const (app.rs), README key table, and the footer only if it is a frequent key. Help panel must still fit 24 rows (2 columns at ≥120 cols): check `docs/help.png` after adding keys.
- New context-menu action = `Action` variant + `context_menu` entry (model.rs) + arm in `run_action` (or early-return branch for modes/forms) + the `unreachable!()` list.
- New API endpoint: method on `JdApi` in api.rs, then a `#[ignore]` live test in `api::live` that creates/uses a throwaway `jdtui-*` grabber package and removes it; run it once against the real device before building UI on it.
- Screenshots: any UI/key change → `cargo run --example screenshots` (regenerates all PNG/SVG; commit them). New panels get a shot + README paragraph.
- Errors: protocol layer returns `myjd::Result`; UI strings via `api::describe_error`; shown in footer `message: Option<(String, bool)>` (bool = is_error) or header `refresh_error`.
- Tests: inline `mod tests` (offline: crypto fixtures, form editing, row filtering) and `mod live` (`#[ignore]`, real service) in api.rs, myjd.rs and poller.rs. Live helpers: `wait_for` (15 s), `wait_for_long` (2 min, for crawls). A failed live test leaves `jdtui-*` packages on the device: remove them before rerunning or the counts are off.
- Commit messages: imperative English sentence, no prefix/scope; body only when a non-obvious decision needs explaining.
- Never commit real credentials/device names; screenshot data is invented on purpose.
