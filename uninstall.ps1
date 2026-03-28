$ErrorActionPreference = "Continue"

$InstallDir = "$env:USERPROFILE\.ivlyrics-musicxmatch"
$TaskName = "ivLyrics-MusicXMatch"
$BinPath = "$env:USERPROFILE\.cargo\bin\ivlyrics-musicxmatch-server.exe"
$RunnerScript = Join-Path $InstallDir "run-server.ps1"
$StartupDir = [Environment]::GetFolderPath("Startup")
$StartupScript = Join-Path $StartupDir "ivLyrics-MusicXMatch.cmd"
$ExtensionsDir = "$env:APPDATA\spicetify\Extensions"
$AddonNames = @("Addon_Lyrics_MusicXMatch.js", "Addon_Lyrics_Deezer.js", "Addon_Lyrics_Bugs.js", "Addon_Lyrics_Genie.js")

function Stop-SpotifyIfRunning {
    $spotifyProcesses = Get-Process -Name "Spotify" -ErrorAction SilentlyContinue
    if ($spotifyProcesses) {
        Write-Host "  [INFO] Spotify is running. Closing it before spicetify apply..." -ForegroundColor DarkYellow
        $spotifyProcesses | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 2
    }
}

function Remove-AddonFromSpicetify {
    param([string]$AddonName)
    spicetify config "extensions-$AddonName" 2>$null | Out-Null
}

Write-Host ""
Write-Host "Removing MusicXMatch Provider..." -ForegroundColor Yellow

Stop-SpotifyIfRunning

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

$Spicetify = Get-Command spicetify -ErrorAction SilentlyContinue
if ($Spicetify) {
    foreach ($AddonName in $AddonNames) {
        Remove-AddonFromSpicetify -AddonName $AddonName
        $AddonPath = Join-Path $ExtensionsDir $AddonName
        if (Test-Path $AddonPath) {
            Remove-Item $AddonPath -Force -ErrorAction SilentlyContinue
            Write-Host "  [OK] Removed addon file: $AddonName" -ForegroundColor Green
        }
    }

    try {
        spicetify apply 2>$null | Out-Null
        Write-Host "  [OK] Spicetify apply completed" -ForegroundColor Green
    } catch {
        Write-Warning "Spicetify apply failed. Run 'spicetify apply' manually if needed."
    }
} else {
    Write-Warning "spicetify not found. Addon files/config may need manual cleanup."
}

Write-Host ""
Write-Host "Uninstall complete." -ForegroundColor Green
Write-Host ""
