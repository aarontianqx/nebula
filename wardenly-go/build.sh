#!/bin/bash
# Wardenly Build Script
# Usage: ./build.sh [-prod]

set -e

# Colors for output
info() { echo -e "\033[36m[INFO]\033[0m $1"; }
ok() { echo -e "\033[32m[OK]\033[0m $1"; }
warn() { echo -e "\033[33m[WARN]\033[0m $1"; }
error() { echo -e "\033[31m[ERROR]\033[0m $1"; }

PROD=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -prod|--prod)
            PROD=true
            shift
            ;;
        *)
            shift
            ;;
    esac
done

info "Wardenly Build Script"
info "====================="

# Ensure we're in the project directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Check go-winres is installed
info "Checking go-winres..."
if ! command -v go-winres &> /dev/null; then
    warn "go-winres not found, installing..."
    go install github.com/tc-hib/go-winres@latest
    ok "go-winres installed"
fi

# Generate Windows resources (in cmd/wardenly directory where main.go is)
info "Generating Windows resources..."
pushd cmd/wardenly > /dev/null
go-winres make
popd > /dev/null
ok "Windows resources generated"

# Build parameters
OUTPUT_NAME="wardenly.exe"
LDFLAGS=""

if [ "$PROD" = true ]; then
    info "Production build enabled (optimized, no console)"
    LDFLAGS="-s -w -H windowsgui"
else
    info "Development build"
fi

# Build command
info "Building..."

if [ "$PROD" = true ]; then
    go build -trimpath -tags prod -ldflags="$LDFLAGS" -o "$OUTPUT_NAME" ./cmd/wardenly
else
    go build -o "$OUTPUT_NAME" ./cmd/wardenly
fi

# Get file size
if [ -f "$OUTPUT_NAME" ]; then
    FILE_SIZE=$(ls -lh "$OUTPUT_NAME" | awk '{print $5}')
    ok "Build completed: $OUTPUT_NAME ($FILE_SIZE)"
    echo ""
    info "Build Summary:"
    info "  - Output: $OUTPUT_NAME"
    info "  - Size: $FILE_SIZE"
    if [ "$PROD" = true ]; then
        info "  - Mode: Production (optimized, no console)"
    else
        info "  - Mode: Development"
    fi
    echo ""
    info "Run with: ./$OUTPUT_NAME"
else
    error "Build failed"
    exit 1
fi
