#!/bin/sh
# Motor de vóxeles (sandbox). Opciones útiles:
#   ./motor-voxel.sh --bench salida/     banco de pruebas: tabla de FPS en salida/bench.md
#   ./motor-voxel.sh --msaa 1            sin suavizado de bordes (más rápido)
cd "$(dirname "$0")" && exec ./motor "$@"
