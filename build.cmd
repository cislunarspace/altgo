@echo off
setlocal
cd /d "%~dp0"
where pwsh >nul 2>&1
if %ERRORLEVEL% equ 0 (
  pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0packaging\scripts\build.ps1" %*
  exit /b %ERRORLEVEL%
)
where powershell >nul 2>&1
if %ERRORLEVEL% equ 0 (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0packaging\scripts\build.ps1" %*
  exit /b %ERRORLEVEL%
)
echo ERROR: PowerShell 7+ ^(pwsh^) or Windows PowerShell not found in PATH.
echo Install: https://learn.microsoft.com/powershell/scripting/install/installing-powershell-on-windows
exit /b 1
