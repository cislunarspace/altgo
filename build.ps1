# Windows 一键发布构建（等同于 Linux 的 `make build`）
$ErrorActionPreference = "Stop"
& "$PSScriptRoot/packaging/scripts/build.ps1" @args
