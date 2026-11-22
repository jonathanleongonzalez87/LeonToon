@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo Prueba de webcam de LeonToon (Stop Motion, Fase 0)
echo.
if not exist target\release\prueba-webcam.exe (
  echo Compilando por primera vez, puede tardar unos minutos...
  cargo build --release
)
target\release\prueba-webcam.exe 1 salida
echo.
echo La foto esta en la carpeta "salida" de esta misma carpeta.
start "" explorer "%~dp0salida"
pause
