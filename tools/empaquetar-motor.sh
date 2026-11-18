#!/bin/sh
# Arma la versión de prueba para Windows SOLO del motor de vóxeles (sin el prototipo de Godot)
# en dist/city-sim-2027-vX.Y.Z-<commit>-windows.zip (versión de engine/Cargo.toml; ver
# docs/VERSIONES.md). Conviene commitear antes: si hay cambios sin commitear, el nombre lo dice.
# Requisitos: Rust con el destino x86_64-pc-windows-gnu y mingw-w64.
set -e
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$RAIZ/dist"
W="$DIST/motor-windows/city-sim-2027"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$RAIZ/engine/Cargo.toml" | head -1)"
COMMIT="$(git -C "$RAIZ" rev-parse --short=7 HEAD)"
[ -n "$(git -C "$RAIZ" status --porcelain --untracked-files=no)" ] && COMMIT="$COMMIT-cambios"
NOMBRE="city-sim-2027-v$VERSION-$COMMIT-windows"
rm -rf "$DIST/motor-windows"
mkdir -p "$W"
(cd "$RAIZ/engine" && cargo build --release --target x86_64-pc-windows-gnu)
cp "$RAIZ/engine/target/x86_64-pc-windows-gnu/release/motor.exe" "$W/motor.exe"
x86_64-w64-mingw32-strip "$W/motor.exe" 2>/dev/null || true
cp "$RAIZ/tools/paquete-motor/windows/"* "$W/"
{ echo "Versión v$VERSION ($COMMIT, $(date -u +%Y-%m-%d))"; echo; cat "$RAIZ/tools/paquete-motor/LEEME.txt"; } | sed 's/$/\r/' > "$W/LEEME.txt"
echo "v$VERSION ($COMMIT)" > "$W/VERSION.txt"
(cd "$DIST/motor-windows" && zip -qr "../$NOMBRE.zip" city-sim-2027)
ls -la "$DIST/$NOMBRE.zip"
