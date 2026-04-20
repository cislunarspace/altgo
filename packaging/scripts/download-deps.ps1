# Download platform dependencies for altgo packaging (Windows).
# Usage: pwsh scripts/download-deps.ps1
#
# Downloads ffmpeg and whisper-cli into target/deps/bin/

$ErrorActionPreference = "Stop"

$DepsDir = "target/deps"
$BinDir = "$DepsDir/bin"

Write-Host "Downloading dependencies for Windows x86_64..."
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

# --- ffmpeg (Gyan FFmpeg essentials) ---
$FfmpegVersion = "7.1.1"
$FfmpegTarget = "$BinDir/ffmpeg.exe"

if (Test-Path $FfmpegTarget) {
    Write-Host "[OK] ffmpeg already exists at $FfmpegTarget" -ForegroundColor Green
} else {
    Write-Host "[INFO] Downloading ffmpeg $FfmpegVersion..."
    $FfmpegUrl = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    $TmpDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "altgo-deps-ffmpeg-$(Get-Random)")

    $ZipFile = "$TmpDir/ffmpeg.zip"
    Invoke-WebRequest -Uri $FfmpegUrl -OutFile $ZipFile -UseBasicParsing
    Expand-Archive -Path $ZipFile -DestinationPath $TmpDir -Force

    # Find ffmpeg.exe in the extracted directory.
    $FfmpegBin = Get-ChildItem -Path $TmpDir -Recurse -Filter "ffmpeg.exe" |
        Where-Object { $_.FullName -match "essentials" -and $_.FullName -match "bin" } |
        Select-Object -First 1

    if (-not $FfmpegBin) {
        # Fallback: just take the first ffmpeg.exe found.
        $FfmpegBin = Get-ChildItem -Path $TmpDir -Recurse -Filter "ffmpeg.exe" |
            Select-Object -First 1
    }

    if (-not $FfmpegBin) {
        Write-Host "[ERROR] ffmpeg.exe not found in archive" -ForegroundColor Red
        Remove-Item -Recurse -Force $TmpDir
        exit 1
    }

    Copy-Item $FfmpegBin.FullName $FfmpegTarget -Force
    Remove-Item -Recurse -Force $TmpDir
    Write-Host "[OK] ffmpeg downloaded to $FfmpegTarget" -ForegroundColor Green
}

# --- whisper-cli ---
$WhisperVersion = "1.8.4"
$WhisperTarget = "$BinDir/whisper-cli.exe"

if (Test-Path $WhisperTarget) {
    Write-Host "[OK] whisper-cli already exists at $WhisperTarget" -ForegroundColor Green
} else {
    Write-Host "[INFO] Downloading whisper-cli $WhisperVersion..."

    $WhisperUrl = "https://github.com/ggml-org/whisper.cpp/releases/download/v$WhisperVersion/whisper-bin-x64.zip"
    $TmpDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "altgo-deps-whisper-$(Get-Random)")

    try {
        $ZipFile = "$TmpDir/whisper.zip"
        Invoke-WebRequest -Uri $WhisperUrl -OutFile $ZipFile -UseBasicParsing
        Expand-Archive -Path $ZipFile -DestinationPath $TmpDir -Force

        # Find whisper-cli or main binary.
        $WhisperBin = Get-ChildItem -Path $TmpDir -Recurse -Filter "whisper-cli.exe" |
            Select-Object -First 1

        if (-not $WhisperBin) {
            $WhisperBin = Get-ChildItem -Path $TmpDir -Recurse -Filter "main.exe" |
                Select-Object -First 1
        }

        if (-not $WhisperBin) {
            Write-Host "[ERROR] whisper-cli.exe not found in archive" -ForegroundColor Red
            Remove-Item -Recurse -Force $TmpDir
            exit 1
        }

        Copy-Item $WhisperBin.FullName $WhisperTarget -Force
        Write-Host "[OK] whisper-cli downloaded to $WhisperTarget" -ForegroundColor Green
    }
    catch {
        Write-Host "[WARN] whisper-cli prebuilt not available: $_" -ForegroundColor Yellow
        Write-Host "[INFO] Trying alternative download..."

        # Try the AVX2 build as fallback.
        $AltUrl = "https://github.com/ggml-org/whisper.cpp/releases/download/v$WhisperVersion/whisper-bin-x64.zip"
        try {
            Invoke-WebRequest -Uri $AltUrl -OutFile "$TmpDir/whisper-alt.zip" -UseBasicParsing
            Expand-Archive -Path "$TmpDir/whisper-alt.zip" -DestinationPath "$TmpDir/alt" -Force

            $WhisperBin = Get-ChildItem -Path "$TmpDir/alt" -Recurse -Filter "*.exe" |
                Where-Object { $_.Name -match "whisper" -or $_.Name -eq "main.exe" } |
                Select-Object -First 1

            if ($WhisperBin) {
                Copy-Item $WhisperBin.FullName $WhisperTarget -Force
                Write-Host "[OK] whisper-cli downloaded (alt build) to $WhisperTarget" -ForegroundColor Green
            } else {
                Write-Host "[ERROR] No whisper-cli binary found" -ForegroundColor Red
                Remove-Item -Recurse -Force $TmpDir
                exit 1
            }
        }
        catch {
            Write-Host "[ERROR] Failed to download whisper-cli: $_" -ForegroundColor Red
            Remove-Item -Recurse -Force $TmpDir
            exit 1
        }
    }

    Remove-Item -Recurse -Force $TmpDir
}

Write-Host "[OK] All dependencies ready in $BinDir/" -ForegroundColor Green
