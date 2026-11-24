# LeonToon

Estudio de animación 2D de escritorio, hecho en Rust con egui. Mezcla ideas de los programas de animación tradicional,
de motion graphics y de stop motion (salas de trabajo, mesa de luz, papel cebolla, línea de tiempo de montaje, hoja de
exposición, Live View) con un motor propio de dibujo **vectorial** y de **píxeles**. Está en desarrollo activo; la versión
actual es la **0.4.0**.

![LeonToon: salas, editores y herramientas](docs/img/00-portada-v2.png)

Todas las imágenes salen del **proyecto de muestra** que trae el programa: una pelota que rebota (animación tradicional, a dos cuadros), un sol que va y viene (interpolación) y un paisaje con paralaje en bucle de cuatro segundos, con cielo, nubes, montañas, arbustos y carretera. Se abre desde la pantalla de inicio.`n`n**Contenido:** [Dibujo](#dibujo) · [Interfaz](#interfaz-por-editores) · [Montaje y motion graphics](#montaje-y-motion-graphics) ·
[Stop Motion](#stop-motion) · [Storyboard y animatic](#storyboard-y-animatic) · [Exportar](#exportar) ·
[Descargar](#descargar) · [Compilar](#compilar) · [Estado y límites](#estado-y-límites)

## Dibujo

- **Lápiz vectorial** (L) con grosor por presión, perfiles de grosor (afilado o dibujado con puntos), extremos y uniones, y una biblioteca de 22 pinceles con vista previa, favoritos, buscador y paquetes `.ltvb` para importar y exportar.
- **Pluma Bézier** (P) con nodos y tiradores; los trazos libres se pueden convertir en curvas.
- **Edición no destructiva**: se eligen uno o varios trazos para cambiar grosor, color, opacidad, perfil o pincel sin volver a dibujarlos, y se puede restaurar el aspecto original.
- **Capas de mapa de bits** con un motor de pinceles de píxeles (dureza, espaciado, dinámica de forma y de transferencia, dispersión, textura, doble pincel, bordes húmedos, puntas de imagen) e importación y exportación de pinceles `.abr` de Photoshop.
- **Mesa de luz** con luz, temperatura, papel cebolla y **papeles de estudio** (Bond, Genga, TMK y otros) con grano orgánico cuya intensidad sigue la luz.

| | |
|---|---|
| ![Lápiz vectorial con su biblioteca de pinceles](docs/img/01-vectorial-biblioteca.png) | ![Edición de trazos](docs/img/03-edicion-de-trazos.png) |
| **Biblioteca de pinceles**: vista previa, favoritos y paquetes `.ltvb` | **Edición de trazos**: un solo panel para el Lápiz y para lo elegido |
| ![Pluma y nodos](docs/img/02-pluma-nodos.png) | ![Perfil de grosor](docs/img/07-perfil-de-grosor.png) |
| **Pluma y nodos**: curvas Bézier editables | **Perfil de grosor** dibujado con puntos |
| ![Pinceles de píxeles](docs/img/04-pinceles-de-pixeles.png) | ![Papeles de estudio](docs/img/05-papeles-de-estudio.png) |
| **Pinceles de píxeles** con presets y dinámicas | **Papeles de estudio** con grano que responde a la luz |

## Interfaz por editores

La pantalla se divide en **huecos** y cada hueco muestra el **editor** que elijas (visor, capas, propiedades, línea de tiempo,
X-Sheet, storyboard…), como en Blender. Cada sala trae su propia disposición, pero se puede cambiar. La barra de herramientas
tiene iconos con color y los ajustes de la herramienta elegida están en la barra lateral **N**, pestaña *Herramienta*.

![Barra de herramientas con iconos de color y panel de la herramienta](docs/img/08-barra-de-herramientas-v2.png)

Las líneas de tiempo (hoja de exposición, montaje y storyboard) tienen **zoom horizontal y vertical**, con un deslizador para cada uno en su cabecera y con la rueda: **Ctrl + rueda** acerca o aleja el tiempo, **Alt + rueda** cambia el alto de las filas, la rueda sola desplaza y **Mayús + rueda** desplaza de lado.

Al abrir el programa, la pantalla de inicio ofrece un archivo nuevo según el tipo de animación o fase del proceso de animación (storyboard, animatic,
animación tradicional, cutout, motion graphics o stop motion), cada uno con su conjunto de salas.

![Pantalla de inicio y lista de tipos de animación](docs/img/09-pantalla-de-inicio.png)

El selector de cada hueco agrupa los editores por familia, con su icono:

![Selector de editores](docs/img/10-selector-de-editores-v2.png)

## Montaje y motion graphics

La **línea de tiempo de montaje** es un editor propio para motion graphics: una fila por capa con barra de duración,
interruptores (visible, solo, bloqueo, tímida), etiqueta de color y, al desplegar la capa, una pista por propiedad
(punto de anclaje, posición, escala, rotación, opacidad y profundidad) con sus propias claves.

![Línea de tiempo de montaje con las pistas de transformación](docs/img/11-linea-de-tiempo-de-montaje-v2.png)

**Fusión y vínculo.** Cada capa tiene modo de fusión (Multiplicar, Diferencia y otros ocho modos), mate y un *primario*:
una capa puede heredar la transformación de otra. En la imagen, las Montañas y las Nubes están en Multiplicar y los Arbustos tienen a las Montañas como primario.

![Modo de fusión y vínculo entre capas](docs/img/12-fusion-y-vinculo-v2.png)

**Colecciones.** Las capas se agrupan en colecciones anidables sin límite, arrastrando capas dentro de ellas, y la vista 3D
dibuja cada colección como un cubo de líneas punteadas.

![Colecciones anidadas y cubo 3D](docs/img/13-colecciones-y-cubo-3d-v2.png)

**Propiedades de capa y espacio 2D/3D.** La selección múltiple (Ctrl y Mayús) permite agrupar varias capas. Cada capa puede
ser 2D o 3D, con profundidad animable, y se coloca en fondo, medio o primer plano.

![Panel de propiedades: claves, capa, espacio 2D/3D, etiqueta y barra](docs/img/14-propiedades-y-espacio-2d-3d-v2.png)

**Precomposición.** Un grupo de capas se convierte en su propia composición; se entra en ella para editarla y se sale con
*Salir*, y la ruta (Escena › Precomp 1) queda a la vista.

![Entrar y salir de una precomposición](docs/img/15-precomposicion-v2.png)

## Stop Motion

Desde la pantalla de inicio se abre **«Ejemplo: Stop Motion con plastilina»**, una secuencia real de 91 fotos que viene dentro del programa (se copia a la carpeta de datos la primera vez). La sala de Stop Motion se parece a un programa de captura: el escenario con su *Live View* arriba, controles de reproducción,
y alrededor las hojas de exposición y la tira de fotos.

![Sala de Stop Motion](docs/img/17-sala-stop-motion-v2.png)

- **Cámara y captura**: elegir cámara, brillo, *animación directa*, foto anterior encima con opacidad, y herramientas del Live View (ajustar, 2×, 4×, cuadrícula de tercios, cruz de centro, enfoque asistido, ver foto anterior con L).
- **Capturar y avanzar** con Intro o F5, o capturar y revisar.

![Live View, controles y panel de cámara y captura](docs/img/18-live-view-y-camara-v2.png)

**X-Sheet**: la hoja de exposición muestra posición, cuadros, foto y exposición (cuántos cuadros dura cada foto).

![X-Sheet de Stop Motion](docs/img/19-x-sheet-de-stop-motion-v2.png)

**Tira de fotos** con secuencia y *Frame Browser*; duplicar, eliminar, vaciar, congelar, ocultar y poses o marcadores.

![Tira de fotos](docs/img/20-tira-de-fotos-v2.png)

## Storyboard y animatic

El **storyboard** muestra los planos como miniaturas con nombre y duración; se reordenan arrastrando el número, y se puede dibujar directamente sobre ellas.
La línea de tiempo del storyboard da la duración de cada plano y su cámara, y *Crear animatic* arma con ellos la composición del animatic.
Cada plano lleva su escena y su número (editables, con «Renumerar de 10 en 10» para poder intercalar planos), y el storyboard se exporta a **PDF**: hojas A4 con la miniatura de cada plano y sus textos.
El **audio** (WAV, MP3, FLAC, OGG y AAC/M4A) se importa como una capa de audio («+ Audio» en la línea de tiempo del storyboard, «Importar audio…» en el panel del animatic o Mayús+A): su barra muestra la onda y se mueve y recorta como cualquier capa, suena al reproducir con la imagen siguiendo al sonido, y se mezcla en el MP4 al exportar (hace falta ffmpeg). Una capa de audio tiene **sus propias propiedades**, no las de imagen: **Ganancia** en dB (la onda de la barra la refleja), **Paneo**, **ecualizador** de tres bandas (agudos, medios, graves), **Mute (M)** y **Solo (S)**, y **FX** de reverberación y eco. Todos los valores se pueden animar con claves (cronómetro y ◆, también en las filas de propiedades de la línea de tiempo). El ejemplo del animatic trae una mezcla breve con sonidos reales: música ambiental, ambiente de escena y los «boings» de A y B recortados al cuadro exacto del bote, con la música bajando mientras hablan.

Desde la pantalla de inicio se abre **«Ejemplo: Animatic con bocetos (A y B)»**: la historia de la pelota en seis planos dibujados como bocetos en mapa de bits (lápiz y marcador), con el Personaje A (roja) y el Personaje B (azul), sus textos, fundidos, cámara y el animatic ya creado.

![Storyboard](docs/img/21-storyboard-v2.png)

El **animatic** es una composición con un clip vivo por plano, en el orden y con la duración de la línea de tiempo del storyboard,
con sus transiciones (corte, fundido, fundido a negro, deslizar y empujar) y su cámara. La sala Animatic trae el visor estándar y la línea de tiempo de montaje; la línea de tiempo del storyboard (transporte, cabeza de reproducción en la regla, bordes de cada plano arrastrables y selector de transición) se puede elegir en su selector de editor; el animatic se rehace solo al cambiar duraciones, transiciones, cámara u orden, y si cambias un plano, el animatic lo refleja.

![El animatic como composición de planos](docs/img/16-animatic-como-composicion-v2.png)

## Exportar

PNG, GIF y MP4 (para MP4 hace falta [ffmpeg](https://ffmpeg.org/) aparte). Los proyectos se guardan como `.ltoon`.

## Descargar

Las versiones para Windows están en [Releases](https://github.com/jonathanleongonzalez87/LeonToon/releases): descarga el `.zip`,
descomprímelo y abre `LeonToon.exe`. Como el programa aún no está firmado, Windows puede mostrar un aviso de SmartScreen:
pulsa «Más información» y luego «Ejecutar de todos modos».

Al abrirlo, **F1** muestra la guía rápida. Atajos principales: **V** Selección, **L** Lápiz vectorial, **P** Pluma,
**B** Pincel, **T** barra de herramientas, **N** barra lateral, **Mayús+A** agregar capa.

## Compilar

Hace falta [Rust](https://www.rust-lang.org/tools/install) estable.

```powershell
cargo run --release --manifest-path app/Cargo.toml
```

Para las pruebas: `cargo test --manifest-path app/Cargo.toml`. El icono del `.exe` lo incrusta `app/build.rs` con
`windres`; si no está disponible, el programa se compila igual, sin icono de archivo.

## Estado y límites

LeonToon está en construcción. Entre lo que todavía falta o no se ha probado a fondo:

- La presión de **tableta gráfica** no se ha probado con hardware real, y la inclinación del lápiz no está disponible.
- Los `.abr` exportados no se han abierto todavía en Photoshop.
- La captura con **cámara real** en Stop Motion depende del dispositivo; las imágenes de este README usan una secuencia de fotos de referencia (`referencia/Sec_Ref_Stop_Motion`) en lugar de la cámara.
- Pendientes: importar **video y audio** al montaje, herramienta Texto, rigging, luz y profundidad de campo en 3D, exportar con la cámara 3D.

## Documentación y desarrollo

Notas de diseño en [`docs/`](docs): [arquitectura de editores](docs/arquitectura-editores.md),
[editor de tiempo de montaje](docs/editor-de-tiempo-montaje.md) y [Stop Motion de estudio](docs/stop-motion-estudio.md).

El trabajo se organiza con [Issues](https://github.com/jonathanleongonzalez87/LeonToon/issues) (con etiquetas de área, tipo y
prioridad, y hitos por versión) y se entrega con Pull Requests. Para publicar una versión, ver [RELEASING.md](RELEASING.md).
Las plantillas de Issue y de PR están en `.github/`.
