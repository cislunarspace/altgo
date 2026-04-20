# Changelog

## v1.0.0 (2026-04-14)

### Features

- Cross-platform desktop voice-to-text tool
- Hold right Alt key to record, release to transcribe
- Dual transcription backends: Whisper API (OpenAI-compatible) and local whisper.cpp
- LLM text polishing with 4 levels (none/light/medium/heavy)
- Automatic clipboard output
- Platform-native notifications
- GUI settings panel with real-time config reload

### Platform Support

- **Linux**: x86_64 and ARM64 builds with DEB packages
- **Windows**: x86_64 with MSI installer

### Bug Fixes

- Fix WiX v5 ComponentGroup configuration for MSI builds
- Fix release workflow runner compatibility
- Fix CJK font rendering in GUI panel
- Fix GUI config save guards
- Resolve multiple code quality and safety issues
