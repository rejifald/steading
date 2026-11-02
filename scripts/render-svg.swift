// Renders an SVG to a square PNG with a transparent background, through AppKit's own
// SVG support, so no renderer needs installing.
//
//   swift scripts/render-svg.swift <in.svg> <px> <out.png>
import AppKit

let args = CommandLine.arguments
guard args.count == 4, let px = Int(args[2]) else {
    FileHandle.standardError.write("usage: render-svg.swift <in.svg> <px> <out.png>\n".data(using: .utf8)!)
    exit(2)
}
guard let image = NSImage(contentsOf: URL(fileURLWithPath: args[1])) else {
    FileHandle.standardError.write("render-svg: cannot read \(args[1]) as an image\n".data(using: .utf8)!)
    exit(1)
}
let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
    samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
    bytesPerRow: 0, bitsPerPixel: 0)!
bitmap.size = NSSize(width: px, height: px)
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
NSGraphicsContext.current?.imageInterpolation = .high
image.draw(in: NSRect(x: 0, y: 0, width: px, height: px))
NSGraphicsContext.restoreGraphicsState()
guard let png = bitmap.representation(using: .png, properties: [:]) else {
    FileHandle.standardError.write("render-svg: cannot encode \(args[3])\n".data(using: .utf8)!)
    exit(1)
}
try png.write(to: URL(fileURLWithPath: args[3]))
