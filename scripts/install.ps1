# Install Atoll from the latest GitHub Release (Windows).
# Usage:
#   irm https://raw.githubusercontent.com/sheepbooy/Atoll/main/scripts/install.ps1 | iex
# Pin a version:
#   $env:ATOLL_VERSION = "0.1.11"; irm .../install.ps1 | iex
# Private repo:
#   $env:GH_TOKEN = "..."; $env:ATOLL_VERSION = "0.1.11"; irm .../install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "sheepbooy/Atoll"
$AssetName = "Atoll-x64.msi"
$UserAgent = "Atoll-Installer/1.0"

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

function Get-GitHubHeaders {
    param([string]$Accept = "application/vnd.github+json")

    $headers = @{
        "Accept" = $Accept
        "User-Agent" = $UserAgent
    }
    $token = Get-GitHubToken
    if ($token) {
        $headers["Authorization"] = "Bearer $token"
    }
    return $headers
}

function Require-Windows {
    if ($env:OS -ne "Windows_NT") {
        Write-Err "Atoll Windows installer only supports Windows."
    }
}

function Normalize-Version {
    param([string]$Version)
    if (-not $Version) { return $null }
    return $Version.Trim().TrimStart("v")
}

function Get-ReleaseDownloadBase {
    param([string]$Version)
    if ($Version) {
        return "https://github.com/$Repo/releases/download/v$Version"
    }
    return "https://github.com/$Repo/releases/latest/download"
}

function Get-ReleaseAssetApiUrl {
    param(
        [string]$Version,
        [string]$Asset
    )

    $headers = Get-GitHubHeaders
    $releaseUrl = "https://api.github.com/repos/$Repo/releases/tags/v$Version"

    try {
        $release = Invoke-RestMethod -Uri $releaseUrl -Headers $headers -Method Get
    }
    catch {
        if (-not (Get-GitHubToken)) {
            Write-Err "Could not access GitHub release metadata. Set GH_TOKEN for private repositories."
        }
        throw
    }

    $match = $release.assets | Where-Object { $_.name -eq $Asset } | Select-Object -First 1
    if (-not $match) {
        Write-Err "Release asset not found: $Asset"
    }

    return $match.url
}

function Download-PublicReleaseAsset {
    param(
        [string]$Version,
        [string]$Asset,
        [string]$Destination
    )

    $base = Get-ReleaseDownloadBase -Version $Version
    $url = "$base/$Asset"
    $headers = Get-GitHubHeaders -Accept "application/octet-stream"
    Invoke-WebRequest -Uri $url -OutFile $Destination -Headers $headers
}

function Download-PrivateReleaseAsset {
    param(
        [string]$Version,
        [string]$Asset,
        [string]$Destination
    )

    if (-not $Version) {
        Write-Err "Private repository installs require ATOLL_VERSION"
    }

    $assetUrl = Get-ReleaseAssetApiUrl -Version $Version -Asset $Asset
    $headers = Get-GitHubHeaders -Accept "application/octet-stream"
    Invoke-WebRequest -Uri $assetUrl -OutFile $Destination -Headers $headers
}

function Download-ReleaseAsset {
    param(
        [string]$Version,
        [string]$Asset,
        [string]$Destination
    )

    $label = if ($Version) { "v$Version" } else { "latest" }
    Write-Info "Downloading $Asset ($label)..."

    if (Get-GitHubToken) {
        Download-PrivateReleaseAsset -Version $Version -Asset $Asset -Destination $Destination
    }
    else {
        Download-PublicReleaseAsset -Version $Version -Asset $Asset -Destination $Destination
    }
}

function Verify-Checksum {
    param(
        [string]$Version,
        [string]$AssetName,
        [string]$AssetPath
    )

    $checksumName = "$AssetName.sha256"
    $checksumPath = Join-Path ([System.IO.Path]::GetTempPath()) ("atoll-checksum-" + [guid]::NewGuid().ToString())

    try {
        Download-ReleaseAsset -Version $Version -Asset $checksumName -Destination $checksumPath
    }
    catch {
        Write-Info "No published sha256 file; skipping checksum verification."
        return
    }

    try {
        $expected = ((Get-Content -Path $checksumPath -Raw).Trim() -replace '\s', '')
        $hash = Get-FileHash -Path $AssetPath -Algorithm SHA256
        $actual = $hash.Hash.ToLowerInvariant()

        if ($expected -ne $actual) {
            Write-Err "Checksum mismatch for $AssetName"
        }

        Write-Info "Checksum verified."
    }
    finally {
        Remove-Item -Path $checksumPath -Force -ErrorAction SilentlyContinue
    }
}

function Print-Success {
    Write-Host ""
    Write-Host "Atoll is installed."
    Write-Host ""
    Write-Host "Start menu: Atoll"
    Write-Host "Install location: C:\Program Files\Atoll\ (typical)"
    Write-Host ""
    Write-Host "If Windows SmartScreen warns on first launch, choose 'More info' -> 'Run anyway'."
    Write-Host ""
    Write-Host "Next steps:"
    Write-Host "  1. Open Atoll from the Start menu."
    Write-Host "  2. Use the tray/island menu and click 'Install hooks' to connect Claude Code."
    Write-Host ""
    Write-Host "Node.js must be installed and available on PATH for agent hooks."
}

Require-Windows

$version = Normalize-Version -Version $env:ATOLL_VERSION

$tempDir = Join-Path $env:TEMP ("atoll-install-" + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tempDir | Out-Null
$msiPath = Join-Path $tempDir $AssetName

try {
    Download-ReleaseAsset -Version $version -Asset $AssetName -Destination $msiPath

    if (-not (Test-Path $msiPath)) {
        Write-Err "Download failed: $msiPath not found"
    }

    Verify-Checksum -Version $version -AssetName $AssetName -AssetPath $msiPath

    Write-Info "Installing Atoll..."
    $arguments = @("/i", $msiPath, "/passive", "/norestart")
    $process = Start-Process -FilePath "msiexec.exe" -ArgumentList $arguments -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        Write-Err "msiexec exited with code $($process.ExitCode)"
    }

    Print-Success
}
finally {
    Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
}
