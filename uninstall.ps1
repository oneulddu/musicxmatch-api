$ErrorActionPreference = "Continue"

$InstallDir = "$env:LOCALAPPDATA\ivLyrics\musicxmatch-provider"
$startupFolder = [System.Environment]::GetFolderPath("Startup")
$shortcutPath = Join-Path $startupFolder "MusicXMatch Provider.lnk"

Write-Host ""
Write-Host "Removing MusicXMatch Provider..." -ForegroundColor Yellow

Get-Process | Where-Object { $_.Path -like "*musicxmatch-provider*" } | Stop-Process -Force -ErrorAction SilentlyContinue

if (Test-Path $shortcutPath) {
    Remove-Item $shortcutPath -Force
    Write-Host "  [OK] Startup shortcut removed" -ForegroundColor Green
}

if (Test-Path $InstallDir) {
    Remove-Item $InstallDir -Recurse -Force
    Write-Host "  [OK] Install directory removed" -ForegroundColor Green
}

Write-Host ""
Write-Host "Uninstall complete." -ForegroundColor Green
Write-Host ""
