# Brief

**Question:** Reconstruir LeonToon (escritorio, Rust/egui) con la interfaz y el sistema de trabajo por salas de la versión web de la persona (leontoon.html / carpeta leontoon), conservando las funciones centrales de animación 2D del programa objetivo.

**Purpose:** La persona es principiante y quiere una versión estable propia; dio su HTML anterior como referencia de UI y salas.

**Targets:**
- C:\Program Files (x86)\Toon Boom Animation\Toon Boom Harmony 24 Premium\win64\bin\HarmonyPremium.exe
- C:\Users\leont\OneDrive\Documents\LeonToon\leontoon (proyecto web propio de la persona)

**Constraints:**
- Deep mode: valores del original leídos del ejecutable y de resources/prefs.xml
- Rust + eframe/egui en app/
- Sin marcas ni logotipos del original; LeonToon es la marca de la persona
- Opción --screenshot

**Done when:** cargo build funciona, la ventana tiene las 11 salas de la versión web, --screenshot produce una imagen correcta y rebuild_check pasa.
