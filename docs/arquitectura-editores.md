# Arquitectura de editores, salas y contexto

Refs #78. **Estado: borrador para decidir.** Nada de esto está implementado. Es el resultado de estudiar cómo organiza Blender sus editores y de revisar cómo están hechos hoy los paneles de LeonToon. Al final hay una lista de decisiones que son tuyas.

## 1. Qué hace Blender (y por qué funciona)

Blender separa cinco ideas que en LeonToon hoy están mezcladas:

| Concepto de Blender | Qué es | Equivalente en LeonToon |
|---|---|---|
| **Workspace** (pestaña de arriba: Layout, Animation, Sculpting…) | Una disposición guardada de áreas, con un modo de trabajo por defecto | Sala (`RoomKind`) |
| **Área** | Un rectángulo de la pantalla; se puede dividir, unir y redimensionar | Hoy no existe: cada hueco está escrito a mano en `shell.rs` |
| **Tipo de editor** | Lo que muestra un área; se elige en el menú de su esquina, y cualquier área puede mostrar cualquiera | Solo la columna derecha (`Editor`, 7 tipos) |
| **Espacio** (*space*) | El estado propio de cada área con su editor (zoom, desplazamiento, filtros). Dos visores 3D tienen vistas distintas | Hoy es global: un solo `zoom`, `pan`, `onion` para todo el programa |
| **Contexto y modo** | Lo que se está editando (objeto activo, su tipo, su modo: Objeto / Edición / Pose…). Los editores se adaptan a eso, no a la sala | `obj_mode` y `cl` (capa actual), pero casi ningún editor los consulta |

Lo que hace que Blender sea flexible **sin ser caótico**:

1. **Los editores trabajan sobre el contexto, no sobre la sala.** El editor de Propiedades muestra pestañas distintas según el tipo de objeto activo. Si el contexto no encaja, el editor muestra un vacío explicativo, no falla.
2. **El modo pertenece al objeto.** Cada tipo de objeto tiene sus modos; el modo cambia las herramientas y los atajos, no la disposición.
3. **Un editor puede tener modos internos** en vez de multiplicar editores. El Dope Sheet tiene modos (Dope Sheet, Action Editor, Shape Key, Grease Pencil, Máscara, Caché); el Image Editor, otros. No hay un editor por cada variante.
4. **Cada área tiene regiones**: cabecera, contenido principal, barra de herramientas, barra lateral (N), pie. La barra de herramientas no es un editor aparte: es una región del visor.
5. **El menú de tipo de editor está agrupado.** Fuente: el [manual de Blender, sección Editores](https://docs.blender.org/manual/en/latest/editors/index.html) (en la 4.0 el manual lo lista como [General, Animación, Scripting y Datos](https://docs.blender.org/manual/en/4.0/editors/index.html); en la versión actual la animación se ha repartido, pero el menú sigue agrupando igual):
   - **General**: Visor 3D, Editor de imagen, UV, Compositor, nodos, Secuenciador de vídeo, Clip de película.
   - **Animación**: Dope Sheet, Línea de tiempo, Editor de curvas (Graph), Drivers, NLA.
   - **Scripting**: Editor de texto, Consola de Python, Info.
   - **Datos**: Outliner, Propiedades, Explorador de archivos, Explorador de recursos, Hoja de cálculo, Preferencias.

Lo que **no** conviene copiar: el scripting (de momento no está en el plan), la geometría y los shaders, y la libertad total de dividir áreas con el ratón (se puede dejar para una fase tardía).

## 2. Qué tenemos hoy en LeonToon

Superficies que el usuario ve, con el archivo donde viven:

| Hoy se llama | Dónde vive | Tipo real |
|---|---|---|
| Mesa de luz / Visor | `canvas.rs`, `view3d.rs` | Visor 2D y visor de composición (3D por planos) |
| Live View + barra de captura | `smcap.rs` | Visor de Stop Motion |
| Storyboard (centro de la sala) | `storyboard.rs` | Editor propio de planos |
| Línea de tiempo | `timebar.rs`, `timeline.rs` | Tiempo por capas (horizontal) |
| Hoja de exposición | `timeline.rs` (misma lógica, vertical) | **El mismo editor que la línea de tiempo, girado** |
| X-Sheet de Stop Motion | `smroom.rs` | Tiempo por fotos (tabla) |
| Tira de fotos | `smroom.rs` | Tiempo por fotos (miniaturas) |
| Capas | `panels.rs` (`layers_area`) | Lista de objetos (tipo Outliner) |
| Esquema de capas | `schematic_room` | **También lista/árbol de objetos** |
| Propiedades de la sala | `panels.rs` (`RightKind`, 9 variantes) | Propiedades, pero con contenido distinto por sala |
| Biblioteca de paletas / Paleta | `palette.rs`, `RightKind::Color/Palettes` | **Dos entradas a lo mismo** |
| Explorador del proyecto | `browser_room` | Explorador de recursos |
| Frame Browser | `smroom.rs` | **Explorador de recursos (fotos)** |
| Editor de curvas / Interpolación | `curves_panel`, `RightKind::Interp` | **Dos entradas a lo mismo** |
| Sustitución de dibujo | `substitution_panel` | Propiedades específicas del dibujo |
| Vista previa | `preview_panel` | Visor en modo «resultado final» |
| Barra de herramientas y cabecera de herramienta | `panels.rs` | Regiones del visor (no son editores) |
| Exportar | `RightKind::Export` | Producción |

Problemas concretos que esto causa:

- **Solo un hueco es intercambiable** (la columna derecha). El centro y la línea de tiempo de cada sala están escritos a mano; añadir una sala nueva exige tocar `shell.rs`.
- **Hay paneles duplicados**: línea de tiempo / hoja de exposición, Capas / Esquema, Explorador / Frame Browser, Paleta / Biblioteca de paletas, Curvas / Interpolación.
- **El estado del visor es global.** No se pueden tener dos visores con vistas distintas (por ejemplo el Live View y la mesa de luz a la vez).
- **El tipo de capa casi no decide nada.** `LayerKind::StopMotion` solo cambia cosas porque la sala Stop Motion lo sabe, no porque los editores se adapten.

## 3. Propuesta

### 3.1 Tres conceptos separados

- **Contexto**: lo que se edita ahora. Capa activa y su tipo, modo (objeto o edición), secuencia, composición actual. Ya existe a medias en `cl` y `obj_mode`.
- **Editor**: un tipo de panel con sus modos internos. Declara qué contexto necesita.
- **Sala**: una disposición de áreas, cada una con un editor, más la herramienta y el modo por defecto. Las 13 salas actuales pasan a ser **presets** hechos con los mismos editores.

### 3.2 Grupos de editores

Con la clasificación de Blender no alcanza, porque Blender no tiene producción (storyboard, animatic, captura, exportación) ni conoce la captura de cámara. Propongo **cinco grupos**:

| Grupo | Editores | Notas |
|---|---|---|
| **Visores** | Visor 2D (mesa de luz), Visor de composición (planos BG/MG/FG), Visor Stop Motion (Live View), Vista previa | Comparten zoom, guías y marco de cámara; cada uno pinta su tipo de capa |
| **Tiempo** | Línea de tiempo, Editor de curvas | La línea de tiempo tiene modos (ver 3.4) |
| **Datos** | Capas (con modo Esquema), Propiedades, Explorador (modos: Proyecto, Fotos, Paletas, Biblioteca) | Reúne los duplicados de la tabla anterior |
| **Producción** | Storyboard, Animatic, Cámara y captura, Exportar | Lo que en Blender no existe |
| **Sistema** | Preferencias, Registro/Info, Atajos | Reservado; Scripting entraría aquí si algún día se quiere |

### 3.3 Modos: el objeto manda, el editor se adapta

Cada tipo de capa declara sus **editores nativos** y sus modos:

| Capa | Modo objeto | Modo edición | Editores que lo acompañan |
|---|---|---|---|
| Vectorial / Mapa de bits | mover, girar, escalar, claves | dibujar sobre el cuadro | Visor 2D + Línea de tiempo en modo *Cuadros* (hoja de exposición) |
| Cutout / Rig | mover piezas, claves | posar el esqueleto | Visor 2D + Editor de curvas |
| Stop Motion | mover/escalar la secuencia entera | capturar, ordenar fotos | Visor Stop Motion + Línea de tiempo en modo *Fotos* + Cámara y captura |
| Composición (precomposición) | mover como un bloque | entrar en ella | Visor de composición + Línea de tiempo en modo *Capas* |
| Texto, Sólido, Nulo, Luz, Cámara | transformar | editar sus propiedades | Propiedades |

### 3.4 La línea de tiempo: un editor con modos, no cuatro

Siguiendo el Dope Sheet de Blender, la línea de tiempo es **un solo editor** cuyo modo sigue al contexto (y se puede forzar a mano):

- **Capas**: una fila por capa, para componer (Motion Graphics, composición por planos). Es la línea de tiempo actual.
- **Cuadros**: una fila por cuadro y una columna por capa (hoja de exposición): animación tradicional.
- **Fotos**: la secuencia de fotos con exposiciones (X-Sheet y tira de Stop Motion como dos vistas del mismo modo).
- **Claves**: claves y curvas de la capa activa.

Esto elimina la duplicación línea de tiempo / hoja de exposición y hace que Stop Motion deje de ser un caso especial en la línea de tiempo.

### 3.5 La idea central: «entrar en la capa»

Lo que describes (objetos que se precomponen en una línea de tiempo y a la vez se editan en su línea propia) ya existe en el código de Stop Motion sin que lo hayamos nombrado así:

> La secuencia de fotos tiene **su propio tiempo** (cuadros 1…N a sus fps) y la capa la **mapea** al tiempo del proyecto con un inicio y una velocidad (`SmRef.start`, `item_for_project_frame`).

Generalizado: **toda capa tiene un tiempo local y una correspondencia con el tiempo de su contenedor.** Una precomposición es una capa cuyo contenido es otra composición. Entonces:

- **Modo objeto** = ver la capa como un bloque dentro de la línea de tiempo del contenedor.
- **Modo edición** (Tab, o doble clic) = **entrar** en la capa: la línea de tiempo y el visor pasan a ser los de su interior, con una ruta visible arriba (Escena › Pelota › dibujos) para salir.
- El tipo de capa decide **qué hay dentro**: dibujos (cuadros), fotos (secuencia), otras capas (composición).

Esta es la pieza más grande y la que más cuidado pide; las demás se pueden hacer antes sin ella.

### 3.6 Reglas de coherencia (para que sea flexible sin perder intuición)

1. Cualquier área puede mostrar cualquier editor, pero **el menú atenúa los que no encajan con el contexto y dice por qué** («pide una capa Stop Motion»).
2. Un editor sin contexto válido muestra un **vacío explicativo con el botón para arreglarlo**, nunca un panel roto ni vacío mudo.
3. La barra de herramientas y la cabecera de herramienta son **regiones del visor**: cada visor trae las suyas.
4. Los atajos dependen del **editor bajo el puntero**, no de la sala (hoy se hace con `hover_area`).
5. Cada área guarda su **propio estado** (zoom, desplazamiento, filtros).
6. El tipo de proyecto (tradicional, cutout, motion graphics, Stop Motion; ya existe en `ROOM_SETS`) sigue eligiendo qué salas se ofrecen primero.

## 4. Plan por fases (cada una con su Issue y sin romper lo que existe)

| Fase | Qué se hace | Riesgo |
|---|---|---|
| **A. Registro de editores** | Un solo catálogo (`EditorKind`) con grupo, nombre y función que lo dibuja. Se envuelven los 7 editores actuales y los paneles sueltos. Menú agrupado en la cabecera de la columna derecha. Sin cambiar ninguna disposición | Bajo |
| **B. Estado por área** | Mover `zoom`, `pan`, onion y Live View de campos globales de `LeonApp` a un `ViewState` por área | Medio: toca mucho `canvas.rs` |
| **C. Salas como presets de áreas** | Cada sala es una lista de áreas (editor + tamaño). Las 13 reproducen exactamente lo de hoy (con capturas de comparación). El menú de editor aparece en cada área | Medio: es el cambio de `shell.rs` |
| **D. Contexto y vacíos explicativos** | Cada editor declara qué necesita; menú atenuado; mensajes con botón | Bajo |
| **E. Fusionar duplicados** | Línea de tiempo con modos; Explorador con modos (Proyecto/Fotos/Paletas); Capas con modo Esquema; Paleta + Biblioteca; Curvas + Interpolación | Medio: toca guardado de ajustes de usuario |
| **F. Tiempo local y «entrar en la capa»** | Composición como contenedor de tiempo, ruta de navegación, precomposiciones editables | Alto: cambia el modelo de datos y el formato `.ltoon` |
| **G. (tardía) Dividir y unir áreas con el ratón, salas del usuario** | Árbol de áreas guardable | Medio |

Orden recomendado: **A → D → C → B → E → F**. A y D se ven poco pero preparan todo; C es donde se nota; F se diseña aparte con calma porque toca el formato del proyecto.

## 5. Decisiones tomadas

Respuestas de la persona a las seis preguntas de la primera versión de este documento:

1. **Grupos**: Visores, Tiempo, Datos, Producción y Sistema, aceptados provisionalmente.
2. **Flexibilidad**: cambiar el editor de cada área **y** dividir y unir áreas con el ratón (la fase G entra en el plan).
3. **Entrar en la capa**: ambos gestos, Tab y doble clic.
4. **Modo objeto / edición**: **por capa**, como en Blender (cada capa recuerda su modo).
5. **Scripting**: sí, algún día (el grupo Sistema lo recibirá).
6. **Primera fase**: A, registro de editores (Issue #80).

## 6. Estado de las fases

| Fase | Estado |
|---|---|
| A. Registro de editores | Hecha en el Issue #80: catálogo con grupos, menú agrupado, y los paneles Capas, X-Sheet de Stop Motion y Cámara y captura disponibles como editores en el hueco de la columna derecha |
| D. Contexto y vacíos explicativos | Hecha en el Issue #83: cada editor declara su requisito (context.rs), explica qué falta y ofrece el botón que lo arregla, y el menú atenúa los que no encajan |
| C. Salas como presets de huecos | Hecha en el Issue #86: cada sala es una lista de huecos (`Slot`) con un editor por defecto; cada hueco tiene su selector de editor y la elección se recuerda por sala en `editores.json`. La geometría de cada hueco sigue siendo la de su tipo de sala (dividir y unir es la fase G) |
| B, E, F, G | Pendientes |

**Ajuste (Issue #151)**: la elección de editores de los huecos ya no es global (editores.json), sino **de cada proyecto**: se guarda en el .ltoon (campo slots, «sala/hueco» → editor), así que un proyecto nuevo siempre arranca con los presets de fábrica y cada archivo conserva su disposición. «Restablecer los editores de la sala» (menú Ventana y menú del selector de cada hueco) borra las elecciones de la sala actual. El archivo editores.json de versiones anteriores se ignora. Además, al entrar en la sala Animatic con planos y sin animatic se crea la composición y se entra en ella, y el botón «Crear animatic» abre la sala Animatic dentro de la composición.
