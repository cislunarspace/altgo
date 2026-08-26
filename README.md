# altgo

**English** | [简体中文](README.zh-CN.md)

![altgo](assets/banner.png)

[![CI](https://github.com/cislunarspace/altgo/actions/workflows/ci.yml/badge.svg)](https://github.com/cislunarspace/altgo/actions/workflows/ci.yml)
[![Documentation](https://img.shields.io/badge/docs-online-2f6feb)](https://cislunarspace.github.io/altgo/)
[![Release](https://img.shields.io/github/v/release/cislunarspace/altgo)](https://github.com/cislunarspace/altgo/releases)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**altgo** is a desktop voice-to-text tool. Hold the trigger key, speak, and release — recording, transcription, and optional polishing run automatically. The result is written to the system clipboard and shown in an overlay window.

It supports **Linux** (Ubuntu 22.04+) on **x86_64** and **aarch64**, as well as **Windows 10+** (x86_64 and arm64). macOS is not supported yet.

- [Online documentation](https://cislunarspace.github.io/altgo/)
- [Download from Releases](https://github.com/cislunarspace/altgo/releases)
- [Report an issue](https://github.com/cislunarspace/altgo/issues)

## Features

- Hold the right Alt key to record; transcription runs automatically when you release
- Double-click the right Alt key for continuous recording; click once more to stop
- Local SenseVoice transcription (embedded sherpa-onnx): the model loads once for fast response, and models can be downloaded and managed on the Settings page
- Text polishing through LLMs, supporting both OpenAI-compatible APIs and the Anthropic Messages API
- Results are written to the clipboard and shown in the overlay window, where they can be copied again
- Automatic update checks: silently checked at startup with a prompt, plus manual checks on the Settings page (in-place updates or guidance to the download page, depending on the install method)
- Tray icon to show the main window or quit the app
- Local transcription history: view, copy, delete, clear, and re-polish entries
- Only text is saved; audio is never stored

## Installation

### Linux

Add your current user to the `input` group before installing, otherwise the keyboard device cannot be read. Log out and log back in afterwards:

```bash
sudo usermod -aG input "$USER"
```

Then:

1. Download the matching package from [Releases](https://github.com/cislunarspace/altgo/releases): `.deb`, `.rpm`, or `.AppImage`.
2. Install the downloaded package, for example:

   ```bash
   sudo apt install ./altgo_*.deb
   # or
   sudo dnf install ./altgo-*.rpm
   ```

   The `.AppImage` needs no installation — grant execute permission and run it directly:

   ```bash
   chmod +x altgo_*.AppImage && ./altgo_*.AppImage
   ```

   Unlike `.deb`/`.rpm`, the AppImage does not resolve dependencies automatically. If a library is missing, install it yourself following the dependency notes in the next paragraph.

3. After logging back in, start altgo and complete transcription setup on the Settings page.

The `.deb` and `.rpm` packages declare dependencies covering desktop integration, audio, clipboard, notifications, and `evtest`. On Wayland, make sure `evtest` is installed and your user can read `/dev/input/event*`.

### Windows

Download an installer from [Releases](https://github.com/cislunarspace/altgo/releases):

- `*-setup.exe` (NSIS installer): double-click to install; suitable for most users.
- `*.msi`: for enterprise environments that require MSI deployment.

Pick the package matching your device architecture for x64 and arm64. After installation, start altgo from the Start menu and complete transcription setup on the Settings page.

## Quick Start

After launching the app, complete the following on the **Settings** page:

1. Download and select a local SenseVoice model.
2. Set the polish level and the polishing provider as needed.
3. Confirm the trigger key — the right Alt key by default.
4. Click Save.

Long-press mode is the default:

```text
Press right Alt → start recording → release right Alt → transcribe → polish (optional) → clipboard + overlay
```

For longer speech, double-click the right Alt key to enter continuous recording, then click once to stop:

```text
Double-click right Alt → continuous recording → single click right Alt → transcribe → polish (optional) → clipboard + overlay
```

Transcription history is saved by default at:

```text
~/.config/altgo/history.json
```

## Documentation

- [Online documentation](https://cislunarspace.github.io/altgo/): quick start, usage, and architecture
- [Configuration guide](https://cislunarspace.github.io/altgo/docs/configuration): config file fields, environment variables, and log levels
- [FAQ](https://cislunarspace.github.io/altgo/docs/faq): troubleshooting for keys, recording, transcription, polishing, and the clipboard
- [`CONTRIBUTING.md`](CONTRIBUTING.md): development environment, build, tests, CI, releases, and documentation site deployment
- [`docs/architecture.md`](docs/architecture.md) and [`AGENTS.md`](AGENTS.md): system architecture and core module reference
- [`docs/README.md`](docs/README.md): index of design and planning documents
- [`CHANGELOG.md`](CHANGELOG.md): version history

## License

[MIT License](LICENSE)
