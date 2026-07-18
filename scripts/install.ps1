# Tokenwise installer — Windows (PowerShell)
# Usage: iwr -useb https://raw.githubusercontent.com/somarimapps/tokenwise/main/scripts/install.ps1 | iex
# Or:   .\install.ps1 [-Version v0.1.0]
param(
    [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"

$Repo    = "somarimapps/tokenwise"
$Binary  = "tokenwise.exe"
$InstDir = "$env:ProgramFiles\tokenwise"

# Detect architecture
$Arch = if ([System.Environment]::Is64BitOperatingSystem) { "x64" } else { "x86" }
$Artifact = "tokenwise-windows-$Arch.exe"

# Resolve latest version if not pinned
if ($Version -eq "latest") {
    Write-Host "Resolving latest version from GitHub..."
    try {
        $Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
        $Version = $Release.tag_name
    } catch {
        Write-Error "Could not determine latest version. Pass a version explicitly: .\install.ps1 -Version v0.1.0"
        exit 1
    }
}

$Url = "https://github.com/$Repo/releases/download/$Version/$Artifact"

Write-Host "Downloading tokenwise $Version for Windows-$Arch..."

$Tmp = [System.IO.Path]::GetTempFileName() + ".exe"
try {
    Invoke-WebRequest -Uri $Url -OutFile $Tmp -UseBasicParsing
} catch {
    Write-Error "Download failed: $Url`n$_"
    exit 1
}

# Create install directory and move binary
if (-not (Test-Path $InstDir)) {
    New-Item -ItemType Directory -Force -Path $InstDir | Out-Null
}
Move-Item -Force $Tmp "$InstDir\$Binary"

# Add install directory to machine PATH (requires elevation)
$CurrentPath = [Environment]::GetEnvironmentVariable("Path", "Machine")
if ($CurrentPath -notlike "*$InstDir*") {
    try {
        [Environment]::SetEnvironmentVariable("Path", "$CurrentPath;$InstDir", "Machine")
        Write-Host "Added $InstDir to system PATH."
    } catch {
        Write-Warning "Could not update system PATH (not elevated?). Add $InstDir to your PATH manually."
    }
}

# Also add to current session PATH so tokenwise is available immediately
$env:PATH = "$env:PATH;$InstDir"

Write-Host "Tokenwise $Version installed at $InstDir\$Binary"

# Run initial stack setup
if (Get-Command tokenwise -ErrorAction SilentlyContinue) {
    Write-Host "Running: tokenwise install"
    & tokenwise install
} else {
    Write-Warning "tokenwise not found in PATH. Add $InstDir to your PATH and run 'tokenwise install'."
}
