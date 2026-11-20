# Propuesta: Issues reescritos (borrador para revisar)

Refs #39. **Nada de esto está subido a GitHub.** Son borradores del texto de los Issues pequeños con la estructura de la
plantilla nueva (`.github/ISSUE_TEMPLATE/funcion.yml`). Tú decides, Issue por Issue, si se sube, se cambia o se descarta.

Lo único que ya se aplicó en GitHub son metadatos: etiquetas de área, tipo y prioridad, y un hito provisional por Issue.
El texto de ningún Issue se tocó.

## Cómo leerlo

- **Cambio propuesto**: qué haría con ese Issue (reescribir, dividir, fusionar, convertir en lista de pruebas).
- **Preguntas abiertas**: lo que no puedo decidir yo; sin tu respuesta, el Issue no está listo para empezar.
- Los números de «criterios» son comprobables sin leer el código.
- Donde no conozco un dato del prototipo o del programa original, lo digo en «Preguntas abiertas» en vez de inventarlo.

## Orden de trabajo sugerido

| Orden | Issue | Por qué |
|---|---|---|
| 1 | #20 Pruebas de guardado y apertura | Red de seguridad: hoy los campos nuevos (`nodes`, `original`, `shape`, tipo de capa) solo se prueban a nivel de modelo |
| 2 | Pruebas manuales de #13 y #31 (nuevos, abajo) | Hay cinco fases de código sin probar con ratón real ni tableta |
| 3 | #11 Anclaje y Cámara → #7 Exportar con cámara 3D → #5 Luz y profundidad de campo | #7 depende de #11 y de #5 |
| 4 | #6 Nulo y emparentado → #4 Rigging | El rigging necesita jerarquía de capas |
| 5 | #3 Texto, #19 Curvas de interpolación | Independientes |
| 6 | #14, #15, #17, #18, #9, #10, #12, #21 | Producción y pulido |

## Solapes que se resuelven

| Tema | Hoy está en | Propuesta |
|---|---|---|
| Pluma y nodos | #2 y Fase 4 de #31 | El código está hecho en #31/#2. #2 queda «implementado, falta prueba manual» y se cierra cuando tú lo des por bueno |
| Presión e inclinación de tableta | #16, #13, #31 | #16 pasa a ser el Issue de **pruebas con tableta** (lista de comprobación) para píxeles y vector |
| «Curvas Bézier» | #2 (dibujar) y #19 (interpolar) | #19 se renombra «Editor de curvas de interpolación de animación» |
| Verificación vs. función | #8 y #20 | Se mantienen, pero como `tipo:verificacion` y `tipo:deuda`, con casillas |

---

## Nuevos: pruebas manuales de lo ya implementado

### N1. [Pruebas] Capas de mapa de bits y pinceles de píxeles (#13)

Cambio propuesto: Issue nuevo con la plantilla `pruebas-manuales`.

- [ ] Pincel: pintar en una capa de mapa de bits con ratón; dureza, espaciado, flujo, redondez y ángulo cambian el trazo.
- [ ] Dinámica de forma, transferencia, dispersión, textura, doble pincel, color y bordes húmedos: cada sección cambia el trazo.
- [ ] Guardar un preset, cerrar y abrir el programa: el preset sigue ahí; borrarlo también lo quita de la biblioteca.
- [ ] Importar un `.abr` de Photoshop (versión 1 y 2): los pinceles aparecen y se avisa de lo que no se conservó.
- [ ] Exportar a `.abr` y abrirlo **en Photoshop** (nunca se probó fuera de LeonToon).
- [ ] Con tableta: la presión cambia tamaño y opacidad (ver Issue de tableta).
- [ ] Guardar y abrir un `.ltoon` con una capa de píxeles: se ve igual.

Entorno: Photoshop para el `.abr`; tableta (marca y modelo por confirmar).

### N2. [Pruebas] Sistema de pinceles vectoriales, Pluma y nodos (#31, #2)

Cambio propuesto: Issue nuevo con la plantilla `pruebas-manuales`.

- [ ] Lápiz vectorial: con ratón sale grosor constante; con presión (tableta) el grosor varía.
- [ ] Perfil afilado, extremos (redondo, cuadrado, plano) y uniones (redonda, bisel, inglete): se ven como en las capturas.
- [ ] Biblioteca: aplicar un pincel, marcar favorito, buscar, «Guardar como…», duplicar, renombrar, mover de categoría, eliminar; «Restaurar» en uno incluido.
- [ ] Exportar un pincel y la colección a `.ltvb` e importarlos en otra sesión.
- [ ] Selección: elegir varios trazos y cambiar grosor, color, opacidad y perfil; los valores mixtos se ven como «(mixto)»; Ctrl+Z deshace un arrastre en un solo paso.
- [ ] Asignar otro pincel a un trazo; copiar y pegar propiedades; «Restaurar originales».
- [ ] Pluma: clic = esquina; clic y arrastrar = nodo suave; clic en el primer nodo cierra; Intro termina; Esc cancela; Retroceso quita el último nodo.
- [ ] Editor de nodos: arrastrar nodo y tirador, Ctrl+clic añade nodo, Supr lo quita, suave/esquina, cerrar/abrir.
- [ ] Convertir un trazo libre en curva: la forma casi no cambia.
- [ ] Editor de perfil de grosor: añadir, mover y quitar puntos; también con varios trazos elegidos.
- [ ] Guardar, cerrar y abrir el `.ltoon`: las curvas conservan nodos y los trazos su aspecto.
- [ ] Papel cebolla y vista 3D muestran los trazos vectoriales igual que en 2D.
- [ ] Con muchos trazos (varios cientos) el visor sigue fluido.

Entorno: tableta para presión; sin tableta se prueba todo salvo la presión.

---

## Issues existentes: texto propuesto

### #2 Pluma vectorial Bézier (herramienta P)

**Cambio propuesto:** actualizar el estado. No reescribir el alcance.

> **Estado:** implementado en la Fase 4 de #31 (PR #37): Pluma con nodos y tiradores, edición de nodos, suave/esquina,
> añadir/quitar nodos, cerrar/abrir, guardado de los nodos en el `.ltoon`.
> **Falta:** que la persona lo pruebe a mano (ver Issue de pruebas manuales N2). Se cierra cuando lo dé por bueno.

### #3 Herramienta Texto y capa de texto (Ctrl+T)

**Cambio propuesto:** reescribir.

- **Contexto:** la herramienta Texto y la capa Texto (Mayús+A) existen en la interfaz pero no dibujan nada.
- **Alcance:** crear y editar texto en la escena con fuente, tamaño, color, alineación y opacidad; la capa Texto se ve en 2D, en la vista 3D y en la exportación; el texto se guarda en el `.ltoon` y sigue siendo editable.
- **Fuera de alcance:** texto sobre trazo, efectos (sombra, contorno), texto animado por letra, importar fuentes del sistema (ver pregunta 1).
- **Criterios:**
  1. Con la herramienta Texto, un clic en la escena crea un texto editable.
  2. Cambiar fuente, tamaño, color y alineación se ve al instante.
  3. El texto se ve igual en 2D, en 3D y en PNG/GIF/MP4.
  4. Guardar y abrir el `.ltoon` conserva texto y estilo; los proyectos anteriores abren igual.
  5. Ctrl+Z deshace crear y editar.
  6. La capa Texto se mueve, gira, escala y se anima con claves como las demás.
- **Verificación:** prueba automática (guardado, exportación con texto), captura, prueba manual de edición.
- **Dependencias:** ninguna.
- **Restricciones:** egui no incluye edición de texto sobre un lienzo propio; hay que decidir cómo se edita en la escena.
- **Preguntas abiertas:** (1) ¿Solo las fuentes incluidas (Inter) o también las del sistema? (2) ¿Texto de una línea o párrafos? (3) ¿Hay tipografías obligatorias del prototipo?

### #4 Rigging: huesos y deformadores para cutout

**Cambio propuesto:** dividir en fases y añadir un estudio previo.

- **Contexto:** la sala Rigging solo tiene sustitución de dibujo y claves de capa.
- **Fases:**
  - **F0 – Estudio (sin código):** decidir el modelo (huesos sobre capas, no sobre malla) y escribir criterios de F1 a F3.
  - **F1 – Huesos y jerarquía:** crear huesos, pivotes, emparentar piezas (depende de #6).
  - **F2 – Animar:** claves de giro por hueso, vista previa y deshacer.
  - **F3 – Deformadores y exportación.**
- **Fuera de alcance:** cinemática inversa, deformación de malla, físicas.
- **Criterios de F1 (los demás se escriben en F0):** 1. Se crea una cadena de 3 huesos con pivotes. 2. Mover un hueso padre arrastra a los hijos. 3. Se guarda y abre en el `.ltoon`.
- **Dependencias:** #6.
- **Preguntas abiertas:** ¿Se parece al rigging de cutout del programa original (qué funciones concretas)? ¿Se necesita IK?

### #5 Vista 3D: luz y profundidad de campo

**Cambio propuesto:** reescribir con valores comprobables.

- **Contexto:** la capa Luz es solo un icono; la cámara no desenfoca.
- **Alcance:** luz con color, intensidad y posición que afecte a los planos; profundidad de campo con foco y apertura de la cámara (`focus` y `ap` del prototipo).
- **Fuera de alcance:** sombras proyectadas, reflejos, luces volumétricas; exportar con estos efectos (eso es #7).
- **Criterios:** 1. Mover una luz cambia el brillo de un plano de forma visible y repetible. 2. Cambiar el color de la luz tiñe los planos. 3. Con foco en MG, BG y FG se desenfocan en proporción a su distancia; la apertura cambia la cantidad. 4. Sin luces ni apertura, la vista se ve igual que hoy. 5. Los valores se guardan en el `.ltoon`.
- **Verificación:** capturas de la vista 3D, prueba manual de la persona.
- **Dependencias:** ninguna. **Es requisito de #7.**
- **Preguntas abiertas:** ¿La luz afecta a la vista 2D también? ¿Fórmula de desenfoque del prototipo (la tengo solo como nombres de variables)?

### #6 Capas Nulo y emparentado

**Cambio propuesto:** reescribir.

- **Alcance:** emparentar una capa a un Nulo u otra capa; heredar movimiento, giro y escala; guardar el parentesco.
- **Fuera de alcance:** huesos (#4), herencia solo de una propiedad.
- **Criterios:** 1. Se elige un padre desde el panel de la capa. 2. Mover, girar y escalar el padre mueve al hijo. 3. Un ciclo (A padre de B, B padre de A) se impide con un aviso. 4. Borrar el padre deja al hijo sin padre, sin perder su posición. 5. Se guarda y abre en el `.ltoon`. 6. Funciona en 2D y en 3D. 7. Ctrl+Z deshace emparentar.
- **Verificación:** pruebas automáticas (herencia, ciclos, guardado), captura.
- **Dependencias:** ninguna. **Es requisito de #4.**
- **Preguntas abiertas:** ¿Al emparentar, el hijo conserva su posición en pantalla o se coloca relativa al padre?

### #7 Vista 3D: que la cámara afecte a la exportación

**Cambio propuesto:** reescribir y marcar dependencias.

- **Alcance:** PNG, GIF y MP4 se exportan con la cámara 3D y los planos cuando la escena los use.
- **Fuera de alcance:** luz y profundidad de campo (#5), cámara animada con curvas (#19).
- **Criterios:** 1. Con la cámara 3D activa, la imagen exportada coincide con la vista 3D del cuadro. 2. Sin cámara 3D, la exportación es idéntica a la actual. 3. Mismo resultado en PNG, GIF y MP4. 4. Si #5 está activo, la exportación respeta luz y desenfoque.
- **Verificación:** prueba automática comparando una exportación con una referencia; prueba manual.
- **Dependencias:** #11 (cámara); #5 si se quiere incluir sus efectos.
- **Preguntas abiertas:** ¿Resolución de salida igual a la de la cámara o configurable?

### #8 Verificar la vista 3D con el ratón y deshacer

**Cambio propuesto:** convertir en lista de pruebas manuales (`tipo:verificacion`, `prueba-manual`).

- [ ] Orbitar: arrastrar, clic central y Alt.
- [ ] Zoom con la rueda; desplazar con Espacio o la mano.
- [ ] Arrastrar los ejes del gizmo (X, profundidad, altura) y deshacer con Ctrl+Z.
- [ ] Control «Cámara X» y su deshacer.
- [ ] Sensación de velocidad al orbitar y facilidad para agarrar el gizmo (anotar si hay que ajustar).

### #9 Barra N con pestañas verticales e iconos de trazo fino

**Cambio propuesto:** reescribir con criterio visual.

- **Alcance:** pestañas Ítem, Herramienta y Vista en vertical con el texto girado; iconos de trazo fino de 16 px; revisar tamaños de texto y radios de esquina contra el prototipo v0.3.
- **Fuera de alcance:** cambiar el contenido de las pestañas.
- **Criterios:** 1. Las pestañas se ven en vertical, con texto girado, y se pueden pulsar. 2. Los iconos se ven con trazo fino a 16 px. 3. Comparado lado a lado con el prototipo, las diferencias que queden están anotadas en el Issue. 4. El panel sigue usable a la anchura actual.
- **Verificación:** capturas antes y después junto a la del prototipo.
- **Preguntas abiertas:** ¿Qué tan fiel debe ser? ¿Se acepta una diferencia anotada?

### #10 Lista de capas: botones de capa 3D y papel cebolla por capa

**Cambio propuesto:** reescribir.

- **Alcance:** botón de capa 3D y de papel cebolla por capa, con sus tooltips, junto al ojo y el candado.
- **Fuera de alcance:** cambiar cómo funciona el papel cebolla global.
- **Criterios:** 1. Cada capa tiene los dos botones y su tooltip. 2. El papel cebolla por capa activa o desactiva solo esa capa. 3. El botón 3D equivale a «Capa 3D» del panel. 4. El estado se guarda en el `.ltoon`. 5. Proyectos anteriores abren con el valor actual (papel cebolla en la capa activa).
- **Preguntas abiertas:** ¿El papel cebolla por capa convive con el global o lo reemplaza?

### #11 Punto de anclaje y herramienta Cámara

**Cambio propuesto:** dividir en dos Issues: **#11a Punto de anclaje** y **#11b Cámara**.

- **#11a Alcance:** mover el pivote de giro y escala de la capa sin mover el dibujo. **Criterios:** 1. Arrastrar el anclaje no cambia lo que se ve. 2. Girar y escalar usan el nuevo pivote. 3. Se guarda por capa. 4. Ctrl+Z.
- **#11b Alcance:** mover y escalar la cámara en 2D y en la vista 3D. **Criterios:** 1. Mover la cámara cambia el encuadre en 2D. 2. La vista Cámara 3D coincide. 3. Se guarda. 4. Ctrl+Z. **Es requisito de #7.**
- **Preguntas abiertas:** ¿La cámara 2D y la 3D son la misma cámara o dos?

### #12 Rotar vista (R)

**Cambio propuesto:** reescribir.

- **Alcance:** girar el visor como una hoja real; restablecer; sin cambiar el dibujo.
- **Fuera de alcance:** guardar el ángulo en el proyecto (ver pregunta 1).
- **Criterios:** 1. Arrastrar con R gira el visor en torno al centro. 2. Un atajo restablece a 0°. 3. Dibujar con la vista girada produce trazos en el lugar correcto. 4. La exportación no cambia. 5. Funciona con todas las herramientas de dibujo y de selección.
- **Restricciones:** todo el código de entrada convierte pantalla a escena sin rotación; hay que revisar `local()` y las herramientas.
- **Preguntas abiertas:** (1) ¿Se guarda el ángulo en el `.ltoon` o es de sesión? (2) ¿Gira también la mesa de luz?

### #13 Capas de mapa de bits

**Cambio propuesto:** no reescribir. Añadir un comentario con el **estado por fase** (ya hecho en los comentarios) y enlazar el Issue N1 de pruebas manuales.

### #14 Exportar el storyboard a PDF

**Cambio propuesto:** reescribir.

- **Alcance:** PDF con una viñeta por plano: imagen, título, acción, diálogo, cámara, transición y notas.
- **Fuera de alcance:** diseños de página personalizados, portadas.
- **Criterios:** 1. Se exporta un PDF con una viñeta por plano en el orden del storyboard. 2. Cada viñeta muestra todos los campos que tenga el plano. 3. El texto largo no se corta ni se sale de la página. 4. Con 0 planos se avisa. 5. El PDF abre en un lector común.
- **Verificación:** prueba automática (el archivo existe y tiene N páginas o viñetas), prueba manual de apariencia.
- **Preguntas abiertas:** ¿Cuántas viñetas por página? ¿Orientación? ¿Se puede usar una biblioteca de PDF nueva (dependencia)?

### #15 Sonido: importar audio y onda en la línea de tiempo

**Cambio propuesto:** dividir en fases y añadir estudio previo.

- **Fases:** F0 decidir biblioteca de audio y formatos; F1 importar y mostrar onda; F2 reproducir sincronizado; F3 mezclar en el MP4.
- **Fuera de alcance:** editar el audio, varias pistas con mezcla, efectos.
- **Criterios de F1:** 1. Se importa WAV (y los formatos acordados en F0). 2. La onda se ve en la línea de tiempo en la posición correcta. 3. Se guarda la referencia al archivo en el `.ltoon` y se avisa si falta.
- **Restricciones:** el MP4 ya usa ffmpeg externo; la mezcla puede apoyarse en él.
- **Preguntas abiertas:** ¿Formatos requeridos (WAV, MP3, OGG)? ¿El audio va dentro del `.ltoon` o enlazado?

### #16 Presión de tableta gráfica

**Cambio propuesto:** convertir en lista de pruebas manuales con tableta (`tipo:verificacion`, `prueba-manual`), para píxeles y vectores.

- [ ] Marca y modelo de la tableta: ______
- [ ] El programa recibe la presión (el grosor del Lápiz vectorial varía y el Pincel de píxeles cambia tamaño u opacidad).
- [ ] La curva de presión del Lápiz vectorial se siente bien (anotar valores que prefieres).
- [ ] ¿La tableta envía inclinación? Si sí, abrir un Issue: egui no la expone (restricción conocida).
- [ ] Con ratón el comportamiento sigue siendo correcto (grosor constante).

### #17 Animatic: transiciones entre planos

**Cambio propuesto:** reescribir.

- **Alcance:** corte, fundido y barrido entre planos, con duración, en vista previa y exportación.
- **Criterios:** 1. Cada plano tiene transición y duración. 2. La vista previa muestra la transición. 3. La exportación coincide con la vista previa. 4. Se guarda en el `.ltoon`. 5. Proyectos anteriores abren con «corte».
- **Preguntas abiertas:** ¿Dirección del barrido configurable? ¿La transición consume tiempo del plano o se suma?

### #18 Preferencias: idioma de términos y escala de interfaz

**Cambio propuesto:** reescribir.

- **Alcance:** ventana de preferencias con idioma de términos y escala de la interfaz, que recuerda la elección entre sesiones.
- **Fuera de alcance:** traducir todo el programa; atajos personalizables.
- **Criterios:** 1. Se abre desde un menú. 2. Cambiar el idioma de términos actualiza los términos del glosario (por ejemplo, «Papel cebolla» / «Onion skin») sin reiniciar. 3. La escala se aplica sin romper los paneles. 4. Se recuerda al reabrir. 5. Si el archivo de preferencias está dañado, el programa arranca con los valores por defecto.
- **Preguntas abiertas:** ¿Qué términos entran en el glosario y en qué idiomas?

### #19 Editor de curvas de interpolación de animación (antes «Editor de curvas Bézier para interpolación»)

**Cambio propuesto:** renombrar y reescribir.

- **Alcance:** editor de curvas por propiedad (x, y, giro, escala, opacidad) con manejadores, que sustituye a Lineal, Suave y Escalón como opción extra.
- **Fuera de alcance:** el dibujo de curvas de la Pluma (#2).
- **Criterios:** 1. Cada propiedad de una clave tiene una curva editable. 2. Mover un manejador cambia la interpolación en la línea de tiempo. 3. Lineal, Suave y Escalón siguen disponibles y dan el mismo resultado que hoy. 4. Se guarda en el `.ltoon`; proyectos anteriores abren igual. 5. Ctrl+Z.
- **Dependencias:** puede reutilizar la matemática de `vector/bezier.rs`.

### #20 Pruebas automáticas de guardado, apertura y exportación

**Cambio propuesto:** actualizar el alcance (hoy hay más de 100 pruebas, no 13) y ponerlo en primer lugar.

- **Alcance:** guardar y abrir un `.ltoon` completo, abrir proyectos antiguos sin los campos nuevos, exportar PNG y GIF y comprobar que salen.
- **Criterios:** 1. Un proyecto con capas de todos los tipos, trazos con `pressure`, `style`, `original`, `nodes` y `shape` se guarda y abre idéntico. 2. Archivos de cada versión anterior (se guardan como ejemplos en el repositorio) abren sin pérdida. 3. Exportar PNG y GIF produce archivos válidos con las dimensiones esperadas. 4. Un archivo dañado da un error claro y no cierra el programa. 5. Las pruebas corren con `cargo test` en menos de 30 s.
- **Verificación:** es el propio Issue.

### #21 Actualizar MECHANICS.md: mesa de luz, Mayús+A y vista 3D

**Cambio propuesto:** añadir una regla de proceso y mantener el alcance.

- **Alcance:** mesa de luz, tipos de capa de Mayús+A (7), vista 3D (planos BG 900, MG 0, FG -380; cámara a 2400; proyección y atajos); marcar cada valor como medido del prototipo o estimado.
- **Criterios:** 1. Cada valor tiene su fuente («medido» o «estimado»). 2. `MECHANICS.md` y `mechanics.json` coinciden. 3. Se añaden los valores nuevos de los pinceles vectoriales (miter, etc.) si se quieren documentar.
- **Pregunta abierta:** ¿se documentan también los motores de pinceles?

### #31 Sistema profesional de pinceles vectoriales

**Cambio propuesto:** no reescribir. Añadir una tabla de estado por criterio de aceptación (20 criterios) y una sección «Restricciones técnicas descubiertas»:

- La inclinación y la orientación del lápiz no están disponibles en egui (criterio 3 queda parcial: solo presión).
- El perfil personalizado no se aplica a trazos cerrados.
- Al borrar con el borrador, una curva pasa a polilínea.
- No se documentó objetivo de rendimiento: se propone «3000 trazos × 120 puntos: contorno en menos de 200 ms» (hoy 90 ms).
