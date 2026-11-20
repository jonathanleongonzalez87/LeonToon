//! Paneles (prototipo v0.3): barra de herramientas por modo, cabecera de herramienta,
//! columna derecha con Capas y Propiedades (según la sala o el editor elegido) y barra lateral N.

use super::shell::{area_header, muted, panel_header, pill};
use super::*;

impl LeonApp {
    // ---------- Herramientas y modos ----------

    pub(super) fn toggle_mode(&mut self) {
        self.finish_gesture();
        self.obj_mode = !self.obj_mode;
        if self.obj_mode && self.tool.edit_only() {
            self.tool = Tool::Select;
        }
        if !self.obj_mode && self.tool.obj_only() {
            self.tool = Tool::Brush;
        }
        self.say(if self.obj_mode { "Modo objeto: mueve, gira y escala capas" } else { "Modo edición: dibuja y elige trazos" });
    }

    /// Elige herramienta; si pertenece al otro modo, cambia de modo (como setTool del prototipo).
    pub(super) fn set_tool(&mut self, t: Tool) {
        if t.edit_only() {
            self.obj_mode = false;
        }
        if t.obj_only() {
            self.obj_mode = true;
        }
        self.tool = t;
        if t.pending() {
            self.say(format!("{}: pendiente. Ya tiene su sitio en la barra.", t.name()));
        }
    }

    pub(super) fn tool_column(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            let set: &[Option<Tool>] = if self.obj_mode { &TOOLS_OBJ } else { &TOOLS_EDIT };
            for t in set {
                match t {
                    None => {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(24.0, 9.0), Sense::hover());
                        ui.painter().hline(r.x_range(), r.center().y, stk(1.0, LINE_STRONG));
                    }
                    Some(t) => {
                        let t = *t;
                        let tip = format!("{}  ({})\n{}", t.name(), t.letter(), t.tip());
                        if tool_button(ui, self.tool == t, t.pending(), &tip, move |p, r, c| draw_tool_icon(t, p, r, c)).clicked() {
                            self.set_tool(t);
                        }
                    }
                }
            }
        });
    }

    /// Cabecera de herramienta: modo, herramienta actual y sus opciones; a la derecha, la capa.
    pub(super) fn tool_head(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.spacing_mut().slider_width = 64.0;
            let narrow = ui.available_width() < 1500.0;
            let mut obj = self.obj_mode;
            egui::ComboBox::from_id_salt("modesel")
                .width(120.0)
                .selected_text(if obj { "Modo objeto" } else { "Modo edición" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut obj, true, "Modo objeto");
                    ui.selectable_value(&mut obj, false, "Modo edición");
                });
            if obj != self.obj_mode {
                self.toggle_mode();
            }
            let t = self.tool;
            let (ir, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
            draw_tool_icon(t, ui.painter(), ir, TX);
            ui.label(RichText::new(t.name()).color(TX).strong());
            ui.separator();
            if t.pending() {
                ui.label(RichText::new("Esta herramienta está pendiente.").color(INK3));
            }
            if t.draws() {
                if pill(ui, "Presión", self.pressure).on_hover_text("Usa la presión del lápiz de tableta en los controles de tamaño, opacidad y flujo del pincel (se configuran en el panel N)").clicked() {
                    self.pressure = !self.pressure;
                }
                ui.label(RichText::new("Grosor").color(MU));
                let size = match t {
                    Tool::Brush => &mut self.brush_size,
                    Tool::Vector => &mut self.pencil_size,
                    Tool::Eraser => &mut self.eraser_size,
                    _ => &mut self.shape_width,
                };
                ui.add(egui::Slider::new(size, 1.0..=300.0).logarithmic(true).max_decimals(0));
                if t != Tool::Eraser {
                    ui.label(RichText::new("Suavizado").color(MU));
                    let mut s = self.smoothing * 100.0;
                    if ui.add(egui::Slider::new(&mut s, 0.0..=85.0).max_decimals(0).suffix(" %")).changed() {
                        self.smoothing = s / 100.0;
                    }
                    ui.label(RichText::new("Opacidad").color(MU));
                    let mut o = self.stroke_opacity * 100.0;
                    if ui.add(egui::Slider::new(&mut o, 15.0..=100.0).max_decimals(0).suffix(" %")).changed() {
                        self.stroke_opacity = o / 100.0;
                    }
                }
            }
            if t == Tool::Shape {
                for (k, n) in [(ShapeKind::Line, "Línea"), (ShapeKind::Rect, "Rectángulo"), (ShapeKind::Ellipse, "Elipse")] {
                    if pill(ui, n, self.shape_kind == k).clicked() {
                        self.shape_kind = k;
                    }
                }
                if self.shape_kind != ShapeKind::Line {
                    ui.checkbox(&mut self.fill_shapes, "Rellenar");
                }
            }
            if matches!(t, Tool::Brush | Tool::Vector | Tool::Fill | Tool::Shape) {
                ui.label(RichText::new("Color").color(MU));
                let (r, resp) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::click());
                ui.painter().rect_filled(r, 3.0, self.color);
                ui.painter().rect_stroke(r, 3.0, stk(1.0, LINE_STRONG), EStrokeKind::Outside);
                if resp.on_hover_text("Clic: intercambiar trazo y relleno").clicked() {
                    std::mem::swap(&mut self.color, &mut self.fill_color);
                }
                if !narrow {
                    ui.label(RichText::new(self.color_name()).color(TX));
                }
            }
            if t == Tool::Select && !self.obj_mode {
                let n = self.picked.len();
                ui.label(RichText::new(if n == 0 { "nada elegido".to_string() } else { format!("{n} trazos") }).color(MU));
                if pill(ui, "Todo", false).clicked() {
                    self.select_all();
                }
                if pill(ui, "Voltear H", false).clicked() {
                    self.picked_flip(true);
                }
                if pill(ui, "Voltear V", false).clicked() {
                    self.picked_flip(false);
                }
                if pill(ui, "Girar 90°", false).clicked() {
                    self.picked_rotate90();
                }
                if pill(ui, "Aplicar color", false).clicked() {
                    self.picked_recolor();
                }
                if pill(ui, "Borrar", false).clicked() {
                    self.picked_delete();
                }
            }
            if (t == Tool::Select && self.obj_mode) || t == Tool::RotateObj {
                let tr = self.layer().transform_at(self.frame);
                ui.label(RichText::new(format!("x {:.0}  y {:.0}  giro {:.0}°  escala {:.0} %", tr.x, tr.y, tr.r, tr.s * 100.0)).color(MU));
                if pill(ui, "+ Clave", self.layer().xform.contains_key(&self.frame)).on_hover_text("Insertar clave (K)").clicked() {
                    self.set_xkey(None);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let l = self.layer();
                ui.label(RichText::new(format!("Capa {}, {}", l.name, l.kind.label().to_lowercase())).size(11.5).color(MU));
            });
        });
    }

    /// Nombre del color actual si está en alguna paleta.
    fn color_name(&self) -> String {
        let c = rgba(self.color);
        self.project
            .palettes
            .iter()
            .flat_map(|p| p.colors.iter())
            .find(|s| s.color == c)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| hex(self.color))
    }

    // ---------- Columna derecha ----------

    pub(super) fn right_column(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let rows = self.project.layers.len() as f32;
        let lh = (28.0 + 8.0 + rows * 28.0 + 40.0).min(rect.height() * 0.45).max(150.0);
        let lr = Rect::from_min_size(rect.min, Vec2::new(rect.width(), lh));
        let pr = Rect::from_min_max(Pos2::new(rect.min.x, lr.max.y + 2.0), rect.max);
        shell::in_area(ui, lr, |ui| self.layers_area(ui));
        shell::in_area(ui, pr, |ui| self.props_area(ui));
    }

    fn layers_area(&mut self, ui: &mut egui::Ui) {
        area_header(ui, "Capas");
        let foot_h = 30.0;
        let list_h = (ui.available_height() - foot_h).max(30.0);
        let mut pick = None;
        let mut vis = None;
        let mut lock = None;
        let mut plane = None;
        egui::ScrollArea::vertical().id_salt("layers_list").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            for (i, l) in self.project.layers.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
                let r = r.shrink2(Vec2::new(4.0, 1.0));
                let on = i == self.cl;
                let p = ui.painter();
                if on {
                    p.rect_filled(r, 4.0, SEL_BG);
                    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(2.0, r.height())), 0.0, ACCENT);
                } else if resp.hovered() {
                    p.rect_filled(r, 4.0, BG3);
                }
                let kt = if l.kind == LayerKind::Vector { if on { ACCENT } else { ACCENT2 } } else { c32(l.kind.tint()) };
                p.text(r.left_center() + Vec2::new(10.0, 0.0), Align2::LEFT_CENTER, l.kind.badge(), FontId::proportional(10.0), kt);
                p.text(r.left_center() + Vec2::new(26.0, 0.0), Align2::LEFT_CENTER, &l.name, FontId::proportional(12.5), if l.visible { TX } else { INK3 });
                if !l.xform.is_empty() {
                    let br = Rect::from_center_size(Pos2::new(r.max.x - 66.0, r.center().y), Vec2::new(22.0, 14.0));
                    p.rect_filled(br, 3.0, BG4);
                    p.text(br.center(), Align2::CENTER_CENTER, "KF", FontId::proportional(9.0), KEY_GOLD);
                }
                // Etiqueta de plano: 2D, BG, MG o FG (clic: activa el 3D o cambia de plano).
                let chip = Rect::from_center_size(Pos2::new(r.max.x - 100.0, r.center().y), Vec2::new(28.0, 14.0));
                if l.kind != LayerKind::Camera {
                    let (txt, col) = if l.is3d {
                        (crate::model::plane_name(l.plane), match l.plane { 0 => Color32::from_rgb(0x8f, 0xb3, 0xcf), 2 => Color32::from_rgb(0xa8, 0xcf, 0x8f), _ => Color32::from_rgb(0xf0, 0xb0, 0x80) })
                    } else {
                        ("2D", INK3)
                    };
                    p.rect_filled(chip, 3.0, BG4);
                    p.text(chip.center(), Align2::CENTER_CENTER, txt, semi(9.0), col);
                }
                let eye = Rect::from_center_size(Pos2::new(r.max.x - 38.0, r.center().y), Vec2::splat(20.0));
                let lk = Rect::from_center_size(Pos2::new(r.max.x - 14.0, r.center().y), Vec2::splat(20.0));
                draw_eye(p, eye, l.visible, false);
                draw_lock(p, lk, l.locked, false);
                if resp.clicked() {
                    let at = resp.interact_pointer_pos().unwrap_or_default();
                    if eye.contains(at) {
                        vis = Some(i);
                    } else if chip.contains(at) && l.kind != LayerKind::Camera {
                        plane = Some(i);
                    } else if lk.contains(at) {
                        lock = Some(i);
                    } else {
                        pick = Some(i);
                    }
                }
                if resp.double_clicked() {
                    self.renaming = None;
                }
            }
        });
        if let Some(i) = vis {
            self.project.layers[i].visible = !self.project.layers[i].visible;
        }
        if let Some(i) = plane {
            self.cycle_plane(i);
        }
        if let Some(i) = lock {
            self.project.layers[i].locked = !self.project.layers[i].locked;
        }
        if let Some(i) = pick {
            self.cl = i;
        }
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.x = 4.0;
            if ui.small_button("+ Vector").on_hover_text("Nueva capa vectorial (Mayús+N)").clicked() {
                self.add_layer();
            }
            if ui.small_button("+ Mapa de bits").on_hover_text("Nueva capa de píxeles: pincel, lápiz, formas y borrador pintan píxeles").clicked() {
                self.add_layer_kind(LayerKind::Bitmap);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if ui.small_button("Eliminar").on_hover_text("Eliminar capa").clicked() {
                    self.delete_layer();
                }
            });
        });
    }

    /// Área de propiedades: su cabecera cambia el tipo de editor, como en el prototipo.
    fn props_area(&mut self, ui: &mut egui::Ui) {
        let def = self.room_kind().def();
        let ed = self.props_editor.unwrap_or(Editor::RoomProps);
        let title = if ed == Editor::RoomProps { def.right.title() } else { ed.name() };
        let hr = area_header(ui, "");
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(hr.shrink2(Vec2::new(4.0, 2.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let mut chosen = ed;
        egui::ComboBox::from_id_salt("props_editor")
            .selected_text(RichText::new(title).color(TX))
            .width(200.0)
            .show_ui(&mut child, |ui| {
                for e in Editor::ALL {
                    let n = if e == Editor::RoomProps { format!("Propiedades de la sala ({})", def.right.title()) } else { e.name().to_string() };
                    ui.selectable_value(&mut chosen, e, n);
                }
            });
        if chosen != ed {
            self.props_editor = if chosen == Editor::RoomProps { None } else { Some(chosen) };
        }
        egui::ScrollArea::vertical().id_salt(("props", title)).auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::default().inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 6.0;
                match chosen {
                    Editor::RoomProps => self.room_props(ui, def.right),
                    Editor::Palettes => self.palette_panel(ui),
                    Editor::Curves => self.curves_panel(ui),
                    Editor::Explorer => self.browser_room(ui),
                    Editor::Schematic => self.schematic_room(ui),
                    Editor::Substitution => self.substitution_panel(ui),
                    Editor::Preview => self.preview_panel(ui),
                }
            });
        });
    }

    fn room_props(&mut self, ui: &mut egui::Ui, k: RightKind) {
        match k {
            RightKind::Color => {
                self.palette_groups(ui);
                ui.add_space(6.0);
                panel_header(ui, "Dibujos de la capa");
                self.level_strip(ui);
            }
            RightKind::Palettes => self.palette_panel(ui),
            RightKind::Interp => {
                panel_header(ui, "Claves de la capa");
                self.anim_panel(ui);
                ui.add_space(6.0);
                panel_header(ui, "Capa");
                self.column_panel(ui);
            }
            RightKind::Rig => {
                pending_note(ui, "Rigging", "Huesos, deformadores y piezas articuladas para cutout están pendientes. Por ahora puedes animar piezas como capas separadas con claves (modo objeto) y cambiar bocas, manos u ojos con la sustitución de dibujo.");
                panel_header(ui, "Sustitución de dibujo");
                self.substitution_panel(ui);
                panel_header(ui, "Claves de la capa");
                self.anim_panel(ui);
            }
            RightKind::Comp => {
                muted(ui, "Vista 3D: clic central o Alt+arrastrar orbita; rueda acerca; Num 1, 3, 7 y 0 cambian de vista. La etiqueta 2D / BG / MG / FG de cada capa la pone en profundidad. Mayús+A agrega una Cámara. La luz y la profundidad de campo quedan pendientes.");
                panel_header(ui, "Capa");
                self.column_panel(ui);
                panel_header(ui, "Esquema");
                self.schematic_room(ui);
            }
            RightKind::StoryPdf => self.story_side(ui),
            RightKind::Animatic => self.animatic_side(ui),
            RightKind::Export => self.render_room(ui),
        }
    }

    /// Editor de curvas: la posición X y Y de la capa a lo largo del tiempo, con sus claves.
    fn curves_panel(&mut self, ui: &mut egui::Ui) {
        let l = self.layer();
        let n = self.project.length.max(2);
        let xs: Vec<f32> = (0..n).map(|f| l.transform_at(f).x).collect();
        let ys: Vec<f32> = (0..n).map(|f| l.transform_at(f).y).collect();
        let keys: Vec<u32> = l.xform.keys().copied().collect();
        let (mn, mx) = xs.iter().chain(ys.iter()).fold((f32::MAX, f32::MIN), |a, v| (a.0.min(*v), a.1.max(*v)));
        let (mn, mx) = if mx - mn < 1.0 { (mn - 10.0, mx + 10.0) } else { (mn - 10.0, mx + 10.0) };
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 170.0), Sense::hover());
        let p = ui.painter_at(r);
        p.rect_filled(r, 4.0, BG);
        for i in 0..7 {
            let y = r.min.y + 10.0 + i as f32 * (r.height() - 20.0) / 6.0;
            p.hline(r.x_range(), y, stk(1.0, LINE));
        }
        let fx = |f: u32| r.min.x + 8.0 + f as f32 / (n - 1) as f32 * (r.width() - 16.0);
        let fy = |v: f32| r.max.y - 10.0 - (v - mn) / (mx - mn) * (r.height() - 20.0);
        for (vals, col) in [(&xs, ACCENT), (&ys, ONION_AFTER)] {
            let pts: Vec<Pos2> = vals.iter().enumerate().map(|(f, v)| Pos2::new(fx(f as u32), fy(*v))).collect();
            p.add(Shape::line(pts, stk(1.6, col)));
        }
        for k in keys {
            if k < n {
                for v in [xs[k as usize], ys[k as usize]] {
                    let c = Pos2::new(fx(k), fy(v));
                    p.add(Shape::convex_polygon(vec![c + Vec2::new(0.0, -5.0), c + Vec2::new(5.0, 0.0), c + Vec2::new(0.0, 5.0), c + Vec2::new(-5.0, 0.0)], KEY_GOLD, EStroke::NONE));
                }
            }
        }
        p.vline(fx(self.frame), r.y_range(), stk(1.0, ACCENT2));
        ui.horizontal(|ui| {
            ui.label(RichText::new("■ X").color(ACCENT));
            ui.label(RichText::new("■ Y").color(ONION_AFTER));
            muted(ui, "· rombos: claves");
        });
        muted(ui, "Edición de tiradores Bézier: pendiente. Cambia la interpolación de cada clave en la sala Interpolación.");
    }

    pub(super) fn anim_panel(&mut self, ui: &mut egui::Ui) {
        let f = self.frame;
        let layer = self.layer();
        let is_key = layer.xform.contains_key(&f);
        let mut t = layer.transform_at(f);
        muted(ui, format!("Capa «{}» · cuadro {}{}", layer.name, f + 1, if is_key { " · CLAVE" } else { "" }));
        let before = t;
        egui::Grid::new("anim_grid").num_columns(2).spacing([8.0, 5.0]).show(ui, |ui| {
            let row = |ui: &mut egui::Ui, label: &str, v: &mut f32, speed: f64| {
                ui.label(RichText::new(label).color(MU));
                ui.add(egui::DragValue::new(v).speed(speed).max_decimals(2));
                ui.end_row();
            };
            row(ui, "Posición X", &mut t.x, 1.0);
            row(ui, "Posición Y", &mut t.y, 1.0);
            row(ui, "Rotación °", &mut t.r, 1.0);
            row(ui, "Escala", &mut t.s, 0.01);
            row(ui, "Opacidad", &mut t.o, 0.01);
            ui.label(RichText::new("Interpolación").color(MU));
            let names = |i: Interp| match i {
                Interp::Linear => "Lineal",
                Interp::Ease => "Suave",
                Interp::Step => "Escalón",
            };
            egui::ComboBox::from_id_salt("interp").selected_text(names(t.i)).show_ui(ui, |ui| {
                for i in [Interp::Linear, Interp::Ease, Interp::Step] {
                    ui.selectable_value(&mut t.i, i, names(i));
                }
            });
            ui.end_row();
        });
        t.o = t.o.clamp(0.0, 1.0);
        if t != before {
            self.set_xkey(Some(t));
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if pill(ui, "+ Clave", is_key).on_hover_text("Insertar clave en este cuadro (K)").clicked() {
                self.set_xkey(None);
            }
            if pill(ui, "Borrar clave", false).clicked() {
                self.delete_xkey();
            }
        });
        muted(ui, "En modo objeto (Tab), Seleccionar (V) mueve la capa y Rotación (W) la gira; cada cambio crea una clave.");
    }

    pub(super) fn column_panel(&mut self, ui: &mut egui::Ui) {
        let cl = self.cl;
        let layer = &mut self.project.layers[cl];
        ui.horizontal(|ui| {
            ui.label(RichText::new("Nombre").color(MU));
            ui.add(egui::TextEdit::singleline(&mut layer.name).desired_width(150.0));
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut layer.visible, "Visible");
            ui.checkbox(&mut layer.locked, "Bloqueada");
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Opacidad").color(MU));
            ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
        });
    }

    pub(super) fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let w = ui.available_width();
        let h = w * self.project.height as f32 / self.project.width as f32;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
        let painter = ui.painter_at(rect);
        let z = w / self.project.width as f32;
        self.paint_scene(&painter, rect.min, z, self.frame, false, false);
        painter.rect_stroke(rect, 0.0, stk(1.0, Color32::BLACK), EStrokeKind::Outside);
        if resp.on_hover_text("Clic: reproducir / pausar").clicked() {
            self.toggle_play();
        }
    }

    // ---------- Barra lateral N (Ítem, Herramienta, Vista) ----------

    pub(super) fn n_sidebar(&mut self, ui: &mut egui::Ui) {
        let tabs = ["Ítem", "Herramienta", "Vista"];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, t) in tabs.iter().enumerate() {
                if pill(ui, t, self.n_tab == i as u8).clicked() {
                    self.n_tab = i as u8;
                }
            }
        });
        ui.add_space(4.0);
        // Fila del prototipo: etiqueta a la izquierda y valor dentro de un campo oscuro con borde.
        let row = |ui: &mut egui::Ui, k: &str, v: String| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
            let p = ui.painter();
            p.text(r.left_center() + Vec2::new(2.0, 0.0), Align2::LEFT_CENTER, k, FontId::proportional(11.5), MU);
            let fw = (r.width() * 0.62).max(80.0);
            let fr = Rect::from_min_max(Pos2::new(r.max.x - fw, r.min.y + 2.0), Pos2::new(r.max.x, r.max.y - 2.0));
            p.rect_filled(fr, 4.0, UI0);
            p.rect_stroke(fr, 4.0, stk(1.0, LINE), EStrokeKind::Inside);
            p.text(fr.left_center() + Vec2::new(7.0, 0.0), Align2::LEFT_CENTER, v, FontId::proportional(11.5), TX);
        };
        match self.n_tab {
            0 => {
                let l = self.layer();
                let key = l.key_at_or_before(self.frame);
                let num = key.and_then(|k| l.drawing_number(k)).map(|n| format!("#{}", n + 1)).unwrap_or_else(|| "vacío".into());
                let strokes = l.drawing_at(self.frame).map_or(0, |d| d.strokes.len());
                panel_header(ui, "Cuadro actual");
                row(ui, "Cuadro", format!("{} de {}", self.frame + 1, self.project.length));
                row(ui, "Dibujo", num);
                row(ui, "Trazos", strokes.to_string());
                let t = l.transform_at(self.frame);
                panel_header(ui, "Transformación");
                row(ui, "Posición X", format!("{:.0} px", t.x));
                row(ui, "Posición Y", format!("{:.0} px", t.y));
                row(ui, "Rotación", format!("{:.0}°", t.r));
                row(ui, "Escala", format!("{:.0} %", t.s * 100.0));
                row(ui, "Opacidad", format!("{:.0} %", t.o * 100.0));
                panel_header(ui, "Capa 3D");
                row(ui, "Modo", if l.is3d { format!("3D, plano {}", crate::model::plane_name(l.plane)) } else { "2D".into() });
                row(ui, "Distancia Y", if l.is3d || l.kind == LayerKind::Camera { format!("{:.0}", l.depth) } else { "—".into() });
            }
            1 => {
                // Los parámetros de la herramienta elegida se configuran aquí.
                let t = self.tool;
                panel_header(ui, t.name());
                row(ui, "Atajo", t.letter().into());
                if t.draws() {
                    let size = match t {
                        Tool::Brush => &mut self.brush_size,
                        Tool::Vector => &mut self.pencil_size,
                        Tool::Eraser => &mut self.eraser_size,
                        _ => &mut self.shape_width,
                    };
                    param_row(ui, if t == Tool::Brush { "Tamaño" } else { "Grosor" }, |ui| {
                        ui.add(egui::Slider::new(size, 1.0..=300.0).logarithmic(true).max_decimals(0).suffix(" px"));
                    });
                    if t != Tool::Eraser {
                        param_row(ui, "Opacidad", |ui| pct_slider(ui, &mut self.stroke_opacity, 15.0, 100.0));
                    }
                    if t == Tool::Brush {
                        param_row(ui, "Dureza", |ui| pct_slider(ui, &mut self.brush.hardness, 0.0, 100.0));
                        param_row(ui, "Espaciado", |ui| {
                            ui.add(egui::Slider::new(&mut self.brush.spacing, 1.0..=200.0).logarithmic(true).max_decimals(0).suffix(" %"));
                        });
                        param_row(ui, "Flujo", |ui| pct_slider(ui, &mut self.brush.flow, 1.0, 100.0));
                        param_row(ui, "Redondez", |ui| pct_slider(ui, &mut self.brush.roundness, 5.0, 100.0));
                        param_row(ui, "Ángulo", |ui| {
                            ui.add(egui::Slider::new(&mut self.brush.angle, 0.0..=360.0).max_decimals(0).suffix("°"));
                        });
                    }
                    if t != Tool::Eraser {
                        param_row(ui, "Suavizado", |ui| pct_slider(ui, &mut self.smoothing, 0.0, 85.0));
                    }
                    param_row(ui, "Presión", |ui| {
                        ui.checkbox(&mut self.pressure, "Activa");
                    });
                    if t == Tool::Brush {
                        self.brush_preview(ui);
                        self.brush_sections(ui);
                        if self.layer().kind != LayerKind::Bitmap {
                            muted(ui, "Dureza, espaciado, flujo, redondez y ángulo actúan en capas de mapa de bits. En capas vectoriales el pincel solo usa tamaño, opacidad y suavizado.");
                        }
                        if ui.small_button("Restablecer pincel").on_hover_text("Vuelve a la punta redonda y dura").clicked() {
                            self.brush = Default::default();
                        }
                    }
                    if t == Tool::Shape {
                        for (k, n) in [(ShapeKind::Line, "Línea"), (ShapeKind::Rect, "Rectángulo"), (ShapeKind::Ellipse, "Elipse")] {
                            if pill(ui, n, self.shape_kind == k).clicked() {
                                self.shape_kind = k;
                            }
                        }
                        if self.shape_kind != ShapeKind::Line {
                            ui.checkbox(&mut self.fill_shapes, "Rellenar");
                        }
                    }
                }
                muted(ui, t.tip());
            }
            _ => {
                panel_header(ui, "Vista");
                row(ui, "Zoom", format!("{:.0} %", self.zoom * 100.0));
                row(ui, "Papel cebolla", if self.onion { format!("{} atrás · {} adelante", self.onion_before, self.onion_after) } else { "Inactivo".into() });
                row(ui, "Fondo de la mesa", format!("{:.0}", self.light_gray));
                if pill(ui, "Ajustar a la vista", false).clicked() {
                    self.need_fit = true;
                }
            }
        }
    }
}

/// Fila de parámetro de herramienta: etiqueta a la izquierda y control dentro de un campo (panel N).
fn param_row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    ui.painter().text(r.left_center() + Vec2::new(2.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(11.5), MU);
    let fw = (r.width() * 0.6).max(90.0);
    let fr = Rect::from_min_max(Pos2::new(r.max.x - fw, r.min.y + 1.0), Pos2::new(r.max.x, r.max.y - 1.0));
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(fr).layout(egui::Layout::left_to_right(egui::Align::Center)));
    c.spacing_mut().slider_width = (fw - 60.0).max(40.0);
    c.spacing_mut().interact_size.y = 22.0;
    add(&mut c);
}

/// Lista para elegir qué controla una propiedad dinámica del pincel.
fn control_combo(ui: &mut egui::Ui, id: &str, c: &mut crate::brush::Control) {
    egui::ComboBox::from_id_salt(id).selected_text(c.label()).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
        for k in crate::brush::Control::ALL {
            ui.selectable_value(c, k, k.label());
        }
    });
}

/// Deslizador de un valor 0…1 que se muestra como porcentaje entre `lo` y `hi`.
fn pct_slider(ui: &mut egui::Ui, v: &mut f32, lo: f32, hi: f32) {
    let mut p = (*v * 100.0).clamp(lo, hi);
    if ui.add(egui::Slider::new(&mut p, lo..=hi).max_decimals(0).suffix(" %")).changed() {
        *v = p / 100.0;
    }
}

/// Sección plegable del panel N (cerrada al principio).
fn fold(ui: &mut egui::Ui, id: &str, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).font(semi(12.0)).color(TX)).id_salt(id).default_open(std::env::var("LEONTOON_FOLDS").is_ok()).show(ui, add);
}

impl LeonApp {
    /// Especificación del pincel actual para el motor (con las imágenes de punta y de textura cargadas).
    pub(super) fn brush_spec(&self, color: [u8; 3], size: f32, seed: u64) -> crate::brush::BrushSpec {
        let [r, g, b, _] = rgba(self.fill_color);
        crate::brush::BrushSpec::new(size, self.stroke_opacity, color, self.brush, seed)
            .with_color2([r, g, b])
            .with_images(self.brush_tip_img.as_ref().map(|(_, g)| g.clone()), self.brush_tex_img.as_ref().map(|(_, g)| g.clone()))
    }

    /// Pide una imagen PNG o WebP y la carga como punta o como textura del pincel.
    fn pick_brush_image(&mut self, tip: bool) {
        let Some(path) = rfd::FileDialog::new().add_filter("Imagen (PNG o WebP)", &["png", "webp"]).pick_file() else { return };
        match crate::brush::load_gray(&path) {
            Ok(g) => {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let msg = format!("{} cargada: {name} ({}×{})", if tip { "Punta" } else { "Textura" }, g.w, g.h);
                if tip {
                    self.brush_tip_img = Some((name, std::sync::Arc::new(g)));
                    self.brush.use_tip_image = true;
                } else {
                    self.brush_tex_img = Some((name, std::sync::Arc::new(g)));
                    self.brush.texture = crate::brush::TexKind::Image;
                }
                self.say(msg);
            }
            Err(e) => self.say(e),
        }
    }

    /// Dinámica de forma, transferencia, dispersión, textura, doble pincel, color y bordes húmedos.
    fn brush_sections(&mut self, ui: &mut egui::Ui) {
        use crate::brush::{Control, TexKind, TexMode};
        fold(ui, "fold_shape", "Dinámica de forma", |ui| {
            param_row(ui, "Var. tamaño", |ui| pct_slider(ui, &mut self.brush.size_jitter, 0.0, 100.0));
            param_row(ui, "Ctrl. tamaño", |ui| control_combo(ui, "ctl_size", &mut self.brush.size_control));
            if self.brush.size_control != Control::Off {
                param_row(ui, "Tamaño mín.", |ui| pct_slider(ui, &mut self.brush.min_size, 0.0, 100.0));
            }
            param_row(ui, "Var. ángulo", |ui| pct_slider(ui, &mut self.brush.angle_jitter, 0.0, 100.0));
            param_row(ui, "Var. redond.", |ui| pct_slider(ui, &mut self.brush.roundness_jitter, 0.0, 100.0));
            if self.brush.roundness_jitter > 0.0 {
                param_row(ui, "Redond. mín.", |ui| pct_slider(ui, &mut self.brush.min_roundness, 5.0, 100.0));
            }
        });
        fold(ui, "fold_transfer", "Transferencia", |ui| {
            param_row(ui, "Var. opac.", |ui| pct_slider(ui, &mut self.brush.opacity_jitter, 0.0, 100.0));
            param_row(ui, "Ctrl. opac.", |ui| control_combo(ui, "ctl_opacity", &mut self.brush.opacity_control));
            param_row(ui, "Var. flujo", |ui| pct_slider(ui, &mut self.brush.flow_jitter, 0.0, 100.0));
            param_row(ui, "Ctrl. flujo", |ui| control_combo(ui, "ctl_flow", &mut self.brush.flow_control));
            muted(ui, "El control por presión usa el lápiz de tableta (la vista previa simula una presión que sube y baja). La inclinación solo actúa si el dispositivo la entrega; hoy el programa no la recibe, y sin dato no cambia nada.");
        });
        fold(ui, "fold_scatter", "Dispersión", |ui| {
            param_row(ui, "Dispersión", |ui| {
                let mut p = self.brush.scatter * 100.0;
                if ui.add(egui::Slider::new(&mut p, 0.0..=1000.0).logarithmic(true).max_decimals(0).suffix(" %")).changed() {
                    self.brush.scatter = p / 100.0;
                }
            });
            param_row(ui, "Ejes", |ui| {
                ui.checkbox(&mut self.brush.scatter_both, "Ambos ejes");
            });
            param_row(ui, "Cantidad", |ui| {
                let mut n = self.brush.scatter_count as f32;
                if ui.add(egui::Slider::new(&mut n, 1.0..=16.0).max_decimals(0)).changed() {
                    self.brush.scatter_count = n.round() as u8;
                }
            });
            param_row(ui, "Var. cantidad", |ui| pct_slider(ui, &mut self.brush.count_jitter, 0.0, 100.0));
            if self.brush.scatter <= 0.0 {
                muted(ui, "Sube la dispersión para que las marcas se repartan; la cantidad solo actúa con dispersión.");
            }
        });
        fold(ui, "fold_texture", "Textura", |ui| {
            param_row(ui, "Patrón", |ui| {
                let before = self.brush.texture;
                egui::ComboBox::from_id_salt("tex_kind").selected_text(self.brush.texture.label()).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                    for k in TexKind::ALL {
                        ui.selectable_value(&mut self.brush.texture, k, k.label());
                    }
                });
                if self.brush.texture == TexKind::Image && before != TexKind::Image && self.brush_tex_img.is_none() {
                    self.brush.texture = before;
                    self.pick_brush_image(false);
                }
            });
            if self.brush.texture == TexKind::Image {
                let name = self.brush_tex_img.as_ref().map_or("ninguna".to_string(), |(n, _)| n.clone());
                ui.horizontal(|ui| {
                    if ui.small_button("Cargar imagen…").clicked() {
                        self.pick_brush_image(false);
                    }
                    ui.label(RichText::new(name).size(11.0).color(MU));
                });
            }
            if self.brush.texture != TexKind::None {
                param_row(ui, "Escala", |ui| pct_slider(ui, &mut self.brush.texture_scale, 10.0, 800.0));
                param_row(ui, "Profundidad", |ui| pct_slider(ui, &mut self.brush.texture_depth, 0.0, 100.0));
                param_row(ui, "Modo", |ui| {
                    egui::ComboBox::from_id_salt("tex_mode").selected_text(self.brush.texture_mode.label()).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                        for m in TexMode::ALL {
                            ui.selectable_value(&mut self.brush.texture_mode, m, m.label());
                        }
                    });
                });
                param_row(ui, "Comportamiento", |ui| {
                    ui.checkbox(&mut self.brush.texture_per_tip, "Por marca");
                });
            }
        });
        fold(ui, "fold_dual", "Doble pincel", |ui| {
            param_row(ui, "Activar", |ui| {
                ui.checkbox(&mut self.brush.dual, "Segunda punta");
            });
            if self.brush.dual {
                param_row(ui, "Tamaño", |ui| pct_slider(ui, &mut self.brush.dual_size, 5.0, 150.0));
                param_row(ui, "Cantidad", |ui| {
                    let mut n = self.brush.dual_count as f32;
                    if ui.add(egui::Slider::new(&mut n, 1.0..=40.0).max_decimals(0)).changed() {
                        self.brush.dual_count = n.round() as u8;
                    }
                });
                param_row(ui, "Reparto", |ui| pct_slider(ui, &mut self.brush.dual_spread, 0.0, 150.0));
            }
        });
        fold(ui, "fold_color", "Dinámica de color", |ui| {
            param_row(ui, "Princ./secund.", |ui| pct_slider(ui, &mut self.brush.fg_bg_jitter, 0.0, 100.0));
            param_row(ui, "Var. tono", |ui| pct_slider(ui, &mut self.brush.hue_jitter, 0.0, 100.0));
            param_row(ui, "Var. saturación", |ui| pct_slider(ui, &mut self.brush.sat_jitter, 0.0, 100.0));
            param_row(ui, "Var. brillo", |ui| pct_slider(ui, &mut self.brush.bri_jitter, 0.0, 100.0));
            muted(ui, "El color secundario es el de relleno de las formas.");
        });
        fold(ui, "fold_other", "Punta y bordes", |ui| {
            param_row(ui, "Punta", |ui| {
                ui.checkbox(&mut self.brush.use_tip_image, "Usar imagen");
            });
            if self.brush.use_tip_image {
                if self.brush_tip_img.is_none() {
                    self.pick_brush_image(true);
                    if self.brush_tip_img.is_none() {
                        self.brush.use_tip_image = false;
                    }
                }
                let name = self.brush_tip_img.as_ref().map_or("ninguna".to_string(), |(n, _)| n.clone());
                ui.horizontal(|ui| {
                    if ui.small_button("Cargar punta…").clicked() {
                        self.pick_brush_image(true);
                    }
                    ui.label(RichText::new(name).size(11.0).color(MU));
                });
                muted(ui, "Con una imagen, lo oscuro (o lo opaco, si tiene transparencia) es pintura. La dureza no se aplica.");
            }
            param_row(ui, "Bordes húmedos", |ui| pct_slider(ui, &mut self.brush.wet_edges, 0.0, 100.0));
        });
        if self.brush.has_dynamics() {
            ui.label(RichText::new("Dinámicas activas en este pincel").size(11.0).color(ACCENT2));
        }
        if self.brush.has_effects() {
            ui.label(RichText::new("Efectos activos en este pincel").size(11.0).color(ACCENT2));
        }
    }

    /// Muestra un trazo de muestra con el pincel actual, hecho con el mismo motor que pinta en la escena.
    fn brush_preview(&mut self, ui: &mut egui::Ui) {
        let colorful = self.brush.hue_jitter > 0.0 || self.brush.sat_jitter > 0.0 || self.brush.bri_jitter > 0.0 || self.brush.fg_bg_jitter > 0.0;
        let spec = self.brush_spec(if colorful { [240, 130, 60] } else { [232, 232, 232] }, self.brush_size.min(34.0), 7).with_color2(if colorful { [60, 150, 240] } else { [232, 232, 232] });
        let names = (self.brush_tip_img.as_ref().map(|(n, _)| n.clone()), self.brush_tex_img.as_ref().map(|(n, _)| n.clone()));
        let key = format!("{:.2}|{:.2}|{:?}|{:?}", spec.size, spec.opacity, spec.params, names);
        if self.brush_prev.as_ref().map(|(k, _)| k) != Some(&key) {
            let r = crate::brush::preview(spec, 240, 60);
            let img = egui::ColorImage::from_rgba_premultiplied([240, 60], &r.data);
            match &mut self.brush_prev {
                Some((k, tex)) => {
                    tex.set(img, egui::TextureOptions::LINEAR);
                    *k = key;
                }
                None => self.brush_prev = Some((key, ui.ctx().load_texture("brush_prev", img, egui::TextureOptions::LINEAR))),
            }
        }
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 64.0), Sense::hover());
        ui.painter().rect_filled(r, 4.0, UI0);
        ui.painter().rect_stroke(r, 4.0, stk(1.0, LINE), EStrokeKind::Inside);
        if let Some((_, tex)) = &self.brush_prev {
            let img = Rect::from_center_size(r.center(), Vec2::new(240.0, 60.0).min(r.size() - Vec2::splat(4.0)));
            ui.painter().image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
    }
}

fn pending_note(ui: &mut egui::Ui, title: &str, text: &str) {
    egui::Frame::default().fill(BG3).corner_radius(6.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(format!("{title}: pendiente")).strong().color(ACCENT2));
        muted(ui, text);
    });
}
