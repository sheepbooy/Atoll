# Install Atoll from the latest GitHub Release (Windows).
# Usage:
#   irm https://raw.githubusercontent.com/sheepbooy/Atoll/main/scripts/install.ps1 | iex
# Pin a version:
#   $env:ATOLL_VERSION = "0.1.8"; irm .../install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "sheepbooy/Atoll"
$AssetName = "Atoll-x64.msi"

function Write-Info($Message) {
    Write-Host "==> $Message"
}

function Write-Err($Message) {
    Write-Host "error: $Message" -ForegroundColor Red
    exit 1
}

function Get-GitHubToken {
    if ($env:GH_TOKEN) { return $env:GH_TOKEN }
    if ($env:GITHUB_TOKEN) { return $env:GITHUB_TOKEN }
    return $null
}

function Get-ReleaseDownloadBase {
    param([string]$Version)
    if ($Version) {
        return "https://github.com/$Repo/releases/download/v$Version"
    }
    return "https://github.com/$Repo/releases/latest/download"
}

function Download-ReleaseAsset {
    param(
        [string]$Version,
        [string]$Asset,
        [string]$Destination
    )

    $base = Get-ReleaseDownloadBase -Version $Version
    $url = "$base/$Asset"
    $label = if ($Version) { "v$Version" } else { "latest" }
    Write-Info "Downloading $Asset ($label)..."

    $headers = @{ "User-Agent" = "Atoll-Installer/1.0" }
    $token = Get-GitHubToken
    if ($token) {
        $headers["Authorization"] = "Bearer $token"
    }

    Invoke-WebRequest -Uri $url -OutFile $Destination -Headers $headers
}

$version = $env:ATOLL_VERSION
if ($version) {
    $version = $version.TrimStart("v")
}

$tempDir = Join-Path $env:TEMP ("atoll-install-" + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tempDir | Out-Null
$msiPath = Join-Path $tempDir $AssetName

try {
    Download-ReleaseAsset -Version $version -Asset $AssetName -Destination $msiPath

    if (-not (Test-Path $msiPath)) {
        Write-Err "Download failed: $msiPath not found"
    }

    Write-Info "Installing Atoll..."
    $arguments = @("/i", $msiPath, "/passive", "/norestart")
    $process = Start-Process -FilePath "msiexec.exe" -ArgumentList $arguments -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        Write-Err "msiexec exited with code $($process.ExitCode)"
    }

    Write-Host ""
    Write-Host "Atoll installed successfully."
    Write-Host "If Windows SmartScreen warns on first launch, choose 'More info' -> 'Run anyway'."
    Write-Host "Node.js must be installed and available on PATH for agent hooks."
}
finally {
    Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
}
