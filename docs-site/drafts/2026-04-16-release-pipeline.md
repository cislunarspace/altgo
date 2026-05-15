# Make Release Pipeline Work

## Goal
用户从 GitHub Release 下载安装包即可使用：Windows 下载 MSI/zip，Linux 下载 deb 包。

## Tasks
- [x] 1. Rename binary from `altgo-cli` to `altgo` in Cargo.toml
- [x] 2. Fix release workflow packaging (binary name matches, remove stale GUI deps)
- [x] 3. Fix MSI Product.wxs (exe name)
- [x] 4. Add cargo-deb metadata (proper binary/assets)
- [x] 5. Clean root directory stray files (test.*, PLAN.md, altgo.exe, etc.)
- [x] 6. Simplify README for end-user release install
- [x] 7. Verify clippy/test/build pass

## Notes
- `src-tauri/`, `frontend/`, `docs-site/` are separate sub-projects — not touched
- `.deps/`, `whisper.cpp/` are gitignored — already excluded from repo
