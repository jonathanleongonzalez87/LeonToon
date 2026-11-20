//! Ventanas: cambios sin guardar, nuevo proyecto, guía rápida y "acerca de".

use super::*;

impl LeonApp {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        self.unsaved_dialog(ctx);
        self.add_layer_menu(ctx);
        self.new_dialog(ctx);
        self.help_window(ctx);
        self.about_window(ctx);
    }

    /// Menú flotante «Agregar capa» (Mayús+A): 7 tipos de capa.
    fn add_layer_menu(&mut self, ctx: &egui::Context) {
        let Some(at) = self.add_menu else { return };
        let screen = ctx.screen_rect();
        let pos = Pos2::new(at.x.min(screen.max.x - 250.0).max(screen.min.x + 4.0), at.y.min(screen.max.y - 290.0).max(screen.min.y + 4.0));
        let mut pick: Option<LayerKind> = None;
        let out = egui::Area::new(egui::Id::new("add_layer_menu")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::default()
                .fill(BG2)
                .stroke(stk(1.0, LINE_STRONG))
                .corner_radius(9.0)
                .inner_margin(egui::Margin::same(5))
                .shadow(egui::epaint::Shadow { offset: [0, 10], blur: 28, spread: 0, color: Color32::from_black_alpha(115) })
                .show(ui, |ui| {
                    ui.set_width(230.0);
                    ui.label(RichText::new("Agregar capa").size(11.5).color(INK3));
                    ui.add_space(2.0);
                    for kind in LayerKind::ALL {
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 5.0, BG4);
                        }
                        let t = c32(kind.tint());
                        let badge = Rect::from_center_size(r.left_center() + Vec2::new(16.0, 0.0), Vec2::splat(18.0));
                        ui.painter().rect_filled(badge, 4.0, with_alpha(t, 0.2));
                        ui.painter().text(badge.center(), Align2::CENTER_CENTER, kind.badge(), FontId::proportional(10.5), t);
                        ui.painter().text(r.left_center() + Vec2::new(34.0, 0.0), Align2::LEFT_CENTER, kind.label(), FontId::proportional(12.5), TX);
                        if kind.pending() {
                            ui.painter().text(r.right_center() - Vec2::new(10.0, 0.0), Align2::RIGHT_CENTER, "pendiente", FontId::proportional(10.5), INK3);
                        }
                        if resp.clicked() {
                            pick = Some(kind);
                        }
                    }
                });
        });
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        let outside = ctx.input(|i| i.pointer.any_pressed()) && ctx.input(|i| i.pointer.interact_pos()).map_or(false, |p| !out.response.rect.contains(p));
        if let Some(k) = pick {
            self.add_menu = None;
            self.add_layer_kind(k);
        } else if esc || (outside && self.frame_no > 1) {
            self.add_menu = None;
        }
    }

    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(p) = self.pending.clone() else { return };
        let mut close = false;
        egui::Window::new("Cambios sin guardar")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Tu proyecto tiene cambios que no se han guardado.");
                ui.label("¿Quieres guardarlos antes de continuar?");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Guardar").strong()).clicked() {
                        close = true;
                        if self.save() {
                            self.perform(ctx, p.clone());
                        }
                    }
                    if ui.button("No guardar").clicked() {
                        close = true;
                        self.dirty = false;
                        self.perform(ctx, p.clone());
                    }
                    if ui.button("Cancelar").clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.pending = None;
        }
    }

    fn new_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_new {
            return;
        }
        let mut open = true;
        let mut create = false;
        let mut cancel = false;
        egui::Window::new("Nuevo proyecto")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Elige un formato:");
                ui.horizontal_wrapped(|ui| {
                    for (name, w, h) in [
                        ("HD 1280×720", 1280, 720),
                        ("Full HD 1920×1080", 1920, 1080),
                        ("Cuadrado 1080×1080", 1080, 1080),
                        ("Vertical 1080×1920", 1080, 1920),
                    ] {
                        if ui.selectable_label(self.new_w == w && self.new_h == h, name).clicked() {
                            self.new_w = w;
                            self.new_h = h;
                        }
                    }
                });
                ui.add_space(6.0);
                egui::Grid::new("new_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label("Tamaño");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.new_w).range(16..=4096).suffix(" px"));
                        ui.label("×");
                        ui.add(egui::DragValue::new(&mut self.new_h).range(16..=4096).suffix(" px"));
                    });
                    ui.end_row();
                    ui.label("Velocidad");
                    ui.horizontal(|ui| {
                        for f in [12, 24, 30] {
                            if ui.selectable_label(self.new_fps == f, format!("{f} fps")).clicked() {
                                self.new_fps = f;
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("Duración");
                    ui.add(egui::DragValue::new(&mut self.new_secs).range(0.1..=600.0).speed(0.1).suffix(" s"));
                    ui.end_row();
                });
                ui.label(
                    RichText::new("Consejo: 12 fps es ideal para empezar a animar a mano.")
                        .size(11.5)
                        .color(Color32::from_gray(160)),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Crear").strong()).clicked() {
                        create = true;
                    }
                    if ui.button("Cancelar").clicked() {
                        cancel = true;
                    }
                });
            });
        if create {
            let len = ((self.new_secs * self.new_fps as f32).round() as u32).max(1);
            self.project = Project::blank(self.new_w, self.new_h, self.new_fps, len);
            self.path = None;
            self.after_load();
            self.say("Proyecto nuevo creado. Elige el pincel (B) y empieza a dibujar.");
        }
        if !open || create || cancel {
            self.show_new = false;
        }
    }

    fn help_window(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let mut open = true;
        egui::Window::new("Guía rápida")
            .open(&mut open)
            .default_width(480.0)
            .collapsible(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                    ui.heading("Tu primera animación en 5 pasos");
                    let steps = [
                        "Al abrir LeonToon eliges el tipo de animación: cada tipo abre su conjunto de salas (pestañas de arriba). Con «+» añades salas; doble clic renombra, x cierra y arrastrar reordena.",
                        "Elige el Pincel (B) y dibuja en la hoja: es la cámara, lo que se verá en tu animación.",
                        "Mayús+I crea un cuadro vacío y Mayús+D duplica el actual. El papel cebolla (O) muestra el anterior en ámbar y el siguiente en azul.",
                        "Tab cambia entre modo edición (dibujar) y modo objeto (mover, girar y escalar capas con claves). Espacio reproduce.",
                        "Guarda con Ctrl+S; tus salas se guardan con el proyecto. Exporta en la sala Exportar (PNG, sprites, GIF o MP4).",
                    ];
                    for (i, t) in steps.iter().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{}.", i + 1)).strong().color(ACCENT));
                            ui.label(*t);
                        });
                    }
                    ui.add_space(8.0);
                    ui.heading("Ideas clave");
                    ui.label("• Cada capa es como una hoja transparente: el fondo en una, el personaje en otra.");
                    ui.label("• En la hoja de exposición, el número marca dónde empieza un dibujo y la raya cuánto se mantiene. Un borde dorado es una clave.");
                    ui.label("• En la barra de cuadros, arrastra los extremos ámbar y azul para ver más o menos dibujos en el papel cebolla.");
                    ui.label("• Si algo sale mal, Ctrl+Z lo deshace. Se guarda una copia automática cada 30 segundos, y al guardar se conserva la versión anterior.");
                    ui.add_space(8.0);
                    ui.heading("Atajos de teclado (estilo Blender)");
                    egui::Grid::new("keys").num_columns(2).striped(true).spacing([16.0, 4.0]).show(ui, |ui| {
                        for (k, d) in [
                            ("T / N", "Mostrar u ocultar herramientas / barra lateral"),
                            ("Tab", "Modo objeto / modo edición"),
                            ("V / W / Y / C", "Seleccionar / Rotación / Anclaje / Cámara"),
                            ("B / L / P / Q", "Pincel / Lápiz vectorial / Pluma / Formas"),
                            ("E / G / I", "Borrador / Relleno / Cuentagotas"),
                            ("H / R / Z", "Mano / Rotar vista / Zoom"),
                            ("O", "Papel cebolla"),
                            ("Espacio", "Reproducir (mantener + arrastrar: mover la vista)"),
                            ("← → / , .", "Cuadro anterior / siguiente"),
                            ("Mayús+← / Mayús+→", "Primer / último cuadro"),
                            ("Mayús+I / Mayús+D / Mayús+Supr", "Cuadro vacío / duplicar / eliminar"),
                            ("Mayús+A / Mayús+N", "Agregar capa"),
                            ("K  /  [  ]", "Insertar clave / clave anterior o siguiente"),
                            ("F5 / Mayús+F5", "Alargar / acortar exposición"),
                            ("Ctrl+Tab", "Línea de tiempo / hoja de exposición"),
                            ("Inicio / Ctrl+1", "Ajustar a la vista / zoom 100 %"),
                            ("Rueda / Ctrl+rueda", "Zoom de la mesa / zoom de la hoja"),
                            ("Ctrl+Z / Ctrl+Mayús+Z", "Deshacer / Rehacer"),
                            ("Ctrl+S / Ctrl+O / Ctrl+N", "Guardar / Abrir / Pantalla de inicio"),
                            ("Ctrl+E", "Ir a la sala Exportar"),                        ] {
                            ui.label(RichText::new(k).monospace().color(ACCENT));
                            ui.label(d);
                            ui.end_row();
                        }
                    });
                });
            });
        if !open {
            self.show_help = false;
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        egui::Window::new("Acerca de LeonToon")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading(RichText::new("LeonToon").color(ACCENT));
                ui.label("Estudio de animación 2D cuadro a cuadro.");
                ui.label(format!("Versión {}", env!("CARGO_PKG_VERSION")));
                ui.label("Trabajo por salas · Proyectos en formato .ltoon · Exporta PNG, sprites y GIF.");
            });
        if !open {
            self.show_about = false;
        }
    }
}
