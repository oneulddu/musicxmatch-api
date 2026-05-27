$ErrorActionPreference = "Continue"

$InstallDir = Join-Path $env:USERPROFILE ".ivlyrics-musicxmatch"
$TaskName = "ivLyrics-MusicXMatch"
$CargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }
$BinPath = Join-Path $CargoHome "bin\ivlyrics-musicxmatch-server.exe"
$RunnerScript = Join-Path $InstallDir "run-server.ps1"
$StartupDir = [Environment]::GetFolderPath("Startup")
$StartupScript = Join-Path $StartupDir "ivLyrics-MusicXMatch.vbs"
$LegacyStartupScript = Join-Path $StartupDir "ivLyrics-MusicXMatch.cmd"
$UpdateResiduals = @(
    (Join-Path $InstallDir "update.lock")
    (Join-Path $InstallDir "run-update.sh")
    (Join-Path $InstallDir "run-update.ps1")
    (Join-Path $InstallDir "update.log")
)

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

foreach ($path in @($StartupScript, $LegacyStartupScript)) {
    if (Test-Path -LiteralPath $path) {
        Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
        Write-Host "  [OK] Startup entry removed: $path" -ForegroundColor Green
    }
}

foreach ($path in $UpdateResiduals) {
    if (Test-Path -LiteralPath $path) {
        Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
        Write-Host "  [OK] Update residual removed: $path" -ForegroundColor Green
    }
}

if (Test-Path -LiteralPath $InstallDir) {
    Remove-Item -LiteralPath $InstallDir -Recurse -Force
    Write-Host "  [OK] Install directory removed" -ForegroundColor Green
}

if (Test-Path -LiteralPath $RunnerScript) {
    Remove-Item -LiteralPath $RunnerScript -Force -ErrorAction SilentlyContinue
}

if (Test-Path -LiteralPath $BinPath) {
    Remove-Item -LiteralPath $BinPath -Force
    Write-Host "  [OK] Binary removed" -ForegroundColor Green
}

Write-Host ""
Write-Host "Uninstall complete." -ForegroundColor Green
Write-Host "Addon removal is managed separately by ivLyrics addon-manager." -ForegroundColor DarkGray
Write-Host ""
