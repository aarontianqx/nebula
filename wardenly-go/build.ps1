# Wardenly Build Script
# For Windows PowerShell
# Usage: .\build.ps1 [-prod]

param(
    [switch]$prod  # Production build with optimizations
)

$ErrorActionPreference = "Stop"

# Colors for output
function Write-Info { param($msg) Write-Host "[INFO] $msg" -ForegroundColor Cyan }
function Write-Success { param($msg) Write-Host "[OK] $msg" -ForegroundColor Green }
function Write-Warning { param($msg) Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Write-Error { param($msg) Write-Host "[ERROR] $msg" -ForegroundColor Red }

Write-Info "Wardenly Build Script"
Write-Info "====================="

# Ensure we're in the project directory
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $scriptDir

# Check go-winres is installed
Write-Info "Checking go-winres..."
$winresInstalled = $null
try {
    $winresInstalled = Get-Command go-winres -ErrorAction SilentlyContinue
} catch {}

if (-not $winresInstalled) {
    Write-Warning "go-winres not found, installing..."
    go install github.com/tc-hib/go-winres@latest
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Failed to install go-winres"
        exit 1
    }
    Write-Success "go-winres installed"
}

# Generate Windows resources (in cmd/wardenly directory where main.go is)
Write-Info "Generating Windows resources..."
Push-Location cmd/wardenly
go-winres make
$winresResult = $LASTEXITCODE
Pop-Location
if ($winresResult -ne 0) {
    Write-Error "Failed to generate Windows resources"
    exit 1
}
Write-Success "Windows resources generated"

# Build parameters
$outputName = "wardenly.exe"
$ldflags = ""

if ($prod) {
    Write-Info "Production build enabled (optimized, no console)"
    $ldflags = "-s -w -H windowsgui"
} else {
    Write-Info "Development build"
}

# Build command
Write-Info "Building..."

if ($prod) {
    go build -trimpath -tags prod -ldflags="$ldflags" -o $outputName ./cmd/wardenly
} else {
    go build -o $outputName ./cmd/wardenly
}

if ($LASTEXITCODE -ne 0) {
    Write-Error "Build failed"
    exit 1
}

# Get file info
$fileInfo = Get-Item $outputName
$fileSize = [math]::Round($fileInfo.Length / 1MB, 2)

Write-Success "Build completed: $outputName ($fileSize MB)"
Write-Info ""
Write-Info "Build Summary:"
Write-Info "  - Output: $outputName"
Write-Info "  - Size: $fileSize MB"
Write-Info "  - Mode: $(if ($prod) { 'Production (optimized, no console)' } else { 'Development' })"
Write-Info ""
Write-Info "Run with: .\$outputName"
