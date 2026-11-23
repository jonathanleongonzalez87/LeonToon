# Salas Storyboard y Animatic: diseño inspirado en Storyboard Pro

Documento de diseño. Nace de comparar la sala Storyboard de LeonToon con Storyboard Pro y de la petición de resolver los Issues **en orden de experiencia de usuario**. Aún no cambia el programa.

## 1. Punto de partida

| En Storyboard Pro | En LeonToon hoy |
|---|---|
| Cada plano tiene **capas** (personaje, atrezo, fondo…) con transformaciones | `Panel.drawing`: un dibujo plano, sin capas |
| Se dibuja con todas las herramientas del programa | Un editor propio con pincel y borrador básicos (`sb_detail_view`), aparte del Visor, los pinceles vectoriales y las paletas |
| **Línea de tiempo** con tira de planos, pista de cámara, pistas de capa y audio | Sin línea de tiempo: cuadrícula, y una tira de miniaturas solo en el detalle |
| Cámara con claves por plano | No existe (Issue #11) |
| Datos del plano (escena, plano, acción, diálogo, notas) | Cinco campos de texto, sin numeración por escena y plano |
| Exportar a PDF | Botón deshabilitado (#14) |
| Animatic: reproducir con sonido y transiciones | «Animatic > línea de tiempo» crea una composición con una capa-clip por plano y fundidos (M7); sin sonido |

## 2. Idea central: el plano es una composición

Cada plano pasa a ser una **composición** (el mismo modelo de las precomposiciones: capas, pistas de claves, duración). Consecuencias:

- El plano se edita con los editores que ya existen: **Visor**, **Capas**, **Propiedades de la herramienta**, **Paletas** y la **línea de tiempo de montaje** (para animar capas y cámara dentro del plano). No hay que mantener un segundo editor de dibujo.
- Los pinceles vectoriales, la Pluma, los pinceles de píxeles, la mesa de luz y los papeles funcionan en el storyboard sin trabajo extra.
- El **animatic** es una composición que coloca los planos uno tras otro (cada uno una capa de precomposición con su barra). `animatic.rs` ya hace algo muy parecido con dibujos planos.
- La **cámara** de un plano es una capa de cámara de esa composición, con claves de posición y escala (conecta con el Issue #11).

### Cambio de formato

`Panel` gana una referencia a su composición (`comp`) y conserva sus campos de texto. Al abrir un archivo antiguo, el `drawing` plano de cada plano se convierte en una composición con una capa «Dibujo». Se mantiene la lectura de archivos anteriores, como en M1 y M6, y hay pruebas de apertura de versiones viejas.

## 3. Qué se reutiliza, qué se crea, qué se termina

| | Piezas |
|---|---|
| **Reutilizar** | Visor y sus herramientas, Capas, Propiedades de la herramienta, Paletas, línea de tiempo de montaje, modelo `Composition`, `animatic.rs`, cuadrícula de planos (`sb_grid`), `paint_thumb` |
| **Crear** | Migración de `Panel.drawing` a composición · editor **«Línea de tiempo del storyboard»** (tira de planos con miniaturas y duración arrastrable, pista de cámara, pista de audio) · editor **«Datos del plano»** con numeración escena/plano · operaciones de plano (dividir, copiar y pegar, desplazar los siguientes, ir al plano anterior y siguiente) · caché de miniaturas |
| **Terminar** | Exportar a PDF (#14) · transiciones entre planos (#17) · cámara y punto de anclaje (#11) · sonido (#15 y M8a, ver `decodificadores-video-audio.md`) |

## 4. Las dos salas

Ambas comparten el Visor, las Capas y la línea de tiempo del storyboard. Cambia el énfasis y el modo del editor de la tira.

| | **Storyboard** | **Animatic** |
|---|---|---|
| Objetivo | Dibujar y anotar | Dar tiempo y sonido |
| Hueco principal | Visor del plano (o cuadrícula, a elegir) | Visor con la reproducción |
| Hueco de tiempo | Tira de planos con duraciones y pista de cámara | Tira de planos con **bordes arrastrables**, transiciones, pista de audio y cabeza de reproducción |
| Panel derecho | Datos del plano, herramientas, paleta | Duración y transición del plano, audio |
| Exportar | **PDF** (cuadrícula con textos) | **MP4 con sonido** |

Salas del conjunto de archivo «Storyboard»: Storyboard y Exportar. Del conjunto «Animatic»: Storyboard, Animatic y Exportar (igual que hoy).

## 5. Fases, cada una con su Issue

El orden sigue la **experiencia de usuario**: primero lo que más se nota al dibujar, después el tiempo, después el sonido.

| Fase | Qué se hace | Depende de |
|---|---|---|
| **S1. Plano como composición** (#132) | Modelo y migración del formato; el plano se abre en el Visor con las herramientas, las capas y las paletas reales; la cuadrícula se mantiene y entrar en un plano es un doble clic | Composiciones (M6, hecha) |
| **S2. Línea de tiempo del storyboard** (#133) | Editor nuevo con tira de planos (miniaturas y duración arrastrable) y pista de cámara con claves; operaciones de plano; **editor «Miniaturas» en vertical**, como el de Storyboard Pro (columna de planos junto al visor, con el mismo dibujo sobre miniaturas), para armar una disposición Visor grande + miniaturas verticales + línea de tiempo | S1 |
| **S3. Datos del plano y PDF** (#134) | Numeración escena/plano editable, panel «Datos del plano», exportar el storyboard a PDF | S1 (S2 no hace falta) |
| **A1. Sala Animatic** (#135) | Reproducción de los planos con sus duraciones, edición de bordes en la tira, transiciones elegibles (corte, fundido, otros) | S2 |
| **A2. Audio del animatic** (#136) | Pista de audio, onda y reproducción sincronizada; exportar el MP4 con sonido | A1 y M8a (Issue de estudio #100) |

### Riesgos

- **Formato**: el cambio de `Panel` toca el guardado; hay que migrar y probar con archivos antiguos.
- **Deshacer**: las composiciones de los planos deben compartirse (`Arc`) para que deshacer no copie todo el proyecto, igual que en M6.
- **Rendimiento**: muchas miniaturas de composiciones obligan a una caché que se invalida solo al cambiar el plano.
- **Dos líneas de tiempo**: la tira del storyboard y la línea de tiempo de montaje conviven; hay que dejar claro cuál es cuál (la tira edita planos; la de montaje edita las capas del plano elegido).

## 6. Decisiones

Tomadas en la conversación:

1. **El plano es una composición**: sí.
2. **Una línea de tiempo del storyboard con dos modos** (Storyboard y Animatic): sí.

Valores por defecto asumidos, a corregir si no convienen:

3. **Numeración**: como en Storyboard Pro, escena y plano (por ejemplo «Escena 2 · Plano 30»), editable; el número simple (1, 2, 3) sigue visible en la cuadrícula.
4. **Audio**: el audio del animatic (A2) sigue a M8a y no se adelanta; el resto de fases no depende del sonido.

## 7. Estado de las fases

| Fase | Estado |
|---|---|
| S1. Plano como composición | Hecha en el Issue #132: Panel.comp y Composition.panel; doble clic abre el plano en el visor con sus capas, las herramientas y las paletas; «< Cuadrícula», plano anterior y siguiente; la miniatura y el animatic se rehacen al salir; duplicar y borrar planos llevan su composición; los archivos antiguos conservan sus dibujos (capa «Dibujo»). La miniatura solo muestra capas vectoriales |
| S2. Línea de tiempo del storyboard | Hecha en el Issue #133: la sala Storyboard tiene hueco de tiempo con el editor **«Línea de tiempo del storyboard»** (regla en segundos con cabeza de reproducción; tira de planos con miniatura, número, título y duración; borde derecho arrastrable para cambiar la duración, ajustada a cuadros; arrastrar el cuerpo para reordenar; doble clic abre el plano) y la **pista de cámara** (doble clic en la barra de un plano añade o quita un movimiento de cámara con claves al principio y al final: zoom y desplazamiento, con arranque suave opcional, que se edita en el panel «Plano» y se aplica en el animatic y al exportarlo). Operaciones: + Plano, Duplicar, **Dividir** por la cabeza de reproducción, Eliminar y Mover. Editor nuevo **«Miniaturas»** en columna vertical (se dibuja sobre ellas como en la cuadrícula), elegible desde el selector de cualquier hueco. La cámara no se ve todavía en el visor del plano, solo en el animatic |
| S3, A1, A2 | Pendientes |

**Ajuste tras probar S1 (Issue #139)**: la cuadrícula es ahora un lienzo para hacer *thumbnails*. Con el Lápiz, el Pincel, las formas, el Borrador o el Bote se dibuja directamente sobre cada miniatura; el número del plano es su asa (clic abre el plano, arrastrar reordena); el tamaño se ajusta a la medida con el deslizador, Ctrl+rueda o «Ajustar» (que haga caber todos los planos). Mientras dura el trazo el plano se abre por dentro y se cierra al soltar; Deshacer vuelve a la escena sin entrar en el plano.
