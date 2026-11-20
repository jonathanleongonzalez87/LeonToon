# LeonToon — hoja de valores y reglas

Fuentes:
- **Original (Deep mode)**: `HarmonyPremium.exe`, función `0x1404e1200` (lee las preferencias del papel cebolla y las limita con `minsd` a [0, 1.0] — constante f64 en `0x140b3a550` = 1.0), y sus valores por defecto en `resources/prefs.xml` del programa instalado.
- **Versión web de la persona** (`leontoon/` y `leontoon.html`, su propio proyecto): interfaz, colores, salas, atajos y reglas de la barra de tiempo / Xsheet / StoryBoard. Se reescribió todo en Rust; no se copió código.

## Papel cebolla
| Valor | Dato | Fuente |
|---|---|---|
| Opacidad del dibujo más cercano | 0.8 | `DRAWING_ONIONSKIN_MAX_OPACITY` en prefs.xml, leído en `0x1404e1200` |
| Opacidad del dibujo más lejano | 0.2 | `DRAWING_ONIONSKIN_MIN_OPACITY` en prefs.xml, leído en `0x1404e1200` |
| Límite de opacidad | 0 … 1 | `0x1404e1200` (`minsd` con 1.0 en `0x140b3a550`) |
| Modo | por fotogramas | `DRAWING_ONIONSKIN_DRAWING_MODE = byFrames` (prefs.xml) |
| Peso por distancia (por defecto) | `w(d) = 1 − (d−1)/10` → 1.0, 0.9, 0.8 … 0.1 | `0x1404dbbc0`: rellena 10 pesos empezando en 1.0 (`0x140b3a550`) y restando 1.0/10 en cada uno |
| Opacidad del dibujo a distancia d | `0.2 + (0.8 − 0.2)·w(d)` → 0.80, 0.74, 0.68 … 0.26 | combinación de `0x1404dbbc0` (pesos) y `0x1404e1200` (mín./máx.); la mezcla exacta en el render no se vio: **derivado** |
| Máximo de dibujos por lado | 10 | `0x1404dbbc0` (10 pesos por lado, máscara `0x3ff` = 10 bits) |
| Dibujos por defecto | 1 antes, 1 después | `0x1404dbbc0` pone 1 en los campos +0x2c/+0x30; PREV/NEXT_DRAWINGS no están en prefs.xml (código: 0) — **derivado** |
| Degradado exponencial (opción) | `0.8 · 0.55^(d−1)` | versión web (`onionWeight`, 'exp') con el máximo del original |
| Colores antes / después | rojo (255,0,0) / verde (55,120,55) | `DRAWING_ONION_SKIN_COLOR_BEFORE/AFTER` en prefs.xml || Intensidad (multiplicador) | 100 % | decidido aquí (estimado) |

## Claves de animación (sala Animación)
- Cada clave guarda x, y, rotación (°), escala, opacidad e interpolación hacia la siguiente: Lineal, Suave o Escalón (versión web, `engine/anim.js`).
- Suave = cúbica `t<0.5 ? 4t³ : 1 − (−2t+2)³/2` (versión web).
- Giro y escala alrededor del centro de la cámara (versión web, `localToScene`).
- Herramienta Animar: arrastrar = mover; Mayús + arrastrar = girar; Ctrl + arrastrar = escalar (estimado: la web solo movía).

## Escena y reproducción
| Valor | Dato | Fuente |
|---|---|---|
| Bucle al reproducir | desactivado por defecto | PLAY_ENABLE_LOOP = false (prefs.xml); 0x1404180b0 guarda el estado en esa preferencia |
| Al llegar al final con bucle | vuelve al inicio sin pausa | 0x14041a1f0: espera PLAY_DELAY_END_OF_LOOP ms (por defecto 0, código) y salta al primer fotograma |
| Al llegar al final sin bucle | se queda en el último fotograma y para | 0x14041a1f0 |
| FPS por defecto | 24 | capturas del original (reference/picture-1.png: «FPS 24») y versión web |
| Tamaño por defecto | 1280 × 720 | versión web (`newProjectDialog`) |
| Bucle | activado | versión web (`loop: true`) |
| Rango de reproducción | doble clic abajo en la barra de tiempo = 1 segundo (fps fotogramas) | versión web (`onDbl`) |

## Barra de tiempo (versión web, `ui/timebar.js`)
Alto 58 px; regla 16 px; banda de cebolla en y=20 (18 px); claves en y=42. Ancho por fotograma = ancho / (n+1), entre 3 y 48 px (zoom 2–80). Paso de la regla: el primero de 1,2,5,10,20,25,50,100… que deje ≥ 38 px.

## Xsheet (versión web, `ui/xsheet.js`)
Filas (por defecto) o columnas. Celda: número del dibujo donde empieza, raya «│» mientras se mantiene, borde dorado en claves de animación, línea más clara cada segundo. Operaciones: ×2, Rev (invertir), Swing (ida y vuelta), Vaciar.

## Interfaz (versión web, `styles/main.css`)
Fondo #1d1d1d / #252525 / #303030 / #3a3a3a, texto #d6d6d6, apagado #8c8c8c, acento #EA7221 (#ff8c42), visor #161616. Barra superior 34 px, herramientas 38–40 px, panel lateral 250 px, Xsheet 230 px.

## Salas (versión web, `rooms/*.js`)
StoryBoard · Basics · Limpieza · Dibujo · Animación · Timing · Xsheet · Paleta · Esquema · Explorador · Render, cada una con su visor/herramientas/barra de tiempo/Xsheet/paneles laterales.

## Pendiente (no hecho)
StoryBoard PDF, Texto, Pluma vectorial y nodos, Cortador, Esqueleto/huesos, Rotar vista, limpieza automática, sonido, presión de tableta.

## Ronda 2 (selección, paletas, sustitución, vídeo)
- Paletas de escena «Boceto», «Limpieza» y «Color» como en reference/picture-1.png; rojo #b33939, verde #3f833f, azul #3f3f95 medidos en esa captura. «Cambiar en la escena» recolorea todos los trazos que usaban ese color (como las paletas del original).
- Sustitución de dibujo (reference/picture-2.png): número y deslizador eligen qué dibujo de la capa se ve en la celda actual; los fotogramas que muestran el mismo dibujo comparten número en el Xsheet.
- Selección (V): clic o recuadro; arrastrar mueve, esquinas escalan (Mayús = libre), asa superior gira; voltear, girar 90°, aplicar color, borrar, copiar/pegar. Tamaño de asas 9 px (estimado).
- Vídeo MP4 con ffmpeg (H.264, yuv420p; estimado); ffmpeg se busca en el PATH, junto al programa o en Descargas.

## Zoom (ronda 3, leído del original)
| Valor | Dato | Fuente |
|---|---|---|
| Niveles de zoom del visor | 1, 1.25, 1.6, 2, 2.5, 3.3, 4, 5, 6.3, 8, 10, 12.5, 16, 20, 25, 32, 40, 50, 63, 80, 100, 125, 160, 200, 250, 320, 400, 500, 630, 800, 1000 … 80000 % | tabla de textos en `0x140c1b840`–`0x140c1b9a0` (selector de zoom). Acercar/alejar, la rueda y la herramienta Zoom saltan de un nivel al siguiente; «Ajustar» y el arrastre siguen libres. |
| Rueda del ratón | un nivel por muesca, alrededor del cursor | `0x14047b850`: umbral = 2 × `WHEEL_ZOOM_THRESHOLD` (por defecto 50 en el código) = 100 unidades; una muesca de Qt = 120 → 1 paso, mínimo ±1 |
| Atajos de zoom (+ / −) | centrados en la vista | `SHORTCUT_ZOOM_ON_MOUSE = false` (prefs.xml) |
| Xsheet: alto de fila | 17 px a zoom 10 | `0x140106d90`: `UT_DPI::scale(17)`, zoom inicial 10.0 (`0x4024000000000000`) |
| Xsheet: zoom mínimo / máximo | 5 / 16 | `XSHEET_MIN_ZOOM` / `XSHEET_MAX_ZOOM` en prefs.xml, leídos en `0x140106d90` (código: 1 / 20) |
| Xsheet: tamaño de letra | `17·zoom/10 − 5` px (12 px a zoom 10) | `0x140106d90` (multiplica por zoom, divide por 10.0 en `0x140b52248`, resta `UT_DPI::scale(5)`) |

## Atajos de herramientas
El original usa Alt + tecla (`DRAWING_*_TOOL_KEY` en `resources/shortcuts.xml`, leídos en `0x1403b45e0`; p. ej. Zoom = Alt+Z). LeonToon conserva las teclas sueltas de la versión web de la persona (B, P, E, V…), porque así lo pidió.

## Tipos de curva del original
`0x14088a790` registra los tipos de columna DRAWING, SOUND, TIMING, 3DPATH, BEZIER, EASE, EXPR, QUATERNIONPATH, ANNOTATION, VELOBASED. LeonToon ofrece Lineal / Suave (cúbica) / Escalón; las curvas Bézier editables siguen pendientes. La fórmula de EASE del original no se encontró en este ejecutable (estará en una biblioteca aparte): la curva Suave sigue siendo **estimada** (la de la versión web).

## Interfaz y arquitectura (prototipo v0.3 de la persona, «LeonToon — Prototipo de interfaz v0.3.html»)
- **Arranque**: bienvenida con la ilustración del puma, «El León Solitario», porcentaje y mensajes (Iniciando, Preparando la mesa de luz, Cargando pinceles, Afilando lápices, Ordenando la hoja de exposición, Listo). Dura 3,4 s con curva `1 − (1 − p)²` y 0,35 s de pausa; «Omitir», Esc o Intro la saltan. Después, la pantalla de inicio.
- **Pantalla de inicio**: «Nuevo archivo según el tipo de animación» (5 tipos), Recientes (los últimos 8 archivos abiertos o guardados; más el ejemplo de la pelota) y Ayuda.
- **Conjuntos de salas** (ANIMT): Animación tradicional = Layout, Dibujo, Limpieza, Animación, Timing, Color, Exportar · Cutout o rigging 2D = Layout, Rigging, Animación, Interpolación, Exportar · Motion graphics = Layout, Interpolación, Composición 3D, Exportar · Storyboard = Storyboard, Exportar · Animatic = Storyboard, Animatic, Exportar.
- **Tipos de sala** (ROOMT): Layout (línea de tiempo abajo, 210 px), Dibujo (170, Pincel), Limpieza (170, Lápiz vectorial), Animación y Timing (hoja de exposición al lado, 330 px), Interpolación (290), Rigging (220), Composición 3D (190), Color (160, Relleno), Storyboard (el centro es el storyboard), Animatic (230), Exportar (150). Cada una con su panel de propiedades: Paleta, Interpolación, Rigging, Composición, Paletas, Plano, Animatic, Exportar.
- **Pestañas**: se guardan en el archivo `.ltoon` (tipo y nombre propio). Clic elige, doble clic renombra, x o clic central cierra, arrastrar reordena; «+» añade una sala por grupos (Producción, Dibujo, Animación, Composición y salida) o reemplaza el conjunto.
- **Paneles**: barra de herramientas (44 px), cabecera de herramienta (34 px), mesa de luz con píldoras (zoom, 2D/3D, Mesa de luz, Guías, Papel cebolla), barra lateral N (236 px; Ítem, Herramienta, Vista), área de línea de tiempo con «Línea de tiempo / Hoja de exposición», columna derecha de 268 px con Capas y Propiedades. La cabecera de Propiedades cambia el editor: Propiedades de la sala, Biblioteca de paletas, Editor de curvas, Explorador, Esquema, Sustitución de dibujo, Vista previa.
- **Herramientas** por modo (Tab): objeto = Seleccionar V, Rotación W, Punto de anclaje Y*, Cámara C*, Mano H, Rotar vista R*, Zoom Z; edición = Seleccionar V, Pincel B, Lápiz vectorial L, Pluma P*, Texto Ctrl+T*, Formas Q, Borrador E, Relleno G, Cuentagotas I, Mano, Rotar vista*, Zoom. (* pendiente: está en la barra pero no hace nada todavía.) T oculta herramientas, N la barra lateral, O el papel cebolla.
- **Papel cebolla de la interfaz**: 2 atrás y 2 adelante, hasta 12, opacidad del más cercano 40 % (10–80 %), ámbar `#fd6724` antes y azul `#5f8fb4` después. La caída por distancia sigue al original (0x1404dbbc0 y 0x1404e1200): `opacidad(d) = O · (0,25 + 0,75 · (1 − (d − 1)/10))`.
- **Colores**: ui-0 `#141312`, ui-1 `#1a1918`, ui-2 `#211f1e`, ui-3 `#2a2826`, ui-4 `#34312e`, línea `#2e2b29`/`#3a3633`, tinta `#f2e8da`/`#b5a898`/`#857a6e`, acento `#fd6724`/`#ff8a52`.
- **Pendiente**: vista 3D, huesos y deformadores, pluma Bézier, texto, punto de anclaje, cámara, rotar vista, capas de mapa de bits, PDF del storyboard, transiciones del animatic, preferencias (idioma de términos, escala de interfaz).