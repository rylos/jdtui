# Tech stack

- Rust, edition 2024, cargo (toolchain 1.93 stable installed; clippy + rustfmt present).
- TUI: `ratatui` 0.30 + `crossterm` 0.29 (feature `bracketed-paste`), used via `ratatui::crossterm` re-export.
- HTTP: `ureq` 3 (blocking). No async runtime; concurrency = one std thread in `poller.rs`.
- Crypto for My.JDownloader protocol: `aes` 0.9 + `cbc` 0.2 (AES-128-CBC), `hmac` 0.13 + `sha2` 0.11, `base64` 0.23, `percent-encoding`.
- Serialization: `serde`/`serde_json`; config `toml` 1.x; paths `dirs` 6.
- CLI: `clap` 4 derive. Errors: `anyhow` at app/bin level, custom `myjd::Error` in the protocol layer.
- No CI config, no Makefile/justfile; plain cargo.
- Serena MCP configured in `.mcp.json` (uvx from git, `--project-from-cwd`).
