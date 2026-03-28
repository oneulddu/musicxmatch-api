$ErrorActionPreference = "Continue"

$InstallDir = "$env:USERPROFILE\.ivlyrics-musicxmatch"
$TaskName = "ivLyrics-MusicXMatch"
$BinPath = "$env:USERPROFILE\.cargo\bin\ivlyrics-musicxmatch-server.exe"
$RunnerScript = Join-Path $InstallDir "run-server.ps1"
$StartupDir = [Environment]::GetFolderPath("Startup")
$StartupScript = Join-Path $StartupDir "ivLyrics-MusicXMatch.cmd"

Write-Host ""
Write-Host "Removing local lyrics server..." -ForegroundColor Yellow

$serverProcesses = Get-CimInstance Win32_Process -Filter "Name = 'ivlyrics-musicxmatch-server.exe'" -ErrorAction SilentlyContinue
if ($serverProcesses) {
    foreach ($process in $serverProcesses) {
        Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue
    }
}

if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
    Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue
    Write-Host "  [OK] Scheduled task removed" -ForegroundColor Green
}

if (Test-Path $StartupScript) {
    Remove-Item $StartupScript -Force -ErrorAction SilentlyContinue
    Write-Host "  [OK] Startup entry removed" -ForegroundColor Green
}

if (Test-Path $InstallDir) {
    Remove-Item $InstallDir -Recurse -Force
    Write-Host "  [OK] Install directory removed" -ForegroundColor Green
}

if (Test-Path $RunnerScript) {
    Remove-Item $RunnerScript -Force -ErrorAction SilentlyContinue
}

if (Test-Path $BinPath) {
    Remove-Item $BinPath -Force
    Write-Host "  [OK] Binary removed" -ForegroundColor Green
}

Write-Host ""
Write-Host "Uninstall complete." -ForegroundColor Green
Write-Host "Addon removal is managed separately by ivLyrics addon-manager." -ForegroundColor DarkGray
Write-Host ""
