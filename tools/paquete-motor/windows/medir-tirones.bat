@echo off
cd /d "%~dp0"
echo Juga un rato (mover, hacer zoom). Al cerrar el juego, enviar el archivo tirones.txt
set CITYSIM_PERFIL=1
motor.exe %* > tirones.txt 2>&1
