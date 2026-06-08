# ivLyrics Lyrics Providers Installer (Windows)

$ErrorActionPreference = "Stop"

Write-Host "=== ivLyrics Lyrics Providers Installer ===" -ForegroundColor Cyan
Write-Host ""

$InstallDir = Join-Path $env:USERPROFILE ".ivlyrics-musicxmatch"
$TaskName = "ivLyrics-MusicXMatch"
$CargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }
$BinPath = Join-Path $CargoHome "bin\ivlyrics-musicxmatch-server.exe"
$ServerUrl = "http://127.0.0.1:8092"
$RunnerScript = Join-Path $InstallDir "run-server.ps1"
$StartupDir = [Environment]::GetFolderPath("Startup")
$StartupScript = Join-Path $StartupDir "ivLyrics-MusicXMatch.vbs"
$LegacyStartupScript = Join-Path $StartupDir "ivLyrics-MusicXMatch.cmd"
$PreferredAutoStartMode = if ($env:IVLYRICS_WINDOWS_AUTOSTART) { $env:IVLYRICS_WINDOWS_AUTOSTART.Trim().ToLowerInvariant() } else { "startup-folder" }
$RawBaseUrl = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main"
$AddonUrls = @(
    "$RawBaseUrl/Addon_Lyrics_MusicXMatch.js",
    "$RawBaseUrl/Addon_Lyrics_Deezer.js",
    "$RawBaseUrl/Addon_Lyrics_Bugs.js",
    "$RawBaseUrl/Addon_Lyrics_Genie.js"
)
$SkipAddons = $env:IVLYRICS_SKIP_ADDONS -eq "1"
$ServerWasRunning = $false
$InstallCompleted = $false
$PreviousBinBackup = Join-Path $InstallDir "previous-server.exe"

function Remove-ScheduledTaskIfExists {
    if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue
    }
}

function Remove-StartupFallbackIfExists {
    foreach ($path in @($StartupScript, $LegacyStartupScript)) {
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
        }
    }
}

function Start-ServerFromRunner {
    if (Test-Path -LiteralPath $RunnerScript) {
        $RunnerScriptArgument = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$RunnerScript`""
        Start-Process -FilePath "powershell.exe" -WindowStyle Hidden -ArgumentList $RunnerScriptArgument -WorkingDirectory $InstallDir | Out-Null
        return $true
    }

    if (Test-Path -LiteralPath $BinPath) {
        Start-Process -FilePath $BinPath -WindowStyle Hidden -WorkingDirectory $InstallDir | Out-Null
        return $true
    }

    return $false
}

function Get-ServerProcesses {
    @(Get-CimInstance Win32_Process -Filter "Name = 'ivlyrics-musicxmatch-server.exe'" -ErrorAction SilentlyContinue)
}

function Backup-PreviousBinary {
    Remove-Item -LiteralPath $PreviousBinBackup -Force -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $BinPath) {
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        Copy-Item -LiteralPath $BinPath -Destination $PreviousBinBackup -Force
    }
}

function Restore-PreviousBinary {
    if (Test-Path -LiteralPath $PreviousBinBackup) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $BinPath) | Out-Null
        Copy-Item -LiteralPath $PreviousBinBackup -Destination $BinPath -Force -ErrorAction SilentlyContinue
    }
}

function Stop-ExistingServer {
    $serverProcesses = Get-ServerProcesses
    if ($serverProcesses.Count -gt 0) {
        foreach ($process in $serverProcesses) {
            Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue
        }
    }

    for ($attempt = 1; $attempt -le 30; $attempt++) {
        if ((Get-ServerProcesses).Count -eq 0) {
            return
        }
        Start-Sleep -Milliseconds 250
    }

    throw "Existing server did not stop cleanly. Please close ivlyrics-musicxmatch-server and retry."
}

function Install-StartupFallback {
    param (
        [string]$RunnerScriptPath,
        [string]$StartupScriptPath
    )

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $StartupScriptPath) | Out-Null

    $RunnerScriptForVbs = $RunnerScriptPath -replace '"', '""'
    $StartupBody = @"
Set shell = CreateObject("WScript.Shell")
shell.Run "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File ""$RunnerScriptForVbs""", 0, False
"@
    $TempStartupScriptPath = "$StartupScriptPath.tmp"
    Set-Content -LiteralPath $TempStartupScriptPath -Value $StartupBody -Encoding Unicode
    Move-Item -LiteralPath $TempStartupScriptPath -Destination $StartupScriptPath -Force
    Remove-ScheduledTaskIfExists
    if (Test-Path -LiteralPath $LegacyStartupScript) {
        Remove-Item -LiteralPath $LegacyStartupScript -Force -ErrorAction SilentlyContinue
    }
}

try {
Write-Host "[1/8] Creating installation directory..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

Write-Host "[2/8] Checking Rust toolchain..." -ForegroundColor Yellow
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo is required. Install Rust first: https://rustup.rs"
}

Write-Host "[3/8] Stopping existing server if running..." -ForegroundColor Yellow
$ServerWasRunning = (Get-ServerProcesses).Count -gt 0
Stop-ExistingServer

Write-Host "[4/8] Installing server binary..." -ForegroundColor Yellow
Backup-PreviousBinary
cargo install --git https://github.com/oneulddu/musicxmatch-api.git --bin ivlyrics-musicxmatch-server --force
if ($LASTEXITCODE -ne 0) {
    throw "cargo install failed with exit code $LASTEXITCODE"
}

Write-Host "[5/8] Setting up login auto-start..." -ForegroundColor Yellow
$RunnerBody = @"
`$env:MXM_SESSION_FILE = "$InstallDir\musixmatch_session.json"
`$env:IVLYRICS_MXM_LOG = "$InstallDir\server.log"
`$stdoutLog = "$InstallDir\server.stdout.log"
`$rotatedStdoutLog = "$InstallDir\server.stdout.log.1"
if ((Test-Path -LiteralPath `$stdoutLog) -and ((Get-Item -LiteralPath `$stdoutLog).Length -gt 2097152)) {
    Remove-Item -LiteralPath `$rotatedStdoutLog -Force -ErrorAction SilentlyContinue
    Move-Item -LiteralPath `$stdoutLog -Destination `$rotatedStdoutLog -Force -ErrorAction SilentlyContinue
}
& "$BinPath" *>> `$stdoutLog
"@
Set-Content -LiteralPath $RunnerScript -Value $RunnerBody -Encoding UTF8

$AutoStartMode = "startup-folder"
if ($PreferredAutoStartMode -eq "scheduled-task") {
    try {
        $Action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$RunnerScript`"" -WorkingDirectory $InstallDir
        $Trigger = New-ScheduledTaskTrigger -AtLogOn
        $Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable
        Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Settings $Settings -Force -ErrorAction Stop | Out-Null
        Remove-StartupFallbackIfExists
        $AutoStartMode = "scheduled-task"
    } catch {
        Write-Warning "Scheduled Task 등록이 거부되어 Startup 폴더 방식으로 대체합니다."
        Install-StartupFallback -RunnerScriptPath $RunnerScript -StartupScriptPath $StartupScript
    }
} else {
    Install-StartupFallback -RunnerScriptPath $RunnerScript -StartupScriptPath $StartupScript
}

Write-Host "[6/8] Registering addons..." -ForegroundColor Yellow
if ($SkipAddons) {
    Write-Host "Addon registration skipped by IVLYRICS_SKIP_ADDONS=1."
} elseif (Get-Command spicetify -ErrorAction SilentlyContinue) {
    try {
        $CompatUrl = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/addon-manager-compat.ps1?ts=$((Get-Date).ToUniversalTime().ToString('yyyyMMddHHmmss'))"
        $CompatScript = [scriptblock]::Create((iwr -useb $CompatUrl).Content)
        & $CompatScript @AddonUrls
        Write-Host "Addons registered successfully." -ForegroundColor Green
    } catch {
        Write-Warning "Addon registration failed. Server install succeeded, but addon registration needs manual retry."
        $JoinedUrls = ($AddonUrls | ForEach-Object { "`"$($_)`"" }) -join " "
        Write-Host "& ([scriptblock]::Create((iwr -useb https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/addon-manager-compat.ps1).Content)) $JoinedUrls"
    }
} else {
    Write-Warning "spicetify was not found, so addon registration was skipped."
    Write-Host "Run the following after installing/configuring spicetify:"
    $JoinedUrls = ($AddonUrls | ForEach-Object { "`"$($_)`"" }) -join " "
    Write-Host "& ([scriptblock]::Create((iwr -useb https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/addon-manager-compat.ps1).Content)) $JoinedUrls"
}

Write-Host "[7/8] Starting server..." -ForegroundColor Yellow
if ($AutoStartMode -eq "scheduled-task") {
    try {
        Start-ScheduledTask -TaskName $TaskName -ErrorAction Stop
    } catch {
        Write-Warning "Scheduled Task immediate start failed; starting the runner directly for this session."
        if (-not (Start-ServerFromRunner)) {
            throw "Server start failed: scheduled task and direct runner both failed. $($_.Exception.Message)"
        }
    }
} elseif (-not (Start-ServerFromRunner)) {
    throw "Server start failed: runner script and binary were not found."
}

Write-Host "[8/8] Verifying health and CORS..." -ForegroundColor Yellow
$HealthUrl = "$ServerUrl/health"
$Response = $null
$LastHealthError = $null
for ($attempt = 1; $attempt -le 30; $attempt++) {
    try {
        $Response = Invoke-WebRequest -Uri $HealthUrl -UseBasicParsing -TimeoutSec 2
        if ($Response.StatusCode -eq 200) {
            break
        }
    } catch {
        $LastHealthError = $_.Exception.Message
    }
    Start-Sleep -Seconds 1
}
if ($null -eq $Response -or $Response.StatusCode -ne 200) {
    throw "Server health check failed: $HealthUrl $LastHealthError"
}

$CorsTestOrigin = "spicetify://ivlyrics"
$CorsResponse = $null
try {
    $CorsResponse = Invoke-WebRequest -Uri $HealthUrl -UseBasicParsing -TimeoutSec 2 -Headers @{ Origin = $CorsTestOrigin }
} catch {
    throw "CORS header check failed: $HealthUrl did not respond to Origin: $CorsTestOrigin $($_.Exception.Message)"
}
if ($null -eq $CorsResponse -or $CorsResponse.StatusCode -ne 200) {
    throw "CORS header check failed: $HealthUrl did not return 200 for Origin: $CorsTestOrigin"
}
$CorsAllowOrigin = $CorsResponse.Headers["Access-Control-Allow-Origin"]
if ($CorsAllowOrigin -is [array]) {
    $CorsAllowOrigin = $CorsAllowOrigin[0]
}
if ($CorsAllowOrigin -ne $CorsTestOrigin) {
    throw "CORS header check failed: expected Access-Control-Allow-Origin $CorsTestOrigin, got $CorsAllowOrigin"
}

$InstallCompleted = $true
Remove-Item -LiteralPath $PreviousBinBackup -Force -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "✓ Installation complete!" -ForegroundColor Green
Write-Host "Server running at $ServerUrl"
Write-Host "Auto-start mode: $AutoStartMode"
Write-Host ""
} catch {
    if ($ServerWasRunning -and -not $InstallCompleted) {
        Write-Warning "Installation failed; trying to restore and restart the previously installed server if available."
        Restore-PreviousBinary
        [void](Start-ServerFromRunner)
    }
    throw
}
