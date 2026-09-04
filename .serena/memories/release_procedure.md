# Release procedure (as done for 1.0.0 and 1.1.0)

1. Bump `version` in Cargo.toml (Cargo.lock follows on build); add a section to CHANGELOG.md; rework README if features changed; `cargo run --example screenshots`.
2. Commit "Release X.Y.Z"; annotated tag `vX.Y.Z` whose message is the release notes (first line "jdtui X.Y.Z").
3. `git push origin main --follow-tags`.
4. GitHub release: `gh release create vX.Y.Z --title "jdtui X.Y.Z" --notes-file <notes>`.
5. Asset, always expected: `cargo build --release`, copy `target/release/jdtui`, `strip` it, `tar czf jdtui-X.Y.Z-x86_64-linux.tar.gz jdtui` (binary at the archive root, nothing else), `gh release upload vX.Y.Z <tar.gz>`. Plain x86_64 glibc build, ~2 MB compressed.
