#!/usr/bin/env bash
# Sign the assets of a release, on the maintainer's machine.
#
# The build happens on GitHub's runners; the key does not go there. This
# downloads what they produced, checks it against the checksums they wrote,
# signs every file with an SSH key held here and uploads the signatures.
#
#   scripts/sign-release.sh v1.8.0
#
# The key is $JDTUI_SIGNING_KEY, or ~/.ssh/id_rsa. Anyone can verify a
# download with the public half, which lives in .github/allowed_signers:
#
#   ssh-keygen -Y verify -f allowed_signers -I <identity> -n file \
#              -s jdtui-1.8.0-x86_64-linux.tar.gz.sig \
#              < jdtui-1.8.0-x86_64-linux.tar.gz
set -euo pipefail

tag=${1:-}
if [ -z "$tag" ]; then
    echo "usage: $0 vX.Y.Z" >&2
    exit 2
fi

key=${JDTUI_SIGNING_KEY:-$HOME/.ssh/id_rsa}
if [ ! -f "$key" ]; then
    echo "no signing key at $key; set JDTUI_SIGNING_KEY" >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "Downloading the assets of $tag…"
gh release download "$tag" --dir "$work" --pattern '*' --clobber
cd "$work"

# Signatures from an earlier run of this script are not what we sign.
rm -f ./*.sig

if [ ! -f SHA256SUMS ]; then
    echo "the release has no SHA256SUMS; did the workflow finish?" >&2
    exit 1
fi

echo "Checking what the runners built…"
grep -v '^SHA256SUMS' SHA256SUMS > expected.txt
sha256sum --check --strict expected.txt
rm expected.txt

echo "Signing with $key…"
for file in *; do
    case "$file" in *.sig) continue ;; esac
    # The `file` namespace is what ssh-keygen verifies against, and keeps
    # these signatures from being replayed as git ones.
    ssh-keygen -Y sign -f "$key" -n file -q "$file"
    echo "  $file.sig"
done

echo "Uploading the signatures…"
gh release upload "$tag" ./*.sig --clobber

echo
echo "Done. Verify one the way anybody else would:"
echo
echo "  ssh-keygen -Y verify -f allowed_signers -I <identity> -n file \\"
echo "             -s SHA256SUMS.sig < SHA256SUMS"
