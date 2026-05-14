# Download platform dependencies for altgo packaging (Windows).
# Usage: pwsh scripts/download-deps.ps1
#
# Downloads ffmpeg and whisper-cli into target/deps/bin/
#
# Optional mirrors (slow network / corporate proxy):
#   $env:ALTGO_FFMPEG_URL   = direct URL to ffmpeg essentials .zip
#   $env:ALTGO_WHISPER_URL  = direct URL to whisper-bin-x64.zip

$ErrorActionPreference = "Stop"

$DepsDir = "target/deps"
$BinDir = "$DepsDir/bin"

function Save-RemoteFile {
    param(
        [Parameter(Mandatory = $true)][string]$Uri,
        [Parameter(Mandatory = $true)][string]$OutFile
    )

    $parent = Split-Path -Parent $OutFile
    if ($parent -and -not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    if (Test-Path -LiteralPath $OutFile) {
        Remove-Item -Force -LiteralPath $OutFile -ErrorAction SilentlyContinue
    }

    # Prefer real curl.exe (PowerShell aliases "curl" to Invoke-WebRequest).
    $curlExe = Join-Path $env:SystemRoot "System32\curl.exe"
    if (-not (Test-Path -LiteralPath $curlExe)) {
        $curlExe = $null
    }

    if ($curlExe) {
        Write-Host "[INFO] Downloading via curl.exe (long timeout, retries) ..." -ForegroundColor DarkGray
        $curlArgs = @(
            '-L', '-f', '-S',
            '--connect-timeout', '120',
            '--max-time', '7200',
            '--retry', '8',
            '--retry-delay', '20',
            '-o', $OutFile,
            $Uri
        )
        & $curlExe @curlArgs
        if ($LASTEXITCODE -eq 0 -and (Test-Path -LiteralPath $OutFile) -and ((Get-Item -LiteralPath $OutFile).Length -gt 0)) {
            return
        }
        Remove-Item -Force -LiteralPath $OutFile -ErrorAction SilentlyContinue
        Write-Host "[WARN] curl.exe failed (exit $LASTEXITCODE), trying BITS ..." -ForegroundColor Yellow
    }

    try {
        Write-Host "[INFO] Downloading via BITS ..." -ForegroundColor DarkGray
        Start-BitsTransfer -Source $Uri -Destination $OutFile -Priority High -ErrorAction Stop
        if ((Test-Path -LiteralPath $OutFile) -and ((Get-Item -LiteralPath $OutFile).Length -gt 0)) {
            return
        }
    } catch {
        Write-Host "[WARN] BITS failed: $($_.Exception.Message), trying HttpClient ..." -ForegroundColor Yellow
    }
    Remove-Item -Force -LiteralPath $OutFile -ErrorAction SilentlyContinue

    Write-Host "[INFO] Downloading via .NET HttpClient (stream, 2h timeout) ..." -ForegroundColor DarkGray
    Add-Type -AssemblyName System.Net.Http -ErrorAction SilentlyContinue
    $handler = New-Object System.Net.Http.HttpClientHandler
    $handler.AllowAutoRedirect = $true
    $client = New-Object System.Net.Http.HttpClient($handler)
    $client.Timeout = [TimeSpan]::FromHours(2)
    try {
        $response = $client.GetAsync($Uri, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
        if (-not $response.IsSuccessStatusCode) {
            throw "HTTP $($response.StatusCode): $($response.ReasonPhrase)"
        }
        $stream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        try {
            $fs = [System.IO.File]::Create($OutFile)
            try {
                $stream.CopyTo($fs)
            } finally {
                $fs.Dispose()
            }
        } finally {
            $stream.Dispose()
            $response.Dispose()
        }
    } finally {
        $client.Dispose()
    }

    if (-not (Test-Path -LiteralPath $OutFile) -or ((Get-Item -LiteralPath $OutFile).Length -eq 0)) {
        throw "Download produced empty or missing file: $OutFile"
    }
}

Write-Host "Downloading dependencies for Windows x86_64..."
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

# --- ffmpeg (Gyan FFmpeg essentials) ---
$FfmpegVersion = "7.1.1"
$FfmpegTarget = "$BinDir/ffmpeg.exe"

if (Test-Path $FfmpegTarget) {
    Write-Host "[OK] ffmpeg already exists at $FfmpegTarget" -ForegroundColor Green
} else {
    Write-Host "[INFO] Downloading ffmpeg $FfmpegVersion..."
    $FfmpegUrl = if ($env:ALTGO_FFMPEG_URL) { $env:ALTGO_FFMPEG_URL } else {
        "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    }
    $TmpDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "altgo-deps-ffmpeg-$(Get-Random)")

    $ZipFile = "$TmpDir/ffmpeg.zip"
    Save-RemoteFile -Uri $FfmpegUrl -OutFile $ZipFile
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

    $WhisperUrl = if ($env:ALTGO_WHISPER_URL) { $env:ALTGO_WHISPER_URL } else {
        "https://github.com/ggml-org/whisper.cpp/releases/download/v$WhisperVersion/whisper-bin-x64.zip"
    }
    $TmpDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "altgo-deps-whisper-$(Get-Random)")

    try {
        $ZipFile = "$TmpDir/whisper.zip"
        Save-RemoteFile -Uri $WhisperUrl -OutFile $ZipFile
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

        # Same URL retry path (e.g. transient failure after curl/BITS/HttpClient).
        $AltUrl = "https://github.com/ggml-org/whisper.cpp/releases/download/v$WhisperVersion/whisper-bin-x64.zip"
        try {
            Save-RemoteFile -Uri $AltUrl -OutFile "$TmpDir/whisper-alt.zip"
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
