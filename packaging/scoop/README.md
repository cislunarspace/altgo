# Scoop 清单（ScoopInstaller/Extras 提交流程）

`altgo.json` 是提交到 [ScoopInstaller/Extras](https://github.com/ScoopInstaller/Extras)
的清单，直接复用 Release 的 NSIS 安装包（Scoop 用 7zip 解压 NSIS，`altgo.exe` 位于根目录，
已实际验证）。

## 提交流程

1. fork `ScoopInstaller/Extras`，把 `altgo.json` 放到 `bucket/` 目录，提 PR。
2. 清单含 `checkver`（GitHub release）与 `autoupdate`（sha256 从 Release 的
   `checksums.txt` 提取），之后发版由 Extras 的自动更新机器人跟进，无需手工维护。

## 注意

- `pre_install` 清理 NSIS 解压残留（`$PLUGINSDIR`、卸载器），这是 Extras 中
  NSIS 包的通行写法。
- 待 Windows arm64 构建（#131）落地后，在 `architecture` 下追加 `arm64` 条目即可。

# Scoop Manifest (ScoopInstaller/Extras Submission)

`altgo.json` is the manifest submitted to [ScoopInstaller/Extras](https://github.com/ScoopInstaller/Extras).
It reuses the NSIS installer from Releases directly (Scoop unpacks NSIS with 7zip, and
`altgo.exe` sits at the root — verified in practice).

## Submission Process

1. Fork `ScoopInstaller/Extras`, place `altgo.json` in the `bucket/` directory, and open a PR.
2. The manifest includes `checkver` (GitHub release) and `autoupdate` (sha256 taken from the
   release's `checksums.txt`); after that, subsequent releases are handled by the Extras
   auto-update bot with no manual maintenance.

## Notes

- `pre_install` cleans up NSIS extraction leftovers (`$PLUGINSDIR`, uninstaller); this is the
  customary pattern for NSIS packages in Extras.
- Once the Windows arm64 build (#131) lands, just add an `arm64` entry under `architecture`.
