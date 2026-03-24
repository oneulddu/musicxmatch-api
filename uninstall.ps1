$ErrorActionPreference = "Continue"

$InstallDir = "$env:USERPROFILE\.ivlyrics-musicxmatch"
$TaskName = "ivLyrics-MusicXMatch"
$BinPath = "$env:USERPROFILE\.cargo\bin\ivlyrics-musicxmatch-server.exe"

Write-Host ""
Write-Host "Removing MusicXMatch Provider..." -ForegroundColor Yellow

Get-Process | Where-Object { $_.Path -like "*ivlyrics-musicxmatch-server*" } | Stop-Process -Force -ErrorAction SilentlyContinue

if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
    Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue
    Write-Host "  [OK] Scheduled task removed" -ForegroundColor Green
}

if (Test-Path $InstallDir) {
    Remove-Item $InstallDir -Recurse -Force
    Write-Host "  [OK] Install directory removed" -ForegroundColor Green
}

if (Test-Path $BinPath) {
    Remove-Item $BinPath -Force
    Write-Host "  [OK] Binary removed" -ForegroundColor Green
}

Write-Host ""
Write-Host "Uninstall complete." -ForegroundColor Green
Write-Host ""
