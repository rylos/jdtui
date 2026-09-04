# Conventions

- rustfmt: `max_width = 120`, `use_small_heuristics = "Max"` (long one-line struct literals/matches are intentional). Always run `cargo fmt`.
- Every module starts with a `//!` doc comment stating its role and its boundary (e.g. "Drawing lives in ui.rs"). Keep them accurate when moving code.
- Doc comments explain *why*/non-obvious behaviour, terse; no boilerplate docs on trivial items.
- State/logic in `app.rs`, rendering in `ui.rs` taking `&App`; `model.rs` holds pure functions over `&[Package]` + `&[Row]` (no App access). Prefer adding pure helpers there and unit-testing them.
- Human formatting helpers (`human_size`, `human_eta`, `human_time`, `truncate`) are `pub` so tests/examples reuse them.
- Errors: protocol layer returns `myjd::Result`; UI-facing strings via `api::describe_error`. Errors shown in footer `message: Option<(String, bool)>` (bool = is_error) or header `refresh_error`.
- Tests: inline `#[cfg(test)] mod tests` (offline, byte-exact fixtures from the reference Python client for crypto) and `mod live` (`#[ignore]`, real service). Keep new network tests in `live` and ignored.
- Commit messages: imperative English sentence, no prefix/scope (e.g. "Release 1.0.0", "Add screenshots to the README").
- README key table must match `handle_list_key` in app.rs when keys change.
- Never commit real credentials/device names; screenshot data is invented on purpose.
