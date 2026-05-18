param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Args
)

$addonDir = "$env:LOCALAPPDATA\spicetify\CustomApps\ivLyrics"
$sourcesDir = "$env:LOCALAPPDATA\spicetify\ivLyrics"
$manifestPath = Join-Path $addonDir "manifest.json"
$sourcesPath = Join-Path $sourcesDir "addon_sources.json"
$repoRawMainPrefix = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/"
$knownAddons = @(
    "Addon_Lyrics_MusicXMatch.js",
    "Addon_Lyrics_Deezer.js",
    "Addon_Lyrics_Bugs.js",
    "Addon_Lyrics_Genie.js"
)
$resolvedRef = $null
$spotifyWasRunning = $false
$spotifyPath = $null
$restoreFromSources = $false
$Urls = @()

if ($Args.Count -gt 0 -and ($Args[0] -eq "--restore" -or $Args[0] -eq "--restore-existing")) {
    $restoreFromSources = $true
    if ($Args.Count -gt 1) {
        $Urls = $Args[1..($Args.Count - 1)]
    }
} else {
    $Urls = $Args
}

if ($Urls.Count -eq 0) {
    $restoreFromSources = $true
}

if (-not (Test-Path $manifestPath)) {
    throw "ivLyrics manifest not found at $manifestPath"
}

New-Item -ItemType Directory -Force -Path $addonDir | Out-Null
New-Item -ItemType Directory -Force -Path $sourcesDir | Out-Null

try {
    $spotifyProcess = Get-Process -Name Spotify -ErrorAction Stop | Select-Object -First 1
    $spotifyWasRunning = $true
    $spotifyPath = $spotifyProcess.Path
    Stop-Process -Id $spotifyProcess.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
} catch {
    $spotifyWasRunning = $false
}

$sources = @{}
if (Test-Path $sourcesPath) {
    try {
        $existing = Get-Content -Path $sourcesPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($existing) {
            $existing.PSObject.Properties | ForEach-Object {
                $sources[$_.Name] = $_.Value
            }
        }
    } catch {
        $sources = @{}
    }
}

if ($restoreFromSources) {
    foreach ($name in $knownAddons) {
        if ($sources.ContainsKey($name) -and $sources[$name] -is [string] -and ($sources[$name].StartsWith("http") -or $sources[$name].StartsWith("local:"))) {
            $Urls += $sources[$name]
        }
    }
}

if ($Urls.Count -eq 0) {
    throw "No addon URLs were provided and no restorable provider sources were found. Usage: addon-manager-compat.ps1 [--restore] <addon-url> [<addon-url> ...]"
}

$manifest = Get-Content -Path $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if (-not $manifest.subfiles_extension) {
    $manifest | Add-Member -NotePropertyName subfiles_extension -NotePropertyValue @()
}

$registered = @()
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
    try {
        if ($cleanUrl.StartsWith("local:")) {
            $localPath = $cleanUrl.Substring("local:".Length)
            Copy-Item -Path $localPath -Destination $destination -Force -ErrorAction Stop
        } else {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $destination -UseBasicParsing -ErrorAction Stop
        }
    } catch {
        Write-Warning "Skipping stale addon source: $url"
        continue
    }

    $sources[$fileName] = $cleanUrl

    if ($manifest.subfiles_extension -notcontains $fileName) {
        $manifest.subfiles_extension += $fileName
    }
    $registered += $fileName
}

if ($registered.Count -eq 0) {
    throw "No addon files could be restored or registered."
}

$utf8NoBom = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($sourcesPath, (($sources | ConvertTo-Json -Depth 5) + "`r`n"), $utf8NoBom)
[System.IO.File]::WriteAllText($manifestPath, (($manifest | ConvertTo-Json -Depth 20) + "`r`n"), $utf8NoBom)

Write-Host "Registered addons:"
foreach ($fileName in $registered) {
    Write-Host " - $fileName"
}

if (Get-Command spicetify -ErrorAction SilentlyContinue) {
    spicetify apply
    if ($spotifyWasRunning) {
        if ($spotifyPath -and (Test-Path $spotifyPath)) {
            Start-Process -FilePath $spotifyPath | Out-Null
        } else {
            Start-Process "spotify" | Out-Null
        }
    }
} else {
    Write-Warning "spicetify not found; addon files were registered but apply was skipped."
}
