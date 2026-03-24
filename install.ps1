$ErrorActionPreference = "Stop"

$RepoZipUrl = "https://github.com/Strvm/musicxmatch-api/archive/refs/heads/main.zip"
$InstallDir = "$env:LOCALAPPDATA\ivLyrics\musicxmatch-provider"
$Port = if ($env:PORT) { $env:PORT } else { "8092" }

function Resolve-Python {
    foreach ($candidate in @("py", "python")) {
        try {
            if ($candidate -eq "py") {
                & py -3 --version | Out-Null
                return @("py", "-3")
            }
            & python --version | Out-Null
            return @("python")
        } catch {}
    }
    throw "Python 3 not found."
}

function Invoke-Python {
    param([string[]]$Args)

    if ($pythonCmd.Count -gt 1) {
        & $pythonCmd[0] $pythonCmd[1] @Args
        return
    }

    & $pythonCmd[0] @Args
}

Write-Host ""
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  MusicXMatch Provider Install" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "[1/4] Checking Python 3..." -ForegroundColor Yellow
$pythonCmd = Resolve-Python
Write-Host "  [OK] Python detected" -ForegroundColor Green

Write-Host "[2/4] Downloading repository..." -ForegroundColor Yellow
$tempRoot = Join-Path $env:TEMP ("musicxmatch-provider-" + [guid]::NewGuid().ToString("N"))
$zipPath = Join-Path $tempRoot "repo.zip"
$extractRoot = Join-Path $tempRoot "extract"
New-Item -ItemType Directory -Force -Path $extractRoot | Out-Null
Invoke-WebRequest -Uri $RepoZipUrl -OutFile $zipPath -UseBasicParsing
Expand-Archive -Path $zipPath -DestinationPath $extractRoot -Force
$sourceDir = Get-ChildItem -Path $extractRoot -Directory | Select-Object -First 1

if (Test-Path $InstallDir) {
    Remove-Item $InstallDir -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item (Join-Path $sourceDir.FullName "*") $InstallDir -Recurse -Force
Write-Host "  [OK] Installed files to $InstallDir" -ForegroundColor Green

Write-Host "[3/4] Creating virtualenv and installing package..." -ForegroundColor Yellow
Invoke-Python @("-m", "venv", "$InstallDir\.venv")
& "$InstallDir\.venv\Scripts\python.exe" -m pip install --upgrade pip setuptools wheel | Out-Null
& "$InstallDir\.venv\Scripts\python.exe" -m pip install -e "$InstallDir" | Out-Null

$startPs1 = "$InstallDir\start.ps1"
@"
& "$InstallDir\.venv\Scripts\musicxmatch-addon-server.exe" --host 127.0.0.1 --port $Port
"@ | Set-Content -Path $startPs1 -Encoding ASCII

$vbsPath = "$InstallDir\start.vbs"
Set-Content -Path $vbsPath -Value 'Set WshShell = CreateObject("WScript.Shell")' -Encoding ASCII
Add-Content -Path $vbsPath -Value ('WshShell.Run "powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File ""' + $startPs1 + '""", 0, False') -Encoding ASCII
Write-Host "  [OK] Virtualenv ready" -ForegroundColor Green

Write-Host "[4/4] Registering auto-start..." -ForegroundColor Yellow
$startupFolder = [System.Environment]::GetFolderPath("Startup")
$shortcutPath = Join-Path $startupFolder "MusicXMatch Provider.lnk"
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = "wscript.exe"
$shortcut.Arguments = "`"$vbsPath`""
$shortcut.WorkingDirectory = $InstallDir
$shortcut.Description = "MusicXMatch ivLyrics provider"
$shortcut.Save()
Start-Process "wscript.exe" -ArgumentList "`"$vbsPath`"" -WorkingDirectory $InstallDir

Write-Host ""
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Installation complete!" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Server URL:   http://localhost:$Port" -ForegroundColor White
Write-Host "  Install path: $InstallDir" -ForegroundColor White
Write-Host "  Addon file:   $InstallDir\Addon_Lyrics_MusicXMatch.js" -ForegroundColor White
Write-Host "  Auto-start registered in Startup." -ForegroundColor White
Write-Host ""

Start-Sleep -Seconds 3
try {
    Invoke-WebRequest -Uri "http://127.0.0.1:$Port/health" -TimeoutSec 5 -UseBasicParsing | Out-Null
    Write-Host "  [OK] Server is running" -ForegroundColor Green
} catch {
    Write-Host "  Server may still be starting. Test the connection in ivLyrics shortly." -ForegroundColor Yellow
}

if (Test-Path $tempRoot) {
    Remove-Item $tempRoot -Recurse -Force
}
Write-Host ""
