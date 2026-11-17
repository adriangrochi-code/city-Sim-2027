#!/bin/sh
cd "$(dirname "$0")"
echo "Juga un rato (mover, hacer zoom). Al cerrar el juego, enviar tirones.txt"
CITYSIM_PERFIL=1 ./motor "$@" > tirones.txt 2>&1
