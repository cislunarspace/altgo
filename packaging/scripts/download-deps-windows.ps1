# Download platform dependencies for altgo Windows packaging.
# Usage: pwsh packaging/scripts/download-deps-windows.ps1
#
# Mirrors packaging/scripts/download-deps.sh: writes ffmpeg.exe and
# whisper-cli.exe into target/deps/bin/ for Tauri to bundle.
#
# Versions are read from packaging/scripts/versions.json so PowerShell
# does not need to parse a bash file.

$ErrorActionPreference = "Stop"

$ScriptDir = $PSScriptRoot
$RepoRoot = (Resolve-Path (Join-Path $ScriptDir "..\..")).Path
$BinDir = Join-Path $RepoRoot "target/deps/bin"
$VersionsFile = Join-Path $ScriptDir "versions.json"

if (-not (Test-Path $VersionsFile)) {
    Write-Error "versions.json not found: $VersionsFile"
    exit 1
}

$Versions = Get-Content -LiteralPath $VersionsFile -Raw | ConvertFrom-Json
$WhisperVersion = $Versions.whisperCppVersion
$FfmpegVersion = $Versions.ffmpegVersion

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

# ─── ffmpeg (Windows static build) ──────────────────────────────────────────
$FfmpegTarget = Join-Path $BinDir "ffmpeg.exe"
if (Test-Path $FfmpegTarget) {
    Write-Host "[OK] ffmpeg.exe already exists at $FfmpegTarget"
} else {
    Write-Host "[INFO] Downloading ffmpeg $FfmpegVersion (Windows x64)..."
    # BtbN/FFmpeg-Builds release artifacts: filename is stable across versions.
    $FfmpegUrl = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
    $TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("altgo-ffmpeg-" + [System.Guid]::NewGuid().ToString("N"))
    $ZipPath = Join-Path $TmpDir "ffmpeg.zip"
    New-Item -ItemType Directory -Force -Path $TmpDir | Out-Null
    try {
        Invoke-WebRequest -Uri $FfmpegUrl -OutFile $ZipPath -UseBasicParsing
        Expand-Archive -LiteralPath $ZipPath -DestinationPath $TmpDir -Force
        $FfmpegBin = Get-ChildItem -Path $TmpDir -Recurse -Filter "ffmpeg.exe" -File | Select-Object -First 1 -ExpandProperty FullName
        if (-not $FfmpegBin) {
            throw "ffmpeg.exe not found inside the downloaded archive"
        }
        Copy-Item -LiteralPath $FfmpegBin -Destination $FfmpegTarget -Force
        Write-Host "[OK] ffmpeg.exe downloaded to $FfmpegTarget"
    } finally {
        Remove-Item -LiteralPath $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# ─── whisper-cli (Windows prebuilt) ─────────────────────────────────────────
# Upstream whisper.cpp ships Windows binaries on its GitHub releases.
$WhisperTarget = Join-Path $BinDir "whisper-cli.exe"
if (Test-Path $WhisperTarget) {
    Write-Host "[OK] whisper-cli.exe already exists at $WhisperTarget"
} else {
    $WhisperTag = "v$WhisperVersion"
    $WhisperUrl = "https://github.com/ggml-org/whisper.cpp/releases/download/${WhisperTag}/whisper-bin-x64.zip"
    Write-Host "[INFO] Downloading whisper-cli $WhisperTag (Windows x64)..."
    $TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("altgo-whisper-" + [System.Guid]::NewGuid().ToString("N"))
    $ZipPath = Join-Path $TmpDir "whisper.zip"
    New-Item -ItemType Directory -Force -Path $TmpDir | Out-Null
    try {
        Invoke-WebRequest -Uri $WhisperUrl -OutFile $ZipPath -UseBasicParsing
        Expand-Archive -LiteralPath $ZipPath -DestinationPath $TmpDir -Force
        $WhisperBin = Get-ChildItem -Path $TmpDir -Recurse -Filter "whisper-cli.exe" -File | Select-Object -First 1 -ExpandProperty FullName
        if (-not $WhisperBin) {
            throw "whisper-cli.exe not found inside the downloaded archive"
        }
        Copy-Item -LiteralPath $WhisperBin -Destination $WhisperTarget -Force
        Write-Host "[OK] whisper-cli.exe downloaded to $WhisperTarget"
    } finally {
        Remove-Item -LiteralPath $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Write-Host ""
Write-Host "[OK] All Windows dependencies ready in $BinDir"
