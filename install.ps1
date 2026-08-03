# Amigo Engine CLI installer for Windows
# Usage: irm https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.ps1 | iex
$ErrorActionPreference = "Stop"

$Repo = "amigo-labs/amigo-engine"
$InstallDir = if ($env:AMIGO_INSTALL_DIR) { $env:AMIGO_INSTALL_DIR } else { "$env:USERPROFILE\.amigo\bin" }
$BinaryName = "amigo.exe"

# ---------------------------------------------------------------------------
# Detect architecture
# ---------------------------------------------------------------------------

$BuildFromSource = "cargo install --git https://github.com/$Repo amigo_cli"

# Must stay in sync with the build matrix in .github/workflows/release.yml --
# a triple that CI does not build is a 404, not an install.
$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($Arch) {
    "X64"   { $Target = "x86_64-pc-windows-msvc" }
    "Arm64" {
        Write-Error @"
No pre-built binary for ARM64 Windows.

Build from source instead (needs the Rust toolchain):
  $BuildFromSource
"@
        exit 1
    }
    default {
        Write-Error @"
Unsupported architecture: $Arch

Build from source instead (needs the Rust toolchain):
  $BuildFromSource
"@
        exit 1
    }
}

# ---------------------------------------------------------------------------
# Resolve version
# ---------------------------------------------------------------------------

$Version = if ($env:AMIGO_VERSION) { $env:AMIGO_VERSION } else { "latest" }

if ($Version -eq "latest") {
    Write-Host "Fetching latest release..."
    try {
        $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
        $Version = $Release.tag_name
    } catch {
        Write-Error "Could not determine latest version. Check https://github.com/$Repo/releases"
        exit 1
    }
}

Write-Host "Installing amigo $Version for Windows/$Arch..."

# ---------------------------------------------------------------------------
# Download and install
# ---------------------------------------------------------------------------

$AssetName = "amigo-$Target.zip"
$DownloadUrl = "https://github.com/$Repo/releases/download/$Version/$AssetName"

$TmpDir = Join-Path $env:TEMP "amigo-install-$(Get-Random)"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

$ZipPath = Join-Path $TmpDir $AssetName

Write-Host "Downloading $DownloadUrl..."
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing
} catch {
    Write-Host ""
    Write-Error @"
Download failed.
  URL: $DownloadUrl

If this is a new installation, make sure a release with attached binaries
exists at:
  https://github.com/$Repo/releases

Alternatively, build from source (needs the Rust toolchain):
  $BuildFromSource
"@
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
    exit 1
}

Write-Host "Extracting..."
Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

# ---------------------------------------------------------------------------
# Install binary
# ---------------------------------------------------------------------------

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$SourceBin = Join-Path $TmpDir $BinaryName
if (-not (Test-Path $SourceBin)) {
    # Binary might be in a subdirectory.
    $SourceBin = Get-ChildItem -Path $TmpDir -Filter $BinaryName -Recurse | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $SourceBin) {
    Write-Error "'$BinaryName' not found in the downloaded archive."
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
    exit 1
}

Copy-Item -Path $SourceBin -Destination (Join-Path $InstallDir $BinaryName) -Force

# Companion MCP servers (present in newer releases; optional otherwise).
foreach ($Extra in @("amigo-artgen.exe", "amigo-audiogen.exe")) {
    $ExtraBin = Get-ChildItem -Path $TmpDir -Filter $Extra -Recurse | Select-Object -First 1 -ExpandProperty FullName
    if ($ExtraBin) {
        Copy-Item -Path $ExtraBin -Destination (Join-Path $InstallDir $Extra) -Force
        Write-Host "Installed $Extra to $InstallDir\$Extra"
    }
}

# Cleanup.
Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "Installed amigo to $InstallDir\$BinaryName"

# ---------------------------------------------------------------------------
# PATH check
# ---------------------------------------------------------------------------

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host ""
    Write-Host "Adding $InstallDir to your user PATH..."
    $NewPath = "$InstallDir;$UserPath"
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:Path = "$InstallDir;$env:Path"
    Write-Host "Done. Restart your terminal for PATH changes to take effect."
}

Write-Host ""
Write-Host "Run 'amigo --help' to get started."
Write-Host "Run 'amigo setup' to install the Python AI toolchain."
