# Suggested commands

- Run: `cargo run` (flags: `--refresh-ms N`, `--choose-device`, `--config-path`).
- Install: `cargo install --path .`
- Unit tests (offline, fast): `cargo test`
- Live tests against the real My.JDownloader service (marked `#[ignore]`, need credentials in `~/.config/jdtui/config.toml`; api.rs one creates+removes a throwaway package on a real device):
  `cargo test -- --ignored --nocapture`
  Choose device by name: `JDTUI_TEST_DEVICE=<name> cargo test -- --ignored --nocapture`
  Only protocol-level: `cargo test live -- --ignored --nocapture`
- Lint: `cargo clippy --all-targets`
- Format: `cargo fmt` (check: `cargo fmt --check`)
- Regenerate README screenshots: `cargo run --example screenshots` → `docs/*.svg`; PNGs must then be re-converted (rsvg-convert/inkscape) and committed.
- Platform is Linux; standard GNU coreutils, nothing special.
