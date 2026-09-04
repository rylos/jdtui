# Task completion checklist

Run from project root, all must pass clean (they did at onboarding, 2026-09-04):

1. `cargo fmt`
2. `cargo clippy --all-targets` — zero warnings
3. `cargo test` — offline suite
4. If protocol/API behaviour changed: `cargo test -- --ignored --nocapture` (live, needs credentials; see `mem:suggested_commands`).
5. If UI layout or keys changed: `cargo run --example screenshots`, reconvert PNGs in `docs/`, update README key table.
