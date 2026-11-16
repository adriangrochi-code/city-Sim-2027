#!/bin/sh
# Prototipo jugable (época colonial). Opciones útiles:
#   ./jugar.sh -- --demo --months=60     ciudad ya armada con 5 años simulados
#   ./jugar.sh --rendering-method gl_compatibility   si tu equipo no tiene Vulkan
cd "$(dirname "$0")" && exec ./ciudad.x86_64 "$@"
