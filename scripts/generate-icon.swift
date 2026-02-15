#!/usr/bin/env swift

/// Generates the Clean Up app icon as a 1024x1024 PNG.
/// Uses CoreGraphics — available on every macOS, no dependencies.
///
/// Usage: swift generate-icon.swift <output-path>

import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

let size = 1024
let outputPath: String

if CommandLine.arguments.count > 1 {
    outputPath = CommandLine.arguments[1]
} else {
    outputPath = "icon_1024.png"
}

// Create bitmap context
let colorSpace = CGColorSpaceCreateDeviceRGB()
guard let ctx = CGContext(
    data: nil,
    width: size,
    height: size,
    bitsPerComponent: 8,
    bytesPerRow: 0,
    space: colorSpace,
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
) else {
    fputs("Error: Could not create graphics context\n", stderr)
    exit(1)
}

let w = CGFloat(size)
let h = CGFloat(size)

// --- Background: macOS squircle with green-to-teal gradient ---

// Superellipse (squircle) path — approximates macOS icon shape
let rect = CGRect(x: 0, y: 0, width: w, height: h)
let inset: CGFloat = w * 0.02
let iconRect = rect.insetBy(dx: inset, dy: inset)
let cornerRadius = iconRect.width * 0.225  // macOS-style radius

let squirclePath = CGPath(roundedRect: iconRect, cornerWidth: cornerRadius, cornerHeight: cornerRadius, transform: nil)

ctx.addPath(squirclePath)
ctx.clip()

// Gradient: #34D399 (green) → #06B6D4 (teal), top-left to bottom-right
let gradientColors = [
    CGColor(red: 0x34/255.0, green: 0xD3/255.0, blue: 0x99/255.0, alpha: 1.0),
    CGColor(red: 0x06/255.0, green: 0xB6/255.0, blue: 0xD4/255.0, alpha: 1.0)
] as CFArray

let locations: [CGFloat] = [0.0, 1.0]
guard let gradient = CGGradient(colorsSpace: colorSpace, colors: gradientColors, locations: locations) else {
    fputs("Error: Could not create gradient\n", stderr)
    exit(1)
}

ctx.drawLinearGradient(
    gradient,
    start: CGPoint(x: 0, y: h),
    end: CGPoint(x: w, y: 0),
    options: [.drawsBeforeStartLocation, .drawsAfterEndLocation]
)

// --- White sparkle (four-pointed star) ---

ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.95))

let cx = w * 0.5
let cy = h * 0.5
let outerR = w * 0.30   // tip distance from center
let innerR = w * 0.06   // waist width

// Four-pointed star: alternate between outer (tip) and inner (waist) points
let starPath = CGMutablePath()
for i in 0..<8 {
    let angle = CGFloat(i) * .pi / 4.0 - .pi / 2.0  // start from top
    let r = (i % 2 == 0) ? outerR : innerR
    let px = cx + r * cos(angle)
    let py = cy + r * sin(angle)
    if i == 0 {
        starPath.move(to: CGPoint(x: px, y: py))
    } else {
        starPath.addLine(to: CGPoint(x: px, y: py))
    }
}
starPath.closeSubpath()

ctx.addPath(starPath)
ctx.fillPath()

// Small accent sparkles
let accentPositions: [(x: CGFloat, y: CGFloat, scale: CGFloat)] = [
    (0.72, 0.28, 0.08),
    (0.28, 0.72, 0.06),
    (0.75, 0.70, 0.05),
]

for accent in accentPositions {
    let ax = w * accent.x
    let ay = h * accent.y
    let ar = w * accent.scale
    let air = ar * 0.2

    let accentPath = CGMutablePath()
    for i in 0..<8 {
        let angle = CGFloat(i) * .pi / 4.0 - .pi / 2.0
        let r = (i % 2 == 0) ? ar : air
        let px = ax + r * cos(angle)
        let py = ay + r * sin(angle)
        if i == 0 {
            accentPath.move(to: CGPoint(x: px, y: py))
        } else {
            accentPath.addLine(to: CGPoint(x: px, y: py))
        }
    }
    accentPath.closeSubpath()

    ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.6))
    ctx.addPath(accentPath)
    ctx.fillPath()
}

// --- Write PNG ---

guard let image = ctx.makeImage() else {
    fputs("Error: Could not create image\n", stderr)
    exit(1)
}

let url = URL(fileURLWithPath: outputPath) as CFURL
guard let dest = CGImageDestinationCreateWithURL(url, "public.png" as CFString, 1, nil) else {
    fputs("Error: Could not create image destination at \(outputPath)\n", stderr)
    exit(1)
}

CGImageDestinationAddImage(dest, image, nil)

if !CGImageDestinationFinalize(dest) {
    fputs("Error: Could not write PNG\n", stderr)
    exit(1)
}

print("Generated icon: \(outputPath) (\(size)x\(size))")
