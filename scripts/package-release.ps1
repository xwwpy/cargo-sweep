$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$version = (Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw -Encoding UTF8 | ConvertFrom-Json).version
$installer = Join-Path $root "src-tauri\target\release\bundle\nsis\Cargo Sweep_${version}_x64-setup.exe"
$portableExe = Join-Path $root "src-tauri\target\release\cargo-sweep.exe"
$releaseDir = Join-Path $root "release"
$stagingRoot = Join-Path $root ".packaging"
$installerStage = Join-Path $stagingRoot "installer"
$portableStage = Join-Path $stagingRoot "portable\Cargo-Sweep"

foreach ($required in @($installer, $portableExe)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "Missing build artifact: $required"
    }
}

if (Test-Path -LiteralPath $stagingRoot) {
    Remove-Item -LiteralPath $stagingRoot -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $installerStage, $portableStage, $releaseDir | Out-Null
Get-ChildItem -LiteralPath $releaseDir -Filter "Cargo-Sweep-*" -File | Remove-Item -Force

Copy-Item -LiteralPath $installer -Destination $installerStage -Force
Copy-Item -LiteralPath $portableExe -Destination $portableStage -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "..\packaging\README-installer.txt") -Destination (Join-Path $installerStage "README-zh.txt") -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "..\packaging\README-portable.txt") -Destination (Join-Path $portableStage "README-zh.txt") -Force

$installCmd = @"
@echo off
setlocal
title Cargo Sweep Installer
set "ROOT=%~dp0"
set "TEMP=%ROOT%temp"
set "TMP=%ROOT%temp"
if not exist "%TEMP%" mkdir "%TEMP%"
echo.
echo Cargo Sweep installer
echo Temporary folder: %TEMP%
echo.
"%ROOT%Cargo Sweep_${version}_x64-setup.exe" %*
set "RC=%ERRORLEVEL%"
if exist "%TEMP%" rd /s /q "%TEMP%" 2>nul
if not "%RC%"=="0" (
  echo.
  echo The installer exited with code %RC%.
  pause
)
exit /b %RC%
"@
Set-Content -LiteralPath (Join-Path $installerStage "INSTALL-CARGO-SWEEP.cmd") -Value $installCmd -Encoding ascii

$portableCmd = @"
@echo off
setlocal
set "ROOT=%~dp0"
set "TEMP=%ROOT%temp"
set "TMP=%ROOT%temp"
if not exist "%TEMP%" mkdir "%TEMP%"
if not exist "%ROOT%webview-data" mkdir "%ROOT%webview-data"
set "WEBVIEW2_USER_DATA_FOLDER=%ROOT%webview-data"
start "" "%ROOT%cargo-sweep.exe"
timeout /t 2 /nobreak >nul
tasklist /fi "imagename eq cargo-sweep.exe" | find /i "cargo-sweep.exe" >nul
if errorlevel 1 (
  echo.
  echo Cargo Sweep did not stay running.
  echo Check the startup log:
  echo %LOCALAPPDATA%\CargoSweep\logs\startup.log
  pause
)
"@
Set-Content -LiteralPath (Join-Path $portableStage "START-CARGO-SWEEP.cmd") -Value $portableCmd -Encoding ascii

$installerZip = Join-Path $releaseDir "Cargo-Sweep-${version}-installer-x64.zip"
$portableZip = Join-Path $releaseDir "Cargo-Sweep-${version}-portable-x64.zip"
foreach ($zip in @($installerZip, $portableZip)) {
    if (Test-Path -LiteralPath $zip) {
        Remove-Item -LiteralPath $zip -Force
    }
}
Compress-Archive -Path (Join-Path $installerStage "*") -DestinationPath $installerZip -CompressionLevel Optimal
Compress-Archive -Path (Join-Path (Join-Path $stagingRoot "portable") "Cargo-Sweep") -DestinationPath $portableZip -CompressionLevel Optimal

$assets = @($installer, $installerZip, $portableZip)
$checksums = foreach ($asset in $assets) {
    $hash = (Get-FileHash -LiteralPath $asset -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $(Split-Path -Leaf $asset)"
}
[System.IO.File]::WriteAllLines(
    (Join-Path $releaseDir "SHA256SUMS.txt"),
    $checksums,
    [System.Text.Encoding]::ASCII
)

Remove-Item -LiteralPath $stagingRoot -Recurse -Force

Write-Output "Packaged Cargo Sweep $version"
Get-ChildItem -LiteralPath $releaseDir | Select-Object Name, Length | Format-Table -AutoSize
