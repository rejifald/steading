#!/usr/bin/env bash
# Writes the mark's SVGs with scripts/generate-mark.mjs, then renders the app and
# tray icons from app/icons/src/*.svg with tools macOS ships (AppKit through
# ../../scripts/render-svg.swift, and iconutil), so no SVG renderer needs installing.
#
#   scripts/render-icons.sh
#
# Writes app/icons/{32x32,128x128,128x128@2x,icon}.png, app/icons/icon.icns and
# app/icons/tray/*.png (18pt at 2x), and the SVGs in docs/brand/ and app/icons/src/.
# Commit the generator, the SVGs and the PNGs together.
set -euo pipefail

cd "$(dirname "$0")/.."
src=app/icons/src
out=app/icons
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Every size is drawn from the vector at its own pixel size, never scaled from
# another PNG. The tray SVGs are drawn at 36px, their own size, so the cube's edge
# lands on whole pixels.
tray_px=36

node scripts/generate-mark.mjs
swiftc -O -o "$tmp/render-svg" ../../scripts/render-svg.swift
render() { # <svg> <px> <png>
  "$tmp/render-svg" "$1" "$2" "$3"
}

mkdir -p "$out/tray"
for state in tidy pending needs-you; do
  render "$src/tray-$state.svg" "$tray_px" "$out/tray/$state.png"
done

app="$src/app-icon.svg"
render "$app" 32 "$out/32x32.png"
render "$app" 128 "$out/128x128.png"
render "$app" 256 "$out/128x128@2x.png"
render "$app" 512 "$out/icon.png"

iconset="$tmp/icon.iconset"
mkdir "$iconset"
for px in 16 32 128 256 512; do
  render "$app" "$px" "$iconset/icon_${px}x${px}.png"
  render "$app" $((px * 2)) "$iconset/icon_${px}x${px}@2x.png"
done
iconutil -c icns "$iconset" -o "$out/icon.icns"
