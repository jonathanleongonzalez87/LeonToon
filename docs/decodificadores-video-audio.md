# Estudio: cómo decodificar vídeo y audio para el montaje (M8)

Estudio del Issue #100. Es **solo estudio y propuesta**: no cambia el programa. Sirve para decidir cómo importar clips de vídeo y audio en la línea de tiempo de montaje (fase M8 de `editor-de-tiempo-montaje.md`) y completa el Issue #15 (sonido).

## 1. Qué hay que resolver

| Necesidad | Detalle |
|---|---|
| **Importar** | Leer clips de vídeo (MP4, MOV, WebM…) y audio (WAV, MP3, FLAC, OGG, AAC) y ponerlos como capas. |
| **Ver** | Mostrar el cuadro del clip que toca en el visor, al arrastrar la cabeza de reproducción y al reproducir. |
| **Oír** | Reproducir el audio sincronizado con la imagen y mostrar su onda en la línea de tiempo. |
| **Exportar** | Que el MP4 salga con la imagen del montaje y con el audio mezclado. |
| **Restricciones** | Windows con la toolchain **GNU** (no hay MSVC en el equipo de desarrollo, ver el flujo de compilación), un `.exe` suelto que se descarga de Releases, sin instaladores. |

Lo que ya existe y se puede reutilizar:

- **ffmpeg como programa externo** para exportar MP4 (`render::find_ffmpeg`, `render::export_video`): hoy el usuario lo instala aparte y la app le pide la ruta si no lo encuentra.
- **Stop Motion**: una capa que apunta a una carpeta de fotos (referencia, no incrustada), con miniaturas y cuadros en caché en segundo plano (`stopmotion/playback.rs`). Es el modelo natural para un clip de vídeo: **un clip es una secuencia de cuadros que no se guarda dentro del `.ltoon`**.
- **Barra de capa** (entrada, salida, desplazamiento), etiqueta de color y precomposición: un clip es una capa con su barra.
- **Media Foundation** ya se usa indirectamente (la webcam, a través de `nokhwa`), pero solo para capturar, no para abrir archivos.

## 2. Opciones

### A. ffmpeg como programa externo

La app lanza `ffmpeg`/`ffprobe` como procesos: `ffprobe` para duración, tamaño y fps; `ffmpeg` para extraer cuadros reducidos (JPEG) a una carpeta de caché y el audio a un WAV.

- **Ventajas**: abre prácticamente todo; no añade dependencias de compilación; no cambia el tamaño del `.exe`; ya se usa para exportar MP4, así que **no pide nada nuevo** a quien ya exporta; funciona con la toolchain GNU.
- **Riesgos**: hay que tener ffmpeg instalado (o empaquetarlo, y entonces el `.zip` crece decenas de MB y hay que cumplir su licencia: las compilaciones «GPL» obligan más que las «LGPL»); saltar a un cuadro cualquiera es lento si no hay caché, por eso se extraen cuadros por adelantado; procesos hijos que fallan o se cuelgan.
- **Coste de integración**: bajo-medio. Es el mismo patrón que Stop Motion (caché en disco + carga en segundo plano).

### B. Enlazar FFmpeg dentro del programa (por ejemplo `ffmpeg-next`)

- **Ventajas**: acceso directo a cualquier cuadro, saltos precisos, sin procesos hijos.
- **Riesgos**: necesita las bibliotecas de desarrollo de FFmpeg y `libclang` para generar los enlaces (bindgen); en Windows el camino habitual es vcpkg y el enlazado estático suele fallar al final por símbolos que faltan; con la toolchain GNU **no está probado** y es lo más probable que dé problemas; se reparten DLL junto al `.exe` o se compila FFmpeg a mano; la licencia afecta al programa entero.
- **Coste de integración**: alto e incierto. No lo recomiendo salvo que A y C fallen.

### C. Media Foundation de Windows (`windows` crate, `IMFSourceReader`)

- **Ventajas**: viene con Windows, no añade binarios; H.264 (Baseline, Main y High hasta el nivel 5.1) y AAC/MP3 se decodifican de serie; acelerada por hardware.
- **Riesgos**: **solo Windows**; los códecs más nuevos (HEVC, VP9, AV1) dependen de extensiones o de la tarjeta gráfica y no se puede prometer que abran en todos los equipos; API COM larga y delicada (inicialización, hilos, formatos de salida); el contenedor WebM/MKV no está cubierto de serie.
- **Coste de integración**: medio-alto, con comportamiento distinto entre equipos.

### D. Decodificadores en Rust puro

- **Audio**: [symphonia](https://github.com/pdeljanov/Symphonia) (licencia MPL-2.0). Lee WAV, FLAC, OGG/Vorbis, MP3 y MP4/M4A; **AAC-LC** está detrás de una opción de compilación y no se activa por defecto; no pude confirmar soporte de Opus. La reproducción sale por `cpal` (WASAPI en Windows).
- **Vídeo**: no hay un decodificador de H.264 en Rust puro maduro; las opciones (por ejemplo `openh264`) compilan código C/C++ y arrastran la cuestión de las patentes de H.264.
- **Ventajas (audio)**: sin instalar nada, un solo `.exe`, compila con GNU, control total del reloj de audio.
- **Riesgos**: formatos limitados frente a ffmpeg; MPL-2.0 pide publicar cambios a los archivos de la propia biblioteca (no afecta al resto del programa).
- **Coste de integración**: bajo para audio; **no sirve para vídeo**.

## 3. Comparación

| | A. ffmpeg externo | B. FFmpeg enlazado | C. Media Foundation | D. Rust puro |
|---|---|---|---|---|
| Audio | Sí (a WAV) | Sí | Sí (MP3, AAC) | **Sí (symphonia)** |
| Vídeo | **Sí, casi todo** | Sí | Sí (H.264; el resto según equipo) | No |
| Funciona con GNU | **Sí** | Dudoso | Sí | **Sí** |
| Qué hay que instalar | ffmpeg (ya lo pide el MP4) | DLL o compilación | Nada | **Nada** |
| Tamaño del `.exe` | Igual | Mucho mayor | Algo mayor | Algo mayor (audio) |
| Saltos precisos | Con caché | Directos | Directos | Directos (audio) |
| Multiplataforma | Sí | Sí | **No** | Sí |
| Riesgo de licencia | Según compilación | Alto | Ninguno | Bajo |
| Coste | Bajo-medio | Alto | Medio-alto | Bajo |

## 4. Recomendación

**Combinar D para el audio y A para el vídeo.**

1. **Audio con symphonia + cpal.** Es lo que da mejor experiencia sin pedir nada: se importa, se calcula la onda y se oye sincronizado desde el primer día, en un solo `.exe`. El reloj de audio manda: la imagen se ajusta al sonido, no al revés.
2. **Vídeo con ffmpeg externo y caché de cuadros.** Al importar un clip se extraen cuadros reducidos (por ejemplo 540 px de alto, JPEG) a la carpeta de datos, en segundo plano y con barra de progreso, y la capa de clip los usa **igual que una capa Stop Motion** (referencia a la carpeta, miniaturas, caché de carga). Así se reutiliza código ya probado y el `.ltoon` sigue siendo pequeño. Para la exportación se usa el cuadro original solo si hace falta calidad final (se puede decodificar de nuevo con ffmpeg).
3. **El audio de un clip de vídeo** se extrae con ffmpeg a un WAV de caché y se trata como cualquier pista de audio (así el audio de vídeo y el de un archivo suelto pasan por el mismo camino).
4. **Exportar**: la mezcla de audio se escribe a un WAV temporal y ffmpeg la une al vídeo al crear el MP4 (el `export_video` actual pasa a recibir una segunda entrada).
5. **Media Foundation (C)** queda como alternativa futura **si más adelante se quiere abrir vídeo sin ffmpeg** en Windows; no se descarta, pero no es la primera.

### Por qué no las otras

- **B** es lo que más puede romper la compilación en este equipo y obliga a repartir DLL.
- **C solo** dejaría fuera formatos según el equipo y ata el programa a Windows.
- **D** no tiene decodificador de vídeo.

## 5. Plan por fases

| Fase | Qué se hace | Dependencias |
|---|---|---|
| **M8a. Audio** | Capa de audio (formato nuevo en `.ltoon`, referencia al archivo); importar WAV, MP3, FLAC y OGG con symphonia; onda (picos guardados en caché) en la barra de la capa; volumen y silencio; reproducción sincronizada (el audio es el reloj); entrada, salida y desplazamiento de la barra | Ninguna nueva salvo `symphonia` y `cpal` |
| **M8b. Mezcla y exportación** | Mezcla de las pistas a un WAV temporal y unión con el MP4 vía ffmpeg | M8a; ffmpeg |
| **M8c. Vídeo** | Capa de clip: `ffprobe`, extracción de cuadros a caché en segundo plano, ver el cuadro en el visor, barra de la capa con entrada y salida | M8a opcional; ffmpeg |
| **M8d. Clip con sonido** | Extraer el audio del clip y enlazarlo a su capa de vídeo (se mueven juntos); velocidad y bucle | M8a, M8c |
| **M8e. (opcional)** | Empaquetar ffmpeg en el `.zip` o abrir H.264 con Media Foundation sin ffmpeg | Decisión de licencias |

El orden propuesto es **audio primero** (menor riesgo, aporta de inmediato al animatic y al stop motion con voz) y vídeo después.

## 6. Cuestiones técnicas por resolver

- **Sincronía**: el reloj de reproducción sale del dispositivo de audio (número de muestras reproducidas); el cuadro mostrado es `muestras / frecuencia × fps`. Sin audio, el reloj actual del programa.
- **Caché de cuadros**: una carpeta por clip en el directorio de datos (como las secuencias de Stop Motion), con una huella del archivo original para saber si hay que rehacerla; límite de espacio y opción para limpiarla.
- **Rendimiento**: la extracción corre en un hilo aparte con cancelación; el visor muestra el cuadro más cercano ya disponible mientras tanto.
- **Rutas**: los clips y los audios se guardan por referencia relativa o absoluta, como las secuencias, con el aviso de «archivo no encontrado».
- **Formato de archivo**: capa de audio y capa de clip son tipos nuevos (`LayerKind`); los archivos antiguos se abren igual.
- **Pruebas**: archivos de audio y de vídeo pequeños dentro del repositorio (segundos), y pruebas que se omiten si no hay ffmpeg, como la de MP4 actual.

## 7. Decisiones que debe tomar la persona

1. **¿Audio primero o vídeo primero?** Recomendado: audio primero (M8a).
2. **¿Se acepta que el vídeo necesite ffmpeg?** Hoy ya lo pide el MP4. Alternativa: empaquetarlo en el `.zip` (más peso y licencia) o esperar a Media Foundation.
3. **Formatos mínimos de audio:** WAV, MP3, FLAC y OGG (de serie). ¿Hace falta AAC/M4A desde el principio? (sería activar una opción de symphonia).
4. **Los medios, ¿por referencia o dentro del `.ltoon`?** Recomendado: por referencia, como Stop Motion.
5. **Calidad de la caché de vídeo:** 540 px de alto por defecto, ¿o más?

## 8. Fuentes consultadas

- [Symphonia](https://github.com/pdeljanov/Symphonia): formatos y códecs, licencia MPL-2.0, AAC detrás de opción.
- [Media Foundation: códecs y decodificador H.264](https://learn.microsoft.com/en-ie/windows/win32/medfound/codecobjects): H.264 de serie; HEVC con requisitos propios.
- Foros y documentación de `ffmpeg-next` / `ffmpeg-sys-next`: necesitan libclang y bibliotecas de FFmpeg; en Windows se usa vcpkg y el enlazado estático suele dar problemas.
