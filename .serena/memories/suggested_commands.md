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
- Watch a CI run: `gh run list --workflow=release.yml`, `gh run view <id> --json jobs --jq '.jobs[] | "\(.name): \(.status) \(.conclusion)"'`, `gh run view <id> --log-failed`. Build without releasing: `gh workflow run release.yml` (no tag input → artifacts only).
- Sign a release after the workflow finishes: `scripts/sign-release.sh vX.Y.Z`.
- Platform is Linux; standard GNU coreutils, nothing special.
