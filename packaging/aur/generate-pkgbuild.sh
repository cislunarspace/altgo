#!/usr/bin/env bash
# Generate PKGBUILD and .SRCINFO from template.
# Usage: ./generate-pkgbuild.sh <version>
# Output: PKGBUILD and .SRCINFO in current directory

set -euo pipefail

VERSION="${1:?Usage: generate-pkgbuild.sh <version>}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE="${SCRIPT_DIR}/PKGBUILD.in"
DEB_URL="https://github.com/cislunarspace/altgo/releases/download/v${VERSION}/altgo_amd64.deb"

echo "[INFO] Generating PKGBUILD for v${VERSION}..."

# Download deb to compute sha256
TMP_DEB=$(mktemp)
trap 'rm -f "${TMP_DEB}"' EXIT

echo "[INFO] Downloading deb for checksum..."
curl --fail --progress-bar -L -o "${TMP_DEB}" "${DEB_URL}"
SHA256=$(sha256sum "${TMP_DEB}" | cut -d' ' -f1)
echo "[OK] sha256: ${SHA256}"

# Generate PKGBUILD from template
sed "s/VERSION/${VERSION}/g; s/SHA256_PLACEHOLDER/${SHA256}/g" "${TEMPLATE}" > PKGBUILD

# Generate .SRCINFO (requires makepkg)
if command -v makepkg &>/dev/null; then
    makepkg --printsrcinfo > .SRCINFO
    echo "[OK] Generated PKGBUILD and .SRCINFO"
else
    echo "[WARN] makepkg not found, skipping .SRCINFO generation"
    echo "[OK] Generated PKGBUILD"
fi
