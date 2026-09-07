# Release procedure

Since 1.8.0 the binaries are built by GitHub Actions and signed locally.

1. Bump `version` in Cargo.toml (Cargo.lock follows on build); rename the CHANGELOG.md "Unreleased" section to "X.Y.Z — date" (unreleased changes are collected there between releases); rework README if features changed; `cargo run --example screenshots`.
2. Commit "Release X.Y.Z"; annotated tag `vX.Y.Z` whose message is the release notes (first line "jdtui X.Y.Z"). The notes carry NO co-author or tool attribution (user asked, 1.8.1) — say what changed and why, then an "Install" section (download, unpack, PATH; the Linux builds are static; nothing to enable on the JD side) and a "Verifying what you downloaded" section with the ssh-keygen commands and the key fingerprint. Commits and tags are SSH-signed automatically (repo-local `gpg.format=ssh`, `user.signingkey=~/.ssh/id_rsa.pub`, `commit.gpgsign`/`tag.gpgsign`).
3. `git push origin main --follow-tags`. This fires `.github/workflows/release.yml`: fmt + clippy + tests, then builds x86_64/aarch64 Linux (musl, static), x86_64/aarch64 macOS and x86_64 Windows, creates the release from the tag message if it does not exist, and uploads the archives plus SHA256SUMS.
4. Wait for the run (`gh run watch`), then `scripts/sign-release.sh vX.Y.Z`: downloads the assets, checks them against SHA256SUMS, signs each with the SSH key HELD ONLY HERE (never in Actions secrets) and uploads the `.sig` files.
5. Install locally: `cargo build --release && cp target/release/jdtui ~/.local/bin/`. Align the other machine (its ssh command is in the user's global CLAUDE.md, not here — this file is published).

Asset names: `jdtui-X.Y.Z-{x86_64,aarch64}-{linux,macos}.tar.gz` (binary at the archive root) and `jdtui-X.Y.Z-x86_64-windows.zip`.

The public half of the signing key is `.github/allowed_signers` (fingerprint SHA256:A8FoTqTFZrY18WXrAuT1mA2xnmoc4xTDCNIzkQPRjdA); the README explains verification. To register it on GitHub for the "Verified" badge the token needs a scope it does not have: `gh auth refresh -h github.com -s admin:ssh_signing_key`, then `gh api /user/ssh_signing_keys -f title=jdtui -f key="$(cat ~/.ssh/id_rsa.pub)"`.
