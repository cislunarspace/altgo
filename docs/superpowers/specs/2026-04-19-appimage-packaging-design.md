# AppImage Packaging Design for altgo

## Goal

Package altgo as a self-contained AppImage so users do not need to install system dependencies beyond WebKit.

## Requirements

- Self-contained: includes ffmpeg and whisper-cli
- whisper-cli: built from source during AppImage creation
- ffmpeg: use existing pre-built static binary from johnvansickle.com
- Target: Ubuntu 20.04+ base (glibc 2.31 compatibility)
- Architectures: x86_64 and aarch64 (ARM64)
- System WebKit: NOT bundled — users install `libwebkit2gtk-4.1-0` via package manager
- Build trigger: separate manual/scheduled process, not automatic on every release tag

## Architecture

### Output Artifacts

Per release version (e.g., v1.4.0):
- `altgo-x86_64-v1.4.0.AppImage` — for most Linux users
- `altgo-aarch64-v1.4.0.AppImage` — for ARM64 devices (Raspberry Pi 4, Apple Silicon via Rosetta)

Each AppImage contains:
- Tauri application binary
- Pre-built static ffmpeg binary
- whisper-cli compiled from source
- Required runtime assets (icons, frontend bundle)

### Build Strategy

**x86_64**: Native GitHub Actions (`ubuntu-22.04`)
- Fast build (~10-15 mins)
- Checkout → install deps → build whisper.cpp → download ffmpeg → build Tauri → assemble AppImage

**aarch64**: Docker-based build
- Build inside Ubuntu 20.04 container on `ubuntu-22.04` runner with Docker
- Same steps as x86_64 but cross-compile whisper.cpp for aarch64 using CMake toolchain
- Reproducible, matches target environment exactly

### AppDir Structure

```
altgo.AppDir/
├── AppRun                     # Launch script
├── altgo.desktop              # Desktop entry
└── usr/
    ├── bin/
    │   ├── altgo              # Tauri binary
    │   ├── ffmpeg             # Static ffmpeg
    │   └── whisper-cli        # Built from source
    └── share/
        └── altgo/
            └── icons/         # App icons
```

**AppRun script**: Sets up environment variables (`LD_LIBRARY_PATH`, `$APPDIR/usr/bin` in PATH for bundled tools), then launches the `altgo` binary.

**appimage-builder.yml**: Configuration for `appimage-builder` tool with `id: com.altgo.app`. No system-components — fully self-contained.

## Files to Create/Modify

| File | Action |
|------|--------|
| `.github/workflows/appimage.yml` | Create — manual dispatch workflow |
| `packaging/appimage/build.sh` | Create — build script |
| `packaging/appimage/Dockerfile.aarch64` | Create — Docker container for aarch64 builds |
| `packaging/appimage/appimage-builder.yml` | Create — AppImage builder config |
| `packaging/appimage/AppRun.in` | Create — AppRun template |
| `packaging/appimage/altgo.desktop.in` | Create — desktop entry template |

## GitHub Actions Workflow

**.github/workflows/appimage.yml**

**Inputs:**
- `version` (required) — version tag, e.g., `v1.4.0`
- `arch` (optional) — `x86_64`, `aarch64`, or `both` (default: `both`)

**Jobs:**

1. `build-x86_64`:
   - Runs on `ubuntu-22.04`
   - Checkout code
   - Install: Rust stable, Node 22, `appimage-builder`, `aarch64-linux-gnu` toolchain (for whisper.cpp cross-compile if needed)
   - Build whisper.cpp: `cmake -B build -DCMAKE_BUILD_TYPE=Release && cmake --build build -j$(nproc)`
   - Download ffmpeg static binary via existing `packaging/scripts/download-deps.sh`
   - Build Tauri: `cargo tauri build --no-bundle`
   - Assemble AppImage via `appimage-builder`
   - Upload artifact

2. `build-aarch64`:
   - Runs on `ubuntu-22.04` with Docker
   - Uses `packaging/appimage/Dockerfile.aarch64` (Ubuntu 20.04 base)
   - Same steps inside container
   - Cross-compile whisper.cpp for aarch64 via CMake toolchain
   - Upload artifact

**Artifact naming:** `altgo-{arch}-v{version}.AppImage`

## Error Handling

| Scenario | Handling |
|----------|----------|
| whisper-cli download/build fails | Build fails with clear error message |
| ffmpeg download fails | Build fails with clear error message |
| Version mismatch (input vs Cargo.toml) | Early build failure with version check |
| Disk space insufficient (~10GB needed) | Pre-build space check, fail fast |
| User missing WebKit | AppRun prints error message with install instructions |

## AppImage Limitations

- System WebKit (`libwebkit2gtk-4.1-0`) is NOT bundled — users must install it once via their package manager
- AppImage is NOT signed (no code signing certificate assumed)

## Version

1.4.0
