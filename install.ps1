# ivLyrics Lyrics Providers Installer (Windows)

Write-Host "=== ivLyrics Lyrics Providers Installer ===" -ForegroundColor Cyan
Write-Host ""

$InstallDir = "$env:USERPROFILE\.ivlyrics-musicxmatch"
$ExtensionsDir = "$env:APPDATA\spicetify\Extensions"
$AddonNames = @("Addon_Lyrics_MusicXMatch.js", "Addon_Lyrics_Deezer.js", "Addon_Lyrics_Bugs.js", "Addon_Lyrics_Genie.js")
$TaskName = "ivLyrics-MusicXMatch"
$BinPath = "$env:USERPROFILE\.cargo\bin\ivlyrics-musicxmatch-server.exe"
$ServerUrl = "http://127.0.0.1:8092"
$RunnerScript = Join-Path $InstallDir "run-server.ps1"
$StartupDir = [Environment]::GetFolderPath("Startup")
$StartupScript = Join-Path $StartupDir "ivLyrics-MusicXMatch.cmd"
$PreferredAutoStartMode = if ($env:IVLYRICS_WINDOWS_AUTOSTART) { $env:IVLYRICS_WINDOWS_AUTOSTART.Trim().ToLowerInvariant() } else { "startup-folder" }
$SkipAddons = $env:IVLYRICS_SKIP_ADDONS -eq "1"
$DownloadRetries = 3

function Stop-SpotifyIfRunning {
    $spotifyProcesses = Get-Process -Name "Spotify" -ErrorAction SilentlyContinue
    if ($spotifyProcesses) {
        Write-Host "Spotify is running. Closing it before spicetify apply..." -ForegroundColor DarkYellow
        $spotifyProcesses | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 2
    }
}

function Invoke-AddonDownloadWithRetry {
    param (
        [string]$Uri,
        [string]$OutFile
    )

    for ($attempt = 1; $attempt -le $DownloadRetries; $attempt++) {
        try {
            Invoke-WebRequest -Uri $Uri -OutFile $OutFile -ErrorAction Stop
            return
        } catch {
            if ($attempt -ge $DownloadRetries) {
                throw
            }
            Write-Host "Retrying addon download ($attempt/$DownloadRetries)..." -ForegroundColor DarkYellow
            Start-Sleep -Seconds 1
        }
    }
}

function Install-Addons {
    param (
        [string[]]$AddonNames,
        [string]$ExtensionsDir
    )

    New-Item -ItemType Directory -Force -Path $ExtensionsDir | Out-Null

    $CurrentExtensions = (spicetify config extensions 2>$null | Out-String).Trim()
    $ExtensionList = @()
    if ($CurrentExtensions) {
        $ExtensionList = $CurrentExtensions -split '\s*\|\s*' | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    }

    foreach ($AddonName in $AddonNames) {
        $AddonPath = Join-Path $ExtensionsDir $AddonName
        $AddonUrl = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/$AddonName"
        Invoke-AddonDownloadWithRetry -Uri $AddonUrl -OutFile $AddonPath

        if ($ExtensionList -notcontains $AddonName) {
            spicetify config extensions $AddonName | Out-Null
            $ExtensionList += $AddonName
        }
    }

    Stop-SpotifyIfRunning
    spicetify apply
}

function Install-StartupFallback {
    param (
        [string]$RunnerScriptPath,
        [string]$StartupScriptPath
    )

    $StartupBody = @"
@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "$RunnerScriptPath"
"@
    Set-Content -Path $StartupScriptPath -Value $StartupBody -Encoding ASCII
}

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
`$env:MXM_SESSION_FILE = "$InstallDir\musixmatch_session.json"
`$env:IVLYRICS_MXM_LOG = "$InstallDir\server.log"
& "$BinPath" *>> "$InstallDir\server.stdout.log"
"@
Set-Content -Path $RunnerScript -Value $RunnerBody -Encoding UTF8

$AutoStartMode = "startup-folder"
if ($PreferredAutoStartMode -eq "scheduled-task") {
    try {
        $Action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-NoProfile -ExecutionPolicy Bypass -File `"$RunnerScript`"" -WorkingDirectory $InstallDir
        $Trigger = New-ScheduledTaskTrigger -AtLogOn
        $Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable
        Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Settings $Settings -Force -ErrorAction Stop | Out-Null
        $AutoStartMode = "scheduled-task"
    } catch {
        Write-Warning "Scheduled Task 등록이 거부되어 Startup 폴더 방식으로 대체합니다."
        Install-StartupFallback -RunnerScriptPath $RunnerScript -StartupScriptPath $StartupScript
    }
} else {
    Install-StartupFallback -RunnerScriptPath $RunnerScript -StartupScriptPath $StartupScript
}

Write-Host "[5/7] Starting server..." -ForegroundColor Yellow
if ($AutoStartMode -eq "scheduled-task") {
    Start-ScheduledTask -TaskName $TaskName
} else {
    Start-Process powershell.exe -WindowStyle Hidden -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $RunnerScript -WorkingDirectory $InstallDir | Out-Null
}
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
if ($SkipAddons) {
    Write-Host "IVLYRICS_SKIP_ADDONS=1 detected. Skipping addon registration." -ForegroundColor DarkYellow
} else {
    $Spicetify = Get-Command spicetify -ErrorAction SilentlyContinue
    if (-not $Spicetify) {
    Write-Warning "spicetify is not installed or not in PATH. Server installation completed, but addon registration was skipped."
    } else {
        Install-Addons -AddonNames $AddonNames -ExtensionsDir $ExtensionsDir
    }
}

Write-Host ""
Write-Host "✓ Installation complete!" -ForegroundColor Green
Write-Host "Server running at $ServerUrl"
Write-Host "Auto-start mode: $AutoStartMode"
Write-Host "Addon paths: $(Join-Path $ExtensionsDir $AddonNames[0]), $(Join-Path $ExtensionsDir $AddonNames[1]), $(Join-Path $ExtensionsDir $AddonNames[2]), $(Join-Path $ExtensionsDir $AddonNames[3])"
Write-Host ""
