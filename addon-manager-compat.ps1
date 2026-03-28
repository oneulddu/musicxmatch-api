param(
    [Parameter(Mandatory = $true, ValueFromRemainingArguments = $true)]
    [string[]]$Urls
)

$addonDir = "$env:LOCALAPPDATA\spicetify\CustomApps\ivLyrics"
$sourcesDir = "$env:LOCALAPPDATA\spicetify\ivLyrics"
$manifestPath = Join-Path $addonDir "manifest.json"
$sourcesPath = Join-Path $sourcesDir "addon_sources.json"
$spotifyWasRunning = $false
$spotifyPath = $null

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
    $existing = Get-Content -Path $sourcesPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($existing) {
        $existing.PSObject.Properties | ForEach-Object {
            $sources[$_.Name] = $_.Value
        }
    }
}

$manifest = Get-Content -Path $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if (-not $manifest.subfiles_extension) {
    $manifest | Add-Member -NotePropertyName subfiles_extension -NotePropertyValue @()
}

foreach ($url in $Urls) {
    $cleanUrl = ($url -split '\?')[0]
    $fileName = [System.IO.Path]::GetFileName($cleanUrl)
    $downloadUrl = $url

    if (-not $fileName -or -not $fileName.EndsWith('.js')) {
        throw "Invalid addon URL: $url"
    }

    if ($cleanUrl.StartsWith('https://raw.githubusercontent.com/')) {
        $separator = '?'
        if ($url.Contains('?')) {
            $separator = '&'
        }
        $downloadUrl = "$url$separator" + "ts=$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())"
    }

    $destination = Join-Path $addonDir $fileName
    Invoke-WebRequest -Uri $downloadUrl -OutFile $destination -UseBasicParsing
    $sources[$fileName] = $cleanUrl

    if ($manifest.subfiles_extension -notcontains $fileName) {
        $manifest.subfiles_extension += $fileName
    }
}

$utf8NoBom = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($sourcesPath, (($sources | ConvertTo-Json -Depth 5) + "`r`n"), $utf8NoBom)
[System.IO.File]::WriteAllText($manifestPath, (($manifest | ConvertTo-Json -Depth 20) + "`r`n"), $utf8NoBom)

Write-Host "Registered addons:"
foreach ($url in $Urls) {
    Write-Host " - $([System.IO.Path]::GetFileName(($url -split '\?')[0]))"
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
