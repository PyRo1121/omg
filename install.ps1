# OMG Installer for Windows
# Usage: irm https://pyro1121.com/install.ps1 | iex

param(
    [string]$Version = "latest",
    [switch]$NoTelemetry,
    [switch]$SkipShell,
    [string]$InstallDir = "$env:LOCALAPPDATA\Programs\omg"
)

$ErrorActionPreference = "Stop"

$RepoOwner = "PyRo1121"
$RepoName = "omg"
$GitHubAPI = "https://api.github.com/repos/$RepoOwner/$RepoName"

function Write-Header {
    param([string]$Text)
    Write-Host "`n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Cyan
    Write-Host "  $Text" -ForegroundColor Cyan
    Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`n" -ForegroundColor Cyan
}

function Write-Success {
    param([string]$Text)
    Write-Host "✓ $Text" -ForegroundColor Green
}

function Write-Info {
    param([string]$Text)
    Write-Host "ℹ $Text" -ForegroundColor Blue
}

function Write-Warning {
    param([string]$Text)
    Write-Host "⚠ $Text" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Text)
    Write-Host "✗ $Text" -ForegroundColor Red
}

function Get-LatestVersion {
    try {
        $response = Invoke-RestMethod -Uri "$GitHubAPI/releases/latest"
        return $response.tag_name
    } catch {
        Write-Error "Failed to fetch latest version from GitHub"
        throw
    }
}

function Get-Architecture {
    if ([Environment]::Is64BitOperatingSystem) {
        return "x86_64"
    } else {
        Write-Error "32-bit Windows is not supported"
        exit 1
    }
}

function Download-OMG {
    param(
        [string]$Version,
        [string]$Arch
    )
    
    $VersionClean = $Version -replace '^v', ''
    $FileName = "omg-v$VersionClean-$Arch-windows.zip"
    $DownloadUrl = "https://github.com/$RepoOwner/$RepoName/releases/download/$Version/$FileName"
    $ChecksumUrl = "$DownloadUrl.sha256"
    
    Write-Info "Downloading OMG $Version for Windows ($Arch)..."
    Write-Host "  URL: $DownloadUrl" -ForegroundColor DarkGray
    
    $TempDir = Join-Path $env:TEMP "omg-install-$(Get-Random)"
    New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
    
    $ZipPath = Join-Path $TempDir $FileName
    $ChecksumPath = Join-Path $TempDir "$FileName.sha256"
    
    try {
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath
        Invoke-WebRequest -Uri $ChecksumUrl -OutFile $ChecksumPath
        Write-Success "Downloaded $FileName"
    } catch {
        Write-Error "Failed to download OMG"
        Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
        throw
    }
    
    Write-Info "Verifying checksum..."
    $ExpectedHash = (Get-Content $ChecksumPath).Split()[0]
    $ActualHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash
    
    if ($ExpectedHash.ToLower() -ne $ActualHash.ToLower()) {
        Write-Error "Checksum verification failed!"
        Write-Host "  Expected: $ExpectedHash" -ForegroundColor Red
        Write-Host "  Got:      $ActualHash" -ForegroundColor Red
        Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
        exit 1
    }
    Write-Success "Checksum verified"
    
    return @{
        ZipPath = $ZipPath
        TempDir = $TempDir
    }
}

function Install-OMG {
    param(
        [string]$ZipPath,
        [string]$TempDir,
        [string]$InstallDir
    )
    
    Write-Info "Installing to $InstallDir..."
    
    if (Test-Path $InstallDir) {
        Write-Warning "Install directory already exists, removing old installation..."
        Remove-Item -Recurse -Force $InstallDir
    }
    
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    
    Expand-Archive -Path $ZipPath -DestinationPath $TempDir -Force
    
    $ExtractedDir = Get-ChildItem -Path $TempDir -Directory | Where-Object { $_.Name -like "omg-v*" } | Select-Object -First 1
    
    if (-not $ExtractedDir) {
        Write-Error "Failed to find extracted directory"
        exit 1
    }
    
    Copy-Item -Path "$($ExtractedDir.FullName)\*" -Destination $InstallDir -Recurse -Force
    
    Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
    
    Write-Success "Installed to $InstallDir"
}

function Add-ToPath {
    param([string]$Dir)
    
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    
    if ($UserPath -like "*$Dir*") {
        Write-Info "Already in PATH"
        return
    }
    
    Write-Info "Adding to PATH..."
    $NewPath = "$UserPath;$Dir"
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:Path = "$env:Path;$Dir"
    Write-Success "Added to PATH (restart terminal to apply)"
}

function Setup-Telemetry {
    if ($NoTelemetry) {
        Write-Info "Telemetry disabled via -NoTelemetry flag"
        return
    }
    
    Write-Host "`n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Yellow
    Write-Host "  📊 Anonymous Telemetry" -ForegroundColor Yellow
    Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`n" -ForegroundColor Yellow
    
    Write-Host "OMG collects anonymous usage statistics to improve the tool:"
    Write-Host "  • Install count, OS/distro, OMG version"
    Write-Host "  • Command usage (e.g., 'search', 'install')"
    Write-Host "  • NO package names, file paths, or personal data"
    Write-Host ""
    Write-Host "Data is aggregated and anonymized. You can opt out anytime:"
    Write-Host "  omg config set telemetry.enabled false"
    Write-Host ""
    
    $response = Read-Host "Enable anonymous telemetry? [Y/n]"
    if ($response -eq "n" -or $response -eq "N") {
        Write-Info "Telemetry disabled"
        $env:OMG_NO_TELEMETRY = "1"
    } else {
        Write-Success "Telemetry enabled (thank you!)"
    }
}

function Main {
    Write-Header "🚀 OMG Installer for Windows"
    
    Write-Host "The fastest unified package manager for all platforms`n" -ForegroundColor DarkGray
    
    $Arch = Get-Architecture
    
    if ($Version -eq "latest") {
        Write-Info "Fetching latest version..."
        $Version = Get-LatestVersion
        Write-Success "Latest version: $Version"
    }
    
    $download = Download-OMG -Version $Version -Arch $Arch
    Install-OMG -ZipPath $download.ZipPath -TempDir $download.TempDir -InstallDir $InstallDir
    Add-ToPath -Dir $InstallDir
    
    if (-not $SkipShell) {
        Setup-Telemetry
    }
    
    Write-Header "✨ Installation Complete!"
    
    Write-Host "OMG is now installed at:" -ForegroundColor Green
    Write-Host "  $InstallDir`n" -ForegroundColor Cyan
    
    Write-Host "Quick Start:" -ForegroundColor Yellow
    Write-Host "  omg search firefox" -ForegroundColor Cyan
    Write-Host "  omg install <package>" -ForegroundColor Cyan
    Write-Host "  omg use node 20" -ForegroundColor Cyan
    Write-Host ""
    
    Write-Host "📚 Full documentation: " -NoNewline
    Write-Host "https://pyro1121.com/docs" -ForegroundColor Blue
    Write-Host ""
    
    Write-Warning "Restart your terminal for PATH changes to take effect"
    Write-Host ""
}

Main
