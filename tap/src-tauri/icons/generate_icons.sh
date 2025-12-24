#!/bin/bash
#
# generate_icons.sh - Generate app icons for Tauri (macOS/Windows/Linux)
#
# Usage:
#   ./generate_icons.sh [source_image.png]
#
# Arguments:
#   source_image.png - Source image file (default: new_icon.png)
#                      Should be at least 512x512, ideally 1024x1024
#
# Output:
#   icon.png - 512x512 RGBA PNG (macOS/Linux)
#   icon.ico - Multi-size ICO (Windows: 256/128/64/48/32/16)
#
# Requirements:
#   - macOS with sips and swift

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SOURCE_IMAGE="${1:-new_icon.png}"

# Resolve to absolute path if relative
if [[ ! "$SOURCE_IMAGE" = /* ]]; then
    SOURCE_IMAGE="$SCRIPT_DIR/$SOURCE_IMAGE"
fi

if [[ ! -f "$SOURCE_IMAGE" ]]; then
    echo "Error: Source image not found: $SOURCE_IMAGE"
    echo "Usage: $0 [source_image.png]"
    exit 1
fi

echo "Source image: $SOURCE_IMAGE"

# Check image info
echo "Checking source image..."
sips -g pixelWidth -g pixelHeight -g hasAlpha "$SOURCE_IMAGE" 2>/dev/null || {
    echo "Error: Cannot read image info. Is this a valid PNG?"
    exit 1
}

# Create temp directory
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

echo ""
echo "Converting to RGBA and generating icons..."

# Use Swift to ensure RGBA format and generate all sizes
swift - "$SOURCE_IMAGE" "$SCRIPT_DIR" "$TEMP_DIR" << 'SWIFT_SCRIPT'
import AppKit
import Foundation

let sourcePath = CommandLine.arguments[1]
let outputDir = CommandLine.arguments[2]
let tempDir = CommandLine.arguments[3]

guard let image = NSImage(contentsOfFile: sourcePath) else {
    print("Error: Failed to load image from \(sourcePath)")
    exit(1)
}

// Function to create RGBA image at specified size
func createRGBAImage(from source: NSImage, size: Int) -> NSBitmapImageRep? {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: size,
        pixelsHigh: size,
        bitsPerSample: 8,
        samplesPerPixel: 4,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: .deviceRGB,
        bytesPerRow: 0,
        bitsPerPixel: 0
    )
    guard let rep = rep else { return nil }
    
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    NSGraphicsContext.current?.imageInterpolation = .high
    
    // Draw with white background for images without alpha, transparent otherwise
    let targetRect = NSRect(x: 0, y: 0, width: size, height: size)
    source.draw(in: targetRect, from: NSRect(origin: .zero, size: source.size), operation: .copy, fraction: 1.0)
    
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

// Generate 512x512 PNG for macOS/Linux
print("  Generating icon.png (512x512 RGBA)...")
if let rep = createRGBAImage(from: image, size: 512),
   let pngData = rep.representation(using: .png, properties: [:]) {
    let pngPath = URL(fileURLWithPath: outputDir).appendingPathComponent("icon.png")
    try! pngData.write(to: pngPath)
    print("  ✓ icon.png created")
} else {
    print("  ✗ Failed to create icon.png")
    exit(1)
}

// Generate ICO for Windows (multiple sizes)
print("  Generating icon.ico (256/128/64/48/32/16)...")
let icoSizes = [256, 128, 64, 48, 32, 16]
var imageDataList: [(size: Int, data: Data)] = []

for size in icoSizes {
    if let rep = createRGBAImage(from: image, size: size),
       let pngData = rep.representation(using: .png, properties: [:]) {
        imageDataList.append((size: size, data: pngData))
    }
}

// Build ICO file
var icoData = Data()

// ICO Header: reserved (2) + type (2, 1=icon) + count (2)
icoData.append(contentsOf: [0, 0])  // reserved
icoData.append(contentsOf: [1, 0])  // type = 1 (icon)
let count = UInt16(imageDataList.count)
icoData.append(UInt8(count & 0xFF))
icoData.append(UInt8(count >> 8))

// Calculate offset for image data (after header + all directory entries)
var offset = 6 + imageDataList.count * 16

// Directory entries
for (size, data) in imageDataList {
    let w = size >= 256 ? 0 : size  // 0 means 256 in ICO format
    let h = size >= 256 ? 0 : size
    
    icoData.append(UInt8(w))        // width
    icoData.append(UInt8(h))        // height
    icoData.append(0)               // color palette (0 = no palette)
    icoData.append(0)               // reserved
    icoData.append(contentsOf: [1, 0])   // color planes
    icoData.append(contentsOf: [32, 0])  // bits per pixel
    
    // Data size (4 bytes, little endian)
    let dataSize = UInt32(data.count)
    icoData.append(UInt8(dataSize & 0xFF))
    icoData.append(UInt8((dataSize >> 8) & 0xFF))
    icoData.append(UInt8((dataSize >> 16) & 0xFF))
    icoData.append(UInt8((dataSize >> 24) & 0xFF))
    
    // Offset (4 bytes, little endian)
    let off = UInt32(offset)
    icoData.append(UInt8(off & 0xFF))
    icoData.append(UInt8((off >> 8) & 0xFF))
    icoData.append(UInt8((off >> 16) & 0xFF))
    icoData.append(UInt8((off >> 24) & 0xFF))
    
    offset += data.count
}

// Append all image data
for (_, data) in imageDataList {
    icoData.append(data)
}

let icoPath = URL(fileURLWithPath: outputDir).appendingPathComponent("icon.ico")
try! icoData.write(to: icoPath)
print("  ✓ icon.ico created")

print("")
print("Done! Icons generated in: \(outputDir)")
SWIFT_SCRIPT

# Verify output
echo ""
echo "Verification:"
echo "─────────────"
file "$SCRIPT_DIR/icon.png"
file "$SCRIPT_DIR/icon.ico"

echo ""
echo "Icon sizes:"
sips -g pixelWidth -g pixelHeight -g hasAlpha "$SCRIPT_DIR/icon.png" 2>/dev/null | grep -E "(pixelWidth|pixelHeight|hasAlpha)"

