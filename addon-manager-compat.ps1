param(
    [Parameter(Mandatory = $true, ValueFromRemainingArguments = $true)]
    [string[]]$Urls
)

$addonDir = "$env:LOCALAPPDATA\spicetify\CustomApps\ivLyrics"
$sourcesDir = "$env:LOCALAPPDATA\spicetify\ivLyrics"
$manifestPath = Join-Path $addonDir "manifest.json"
$sourcesPath = Join-Path $sourcesDir "addon_sources.json"

if (-not (Test-Path $manifestPath)) {
    throw "ivLyrics manifest not found at $manifestPath"
}

New-Item -ItemType Directory -Force -Path $addonDir | Out-Null
New-Item -ItemType Directory -Force -Path $sourcesDir | Out-Null

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

    if (-not $fileName -or -not $fileName.EndsWith('.js')) {
        throw "Invalid addon URL: $url"
    }

    $destination = Join-Path $addonDir $fileName
    Invoke-WebRequest -Uri $cleanUrl -OutFile $destination -UseBasicParsing
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
} else {
    Write-Warning "spicetify not found; addon files were registered but apply was skipped."
}
