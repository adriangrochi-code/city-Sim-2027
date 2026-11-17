#!/bin/sh
# Arma la versión de prueba para Windows SOLO del motor de vóxeles (sin el prototipo de Godot)
# en dist/city-sim-2027-motor-windows-x86_64.zip.
# Requisitos: Rust con el destino x86_64-pc-windows-gnu y mingw-w64.
set -e
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$RAIZ/dist"
W="$DIST/motor-windows/city-sim-2027"
rm -rf "$DIST/motor-windows" "$DIST/city-sim-2027-motor-windows-x86_64.zip"
mkdir -p "$W"
(cd "$RAIZ/engine" && cargo build --release --target x86_64-pc-windows-gnu)
cp "$RAIZ/engine/target/x86_64-pc-windows-gnu/release/motor.exe" "$W/motor.exe"
x86_64-w64-mingw32-strip "$W/motor.exe" 2>/dev/null || true
cp "$RAIZ/tools/paquete-motor/windows/"* "$W/"
sed 's/$/\r/' "$RAIZ/tools/paquete-motor/LEEME.txt" > "$W/LEEME.txt"
(cd "$DIST/motor-windows" && zip -qr "../city-sim-2027-motor-windows-x86_64.zip" city-sim-2027)
ls -la "$DIST/city-sim-2027-motor-windows-x86_64.zip"
