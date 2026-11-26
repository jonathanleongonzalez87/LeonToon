# Pendientes (25-26 de noviembre de 2026)

Estado: versión **0.4.0** publicada, `main` al día. Reglas de trabajo en `CLAUDE.md`/memoria: cada cambio = Issue + PR (`Refs #N`), no cerrar Issues sin que la persona lo diga, respuestas en español.

## Siguiente
1. **VU de audio (#159)**: que el rojo exija varias muestras seguidas sobre 0 dBFS (o más de 0,5 dB de exceso) en vez de una sola; rango del deslizador de ganancia a −24…+12 dB.
2. **Compilación**: en `[profile.dev]` de `app/Cargo.toml` poner `debug = 0` e `incremental = false`, y mover `target` fuera de OneDrive con `build.target-dir` en `app/.cargo/config.toml` (el debug de 10 GB ya se borró; a C: le quedaban 3 GB libres).
3. **Video M8c–M8e**: importar video al montaje (ver `docs/decodificadores-video-audio.md`).
4. Limpieza opcional: `tools/prueba-webcam/target` (~225 MB), `app/shots`, `git gc`, carpeta vacía `app/src_backup_2026-11-19`.

## Datos útiles
- ffmpeg: `%LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg*\bin` (no está en el PATH de la sesión).
- Ganchos de captura (`--screenshot ruta`): `LEONTOON_MUESTRA=animatic`, `LEONTOON_SALA`, `LEONTOON_TAM=1500x900`, `LEONTOON_HUECO=animatic/props=audio_vu`, `LEONTOON_VU=izq,der`, `LEONTOON_EXPORT_ESTADO`.
- Atajos del montaje: Ctrl+Mayús+D corta en el cabezal, Ctrl+D duplica, botón ✂ Cuchilla.
- Si la cuota semanal está baja, abrir sesión nueva en vez de seguir en una conversación larga.