# ivLyrics MusicXMatch Provider Installer (Windows)

Write-Host "=== ivLyrics MusicXMatch Provider Installer ===" -ForegroundColor Cyan
Write-Host ""

$InstallDir = "$env:USERPROFILE\.ivlyrics-musicxmatch"
$TaskName = "ivLyrics-MusicXMatch"

Write-Host "[1/5] Creating installation directory..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

Write-Host "[2/5] Downloading files..." -ForegroundColor Yellow
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/server.js" -OutFile "$InstallDir\server.js"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/musicxmatch.js" -OutFile "$InstallDir\musicxmatch.js"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/package.json" -OutFile "$InstallDir\package.json"

Write-Host "[3/5] Installing dependencies..." -ForegroundColor Yellow
Set-Location $InstallDir
npm install --production

Write-Host "[4/5] Setting up auto-start..." -ForegroundColor Yellow
$Action = New-ScheduledTaskAction -Execute "node" -Argument "$InstallDir\server.js" -WorkingDirectory $InstallDir
$Trigger = New-ScheduledTaskTrigger -AtLogOn
$Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable
Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Settings $Settings -Force | Out-Null

Write-Host "[5/5] Starting server..." -ForegroundColor Yellow
Start-ScheduledTask -TaskName $TaskName
Start-Sleep -Seconds 2

Write-Host ""
Write-Host "✓ Installation complete!" -ForegroundColor Green
Write-Host "Server running at http://localhost:8092"
Write-Host ""


