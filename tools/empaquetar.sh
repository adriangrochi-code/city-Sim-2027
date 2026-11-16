#!/bin/sh
# Arma las versiones de prueba para Linux y Windows en dist/.
#
# Requisitos (en Linux):
#   - Godot 4.3: binarios de Linux y de Windows (variables GODOT_LINUX y GODOT_WIN)
#   - Rust con el destino x86_64-pc-windows-gnu y mingw-w64 (para motor.exe)
#
# Uso:  GODOT_LINUX=/ruta/Godot_v4.3-stable_linux.x86_64 \
#       GODOT_WIN=/ruta/Godot_v4.3-stable_win64.exe  tools/empaquetar.sh
set -e
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$RAIZ/dist"
: "${GODOT_LINUX:?Falta GODOT_LINUX (binario de Godot 4.3 para Linux)}"
: "${GODOT_WIN:?Falta GODOT_WIN (binario de Godot 4.3 para Windows)}"
rm -rf "$DIST"
mkdir -p "$DIST/linux/city-sim-2027" "$DIST/windows/city-sim-2027"

echo "== Juego (Godot): paquete de datos"
"$GODOT_LINUX" --headless --path "$RAIZ" --export-pack "Linux" "$DIST/ciudad.pck" >/dev/null 2>&1
test -s "$DIST/ciudad.pck"

echo "== Motor de vóxeles (Rust)"
(cd "$RAIZ/engine" && cargo build --release && cargo build --release --target x86_64-pc-windows-gnu)

echo "== Linux"
L="$DIST/linux/city-sim-2027"
cp "$GODOT_LINUX" "$L/ciudad.x86_64"
cp "$DIST/ciudad.pck" "$L/ciudad.pck"
cp "$RAIZ/engine/target/release/motor" "$L/motor"
strip "$L/motor" 2>/dev/null || true
cp "$RAIZ/tools/paquete/linux/"* "$L/"
cp "$RAIZ/tools/paquete/LEEME.txt" "$L/"
chmod +x "$L/ciudad.x86_64" "$L/motor" "$L/"*.sh
(cd "$DIST/linux" && tar -cJf "../city-sim-2027-linux-x86_64.tar.xz" city-sim-2027)

echo "== Windows"
W="$DIST/windows/city-sim-2027"
cp "$GODOT_WIN" "$W/ciudad.exe"
cp "$DIST/ciudad.pck" "$W/ciudad.pck"
cp "$RAIZ/engine/target/x86_64-pc-windows-gnu/release/motor.exe" "$W/motor.exe"
x86_64-w64-mingw32-strip "$W/motor.exe" 2>/dev/null || true
cp "$RAIZ/tools/paquete/windows/"* "$W/"
sed 's/$/\r/' "$RAIZ/tools/paquete/LEEME.txt" > "$W/LEEME.txt"
(cd "$DIST/windows" && zip -qr "../city-sim-2027-windows-x86_64.zip" city-sim-2027)

ls -la "$DIST"/*.tar.xz "$DIST"/*.zip
