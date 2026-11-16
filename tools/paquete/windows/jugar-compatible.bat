@echo off
cd /d "%~dp0"
start "" ciudad.exe --rendering-method gl_compatibility %*
