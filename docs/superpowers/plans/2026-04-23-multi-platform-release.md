# Multi-Platform Linux Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify altgo's release pipeline to auto-produce `.deb`, `.rpm`, `.AppImage`, `.flatpak`, and AUR PKGBUILD on every `v*` tag push.

**Architecture:** Single `release.yml` workflow with parallel build jobs per format. Shared version constants in `packaging/scripts/versions.sh`. Tauri bundler handles deb+rpm, existing AppImage script integrated, new Flatpak manifest and AUR template added.

**Tech Stack:** GitHub Actions, Tauri bundler, appimage-builder, flatpak-builder, bash scripts, PKGBUILD

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `packaging/scripts/versions.sh` | Create | Single source of truth for whisper.cpp and ffmpeg versions |
| `packaging/appimage/build.sh` | Modify | Source versions.sh instead of hardcoding v1.7.5 |
| `packaging/scripts/download-deps.sh` | Modify | Source versions.sh instead of hardcoding v1.8.4 |
| `src-tauri/tauri.conf.json` | Modify | Add `"rpm"` to bundle targets, add `linux.rpm` depends |
| `packaging/flatpak/com.github.altgo.yml` | Create | Flatpak manifest for building .flatpak bundle |
| `packaging/aur/PKGBUILD.in` | Create | AUR PKGBUILD template with VERSION/SHA256 placeholders |
| `packaging/aur/generate-pkgbuild.sh` | Create | Generates final PKGBUILD + .SRCINFO from template |
| `.github/workflows/release.yml` | Modify | Add rpm, appimage, flatpak, aur jobs; update release job |
| `.github/workflows/appimage.yml` | Delete | Functionality moved to release.yml |

---

### Task 1: Create unified version constants

**Files:**
- Create: `packaging/scripts/versions.sh`

- [ ] **Step 1: Create versions.sh**

```bash
#!/usr/bin/env bash
# Unified version constants for all packaging scripts.
# Source this file: source "$(dirname "${BASH_SOURCE[0]}")/versions.sh"

WHISPER_CPP_VERSION="1.8.4"
FFMPEG_VERSION="7.1.1"
```

- [ ] **Step 2: Make it executable**

```bash
chmod +x packaging/scripts/versions.sh
```

- [ ] **Step 3: Verify it sources correctly**

```bash
source packaging/scripts/versions.sh && echo "whisper=${WHISPER_CPP_VERSION} ffmpeg=${FFMPEG_VERSION}"
```

Expected: `whisper=1.8.4 ffmpeg=7.1.1`

- [ ] **Step 4: Commit**

```bash
git add packaging/scripts/versions.sh
git commit -m "feat: add unified version constants for packaging scripts"
```

---

### Task 2: Update build scripts to use versions.sh

**Files:**
- Modify: `packaging/appimage/build.sh:47,87`
- Modify: `packaging/scripts/download-deps.sh:139`

- [ ] **Step 1: Update AppImage build.sh — add source line**

Add after line 31 (`BUILD_APPIMAGE_DIR=...`):

```bash
# Source unified version constants
source "${SCRIPT_DIR}/../scripts/versions.sh"
```

- [ ] **Step 2: Update AppImage build.sh — replace hardcoded WHISPER_VERSION**

Change line 47:
```bash
WHISPER_VERSION="1.7.5"
```
To:
```bash
WHISPER_VERSION="${WHISPER_CPP_VERSION}"
```

- [ ] **Step 3: Update AppImage build.sh — remove hardcoded FFMPEG_VERSION**

Delete line 87 (`FFMPEG_VERSION="7.1.1"`) entirely since `FFMPEG_VERSION` is already set by `versions.sh`.

- [ ] **Step 4: Update download-deps.sh — add source line**

Add after line 11 (`REPO_ROOT=...`):

```bash
# Source unified version constants
source "${SCRIPT_DIR}/versions.sh"
```

- [ ] **Step 5: Update download-deps.sh — replace hardcoded WHISPER_VERSION**

Change line 139:
```bash
WHISPER_VERSION="1.8.4"
```
To:
```bash
WHISPER_VERSION="${WHISPER_CPP_VERSION}"
```

- [ ] **Step 6: Verify both scripts parse correctly**

```bash
bash -n packaging/appimage/build.sh && bash -n packaging/scripts/download-deps.sh
```

Expected: no output (syntax OK)

- [ ] **Step 7: Commit**

```bash
git add packaging/appimage/build.sh packaging/scripts/download-deps.sh
git commit -m "fix: use unified versions.sh in all build scripts (sync whisper.cpp version)"
```

---

### Task 3: Add RPM support to Tauri config

**Files:**
- Modify: `src-tauri/tauri.conf.json:48,59-63`

- [ ] **Step 1: Add "rpm" to bundle targets**

Change line 48:
```json
"targets": ["deb"],
```
To:
```json
"targets": ["deb", "rpm"],
```

- [ ] **Step 2: Add rpm depends section inside `linux`**

After the `deb` block (line 62), add:

```json
"rpm": {
  "depends": ["webkit2gtk4.1", "gtk3", "libayatana-appindicator-gtk3", "xclip", "pulseaudio-utils", "libnotify"]
}
```

The full `linux` section becomes:
```json
"linux": {
  "deb": {
    "depends": ["libwebkit2gtk-4.1-0", "libgtk-3-0", "libayatana-appindicator3-1", "xclip", "pulseaudio-utils", "libnotify-bin"]
  },
  "rpm": {
    "depends": ["webkit2gtk4.1", "gtk3", "libayatana-appindicator-gtk3", "xclip", "pulseaudio-utils", "libnotify"]
  }
}
```

- [ ] **Step 3: Validate JSON syntax**

```bash
python3 -c "import json; json.load(open('src-tauri/tauri.conf.json'))"
```

Expected: no output (valid JSON)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "feat: add RPM bundle target with dependency declarations"
```

---

### Task 4: Create Flatpak manifest

**Files:**
- Create: `packaging/flatpak/com.github.altgo.yml`

- [ ] **Step 1: Create the directory**

```bash
mkdir -p packaging/flatpak
```

- [ ] **Step 2: Create the Flatpak manifest**

```yaml
app-id: com.github.altgo
runtime: org.freedesktop.Platform
runtime-version: '23.08'
sdk: org.freedesktop.Sdk
command: altgo
finish-args:
  - --socket=x11
  - --socket=wayland
  - --socket=pulseaudio
  - --share=ipc
  - --talk-name=org.freedesktop.Notifications
  - --device=input
  - --filesystem=home
  - --env=PATH=/app/bin:/app/altgo/bin

modules:
  - name: altgo
    buildsystem: simple
    build-commands:
      - install -Dm755 altgo /app/bin/altgo
      - cp -r bin/* /app/altgo/bin/
      - mkdir -p /app/share/icons/hicolor/128x128/apps
      - install -Dm644 icons/128x128.png /app/share/icons/hicolor/128x128/apps/com.github.altgo.png
      - install -Dm644 com.github.altgo.desktop /app/share/applications/com.github.altgo.desktop
    sources:
      - type: file
        path: ../../src-tauri/target/release/altgo
      - type: dir
        path: ../../target/deps/bin
        dest: bin
      - type: dir
        path: ../../src-tauri/icons
        dest: icons
      - type: file
        path: ../../packaging/appimage/altgo.desktop.in
        dest-filename: com.github.altgo.desktop
```

- [ ] **Step 3: Validate YAML syntax**

```bash
python3 -c "import yaml; yaml.safe_load(open('packaging/flatpak/com.github.altgo.yml'))"
```

If `pyyaml` is not installed:
```bash
python3 -c "import json, sys; [json.loads(line) for line in open('/dev/null')]" && echo "YAML syntax check skipped (no pyyaml), verify manually"
```

- [ ] **Step 4: Commit**

```bash
git add packaging/flatpak/
git commit -m "feat: add Flatpak manifest for .flatpak bundle builds"
```

---

### Task 5: Create AUR PKGBUILD template and generator

**Files:**
- Create: `packaging/aur/PKGBUILD.in`
- Create: `packaging/aur/generate-pkgbuild.sh`

- [ ] **Step 1: Create the directory**

```bash
mkdir -p packaging/aur
```

- [ ] **Step 2: Create PKGBUILD.in**

```bash
# Maintainer: altgo contributors
pkgname=altgo
pkgver=VERSION
pkgrel=1
pkgdesc="Linux voice-to-text tool with local Whisper and LLM polishing"
arch=('x86_64')
url="https://github.com/cislunarspace/altgo"
license=('MIT')
depends=('webkit2gtk-4.1' 'gtk3' 'libayatana-appindicator' 'xclip' 'pulseaudio-utils' 'libnotify')
source=("${pkgname}_${pkgver}_amd64.deb::https://github.com/cislunarspace/altgo/releases/download/v${pkgver}/altgo_amd64.deb")
sha256sums=('SHA256_PLACEHOLDER')

package() {
    bsdtar -xf "${srcdir}/data.tar.xz" -C "${pkgdir}"
}
```

- [ ] **Step 3: Create generate-pkgbuild.sh**

```bash
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
```

- [ ] **Step 4: Make generator executable**

```bash
chmod +x packaging/aur/generate-pkgbuild.sh
```

- [ ] **Step 5: Verify script syntax**

```bash
bash -n packaging/aur/generate-pkgbuild.sh
```

Expected: no output (syntax OK)

- [ ] **Step 6: Commit**

```bash
git add packaging/aur/
git commit -m "feat: add AUR PKGBUILD template and generator script"
```

---

### Task 6: Rewrite release.yml with all build jobs

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Replace the entire release.yml**

The new workflow has 6 jobs: `build-deb`, `build-rpm`, `build-appimage`, `build-flatpak`, `gen-aur`, and `release`.

```yaml
name: Release

on:
  push:
    tags: ["v*"]

env:
  CARGO_TERM_COLOR: always

permissions:
  contents: write

jobs:
  build-deb:
    name: Build DEB
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf libasound2-dev build-essential cmake

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-unknown-linux-gnu
      - uses: Swatinem/rust-cache@v2
        with:
          key: x86_64-unknown-linux-gnu-release

      - name: Install Tauri CLI
        run: cargo install tauri-cli --version "^2" --locked

      - uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Install frontend dependencies
        run: npm ci
        working-directory: frontend

      - name: Download bundled binaries
        run: bash packaging/scripts/download-deps.sh x86_64

      - name: Build deb
        run: cargo tauri build --bundles deb
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Upload deb artifact
        uses: actions/upload-artifact@v4
        with:
          name: altgo-amd64-deb
          path: src-tauri/target/release/bundle/deb/*.deb
          if-no-files-found: error

  build-rpm:
    name: Build RPM
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf libasound2-dev build-essential cmake rpm

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-unknown-linux-gnu
      - uses: Swatinem/rust-cache@v2
        with:
          key: x86_64-unknown-linux-gnu-release-rpm

      - name: Install Tauri CLI
        run: cargo install tauri-cli --version "^2" --locked

      - uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Install frontend dependencies
        run: npm ci
        working-directory: frontend

      - name: Download bundled binaries
        run: bash packaging/scripts/download-deps.sh x86_64

      - name: Build rpm
        run: cargo tauri build --bundles rpm
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Upload rpm artifact
        uses: actions/upload-artifact@v4
        with:
          name: altgo-x86_64-rpm
          path: src-tauri/target/release/bundle/rpm/*.rpm
          if-no-files-found: error

  build-appimage:
    name: Build AppImage
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf libasound2-dev build-essential cmake git curl xz-utils libudev-dev pkg-config libssl-dev python3 python3-pip

      - name: Install appimage-builder
        run: pip3 install appimage-builder

      - uses: dtolnay/rust-toolchain@stable

      - uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Get version from tag
        id: version
        run: echo "value=${GITHUB_REF_NAME}" >> $GITHUB_OUTPUT

      - name: Build AppImage
        run: bash packaging/appimage/build.sh x86_64 "${{ steps.version.outputs.value }}" appimage-builder

      - name: Find AppImage artifact
        id: find_artifact
        run: |
          APPIMAGE=$(find target/appimage-build -name '*.AppImage' | head -1)
          echo "path=${APPIMAGE}" >> $GITHUB_OUTPUT
          echo "name=$(basename ${APPIMAGE})" >> $GITHUB_OUTPUT

      - uses: actions/upload-artifact@v4
        with:
          name: ${{ steps.find_artifact.outputs.name }}
          path: ${{ steps.find_artifact.outputs.path }}
          if-no-files-found: error

  build-flatpak:
    name: Build Flatpak
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf libasound2-dev build-essential cmake flatpak flatpak-builder

      - name: Install Flatpak SDK
        run: |
          flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
          flatpak install -y --noninteractive flathub org.freedesktop.Platform//23.08 org.freedesktop.Sdk//23.08

      - uses: dtolnay/rust-toolchain@stable

      - uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Install frontend dependencies
        run: npm ci
        working-directory: frontend

      - name: Download bundled binaries
        run: bash packaging/scripts/download-deps.sh x86_64

      - name: Build Tauri binary (no bundle)
        run: |
          cargo install tauri-cli --version "^2" --locked
          cargo tauri build --no-bundle
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Build Flatpak bundle
        run: |
          flatpak-builder --force-clean --repo=repo build-dir packaging/flatpak/com.github.altgo.yml
          flatpak build-bundle repo altgo-x86_64.flatpak com.github.altgo

      - name: Upload flatpak artifact
        uses: actions/upload-artifact@v4
        with:
          name: altgo-x86_64-flatpak
          path: altgo-x86_64.flatpak
          if-no-files-found: error

  gen-aur:
    name: Generate AUR PKGBUILD
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Get version from tag
        id: version
        run: echo "value=${GITHUB_REF_NAME#v}" >> $GITHUB_OUTPUT

      - name: Generate PKGBUILD
        run: |
          mkdir -p aur-output
          cd aur-output
          bash ../packaging/aur/generate-pkgbuild.sh "${{ steps.version.outputs.value }}"

      - name: Upload AUR artifacts
        uses: actions/upload-artifact@v4
        with:
          name: altgo-aur-pkgbuild
          path: |
            aur-output/PKGBUILD
            aur-output/.SRCINFO
          if-no-files-found: warn

  release:
    name: Create Release
    needs: [build-deb, build-rpm, build-appimage, build-flatpak, gen-aur]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Write release notes from CHANGELOG
        env:
          GITHUB_REF_NAME: ${{ github.ref_name }}
          GITHUB_REPOSITORY: ${{ github.repository }}
        run: bash packaging/scripts/extract-release-notes.sh release_notes.md

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: release-assets
          merge-multiple: true

      - name: List release assets
        run: find release-assets -type f | sort

      - name: Generate checksums
        run: |
          cd release-assets
          find . -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' -o -name '*.flatpak' \) -exec sha256sum {} \; > ../checksums.txt
          cd ..
          cat checksums.txt

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          body_path: release_notes.md
          files: |
            release-assets/*
            checksums.txt
```

- [ ] **Step 2: Validate YAML syntax**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))" 2>/dev/null || python3 -c "
import sys
# Basic check: ensure file is valid YAML-like structure
with open('.github/workflows/release.yml') as f:
    content = f.read()
    assert 'jobs:' in content
    assert 'build-deb:' in content
    assert 'build-rpm:' in content
    assert 'build-appimage:' in content
    assert 'build-flatpak:' in content
    assert 'gen-aur:' in content
    assert 'release:' in content
print('Structure OK')
"
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "feat: unified release workflow with deb, rpm, AppImage, Flatpak, AUR"
```

---

### Task 7: Delete standalone AppImage workflow

**Files:**
- Delete: `.github/workflows/appimage.yml`

- [ ] **Step 1: Remove the file**

```bash
git rm .github/workflows/appimage.yml
```

- [ ] **Step 2: Commit**

```bash
git commit -m "chore: remove standalone appimage.yml (integrated into release.yml)"
```

---

### Task 8: Validate end-to-end

- [ ] **Step 1: Verify all new files exist**

```bash
ls -la packaging/scripts/versions.sh packaging/flatpak/com.github.altgo.yml packaging/aur/PKGBUILD.in packaging/aur/generate-pkgbuild.sh
```

Expected: all 4 files listed

- [ ] **Step 2: Verify versions.sh is sourced correctly**

```bash
source packaging/scripts/versions.sh && echo "whisper=${WHISPER_CPP_VERSION} ffmpeg=${FFMPEG_VERSION}"
```

Expected: `whisper=1.8.4 ffmpeg=7.1.1`

- [ ] **Step 3: Verify tauri.conf.json has rpm target**

```bash
python3 -c "
import json
conf = json.load(open('src-tauri/tauri.conf.json'))
targets = conf['bundle']['targets']
assert 'deb' in targets, 'deb missing'
assert 'rpm' in targets, 'rpm missing'
assert 'rpm' in conf['bundle']['linux'], 'rpm depends missing'
print(f'targets: {targets}')
print(f'rpm depends: {conf[\"bundle\"][\"linux\"][\"rpm\"][\"depends\"]}')
"
```

Expected: targets include both `deb` and `rpm`, rpm depends listed

- [ ] **Step 4: Verify appimage.yml is deleted**

```bash
test ! -f .github/workflows/appimage.yml && echo "OK: appimage.yml deleted"
```

- [ ] **Step 5: Verify release.yml has all jobs**

```bash
python3 -c "
import yaml
wf = yaml.safe_load(open('.github/workflows/release.yml'))
jobs = list(wf['jobs'].keys())
expected = ['build-deb', 'build-rpm', 'build-appimage', 'build-flatpak', 'gen-aur', 'release']
for j in expected:
    assert j in jobs, f'{j} missing'
print(f'Jobs: {jobs}')
"
```

Expected: all 6 jobs present

- [ ] **Step 6: Run existing tests to ensure no regressions**

```bash
cargo test --manifest-path=src-tauri/Cargo.toml
```

Expected: all tests pass

- [ ] **Step 7: Final commit with all changes**

```bash
git add -A
git status
```

Verify only expected files are staged, then:
```bash
git commit -m "feat: multi-platform Linux release pipeline

- Add RPM bundle target to tauri.conf.json
- Add Flatpak manifest (packaging/flatpak/)
- Add AUR PKGBUILD template and generator (packaging/aur/)
- Add unified versions.sh for whisper.cpp/ffmpeg versions
- Integrate AppImage build into release.yml
- Remove standalone appimage.yml
- Release workflow now produces: deb, rpm, AppImage, Flatpak, AUR PKGBUILD"
```
