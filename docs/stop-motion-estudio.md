# Stop Motion — Estudio técnico (Fase 0)

Issue [#58](https://github.com/jonathanleongonzalez87/LeonToon/issues/58), parte del épico [#57](https://github.com/jonathanleongonzalez87/LeonToon/issues/57).
Fecha del estudio: 22 de noviembre de 2026. Equipo de pruebas: Windows 11, Intel i5-11400H (12 hilos), Rust 1.100 con la
cadena **GNU** (sin MSVC), dos cámaras: la HP Wide Vision HD de la portátil y la **Trust Full HD Webcam** de la persona.

Este documento dice **qué se probó de verdad**, **qué se leyó** y **qué no se pudo verificar**. Cada decisión indica su base.

## 1. Resumen de decisiones

| Tema | Decisión | Base |
|---|---|---|
| Webcams UVC en Windows | Usar **`nokhwa` 0.10** (MediaFoundation) detrás de una interfaz propia (`CaptureProvider`), para poder cambiarla | Probada con dos cámaras reales (sección 2) |
| Fotos de webcam | Guardar **los bytes JPEG tal como los entrega la cámara** (MJPEG) como original, sin recomprimir | Probado: 1080p, 0,27 MB por foto |
| Cámaras réflex y sin espejo | Tres vías, de menos a más trabajo: **carpeta vigilada** (cualquier marca, sin SDK), **tarjeta de captura HDMI** como UVC (live view de cualquier cámara), y **SDK de Canon** más adelante | Secciones 3 y 4 |
| Canon | El SDK **exige solicitar acceso** y sus condiciones de redistribución varían por región: no se puede incluir en el repositorio ni en las versiones sin un acuerdo | Fuentes de Canon (sección 3) |
| Nikon | **No se pudo confirmar un SDK público** para cámaras actuales; solo software de tethering para el usuario final | Búsqueda (sección 4) |
| Fotos grandes | Nunca mantener fotos descodificadas a resolución completa: **miniatura 320 px**, **proxy 1280 px**, original en disco y carga bajo demanda con caché | Medido (sección 5) |
| Formatos | JPEG y PNG con la biblioteca `image` (ya en el proyecto); TIFF y RAW, después | Sección 5 |
| Almacenamiento | Carpeta elegida por la persona con los originales y un archivo `sequence.json` dentro; el `.ltoon` guarda solo la referencia y puede **relocalizarla** | Diseño (sección 6) |

## 2. Webcams UVC en Windows

### Opciones comparadas

| Opción | Estado | Veredicto |
|---|---|---|
| **[nokhwa](https://docs.rs/crate/nokhwa) 0.10.11** con [nokhwa-bindings-windows](https://lib.rs/crates/nokhwa-bindings-windows) (MediaFoundation) | Versión actual 0.10.11; el crate de bindings tuvo versiones hasta noviembre de 2025 según lib.rs; su documentación pide usar el crate principal con la característica `input-native`. No pude comprobar la actividad del repositorio original | **Elegida.** Compila y funciona aquí |
| [escapi](https://docs.rs/crate/escapi/4.0.0/source/README.md) | Última versión de 2019 (Rust 2015); necesita el SDK de Windows y cabeceras de MediaFoundation | Descartada: abandonada |
| [ccap-rs](https://docs.rs/crate/ccap-rs/1.6.0) | Usa DirectShow por defecto en Windows (mejor con cámaras virtuales, como OBS) | **No probada**; queda como alternativa si `nokhwa` falla con alguna cámara |
| MediaFoundation directo con el crate `windows` | Control total | Más trabajo; solo si `nokhwa` se queda corta |
| OpenCV | Funciona, pero es una dependencia de C++ enorme y con MinGW no trae MSMF según un foro de OpenCV | Descartada |

### Prueba real (`tools/prueba-webcam`)

Programa aislado, fuera de LeonToon: lista las cámaras, muestra sus formatos, mide el live view y guarda una foto.
Se compila con `cargo build --release` dentro de esa carpeta y se ejecuta con `prueba-webcam.exe <índice de cámara> <carpeta>`.

**Resultados medidos** (el 22-nov-2026, con las cámaras de la persona):

- **Compila y funciona con la cadena GNU** (`stable-x86_64-pc-windows-gnu`): `nokhwa` 0.10.11 y sus bindings de Windows se compilaron sin MSVC en 1 minuto 7 segundos. Con MSVC no se probó.
- **Detecta las dos cámaras** (`HP Wide Vision HD Camera` y `Trust Full HD Webcam`) con el identificador único de cada una. **El índice cambia** si se enchufa o quita una cámara, así que hay que guardar la ruta del dispositivo (no el número) para recordar la elegida.
- **Trust Full HD**: hasta **1920×1080 a 30 fps** en NV12 y MJPEG. En la prueba entregó **26,9 cuadros/s reales** con decodificación incluida (probablemente limitada por la luz de la habitación).
- **HP Wide Vision**: hasta 1280×720 a 30 fps, unos 28 cuadros/s.
- **Decodificación**: 1080p MJPEG en **8 ms** por cuadro y NV12 en 6 ms; a 1280×800, 4 y 2 ms. Sobra para un live view fluido.
- **Foto**: un cuadro 1080p MJPEG llegó en 31 ms y pesa **0,27 MB tal como lo entrega la cámara**; recodificada a JPEG quedó en 0,16 MB (pierde calidad) y a PNG en 1,76 MB. La foto salió con brillo y detalle normales (no en negro).
- **La lista de formatos trae muchos duplicados** (330 entradas para pocas combinaciones distintas): hay que quitar repetidos antes de mostrarla.
- **Controles que declara la Trust** (la interfaz solo debe mostrar los que existan): brillo, contraste, matiz, saturación, nitidez, gamma, balance de blancos (2800–6500 K), compensación de contraluz, desplazamiento y inclinación, zoom (0–3), **exposición** y **enfoque**. Se leen con `camera_controls()`; cambiarlos **no se probó**.

**Importante para el diseño**
1. **Una webcam no tiene «foto» distinta del live view**: la foto es un cuadro del mismo flujo. Una réflex sí (el live view es de baja resolución y la foto, completa). La interfaz de proveedores debe separar `live_frame()` de `capture_still()`; en la webcam, el segundo devuelve un cuadro del flujo a la mejor resolución.
2. **Los primeros cuadros salen oscuros** mientras la cámara ajusta la exposición: la captura debe descartar unos 15–20 cuadros al abrir (la prueba lo hace).
3. **MJPEG se decodifica con la biblioteca `image`** (que ya usa LeonToon); no hace falta activar la decodificación de `nokhwa`, que arrastra una dependencia de C.
4. **Permisos de Windows**: no apareció ningún aviso en esta prueba, pero Windows tiene un ajuste de privacidad de la cámara para aplicaciones de escritorio; hay que mostrar un error claro si está bloqueada. **No se probó el caso bloqueado.**

### Riesgos
- Dependencia de una biblioteca de terceros cuyo mantenimiento no pude confirmar: por eso la interfaz `CaptureProvider` (se puede cambiar por `ccap-rs` o por MediaFoundation directo sin tocar el resto).
- El tiempo de cambio de resolución o de apertura de cámaras con muchos formatos no se midió.

## 3. Canon (EDSDK)

Lo que dicen las fuentes de Canon (no pude solicitar el SDK, así que **no se probó**):

- El acceso al SDK se aprueba mediante una **solicitud** en las páginas de desarrolladores de Canon de cada región ([Canon Europa](https://canon-europe.com/business/imaging-solutions/sdk), [formulario de Canon Asia](https://asia.canon/en/campaign/developerresources/sdk/form?sdk=291)).
- La licencia que describen los [términos de Canon Asia](https://asia.canon/en/campaign/developerresources/terms-conditions-for-digital-camera-software-development-kit-sdk) es **no exclusiva, intransferible y revocable**, limitada al país de la solicitud y con **vigencia de un año con renovación automática**; permite distribuir el SDK **solo dentro del software propio** y prohíbe crear herramientas de desarrollo con él.
- Su [FAQ](https://asia.canon/en/consumer/faq/23) indica que se pueden distribuir las DLL y cabeceras del EDSDK con la aplicación, pero no el paquete del SDK. Otros SDK de Canon tienen condiciones distintas (por ejemplo, entregar una copia del programa a Canon), así que **las condiciones dependen de la región y del acuerdo**.
- Soporta cámaras EOS (más de 60 modelos según Canon Asia) con live view por USB en Windows, macOS y Linux, y añade modelos en cada versión ([notas de versión](https://asia.canon/en/campaign/developerresources/camera/cap/edsdk-eos-digital-camera-sdk-release-note)). **No encontré una tabla de qué modelos tienen live view**: hay que consultarla en Canon al solicitar el acceso.

**Consecuencias para LeonToon**
- Las DLL de Canon **no pueden ir en el repositorio público ni en las versiones** sin una solicitud aprobada y sin revisar las condiciones de redistribución para el país de la persona.
- El proveedor de Canon debe ser **opcional**: si las DLL no están, la opción no aparece y el resto del programa funciona igual.
- Requiere **una cámara Canon** para probarlo (F6). Hoy no hay.

## 4. Nikon y otras marcas

- **No encontré un SDK público de Nikon** para cámaras actuales. Las búsquedas solo devolvieron software para el usuario final: [NX Tether](https://www.nikon.com/company/news/2024/0207_tether_02/) 2.0 (control completo en Z 9, Z 8, Z 7II, Z 6II, Z 7 y Z 6 y funciones básicas en Z f, Z 5, Z fc, Z 50 y Z 30) y [Camera Control Pro 2](https://downloadcenter.nikonimglib.com/en/download/sw/152.html), que Nikon ya no desarrolla. Hay un SDK antiguo de 2015 para Nikon 1 V3 ([nikonrumors](https://nikonrumors.com/2015/06/16/sdk-for-the-nikon-1-v3-camera-released.aspx/)). **Sigue sin confirmar** cómo obtener un SDK vigente: hay que preguntar directamente a Nikon.
- No se estudiaron Sony, Panasonic ni Fujifilm.

### Vías que no dependen de un SDK (recomendadas)

1. **Carpeta vigilada.** El programa del fabricante (NX Tether, EOS Utility, Imaging Edge…) guarda cada foto en una carpeta; LeonToon la vigila e **importa cada foto nueva a la secuencia**. Funciona con cualquier marca y sin licencias. No da live view dentro de LeonToon. **No se probó** (no hay cámara réflex).
2. **Tarjeta de captura HDMI.** Windows la ve como una cámara UVC, así que el proveedor UVC ya da **live view de cualquier cámara con salida HDMI limpia**; las fotos se importan con la carpeta vigilada. **No se probó** (no hay tarjeta).

Con esto el «Capture Device Manager» queda con proveedores: **UVC** (webcams y tarjetas de captura), **carpeta vigilada** (cualquier cámara) y, más adelante, **Canon** (SDK, opcional).

## 5. Fotos grandes y formatos

La biblioteca `image` 0.25 (ya en LeonToon; hoy solo con WebP) decodifica JPEG con `zune-jpeg` (comprobado con `cargo tree` en la prueba). Las [mediciones del propio autor de zune-jpeg](https://docs.rs/crate/zune-jpeg/0.4.21/source/Benches.md) lo sitúan a la par de libjpeg-turbo; no pude contrastar una comparación independiente. Se midió aquí, con fotos **sintéticas** (degradados con ruido: los JPEG reales de una cámara pesan más y pueden tardar algo más):

| Foto | JPEG | Sin comprimir | Decodificar | Miniatura 320 px | Proxy 1280 px |
|---|---|---|---|---|---|
| 2 MP (1920×1080) | 0,2 MB | 6 MB | 9 ms | 7 ms | 76 ms |
| 12 MP (4000×3000) | 1,4 MB | 36 MB | 52 ms | 21 ms | 140 ms |
| 24 MP (6000×4000) | 2,8 MB | 72 MB | 131 ms | 39 ms | 229 ms |
| 45 MP (8192×5464) | 5,3 MB | 134 MB | 206 ms | 63 ms | 424 ms |

**Conclusiones**
- Una foto de 24 MP descodificada ocupa **72 MB**: con 3000 fotos serían 216 GB. Nunca se mantienen descodificadas a resolución completa. Se descodifica **bajo demanda** el original solo para ver al 100 % o exportar.
- La **miniatura de 320 px** (de 7 a 63 ms) y el **proxy de 1280 px** (de 76 a 424 ms) se generan **en hilos de segundo plano** y se guardan en disco; así la interfaz no se bloquea y la reproducción usa los proxies.
- Un proxy de 1280×720 descodificado ocupa unos 2,8 MB: una caché con límite de memoria (por ejemplo, 500 MB, unos 180 proxies) cubre una reproducción fluida.
- **JPEG y PNG**: soportados hoy. **TIFF**: la biblioteca lo soporta con una característica adicional, sin medir. **RAW** de cámara: fuera de alcance al principio.

## 6. Diseño de la carpeta de la secuencia

Decisión de la persona (#57): **el usuario elige la carpeta de cada secuencia** y la capa recuerda la referencia; al abrir el proyecto, si la carpeta no está, se puede **volver a localizar**.

```text
MiSecuencia/                       ← carpeta elegida por la persona
├── sequence.json                  ← datos de la secuencia (autocontenidos)
├── Sources/                       ← originales: nunca se modifican ni se renombran
│   ├── frame_000001.jpg
│   └── frame_000002.jpg
├── Thumbnails/                    ← 320 px, regenerables
├── Proxies/                       ← 1280 px, regenerables
└── Journal/                       ← registro de operaciones y recuperación
```

**`sequence.json` (esquema inicial, versión 1)**

```json
{
  "format": "leontoon-stopmotion", "version": 1,
  "id": "9f1c…", "name": "Escena 1", "fps": 12,
  "sources": [
    { "id": "s000001", "file": "Sources/frame_000001.jpg", "width": 1920, "height": 1080,
      "format": "jpeg", "bytes": 268694, "hash": "blake3:…", "captured_at": "2026-11-22T20:14:03Z",
      "device": { "name": "Trust Full HD Webcam", "provider": "uvc" }, "exif": null }
  ],
  "items": [
    { "id": "i000001", "source": "s000001", "exposure": 2, "visible": true, "transform": null, "note": null }
  ],
  "markers": [ { "at": 0, "kind": "pose", "label": "Contacto" } ]
}
```

- **Posición**: es el orden de `items` más las exposiciones acumuladas; no se guarda aparte (no puede quedar incoherente). Una misma foto puede aparecer en varios `items` sin duplicar el archivo.
- **Nombres de archivo**: numeración creciente que **nunca se reutiliza**, aunque se borre una foto, para que las referencias sean estables.
- **Escritura segura de una foto**: se escribe a un archivo temporal, se comprueba (se vuelve a leer, tamaño y hash), se renombra y **solo entonces** se añade a `sequence.json`. Un fallo no deja ninguna referencia a un archivo incompleto.
- **Escritura de `sequence.json`**: archivo temporal y renombrado atómico, igual que hace `write_project` hoy, con copia `.bak`.
- **Recuperación tras un cierre inesperado**: al abrir, las fotos de `Sources/` que no aparezcan en `sequence.json` se ofrecen para **reincorporar al final** (el diario dice cuáles fueron las últimas capturas).
- **Integridad**: el hash se calcula al capturar o importar y se verifica bajo demanda y en una revisión de fondo; un archivo faltante o con otro hash se marca, sin romper la secuencia.
- **Hash**: **SHA-256** (decidido en F1 con la biblioteca sha2, escrita solo en Rust: no añade ninguna dependencia de C, que era el riesgo con la cadena GNU). BLAKE3 queda como mejora posible si algún día el hash pesa; en las pruebas de F1 verificar 5000 fotos registradas no es el cuello de botella.
- **Referencia desde el `.ltoon`**: id de la secuencia, **ruta relativa** a la carpeta del proyecto si es posible y ruta absoluta si no, y un resumen (nombre, número de fotos, fotograma de inicio). **Relocalización**: si la ruta no existe, se pide la carpeta; se acepta si su `sequence.json` tiene el **mismo id**.
- **Dos capas con la misma carpeta**: se avisa (comparten originales; cada capa guardaría su propio `sequence.json` y se pisarían).
- **Importar fotos**: por defecto **copiar** a `Sources/` (la secuencia queda autocontenida y movible); referenciar el original donde está queda como opción avanzada. Pendiente de confirmar con la persona (pregunta abierta de F1).
- **Discos externos**: antes de capturar se comprueba que la unidad existe y que hay espacio libre para varias fotos más.
- **Compatibilidad**: la versión del archivo permite migrar; un `sequence.json` de una versión más nueva se rechaza con un mensaje claro y no se modifica.

## 7. Lo que no se verificó

- **Cámara Canon, Nikon o cualquier réflex**: no hay equipo. Canon y Nikon, solo leídos.
- **SDK de Canon y de Nikon**: no se solicitó acceso ni se descargó nada.
- **Tarjeta de captura HDMI** y **carpeta vigilada**: no se probaron.
- **Cambiar controles de la cámara** (exposición, enfoque, balance de blancos): solo se leyó qué declara; no se escribieron valores.
- **MSVC**: solo se probó con la cadena GNU.
- **Cámara bloqueada por privacidad de Windows** y **desconexión durante la captura**: no se probaron.
- **Fotos reales grandes**: las medidas son con imágenes sintéticas.
- **Mantenimiento de `nokhwa`**: no pude revisar su repositorio.

## 8. Propuesta para F1 (#59) y F4 (#62)

1. **F1** puede empezar ya: el diseño de la sección 6 no depende de ninguna cámara. Añade BLAKE3 (o SHA-256) y las características JPEG y PNG de `image` a `app/Cargo.toml`.
2. **F4** usa `nokhwa` con la interfaz `CaptureProvider` (`list`, `open`, `live_frame`, `capture_still`, `controls`, `close`), quita los formatos repetidos, descarta los primeros cuadros, guarda la ruta del dispositivo y guarda el JPEG original de la cámara.
3. Añadir un Issue nuevo para el **proveedor de carpeta vigilada**, que da soporte inmediato a cualquier cámara réflex sin SDK.
4. Preguntas para la persona: ¿qué cámara réflex o sin espejo (marca y modelo) existe o se prevé usar?, ¿hay tarjeta de captura HDMI?, ¿importar copia o referencia en F1?

## Fuentes

- nokhwa: [docs.rs](https://docs.rs/crate/nokhwa), [nokhwa-bindings-windows en lib.rs](https://lib.rs/crates/nokhwa-bindings-windows)
- escapi: [docs.rs](https://docs.rs/crate/escapi/4.0.0/source/README.md); ccap-rs: [docs.rs](https://docs.rs/crate/ccap-rs/1.6.0)
- Canon: [términos del SDK](https://asia.canon/en/campaign/developerresources/terms-conditions-for-digital-camera-software-development-kit-sdk), [FAQ](https://asia.canon/en/consumer/faq/23), [formulario de solicitud](https://asia.canon/en/campaign/developerresources/sdk/form?sdk=291), [notas de versión del EDSDK](https://asia.canon/en/campaign/developerresources/camera/cap/edsdk-eos-digital-camera-sdk-release-note), [Canon Europa](https://canon-europe.com/business/imaging-solutions/sdk)
- Nikon: [NX Tether 2.0](https://www.nikon.com/company/news/2024/0207_tether_02/), [Camera Control Pro 2](https://downloadcenter.nikonimglib.com/en/download/sw/152.html), [SDK de Nikon 1 V3 (2015)](https://nikonrumors.com/2015/06/16/sdk-for-the-nikon-1-v3-camera-released.aspx/)
- JPEG en Rust: [zune-jpeg](https://docs.rs/crate/zune-jpeg/0.4.21/source/Benches.md)
