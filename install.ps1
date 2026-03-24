# ivLyrics MusicXMatch Provider Installer (Windows)

Write-Host "=== ivLyrics MusicXMatch Provider Installer ===" -ForegroundColor Cyan
Write-Host ""

$InstallDir = "$env:USERPROFILE\.ivlyrics-musicxmatch"
$TaskName = "ivLyrics-MusicXMatch"
$BinPath = "$env:USERPROFILE\.cargo\bin\ivlyrics-musicxmatch-server.exe"
$ServerUrl = "http://127.0.0.1:8092"

Write-Host "[1/6] Creating installation directory..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

Write-Host "[2/6] Checking Rust toolchain..." -ForegroundColor Yellow
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo is required. Install Rust first: https://rustup.rs"
}

Write-Host "[3/6] Installing server binary..." -ForegroundColor Yellow
cargo install --git https://github.com/oneulddu/musicxmatch-api.git --bin ivlyrics-musicxmatch-server --force

Write-Host "[4/6] Setting up auto-start..." -ForegroundColor Yellow
$Action = New-ScheduledTaskAction -Execute $BinPath -WorkingDirectory $InstallDir
$Trigger = New-ScheduledTaskTrigger -AtLogOn
$Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable
Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Settings $Settings -Force | Out-Null

Write-Host "[5/6] Starting server..." -ForegroundColor Yellow
Start-ScheduledTask -TaskName $TaskName
Start-Sleep -Seconds 2

Write-Host "[6/6] Verifying health and CORS..." -ForegroundColor Yellow
$Response = Invoke-WebRequest -Uri "$ServerUrl/health" -UseBasicParsing
if ($Response.StatusCode -ne 200) {
    throw "Server health check failed: $ServerUrl/health"
}
if ($Response.Headers["Access-Control-Allow-Origin"] -ne "*") {
    throw "CORS header check failed: Access-Control-Allow-Origin header missing"
}

Write-Host ""
Write-Host "✓ Installation complete!" -ForegroundColor Green
Write-Host "Server running at $ServerUrl"
Write-Host ""
