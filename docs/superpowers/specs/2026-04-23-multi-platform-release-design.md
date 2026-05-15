# Multi-Platform Linux Release Design

## Goal

Unify and expand altgo's release pipeline to automatically produce packages for all major Linux distribution formats on every version tag push: `.deb`, `.rpm`, `.AppImage`, `.flatpak`, and AUR PKGBUILD.

## Requirements

- **Trigger**: All formats auto-built on `v*` tag push (single `release.yml` workflow)
- **Architecture**: x86_64 only
- **Formats**:
  - `.deb` — existing, no changes needed
  - `.rpm` — new, via Tauri bundler
  - `.AppImage` — existing manual workflow, integrate into release
  - `.flatpak` — new, build `.flatpak` file as artifact
  - AUR — generate PKGBUILD template, manual AUR publish
- **Version sync**: Fix whisper.cpp version inconsistency (v1.7.5 in AppImage vs v1.8.4 elsewhere)
- **All artifacts** uploaded to GitHub Release with checksums

## Architecture

### Release Workflow

```
v* tag push
  ├── Job: build-deb        (ubuntu-22.04, cargo tauri build --bundles deb)
  ├── Job: build-rpm        (ubuntu-22.04, cargo tauri build --bundles rpm)
  ├── Job: build-appimage   (ubuntu-22.04, appimage-builder)
  ├── Job: build-flatpak    (ubuntu-22.04, flatpak-builder)
  ├── Job: gen-aur          (ubuntu-22.04, generate PKGBUILD + .SRCINFO)
  └── Job: release          (depends on all above, collects artifacts, creates GitHub Release)
```

All build jobs run in parallel. The `release` job waits for all to complete, then:
1. Downloads all artifacts via `actions/download-artifact`
2. Generates `checksums.txt` with `sha256sum`
3. Extracts release notes from `CHANGELOG.md`
4. Creates GitHub Release with all files

### Job Details

**build-deb** (existing, no changes)
- Runs `cargo tauri build --bundles deb`
- Produces `altgo_amd64.deb`

**build-rpm** (new)
- Runs `cargo tauri build --bundles rpm`
- Produces `altgo-*.rpm`
- Requires `rpm` package on runner

**build-appimage** (integrated from existing `appimage.yml`)
- Runs existing `packaging/appimage/build.sh`
- Produces `altgo-x86_64-v*.AppImage`
- Fix: source `versions.sh` for consistent whisper.cpp version

**build-flatpak** (new)
- Installs `flatpak-builder` and Freedesktop SDK runtime
- Runs `flatpak-builder --repo=repo build-dir packaging/flatpak/com.github.altgo.yml`
- Produces `altgo-x86_64.flatpak` via `flatpak build-bundle`

**gen-aur** (new)
- Downloads whisper-cli and ffmpeg binaries (for checksum calculation)
- Runs `packaging/aur/generate-pkgbuild.sh` to produce PKGBUILD + .SRCINFO
- Uploads as artifact

## Package Format Details

### RPM

Tauri 2.x bundler natively supports RPM. Add to `tauri.conf.json`:

```json
{
  "bundle": {
    "targets": ["deb", "rpm"],
    "linux": {
      "rpm": {
        "depends": [
          "webkit2gtk4.1",
          "gtk3",
          "libayatana-appindicator-gtk3",
          "xclip",
          "pulseaudio-utils",
          "libnotify"
        ]
      }
    }
  }
}
```

### AppImage

No structural changes to existing AppImage build. Key change:
- `packaging/appimage/build.sh` sources `packaging/scripts/versions.sh` for `WHISPER_CPP_VERSION` instead of hardcoding v1.7.5

### Flatpak

New manifest at `packaging/flatpak/com.github.altgo.yml`:

- **Base**: `org.freedesktop.Platform//23.08`
- **SDK**: `org.freedesktop.Sdk//23.08`
- **Modules**:
  1. `altgo` — main binary from Tauri build output
  2. `whisper-cli` — pre-built binary + shared libs
  3. `ffmpeg` — pre-built static binary
- **finish-args**:
  - `--socket=x11`
  - `--socket=wayland`
  - `--socket=pulseaudio`
  - `--share=ipc`
  - `--talk-name=org.freedesktop.Notifications`
  - `--device=input` (keyboard listening via evdev)
  - `--filesystem=home` (config at `~/.config/altgo/`)
  - `--env=PATH=/app/bin:/app/altgo/bin`

### AUR

Template at `packaging/aur/PKGBUILD.in`:

```bash
pkgname=altgo
pkgver=VERSION
pkgrel=1
pkgdesc="Linux voice-to-text tool with local Whisper and LLM polishing"
arch=('x86_64')
url="https://github.com/cislunarspace/altgo"
license=('MIT')
depends=('webkit2gtk-4.1' 'gtk3' 'libayatana-appindicator' 'xclip' 'pulseaudio-utils' 'libnotify')
source=("altgo_${pkgver}_amd64.deb::https://github.com/cislunarspace/altgo/releases/download/v${pkgver}/altgo_amd64.deb")
sha256sums=('SHA256_PLACEHOLDER')

package() {
    bsdtar -xf data.tar.xz -C "$pkgdir"
    install -Dm755 "$pkgdir/usr/bin/altgo" "$pkgdir/usr/bin/altgo"
}
```

`packaging/aur/generate-pkgbuild.sh`:
- Reads `VERSION` from tag or Cargo.toml
- Downloads deb, computes sha256
- Replaces `VERSION` and `SHA256_PLACEHOLDER` in template
- Runs `makepkg --printsrcinfo > .SRCINFO`

## Unified Version Management

New file `packaging/scripts/versions.sh`:

```bash
WHISPER_CPP_VERSION="v1.8.4"
FFMPEG_VERSION="7.1.1"
```

All build scripts source this file:
- `packaging/scripts/download-deps.sh` — already uses v1.8.4, no change needed
- `packaging/appimage/build.sh` — change from hardcoded v1.7.5 to source versions.sh
- `packaging/flatpak/com.github.altgo.yml` — reference versions in module definitions

## Dependency Mapping

| Runtime Tool | deb | rpm | Flatpak | AUR | AppImage |
|-------------|-----|-----|---------|-----|----------|
| WebKit | libwebkit2gtk-4.1-0 | webkit2gtk4.1 | freedesktop runtime | webkit2gtk-4.1 | user installs |
| GTK | libgtk-3-0 | gtk3 | freedesktop runtime | gtk3 | user installs |
| Tray | libayatana-appindicator3-1 | libayatana-appindicator-gtk3 | freedesktop runtime | libayatana-appindicator | user installs |
| Clipboard | xclip | xclip | bundled or runtime | xclip | bundled |
| Audio | pulseaudio-utils | pulseaudio-utils | --socket=pulseaudio | pulseaudio | user installs |
| Notifications | libnotify-bin | libnotify | --talk-name | libnotify | user installs |
| whisper-cli | bundled | bundled | bundled in manifest | bundled in deb | bundled |
| ffmpeg | bundled | bundled | bundled in manifest | bundled in deb | bundled |

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `.github/workflows/release.yml` | Modify | Add build-rpm, build-appimage, build-flatpak, gen-aur jobs |
| `.github/workflows/appimage.yml` | Delete | Functionality moved to release.yml |
| `src-tauri/tauri.conf.json` | Modify | Add `"rpm"` to targets, add `linux.rpm` config |
| `packaging/scripts/versions.sh` | Create | Unified version constants |
| `packaging/appimage/build.sh` | Modify | Source versions.sh, remove hardcoded version |
| `packaging/flatpak/com.github.altgo.yml` | Create | Flatpak manifest |
| `packaging/aur/PKGBUILD.in` | Create | AUR package template |
| `packaging/aur/generate-pkgbuild.sh` | Create | PKGBUILD generation script |

## Testing & Validation

**CI (PR/push)**: No changes — continues to build deb only for fast feedback.

**Release (tag push)**:
- Each build job runs its format's validation:
  - AppImage: `./altgo-x86_64.AppImage --appimage-extract-and-run --version`
  - RPM: `rpm -qip altgo-*.rpm` to verify metadata
  - Flatpak: `flatpak-builder` validates manifest syntax during build
  - AUR: `namcap PKGBUILD` for lint (optional)
- Release job verifies all artifacts exist and checksums match
- Version consistency check: all artifacts' version matches tag

## Out of Scope

- aarch64 support (user chose x86_64 only)
- Flathub submission (user chose build-as-artifact)
- Snap/other formats
- AUR auto-publish (user chose template-only)
- CHANGELOG automation (separate spec exists)
