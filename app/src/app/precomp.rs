//! Precomposiciones en la interfaz (Issue #114, fase M6): precomponer, entrar en la capa, salir y deshacer la precomposición.
//!
//! Entrar en una precomposición cambia lo que se edita: sus capas pasan a ser las del proyecto y las de fuera esperan en la
//! pila (`Project::stack`), que viaja con el proyecto, así que deshacer y rehacer funcionan también al entrar y salir.

use super::*;

impl LeonApp {
    /// Precompone las capas elegidas (Ctrl+Mayús+C): las mete en una precomposición nueva, en su sitio.
    pub(super) fn precompose_current(&mut self) {
        let sel = self.selected_layers();
        if sel.iter().any(|&i| self.project.layers[i].kind == LayerKind::Camera) {
            self.say("La cámara no se puede precomponer");
            return;
        }
        self.checkpoint();
        self.lsel.clear();
        if let Some(i) = self.project.precompose(&sel) {
            self.cl = i;
            self.sel = None;
            let name = self.project.layers[i].name.clone();
            self.say(format!("«{name}» es una precomposición con su propia línea de tiempo (Tab o doble clic para entrar)"));
        }
    }

    /// Entra a editar la precomposición de la capa `li` (Tab o doble clic).
    pub(super) fn enter_precomp(&mut self, li: usize) {
        let Some(l) = self.project.layers.get(li) else { return };
        if l.kind != LayerKind::Precomp {
            return;
        }
        let in_f = l.in_f;
        let (cl, frame) = (self.cl, self.frame);
        self.checkpoint();
        if self.project.enter_comp(li, cl, frame) {
            self.reset_after_level_change();
            // El cuadro de dentro es el de fuera menos la entrada de su barra.
            self.frame = frame.saturating_sub(in_f).min(self.project.length.saturating_sub(1));
            let path = self.project.comp_path().join(" › ");
            self.say(format!("Dentro de {path}: Mayús+Tab o «Salir» para volver"));
        } else {
            self.say("Esta precomposición no se puede abrir (está vacía o ya está abierta)");
        }
    }

    /// Sale de la precomposición que se edita (Mayús+Tab).
    pub(super) fn exit_precomp(&mut self) {
        if !self.project.in_comp() {
            return;
        }
        self.checkpoint();
        if let Some((cl, frame)) = self.project.exit_comp() {
            self.reset_after_level_change();
            self.cl = cl.min(self.project.layers.len().saturating_sub(1));
            self.frame = frame.min(self.project.length.saturating_sub(1));
        }
    }

    /// Sube hasta el nivel `depth` de la ruta (0 = la composición principal): lo que se pulsa en la ruta de navegación.
    pub(super) fn exit_to_level(&mut self, depth: usize) {
        while self.project.stack.len() > depth {
            self.exit_precomp();
        }
    }

    /// Deshace la precomposición de la capa `li`: sus capas vuelven a la composición de fuera.
    pub(super) fn unprecompose_layer(&mut self, li: usize) {
        self.checkpoint();
        self.lsel.clear();
        if let Some(i) = self.project.unprecompose(li) {
            self.cl = i;
            self.sel = None;
            self.say("Precomposición deshecha: sus capas vuelven a esta composición");
        }
    }

    pub(super) fn reset_after_level_change(&mut self) {
        self.cl = 0;
        self.lsel.clear();
        self.sel = None;
        self.playing = false;
        self.picked.clear();
        self.mt.reveal = None;
    }

    /// La ruta de navegación («Escena › Precomp 1») con un botón por nivel para subir hasta él y «Salir». No dibuja nada fuera
    /// de una precomposición.
    pub(super) fn breadcrumb_ui(&mut self, ui: &mut egui::Ui) {
        if !self.project.in_comp() {
            return;
        }
        let path = self.project.comp_path();
        let mut go: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (i, name) in path.iter().enumerate() {
                if i > 0 {
                    ui.label(RichText::new("›").color(INK3));
                }
                let last = i + 1 == path.len();
                if last {
                    ui.label(RichText::new(name).strong().color(ACCENT));
                } else if ui.add(egui::Label::new(RichText::new(name).color(MU)).sense(Sense::click())).on_hover_text("Subir hasta aquí").clicked() {
                    go = Some(i);
                }
            }
            if ui.small_button("↩ Salir").on_hover_text("Salir de la precomposición (Mayús+Tab)").clicked() {
                go = Some(path.len().saturating_sub(2));
            }
        });
        if let Some(d) = go {
            self.exit_to_level(d);
        }
    }
}