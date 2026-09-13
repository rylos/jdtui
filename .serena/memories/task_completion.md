# Task completion checklist

Run from project root, all must pass clean (they did at onboarding, 2026-09-04):

1. `cargo fmt`
2. `cargo clippy --all-targets` — zero warnings
3. `cargo test` — offline suite
4. If protocol/API behaviour changed: `cargo test -- --ignored --nocapture` (live, needs credentials; see `mem:suggested_commands`).
5. If a signature in the lib changed and `cargo run --example screenshots` then fails with "function takes N arguments" or "unknown field", the example is linking a stale rlib: `cargo clean -p jdtui` and rerun. Happened twice on 2026-09-08; the screenshots silently kept the OLD text until cleaned, so check the SVG text when a status string changes.
6. If UI layout or keys changed: `cargo run --example screenshots`, reconvert PNGs in `docs/`, update README key table.
7. Before tagging, run the built binary against a real JDownloader once: `./target/release/jdtui status`. Unit tests never deserialize a real package, so a serde slip goes unnoticed by them (1.9.1 shipped with `Package.links` missing its `#[serde(skip)]` and failed on every list; 1.9.2 fixed it, 1.9.1 was yanked and its release deleted).
8. Never edit attribute lines with a global `sed` (`/^    #\[serde(skip)\]$/d` was how 1.9.1 broke): edit the one place, by hand.
