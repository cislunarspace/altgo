#!/usr/bin/env bash
# Generate PKGBUILD and .SRCINFO from template.
# Usage: ./generate-pkgbuild.sh <version> [path-to-deb]
# Output: PKGBUILD and .SRCINFO in current directory

set -euo pipefail

VERSION="${1:?Usage: generate-pkgbuild.sh <version> [path-to-deb]}"
LOCAL_DEB="${2:-}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE="${SCRIPT_DIR}/PKGBUILD.in"

echo "[INFO] Generating PKGBUILD for v${VERSION}..."

# Compute sha256 from local deb or download from GitHub Releases
if [[ -n "${LOCAL_DEB}" && -f "${LOCAL_DEB}" ]]; then
    echo "[INFO] Using local deb: ${LOCAL_DEB}"
    SHA256=$(sha256sum "${LOCAL_DEB}" | cut -d' ' -f1)
else
    DEB_URL="https://github.com/cislunarspace/altgo/releases/download/v${VERSION}/altgo_amd64.deb"
    TMP_DEB=$(mktemp)
    trap 'rm -f "${TMP_DEB}"' EXIT
    echo "[INFO] Downloading deb for checksum..."
    curl --fail --progress-bar -L -o "${TMP_DEB}" "${DEB_URL}"
    SHA256=$(sha256sum "${TMP_DEB}" | cut -d' ' -f1)
fi
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
