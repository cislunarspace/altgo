#Requires -Version 5.1
<#
.SYNOPSIS
    altgo one-click installer for Windows.

.DESCRIPTION
    Downloads and configures all dependencies for altgo voice-to-text tool.
    - Checks/installs Rust toolchain
    - Checks ffmpeg availability
    - Builds altgo from source
    - Downloads whisper.cpp prebuilt binary
    - Downloads Whisper model from HuggingFace
    - Generates config file with correct paths

.PARAMETER SkipRust
    Skip Rust toolchain check.

.PARAMETER SkipBuild
    Skip building altgo.

.PARAMETER SkipWhisper
    Skip whisper.cpp download.

.PARAMETER SkipModel
    Skip model download.

.PARAMETER Model
    Model size: tiny, base (default), small, medium, large.

.EXAMPLE
    .\install.ps1
    .\install.ps1 -Model small
    .\install.ps1 -SkipBuild -SkipModel
#>

[CmdletBinding()]
param(
    [switch]$SkipRust,
    [switch]$SkipBuild,
    [switch]$SkipWhisper,
    [switch]$SkipModel,
    [ValidateSet("tiny", "base", "small", "medium", "large")]
    [string]$Model = "base"
)

$ErrorActionPreference = "Stop"

# ─── Model metadata ─────────────────────────────────────────────────────────
$ModelFiles = @{
    tiny   = "ggml-tiny.bin"
    base   = "ggml-base.bin"
    small  = "ggml-small.bin"
    medium = "ggml-medium.bin"
    large  = "ggml-large-v3.bin"
}

$ModelSizes = @{
    tiny   = "75MB"
    base   = "142MB"
    small  = "466MB"
    medium = "1.5GB"
    large  = "2.9GB"
}

$ModelFile = $ModelFiles[$Model]
$ModelUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$ModelFile"

# ─── Resolve directories ────────────────────────────────────────────────────
$ProjectDir = $PSScriptRoot
if (-not (Test-Path "$ProjectDir\Cargo.toml")) {
    Write-Host "[ERROR] Cargo.toml not found in $ProjectDir. Run from altgo project root." -ForegroundColor Red
    exit 1
}

$DepsDir = Join-Path $ProjectDir ".deps"
$BinDir = Join-Path $DepsDir "bin"
$ModelsDir = Join-Path $DepsDir "models"

Write-Host ""
Write-Host "=== altgo Installer for Windows ===" -ForegroundColor Blue
Write-Host ""
Write-Host "[INFO] Project directory: $ProjectDir" -ForegroundColor Blue
Write-Host "[INFO] Dependencies directory: $DepsDir" -ForegroundColor Blue

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
New-Item -ItemType Directory -Force -Path $ModelsDir | Out-Null

# ─── Step 1: Check Rust toolchain ───────────────────────────────────────────
function Check-Rust {
    if ($SkipRust) {
        Write-Host "[INFO] Skipping Rust toolchain check (-SkipRust)" -ForegroundColor Blue
        return
    }

    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargo) {
        Write-Host "[OK] Rust toolchain found: $(cargo --version)" -ForegroundColor Green
        return
    }

    Write-Host "[WARN] Rust toolchain not found." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  Install Rust via one of:"
    Write-Host "    winget install Rustlang.Rustup"
    Write-Host "    OR download from https://rustup.rs"
    Write-Host ""

    $answer = Read-Host "Install Rust now via winget? [y/N]"
    if ($answer -eq 'y' -or $answer -eq 'Y') {
        winget install Rustlang.Rustup --accept-source-agreements --accept-package-agreements
        # Refresh PATH for current session
        $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "Machine") + ";" +
                     [System.Environment]::GetEnvironmentVariable("PATH", "User")

        $cargo = Get-Command cargo -ErrorAction SilentlyContinue
        if ($cargo) {
            Write-Host "[OK] Rust installed: $(cargo --version)" -ForegroundColor Green
        } else {
            Write-Host "[ERROR] Rust installed but cargo not found. Restart your terminal and re-run." -ForegroundColor Red
            exit 1
        }
    } else {
        Write-Host "[ERROR] Rust toolchain is required. Aborting." -ForegroundColor Red
        exit 1
    }
}

# ─── Step 2: Check ffmpeg ───────────────────────────────────────────────────
function Check-FFmpeg {
    Write-Host "[INFO] Checking ffmpeg..." -ForegroundColor Blue

    $ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue
    if ($ffmpeg) {
        Write-Host "[OK] ffmpeg found: $($ffmpeg.Source)" -ForegroundColor Green
        return
    }

    # Also check winget install path
    $wingetPath = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Gyan.FFmpeg*" -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($wingetPath) {
        $ffmpegExe = Get-ChildItem -Path $wingetPath.FullName -Filter "ffmpeg.exe" -Recurse |
            Select-Object -First 1
        if ($ffmpegExe) {
            Write-Host "[OK] ffmpeg found at: $($ffmpegExe.FullName)" -ForegroundColor Green
            return
        }
    }

    Write-Host "[WARN] ffmpeg not found." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  altgo needs ffmpeg for audio recording on Windows."
    Write-Host "  Install via one of:"
    Write-Host "    winget install Gyan.FFmpeg"
    Write-Host "    OR download from https://www.gyan.dev/ffmpeg/builds/"
    Write-Host ""
    Write-Host "  Alternatively, install sox: winget install sox"
    Write-Host ""

    $answer = Read-Host "Continue anyway? [y/N]"
    if ($answer -ne 'y' -and $answer -ne 'Y') {
        exit 1
    }
}

# ─── Step 3: Build altgo ───────────────────────────────────────────────────
function Build-Altgo {
    if ($SkipBuild) {
        Write-Host "[INFO] Skipping altgo build (-SkipBuild)" -ForegroundColor Blue
        return
    }

    Write-Host "[INFO] Building altgo (cargo build --release)..." -ForegroundColor Blue
    Push-Location $ProjectDir
    try {
        cargo build --release
        Copy-Item "target\release\altgo.exe" ".\altgo.exe" -Force
        Write-Host "[OK] altgo built and copied to $ProjectDir\altgo.exe" -ForegroundColor Green
    } finally {
        Pop-Location
    }
}

# ─── Step 4: Download whisper.cpp ───────────────────────────────────────────
function Install-Whisper {
    if ($SkipWhisper) {
        Write-Host "[INFO] Skipping whisper.cpp download (-SkipWhisper)" -ForegroundColor Blue
        return
    }

    $whisperExe = Join-Path $BinDir "whisper-cli.exe"

    if (Test-Path $whisperExe) {
        Write-Host "[OK] whisper-cli.exe already installed at $whisperExe" -ForegroundColor Green
        return
    }

    # Check if already on system
    $sysWhisper = Get-Command whisper-cli -ErrorAction SilentlyContinue
    if ($sysWhisper) {
        Write-Host "[OK] Found whisper-cli on PATH, copying to $whisperExe" -ForegroundColor Blue
        Copy-Item $sysWhisper.Source $whisperExe -Force
        # Also copy any companion DLLs from the same directory
        $dllDir = Split-Path $sysWhisper.Source
        Get-ChildItem -Path $dllDir -Filter "*.dll" | Copy-Item -Destination $BinDir -Force
        return
    }

    Write-Host "[INFO] Downloading whisper.cpp prebuilt binary..." -ForegroundColor Blue

    # Get latest release info from GitHub API
    try {
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/ggml-org/whisper.cpp/releases/latest" `
            -Headers @{ "User-Agent" = "altgo-installer" }
    } catch {
        Write-Host "[ERROR] Failed to query GitHub API for whisper.cpp releases." -ForegroundColor Red
        Write-Host "  Error: $_" -ForegroundColor Red
        Write-Host "  Try downloading manually from https://github.com/ggml-org/whisper.cpp/releases" -ForegroundColor Yellow
        exit 1
    }

    # Find the x64 zip asset
    $asset = $release.assets | Where-Object { $_.name -eq "whisper-bin-x64.zip" } | Select-Object -First 1
    if (-not $asset) {
        Write-Host "[ERROR] Could not find whisper-bin-x64.zip in latest release." -ForegroundColor Red
        Write-Host "  Available assets:" -ForegroundColor Yellow
        $release.assets | ForEach-Object { Write-Host "    $($_.name)" -ForegroundColor Yellow }
        exit 1
    }

    $downloadUrl = $asset.browser_download_url
    $tempZip = Join-Path $env:TEMP "whisper-bin-x64.zip"

    Write-Host "[INFO] Downloading $($asset.name)..." -ForegroundColor Blue
    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $tempZip -UseBasicParsing
    } catch {
        Write-Host "[ERROR] Failed to download whisper.cpp: $_" -ForegroundColor Red
        exit 1
    }

    # Extract
    Write-Host "[INFO] Extracting to $BinDir..." -ForegroundColor Blue
    $tempExtract = Join-Path $env:TEMP "whisper-cpp-extract"
    if (Test-Path $tempExtract) { Remove-Item $tempExtract -Recurse -Force }
    Expand-Archive -Path $tempZip -DestinationPath $tempExtract -Force

    # Find and copy whisper-cli.exe and companion DLLs
    $extractedExe = Get-ChildItem -Path $tempExtract -Filter "whisper-cli.exe" -Recurse | Select-Object -First 1
    if (-not $extractedExe) {
        # Older releases may name it "main.exe"
        $extractedExe = Get-ChildItem -Path $tempExtract -Filter "main.exe" -Recurse | Select-Object -First 1
        if ($extractedExe) {
            Copy-Item $extractedExe.FullName $whisperExe -Force
        }
    } else {
        Copy-Item $extractedExe.FullName $whisperExe -Force
    }

    if (-not (Test-Path $whisperExe)) {
        Write-Host "[ERROR] Could not find whisper-cli.exe in downloaded archive." -ForegroundColor Red
        exit 1
    }

    # Copy all companion DLLs from the same directory as the exe
    $dllSourceDir = Split-Path $extractedExe.FullName
    Get-ChildItem -Path $dllSourceDir -Filter "*.dll" | ForEach-Object {
        Copy-Item $_.FullName $BinDir -Force
        Write-Host "[INFO] Copied $($_.Name)" -ForegroundColor Blue
    }

    # Cleanup
    Remove-Item $tempZip -Force -ErrorAction SilentlyContinue
    Remove-Item $tempExtract -Recurse -Force -ErrorAction SilentlyContinue

    Write-Host "[OK] whisper-cli installed to $whisperExe" -ForegroundColor Green
}

# ─── Step 5: Download model ─────────────────────────────────────────────────
function Download-Model {
    if ($SkipModel) {
        Write-Host "[INFO] Skipping model download (-SkipModel)" -ForegroundColor Blue
        return
    }

    $modelPath = Join-Path $ModelsDir $ModelFile

    if (Test-Path $modelPath) {
        Write-Host "[OK] Model already exists: $modelPath" -ForegroundColor Green
        return
    }

    $sizeLabel = $ModelSizes[$Model]
    Write-Host "[INFO] Downloading $Model model ($ModelFile, ~$sizeLabel)..." -ForegroundColor Blue
    Write-Host "[INFO] URL: $ModelUrl" -ForegroundColor Blue

    try {
        # Use WebClient for better progress display on PowerShell 5.1
        $wc = New-Object System.Net.WebClient
        $wc.DownloadFile($ModelUrl, "$modelPath.tmp")
        Move-Item "$modelPath.tmp" $modelPath -Force
    } catch {
        Remove-Item "$modelPath.tmp" -Force -ErrorAction SilentlyContinue
        Write-Host "[ERROR] Failed to download model: $_" -ForegroundColor Red
        exit 1
    }

    # Verify file size (at least 10MB)
    $fileInfo = Get-Item $modelPath
    if ($fileInfo.Length -lt 10MB) {
        Remove-Item $modelPath -Force
        Write-Host "[ERROR] Downloaded model is too small ($($fileInfo.Length) bytes). Download may have failed." -ForegroundColor Red
        exit 1
    }

    Write-Host "[OK] Model downloaded: $modelPath" -ForegroundColor Green
}

# ─── Step 6: Generate config ────────────────────────────────────────────────
function Generate-Config {
    $configDir = Join-Path $env:APPDATA "altgo"
    $configPath = Join-Path $configDir "altgo.toml"

    # Compute absolute paths with forward slashes (TOML-safe on Windows)
    $whisperPath = (Join-Path $BinDir "whisper-cli.exe") -replace "\\", "/"
    $modelPath = (Join-Path $ModelsDir $ModelFile) -replace "\\", "/"

    if (Test-Path $configPath) {
        Write-Host "[WARN] Config file already exists: $configPath" -ForegroundColor Yellow
        $answer = Read-Host "Overwrite with new configuration? [y/N]"
        if ($answer -ne 'y' -and $answer -ne 'Y') {
            Write-Host "[INFO] Keeping existing config. Update these fields manually:" -ForegroundColor Blue
            Write-Host "  whisper_path = `"$whisperPath`""
            Write-Host "  model = `"$modelPath`""
            return
        }
        # Backup existing config
        Copy-Item $configPath "$configPath.bak" -Force
        Write-Host "[INFO] Backed up existing config to $configPath.bak" -ForegroundColor Blue
    }

    New-Item -ItemType Directory -Force -Path $configDir | Out-Null

    # Read template (UTF-8 with BOM) and substitute paths.
    # install_config.toml is stored as UTF-8 with BOM so PowerShell 5.1 reads it correctly.
    $templatePath = Join-Path $ProjectDir "install_config.toml"
    $configContent = [System.IO.File]::ReadAllText($templatePath, [System.Text.UTF8]::new($true)) `
        -replace "\{MODEL_PATH\}", $modelPath `
        -replace "\{WHISPER_PATH\}", $whisperPath

    # Write with BOM so PowerShell 5.1 reads UTF-8 correctly (avoids Chinese garbling)
    $utf8 = [System.Text.UTF8Encoding]::new($true)
    [System.IO.File]::WriteAllText($configPath, $configContent, $utf8)
    Write-Host "[OK] Config written to $configPath" -ForegroundColor Green
}

# ─── Step 7: Verification ───────────────────────────────────────────────────
function Verify-Install {
    Write-Host ""
    Write-Host "=== Installation Summary ===" -ForegroundColor Blue
    Write-Host ""

    $altgoExe = Join-Path $ProjectDir "altgo.exe"
    if (Test-Path $altgoExe) {
        Write-Host "[OK] altgo binary: $altgoExe" -ForegroundColor Green
    } else {
        Write-Host "[WARN] altgo binary not found at $altgoExe" -ForegroundColor Yellow
    }

    $whisperExe = Join-Path $BinDir "whisper-cli.exe"
    if (Test-Path $whisperExe) {
        Write-Host "[OK] whisper-cli: $whisperExe" -ForegroundColor Green
    } else {
        Write-Host "[WARN] whisper-cli not found at $whisperExe" -ForegroundColor Yellow
    }

    $modelPath = Join-Path $ModelsDir $ModelFile
    if (Test-Path $modelPath) {
        Write-Host "[OK] Whisper model: $modelPath" -ForegroundColor Green
    } else {
        Write-Host "[WARN] Whisper model not found at $modelPath" -ForegroundColor Yellow
    }

    $configPath = Join-Path $env:APPDATA "altgo\altgo.toml"
    if (Test-Path $configPath) {
        Write-Host "[OK] Config file: $configPath" -ForegroundColor Green
    } else {
        Write-Host "[WARN] Config file not found at $configPath" -ForegroundColor Yellow
    }

    Write-Host ""
    Write-Host "To start altgo, run:" -ForegroundColor Blue
    Write-Host "  $altgoExe"
    Write-Host ""
}

# ─── Main ────────────────────────────────────────────────────────────────────
Check-Rust
Check-FFmpeg
Build-Altgo
Install-Whisper
Download-Model
Generate-Config
Verify-Install
