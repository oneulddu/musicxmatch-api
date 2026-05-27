param(
    [Parameter(Mandatory = $true, ValueFromRemainingArguments = $true)]
    [string[]]$Urls
)

$ErrorActionPreference = "Stop"

$addonDir = "$env:LOCALAPPDATA\spicetify\CustomApps\ivLyrics"
$sourcesDir = "$env:LOCALAPPDATA\spicetify\ivLyrics"
$manifestPath = Join-Path $addonDir "manifest.json"
$sourcesPath = Join-Path $sourcesDir "addon_sources.json"
$repoRawMainPrefix = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/"
$patchScriptPath = "scripts/patch-ivlyrics-selection.ps1"
$resolvedRef = $null
$spotifyWasRunning = $false
$spotifyPaths = @()
$backupDir = Join-Path ([System.IO.Path]::GetTempPath()) ("ivlyrics-addon-backup-" + [guid]::NewGuid().ToString("N"))
$backups = @{}

function Backup-FileForRollback {
    param([string]$Path)

    if ($backups.ContainsKey($Path)) {
        return
    }

    if (Test-Path -LiteralPath $Path) {
        New-Item -ItemType Directory -Force -Path $backupDir | Out-Null
        $backupPath = Join-Path $backupDir ([guid]::NewGuid().ToString("N"))
        Copy-Item -LiteralPath $Path -Destination $backupPath -Force
        $backups[$Path] = $backupPath
    } else {
        $backups[$Path] = $null
    }
}

function Restore-Backups {
    foreach ($path in $backups.Keys) {
        $backupPath = $backups[$path]
        if ($backupPath -and (Test-Path -LiteralPath $backupPath)) {
            Copy-Item -LiteralPath $backupPath -Destination $path -Force
        } elseif (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
        }
    }
}

function Stop-SpotifyIfRunning {
    $processes = @(Get-Process -Name Spotify -ErrorAction SilentlyContinue)
    if ($processes.Count -eq 0) {
        return
    }

    $script:spotifyWasRunning = $true
    $script:spotifyPaths = @($processes | ForEach-Object { $_.Path } | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -Unique)
    foreach ($process in $processes) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Seconds 2
}

function Restart-SpotifyIfNeeded {
    if (-not $script:spotifyWasRunning) {
        return
    }

    $started = $false
    foreach ($path in $script:spotifyPaths) {
        if ($path -and (Test-Path -LiteralPath $path)) {
            Start-Process -FilePath $path | Out-Null
            $started = $true
            break
        }
    }
    if (-not $started) {
        Start-Process "spotify" -ErrorAction SilentlyContinue | Out-Null
    }
}

if (-not (Test-Path -LiteralPath $manifestPath)) {
    throw "ivLyrics manifest not found at $manifestPath"
}

New-Item -ItemType Directory -Force -Path $addonDir | Out-Null
New-Item -ItemType Directory -Force -Path $sourcesDir | Out-Null

try {
    $sources = @{}
    if (Test-Path -LiteralPath $sourcesPath) {
        $existing = Get-Content -LiteralPath $sourcesPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($existing) {
            $existing.PSObject.Properties | ForEach-Object {
                $sources[$_.Name] = $_.Value
            }
        }
    }

    $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if (-not ($manifest.PSObject.Properties.Name -contains "subfiles_extension")) {
        $manifest | Add-Member -NotePropertyName subfiles_extension -NotePropertyValue @()
    } elseif ($null -eq $manifest.subfiles_extension) {
        $manifest.subfiles_extension = @()
    }

    foreach ($url in $Urls) {
        $cleanUrl = ($url -split '\?')[0]
        $fileName = [System.IO.Path]::GetFileName($cleanUrl)
        $downloadUrl = $url

        if (-not $fileName -or -not $fileName.EndsWith('.js')) {
            throw "Invalid addon URL: $url"
        }

        if ($cleanUrl.StartsWith($repoRawMainPrefix)) {
            if (-not $resolvedRef) {
                try {
                    $resolvedRef = (Invoke-RestMethod -Uri "https://api.github.com/repos/oneulddu/musicxmatch-api/commits/main" -UseBasicParsing).sha
                } catch {
                    $resolvedRef = $null
                }
            }

            if ($resolvedRef) {
                $relativePath = $cleanUrl.Substring($repoRawMainPrefix.Length)
                $downloadUrl = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/$resolvedRef/$relativePath"
            } else {
                $separator = '?'
                if ($url.Contains('?')) {
                    $separator = '&'
                }
                $downloadUrl = "$url$separator" + "ts=$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())"
            }
        } elseif ($cleanUrl.StartsWith('https://raw.githubusercontent.com/')) {
            $separator = '?'
            if ($url.Contains('?')) {
                $separator = '&'
            }
            $downloadUrl = "$url$separator" + "ts=$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())"
        }

        $destination = Join-Path $addonDir $fileName
        Backup-FileForRollback -Path $destination
        Invoke-WebRequest -Uri $downloadUrl -OutFile $destination -UseBasicParsing
        $sources[$fileName] = $cleanUrl

        if ($manifest.subfiles_extension -notcontains $fileName) {
            $manifest.subfiles_extension += $fileName
        }
    }

    Backup-FileForRollback -Path $sourcesPath
    Backup-FileForRollback -Path $manifestPath
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($sourcesPath, (($sources | ConvertTo-Json -Depth 5) + "`r`n"), $utf8NoBom)
    [System.IO.File]::WriteAllText($manifestPath, (($manifest | ConvertTo-Json -Depth 20) + "`r`n"), $utf8NoBom)

    Write-Host "Registered addons:"
    foreach ($url in $Urls) {
        Write-Host " - $([System.IO.Path]::GetFileName(($url -split '\?')[0]))"
    }

    if (Get-Command spicetify -ErrorAction SilentlyContinue) {
        $lyricsManagerPath = Join-Path $addonDir "LyricsAddonManager.js"
        if (Test-Path -LiteralPath $lyricsManagerPath) {
            Backup-FileForRollback -Path $lyricsManagerPath
            try {
                $localPatchScript = Join-Path (Get-Location) $patchScriptPath
                if (Test-Path -LiteralPath $localPatchScript) {
                    & $localPatchScript -TargetPath $lyricsManagerPath -NoApply
                } else {
                    $patchUrl = "$repoRawMainPrefix$patchScriptPath" + "?ts=$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())"
                    $patchScript = [scriptblock]::Create((Invoke-WebRequest -Uri $patchUrl -UseBasicParsing).Content)
                    & $patchScript -TargetPath $lyricsManagerPath -NoApply
                }
            } catch {
                Write-Warning "ivLyrics selection patch failed; continuing without it. $($_.Exception.Message)"
            }
        }

        Stop-SpotifyIfRunning
        spicetify apply
        if ($LASTEXITCODE -ne 0) {
            throw "spicetify apply failed with exit code $LASTEXITCODE"
        }
    } else {
        Write-Warning "spicetify not found; addon files were registered but apply was skipped."
    }
} catch {
    Restore-Backups
    throw
} finally {
    if (Test-Path -LiteralPath $backupDir) {
        Remove-Item -LiteralPath $backupDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    Restart-SpotifyIfNeeded
}
