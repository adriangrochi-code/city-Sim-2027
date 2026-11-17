@echo off
cd /d "%~dp0"
echo Midiendo el rendimiento del motor (tarda unos minutos)...
motor.exe --bench salida
echo.
echo Listo: enviar el archivo salida\bench.md
pause
