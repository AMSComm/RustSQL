# ==============================================================================
# RustSQL (ruql) Universal Installer for Windows PowerShell
# Repository: https://github.com/AMSComm/RustSQL
# Run via: irm https://raw.githubusercontent.com/AMSComm/RustSQL/main/install.ps1 | iex
# ==============================================================================

$ErrorActionPreference = 'Stop'

$Repo = "AMSComm/RustSQL"
$BinName = "ruql.exe"
$TargetTriple = "x86_64-pc-windows-msvc"

Write-Host "=== Installing RustSQL (ruql) for Windows ===" -ForegroundColor Cyan

# 1. Fetch latest release from GitHub API
$ApiUrl = "https://api.github.com/repos/$Repo/releases/latest"
Write-Host "Fetching latest release from https://github.com/$Repo..." -ForegroundColor DarkGray

try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $Release = Invoke-RestMethod -Uri $ApiUrl -Method Get -Headers @{ "User-Agent" = "ruql-installer" }
} catch {
    Write-Warning "Could not reach GitHub releases for $Repo (HTTP error or repository is private)."
    Write-Host "You can build directly from source using 'cargo build --release'."
    return
}

$TagName = $Release.tag_name
Write-Host "Latest release: $TagName" -ForegroundColor Green

# 2. Find Windows asset
$Asset = $Release.assets | Where-Object { $_.name -like "*$TargetTriple*.zip" } | Select-Object -First 1

if (-not $Asset) {
    Write-Error "No Windows release asset found matching $TargetTriple in $TagName."
    return
}

$DownloadUrl = $Asset.browser_download_url
Write-Host "Downloading $($Asset.name)..." -ForegroundColor Cyan

$TempZip = Join-Path $env:TEMP "$($Asset.name)"
Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZip -UseBasicParsing

# 3. Extract Archive
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\ruql"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Write-Host "Extracting to $InstallDir..." -ForegroundColor DarkGray
Expand-Archive -Path $TempZip -DestinationPath $InstallDir -Force
Remove-Item -Path $TempZip -Force

# Locate ruql.exe
$InstalledBin = Join-Path $InstallDir $BinName
if (-not (Test-Path $InstalledBin)) {
    # Check subfolders
    $Found = Get-ChildItem -Path $InstallDir -Filter $BinName -Recurse | Select-Object -First 1
    if ($Found) {
        Move-Item -Path $Found.FullName -Destination $InstalledBin -Force
    } else {
        Write-Error "Could not find $BinName inside extracted archive."
        return
    }
}

# Also create psql.exe copy/symlink if desired
$PsqlBin = Join-Path $InstallDir "psql.exe"
if (-not (Test-Path $PsqlBin)) {
    Copy-Item -Path $InstalledBin -Destination $PsqlBin -Force
}

# 4. Add to User PATH if not present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path = "$env:Path;$InstallDir"
}

Write-Host "`n✓ RustSQL (ruql) installed successfully to $InstallDir!" -ForegroundColor Green
Write-Host "Restart your terminal and run:"
Write-Host "  ruql --version"
Write-Host "  ruql --help"
Write-Host "To update in the future, simply run:"
Write-Host "  ruql --update"
