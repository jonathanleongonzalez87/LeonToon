# Pruebas aisladas de Stop Motion (Fase 0, Issue #58)

Programas pequeños, **fuera de LeonToon**, que sirvieron para el estudio técnico de [`docs/stop-motion-estudio.md`](../../docs/stop-motion-estudio.md).
Tienen su propio `Cargo.toml` y su propia carpeta `target`; no se compilan con el programa principal.

**Lo más fácil:** haz doble clic en **`EJECUTAR_PRUEBA.bat`** (en esta carpeta). Compila la primera vez, ejecuta la prueba con la cámara número 1 y abre la carpeta con la foto. Si tu cámara es otra, edita el número del final de la línea `prueba-webcam.exe 1 salida`.

O desde una terminal:

```powershell
cd tools/prueba-webcam
cargo build --release
.\target\release\prueba-webcam.exe 0 salida   # lista las cámaras, mide el live view y guarda una foto (0 = índice de la cámara)
.\target\release\bench_jpeg.exe               # mide cuánto cuesta leer fotos grandes (no necesita cámara)
```

`prueba-webcam` usa la biblioteca `nokhwa` (MediaFoundation en Windows). El primer parámetro es el índice de la cámara tal como lo
lista el propio programa. Guarda `foto_original_de_la_camara.jpg` (los bytes de la cámara tal cual), una versión recodificada y un PNG.
Las fotos que guarda son de tu cámara: no se suben al repositorio (`tools/*/salida/` está ignorado).