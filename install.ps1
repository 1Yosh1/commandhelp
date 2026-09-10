# CommandHelp (`chelp`) Windows PowerShell Installer
# Run via: irm https://raw.githubusercontent.com/1Yosh1/commandhelp/master/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "1Yosh1/commandhelp"
$Target = "x86_64-pc-windows-msvc"
$InstallDir = Join-Path $HOME ".chelp\bin"
$BinaryPath = Join-Path $InstallDir "chelp.exe"
$ReleaseUrl = "https://github.com/$Repo/releases/latest/download/chelp-$Target.zip"

Write-Host "🚀 Installing CommandHelp (chelp) for Windows..." -ForegroundColor Cyan

# 1. Ensure target directory exists
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# 2. Download release zip
$TempZip = Join-Path $env:TEMP "chelp-$Target.zip"
Write-Host "Downloading latest release ($Target)..." -ForegroundColor Gray
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -Uri $ReleaseUrl -OutFile $TempZip -UseBasicParsing

# 3. Extract and place binary
Write-Host "Extracting chelp.exe..." -ForegroundColor Gray
Expand-Archive -Path $TempZip -DestinationPath $env:TEMP\chelp-extract -Force
Copy-Item -Path "$env:TEMP\chelp-extract\chelp.exe" -Destination $BinaryPath -Force
Remove-Item -Path $TempZip -Force -ErrorAction SilentlyContinue
Remove-Item -Path "$env:TEMP\chelp-extract" -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "✔ Installed chelp.exe to $BinaryPath" -ForegroundColor Green

# 4. Add to User PATH if not already present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$PathEntries = $UserPath -split ";"

if ($PathEntries -notcontains $InstallDir) {
    Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Gray
    $NewPath = if ([string]::IsNullOrWhiteSpace($UserPath)) { $InstallDir } else { "$UserPath;$InstallDir" }
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:PATH = "$InstallDir;$env:PATH"
    Write-Host "✔ PATH updated successfully." -ForegroundColor Green
}

Write-Host ""
Write-Host "✨ CommandHelp is ready to configure!" -ForegroundColor Cyan
Write-Host "Launching automatic 1-step setup wizard..." -ForegroundColor Yellow
Write-Host ""

# 5. Launch interactive setup
& $BinaryPath setup
