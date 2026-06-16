# 与 Linux 上 `make build` 等价：按需拉取 ffmpeg / whisper-cli → cargo tauri build → 拷贝到 src-tauri/target/release/bin/
# 在仓库根目录执行：pwsh packaging/scripts/build.ps1
# 亦可使用根目录的 .\build.ps1

$ErrorActionPreference = "Stop"

$ScriptDir = $PSScriptRoot
$RepoRoot = (Resolve-Path (Join-Path $ScriptDir "..\..")).Path
Set-Location $RepoRoot

$BinDeps = Join-Path $RepoRoot "target/deps/bin"
$Placeholder = "00_tauri_deps_placeholder.txt"
$Ffmpeg = Join-Path $BinDeps "ffmpeg.exe"
$Whisper = Join-Path $BinDeps "whisper-cli.exe"

if (-not (Test-Path $Ffmpeg) -or -not (Test-Path $Whisper)) {
    Write-Host "[INFO] Missing ffmpeg.exe or whisper-cli.exe under target/deps/bin — running download-deps.ps1" `
        -ForegroundColor Cyan
    & (Join-Path $ScriptDir "download-deps-windows.ps1")
}

Write-Host "[INFO] cargo tauri build ..." -ForegroundColor Cyan
$tauri = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $tauri) {
    Write-Error "cargo not found. Install Rust: https://rustup.rs/"
    exit 1
}
& cargo tauri build
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$ReleaseBin = Join-Path $RepoRoot "src-tauri/target/release/bin"
New-Item -ItemType Directory -Force -Path $ReleaseBin | Out-Null

if (-not (Test-Path $BinDeps)) {
    Write-Error "Expected directory missing: $BinDeps"
    exit 1
}

$any = $false
Get-ChildItem -Path $BinDeps -File -ErrorAction SilentlyContinue | ForEach-Object {
    if ($_.Name -eq $Placeholder) {
        return
    }
    $any = $true
    $dest = Join-Path $ReleaseBin $_.Name
    Copy-Item -LiteralPath $_.FullName -Destination $dest -Force
    Write-Host ("bundled {0} -> {1}" -f $_.Name, $ReleaseBin) -ForegroundColor Green
}

if (-not $any) {
    Write-Error "No files to bundle from $BinDeps (only placeholder? run download-deps-windows.ps1)"
    exit 1
}

$whisperRelease = Join-Path $ReleaseBin "whisper-cli.exe"
if (-not (Test-Path $whisperRelease)) {
    Write-Error "whisper-cli.exe missing in $ReleaseBin — run packaging/scripts/download-deps-windows.ps1"
    exit 1
}

Write-Host ""
Write-Host "[OK] Run: src-tauri\target\release\altgo.exe   (GGML model from Settings)" -ForegroundColor Green
Write-Host "     MSI / bundle under src-tauri\target\release\bundle\ if configured." -ForegroundColor DarkGray
