# Editor de tiempo para montaje audiovisual y precomposición

Refs #92. **Estado: borrador para decidir.** Nada de esto está implementado. Parte del pedido de la persona: la sala de **Interpolación** del prototipo v0.3 se acerca más a lo que se busca, como editor de montaje de capas y de su animación por interpolación, con **precomposiciones que pueden ser colecciones** (grupos o carpetas) que agrupan capas, y que a la vez sirva como línea de tiempo del animatic.

Las referencias a líneas son del archivo `referencia/LeonToon — Prototipo de interfaz v0.3.html`.

## 1. Cómo funciona hoy la sala de Interpolación del prototipo

La sala `interp` (y la de Composición 3D, `comp`) no usa la línea de tiempo de dibujos: usa una **línea de tiempo de capas al estilo de After Effects** (`renderAE`, líneas 1121 a 1179). Lo que tiene:

| Elemento | Qué hace en el prototipo |
|---|---|
| **Columnas de la capa** | Vídeo (ojo), audio, solo, bloqueo, etiqueta de color, número y nombre (línea 1136) |
| **Interruptores** | Tímida (ocultable con un botón maestro), contraer transformaciones (para precomposiciones), calidad, efectos, desenfoque de movimiento, capa de ajuste (línea 1137) |
| **Modos** | Modo de fusión y mate de seguimiento (línea 1148) |
| **Primario y vínculo** | Un selector para emparentar la capa a otra: la hija hereda la posición de la madre (`parentOf`, `mapper`, líneas 809 a 814) |
| **Barra de la capa** | Cada capa tiene una barra con su entrada y salida (`inF`, `outF`) en la zona de cuadros (línea 1151) |
| **Propiedades** | Al desplegar una capa salen Punto de anclaje, Posición, Escala, Rotación y Opacidad, cada una con **cronómetro** (activa la animación), valor actual y claves propias (líneas 1157 a 1159). Las teclas **A, P, S, R, T** muestran solo esa propiedad, **U** las que tienen claves, **M** la máscara (líneas 1733 a 1735) |
| **Claves** | Una lista de claves por propiedad (`L.keys[k]`), interpolación con aceleración suave entre claves (`valAt`, líneas 803 a 805) |
| **Cámara** | Al desplegarla, sus opciones: zoom, lente, profundidad de campo, enfoque, apertura (línea 1162) |
| **Alternar columnas** | F4 pasa entre «Interruptores y modos», solo interruptores y solo modos (línea 1186) |
| **Modos de fusión** | Normal, Multiplicar, Pantalla, Superponer, Oscurecer, Aclarar, Diferencia y Añadir (línea 816) |
| **Tipos de capa** | vectorial, mapa de bits, sólido, nulo, texto, luz, cámara y **precomposición** (línea 904) |
| **Precomposición** | Ctrl+Mayús+C convierte la capa en una precomposición «con su propia línea de tiempo» (`precompose`, línea 1600); el doble clic para abrirla es solo un aviso en el prototipo (línea 1171). Es decir, **la idea está dibujada, el comportamiento no** |
| **Curvas** | El panel derecho de la sala (`right:'interp'`) ofrece curvas de aceleración predefinidas (líneas 1045 a 1058) |

La sala cambia además a **modo objeto** al entrar (línea 1654): coherente con la propuesta de modo por capa de `arquitectura-editores.md`.

## 2. Qué tiene LeonToon hoy

| Pieza | Estado en la aplicación |
|---|---|
| Capas | `Layer` con nombre, visibilidad, bloqueo, opacidad, tipo, color de sólido, plano 3D y claves de dibujo |
| Claves de movimiento | **Una sola pista** por capa (`xform`): cada clave guarda posición, giro, escala y opacidad **a la vez**, más una interpolación (escalón, suave o lineal) por clave. No hay clave de punto de anclaje |
| Línea de tiempo | Capas en filas y cuadros en columnas con las celdas de dibujo; no muestra propiedades ni claves por propiedad |
| Modos de fusión, mates, vínculo (madre e hija), solo, tímida, capa de ajuste | **No existen** |
| Entrada y salida de una capa (barra) | **No existe**: una capa dura todo el proyecto, salvo Stop Motion, que tiene `start` |
| Precomposición, colecciones | **No existen** |
| Sala Interpolación | La línea de tiempo normal, y a la derecha «Claves de la capa» y las propiedades de la capa. El editor de curvas dibuja la posición |
| Animatic | Planos del storyboard con duración y transición en su propia sala |

Lo que sí hay y ayuda: el **tiempo local de Stop Motion** (una secuencia con sus propios cuadros que la capa traslada al tiempo del proyecto con un inicio y una velocidad) es exactamente el mecanismo que necesita una precomposición.

## 3. Qué falta, en una tabla

| Del prototipo | Falta en LeonToon |
|---|---|
| Barra de entrada y salida por capa | Campos de entrada, salida y desplazamiento en el modelo y en la línea de tiempo |
| Propiedades con cronómetro y claves propias | Pasar de una pista de transformación a **una pista por propiedad** (anclaje, posición, escala, rotación, opacidad) |
| Modos de fusión y mates | Campo `blend` y `matte` por capa y su dibujo en el visor y en la exportación (`render.rs`) |
| Vínculo madre e hija | Campo `parent` por capa y la cadena de transformaciones |
| Solo, tímida, etiqueta de color, capa de ajuste | Campos y su efecto al dibujar |
| Precomposición y entrar en ella | Modelo de **composición**, ruta de navegación, doble clic y Tab (fase F de la arquitectura) |
| **Colecciones** | No están en el prototipo: son idea nueva de la persona |

## 4. Propuesta

### 4.1 Tres conceptos, no dos

1. **Composición**: un contenedor de tiempo (duración, fps, tamaño) con una lista de capas. El proyecto tiene una composición raíz.
2. **Colección** (grupo o carpeta): **organiza** capas dentro de una misma composición. No tiene tiempo propio. Se pliega en la lista, y su visibilidad, bloqueo, opacidad y modo de fusión se aplican a todo el grupo como una unidad.
3. **Precomposición**: una capa cuyo contenido es **otra composición**, con su propio tiempo, que la capa traslada al tiempo de su contenedor (inicio, velocidad, repetición). Es lo que ya hace Stop Motion con su secuencia.

La pasarela entre ellas: **«Precomponer» (Ctrl+Mayús+C) convierte las capas elegidas, o una colección, en una precomposición**, y «Deshacer precomposición» la vuelve colección. Así la persona empieza organizando con colecciones, que son ligeras, y solo pasa a precomposición cuando necesita tiempo propio o efectos sobre el conjunto.

### 4.2 Un editor de tiempo con modo «Montaje»

Sigue lo de `arquitectura-editores.md` (una línea de tiempo con modos): el modo **Montaje** es la línea de tiempo del prototipo (columnas de interruptores y modos, barra de capa, propiedades con cronómetro). El mismo modo sirve para:

- **Interpolación y Composición 3D** (el prototipo ya los comparte).
- **Animatic**: los planos del storyboard pasan a ser **capas-clip** de una composición, con su barra de entrada y salida; las transiciones son claves de opacidad o fundidos entre barras.
- **Montaje de vídeo y audio** (cuando se decida importar clips; ver preguntas).

Los modos de dibujo (Cuadros) y de fotos (Stop Motion) siguen siendo modos del mismo editor, y el contexto elige el modo según el tipo de capa y si se ha entrado en ella.

### 4.3 Plan por fases (cada una con su Issue)

| Fase | Qué se hace | Dependencias |
|---|---|---|
| **M1. Pistas por propiedad** | Una pista de claves por propiedad (anclaje, posición, escala, rotación, opacidad), con migración de las claves actuales; cronómetro en la interfaz | Cambia el formato `.ltoon` (se conservan los archivos viejos) |
| **M2. Barra de capa** | Entrada, salida y desplazamiento por capa; solo, tímida, etiqueta de color | M1 no es necesaria |
| **M3. Fusión, mates y vínculo** | Modos de fusión, mates de seguimiento y madre e hija, con su dibujo en visor y exportación | Toca `render.rs` |
| **M4. Modo Montaje** | La línea de tiempo del prototipo como modo del editor de tiempo, con las teclas A, P, S, R, T, U y F4 | M1 a M3 y fase C de la arquitectura (hecha) |
| **M5. Colecciones** | Agrupar y desagrupar (Ctrl+G), plegar, propiedades del grupo | M2 |
| **M6. Precomposición** | Modelo de composición, Precomponer, entrar con Tab y doble clic, ruta de navegación | Fase F de la arquitectura; M5 |
| **M7. Animatic como composición** | Los planos como capas-clip; transiciones | M4 y M6 |
| **M8. Vídeo y audio** | Importar clips (decodificación) y pista de audio | Decisión de la persona; Issue #15 |

### 4.4 Riesgos

- **Formato**: M1 y M6 cambian el modelo guardado. Hay que subir la versión y mantener la lectura de archivos anteriores (hoy hay pruebas de apertura de versiones antiguas).
- **Rendimiento**: las precomposiciones anidadas obligan a guardar en caché lo que no cambia; el visor ya carga fotos de Stop Motion con caché, que sirve de modelo.
- **Deshacer**: el historial actual guarda instantáneas del proyecto con dibujos compartidos; las composiciones nuevas deben compartirse igual para que deshacer no copie todo.
- **Planos 3D (BG, MG, FG)**: hay que decidir cómo se combinan con las precomposiciones (¿el plano es de la capa precompuesta o de las de dentro?).

## 5. Decisiones que necesito de ti

1. **Colección y precomposición**: ¿te sirve la separación de 4.1 (la colección organiza, la precomposición tiene tiempo propio, y Precomponer convierte una en otra), o prefieres que toda colección tenga tiempo propio?
2. **Pistas por propiedad (M1)**: ¿estás de acuerdo en cambiar el formato de las claves de movimiento, conservando la lectura de lo viejo?
3. **Modos de fusión**: ¿los ocho del prototipo bastan para empezar?
4. **Mates de seguimiento**: ¿entran en M3 o los dejamos para después?
5. **Vídeo y audio (M8)**: ¿quieres importar clips de vídeo y audio? Si sí, hay que elegir cómo decodificarlos (afecta a las dependencias y al tamaño del programa).
6. **Interpolación y Composición 3D**: el prototipo las deja con la misma línea de tiempo. ¿Las fusionamos en una sola sala de «Montaje»?
7. **Orden**: ¿empiezo por M1 y M2 (lo más ligero) o prefieres que el siguiente paso visible sea M4, el modo Montaje, aunque al principio sea solo de lectura?
