# ivLyrics Lyrics Providers Installer (Windows)

Write-Host "=== ivLyrics Lyrics Providers Installer ===" -ForegroundColor Cyan
Write-Host ""

$InstallDir = "$env:USERPROFILE\.ivlyrics-musicxmatch"
$ExtensionsDir = "$env:APPDATA\spicetify\Extensions"
$AddonNames = @("Addon_Lyrics_MusicXMatch.js", "Addon_Lyrics_Deezer.js")
$TaskName = "ivLyrics-MusicXMatch"
$BinPath = "$env:USERPROFILE\.cargo\bin\ivlyrics-musicxmatch-server.exe"
$ServerUrl = "http://127.0.0.1:8092"
$RunnerScript = Join-Path $InstallDir "run-server.ps1"

Write-Host "[1/7] Creating installation directory..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

Write-Host "[2/7] Checking Rust toolchain..." -ForegroundColor Yellow
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo is required. Install Rust first: https://rustup.rs"
}

Write-Host "[3/7] Installing server binary..." -ForegroundColor Yellow
cargo install --git https://github.com/oneulddu/musicxmatch-api.git --bin ivlyrics-musicxmatch-server --force

Write-Host "[4/7] Setting up auto-start..." -ForegroundColor Yellow
$RunnerBody = @"
$env:MXM_SESSION_FILE = "$InstallDir\musixmatch_session.json"
$env:IVLYRICS_MXM_LOG = "$InstallDir\server.log"
& "$BinPath" *>> "$InstallDir\server.stdout.log"
"@
Set-Content -Path $RunnerScript -Value $RunnerBody -Encoding UTF8

$Action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-NoProfile -ExecutionPolicy Bypass -File `"$RunnerScript`"" -WorkingDirectory $InstallDir
$Trigger = New-ScheduledTaskTrigger -AtLogOn
$Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable
Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Settings $Settings -Force | Out-Null

Write-Host "[5/7] Starting server..." -ForegroundColor Yellow
Start-ScheduledTask -TaskName $TaskName
Start-Sleep -Seconds 2

Write-Host "[6/7] Verifying health and CORS..." -ForegroundColor Yellow
$Response = Invoke-WebRequest -Uri "$ServerUrl/health" -UseBasicParsing
if ($Response.StatusCode -ne 200) {
    throw "Server health check failed: $ServerUrl/health"
}
if ($Response.Headers["Access-Control-Allow-Origin"] -ne "*") {
    throw "CORS header check failed: Access-Control-Allow-Origin header missing"
}

Write-Host "[7/7] Installing ivLyrics addons..." -ForegroundColor Yellow
$Spicetify = Get-Command spicetify -ErrorAction SilentlyContinue
if (-not $Spicetify) {
    Write-Warning "spicetify is not installed or not in PATH. Skipping addon registration."
} else {
    New-Item -ItemType Directory -Force -Path $ExtensionsDir | Out-Null

    $CurrentExtensions = (spicetify config extensions 2>$null | Out-String).Trim()
    $ExtensionList = @()
    if ($CurrentExtensions) {
        $ExtensionList = $CurrentExtensions -split '\s*\|\s*' | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    }

    foreach ($AddonName in $AddonNames) {
        $AddonPath = Join-Path $ExtensionsDir $AddonName
        $AddonUrl = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/$AddonName"
        Invoke-WebRequest -Uri $AddonUrl -OutFile $AddonPath

        if ($ExtensionList -notcontains $AddonName) {
            spicetify config extensions $AddonName | Out-Null
            $ExtensionList += $AddonName
        }
    }

    spicetify apply
}

Write-Host ""
Write-Host "✓ Installation complete!" -ForegroundColor Green
Write-Host "Server running at $ServerUrl"
Write-Host "Addon paths: $(Join-Path $ExtensionsDir $AddonNames[0]), $(Join-Path $ExtensionsDir $AddonNames[1])"
Write-Host ""
