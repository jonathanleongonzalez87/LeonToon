//! Armazón de la ventana (prototipo v0.3): barra de título con menús y pestañas de salas,
//! barra de estado con pistas, y las áreas de cada sala (herramientas, cabecera de herramienta,
//! mesa de luz, línea de tiempo u hoja, capas y propiedades).

use super::*;

/// Botón con el estilo del prototipo: gris, y naranja con letra oscura cuando está activo.
pub(super) fn pill(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let fill = if on { ACCENT } else { BG4 };
    let col = if on { ACCENT_INK } else { TX };
    ui.add(
        egui::Button::new(RichText::new(text).color(col).size(12.0))
            .fill(fill)
            .stroke(stk(1.0, if on { ACCENT } else { LINE_STRONG }))
            .corner_radius(4.0),
    )
}

/// Cabecera de área (28 px, gris más claro), con título.
pub(super) fn area_header(ui: &mut egui::Ui, title: &str) -> Rect {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), UiScale::AREA_HEADER), Sense::hover());
    ui.painter().rect_filled(rect, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, BG3);
    ui.painter().hline(rect.x_range(), rect.max.y, stk(1.0, LINE));
    ui.painter().text(rect.left_center() + Vec2::new(10.0, 0.0), Align2::LEFT_CENTER, title, semi(11.5), TX);
    rect
}

/// Cabecera de panel lateral (sección plegable con título en mayúsculas).
pub(super) fn panel_header(ui: &mut egui::Ui, title: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
    let c = rect.left_center() + Vec2::new(8.0, 0.0);
    // Triángulo de sección desplegada, como en el prototipo.
    tri(ui.painter(), c + Vec2::new(-3.0, -2.0), c + Vec2::new(3.0, -2.0), c + Vec2::new(0.0, 2.5), INK3);
    ui.painter().text(rect.left_center() + Vec2::new(18.0, 0.0), Align2::LEFT_CENTER, title, semi(11.5), TX);
}

pub(super) fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(11.0).color(MU));
}

/// Pinta el fondo de un área (gris, esquinas de 6 px) y da un Ui dentro.
pub(super) fn in_area<R>(ui: &mut egui::Ui, rect: Rect, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.painter().rect_filled(rect, 6.0, BG2);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    f(&mut child)
}

pub(super) const GUIDE_NAMES: [&str; 6] = [
    "Área segura de animación",
    "Área segura de texto",
    "Ley de tercios",
    "Composición diagonal",
    "Sección áurea",
    "Cruz de centro",
];

const TOOLHEAD_H: f32 = UiScale::TOOL_HEAD;

impl LeonApp {
    pub(super) fn shell(&mut self, ctx: &egui::Context) {
        // El animatic sigue a los planos: si cambiaron duraciones, transiciones, cámara u orden, se rehace.
        self.sync_animatic();
        self.title_bar(ctx);
        self.status_bar(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(UI0).inner_margin(egui::Margin::same(2)))
            .show(ctx, |ui| self.main_areas(ui));
    }

    fn main_areas(&mut self, ui: &mut egui::Ui) {
        let full = ui.max_rect();
        let gap = 2.0;
        let def = self.room_kind().def();
        let kind = self.room_kind();
        let sm_room = kind == RoomKind::StopMotion;
        // Tamaños de los paneles: los elegidos por la persona, ajustados a la ventana (Issue #44).
        let sz = self.sizes.effective(full.size());
        self.sz = sz;
        let right = Rect::from_min_max(Pos2::new(full.max.x - sz.right, full.min.y), full.max);
        // La cabecera y la barra de herramientas son del visor: solo están si el hueco principal es el visor (o la sala
        // Stop Motion, que siempre las tuvo). La barra entra y sale cambiando de ancho (T), con el contenido recortado.
        let toolhead = self.main_toolhead();
        let tb_t = panel_t(ui.ctx(), "toolbar", self.show_toolbar && toolhead);
        let left_w = (sz.toolbar + gap) * tb_t;
        let work = Rect::from_min_max(full.min, Pos2::new(right.min.x - gap, full.max.y));
        let pointer = ui.input(|i| i.pointer.hover_pos());
        self.hover_area = 0;

        // Columna derecha: Capas y Propiedades (dos huecos)
        let split_y = self.right_column(ui, right);

        // Cabecera de herramienta (ocupa también el ancho de la barra de herramientas)
        let below = if toolhead {
            let head = Rect::from_min_size(work.min, Vec2::new(work.width(), TOOLHEAD_H));
            in_area(ui, head, |ui| self.tool_head(ui));
            Rect::from_min_max(Pos2::new(work.min.x, head.max.y + gap), work.max)
        } else {
            work
        };
        if tb_t > 0.01 {
            let vis = Rect::from_min_size(below.min, Vec2::new(sz.toolbar * tb_t, below.height()));
            let full_r = Rect::from_min_size(below.min, Vec2::new(sz.toolbar, below.height()));
            ui.painter().rect_filled(vis, 6.0, BG2);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(full_r).layout(egui::Layout::top_down(egui::Align::Min)));
            child.set_clip_rect(vis.intersect(ui.clip_rect()));
            self.tool_column(&mut child);
        }
        let center = Rect::from_min_max(Pos2::new(below.min.x + left_w, below.min.y), below.max);
        if !sm_room && !kind.slots().contains(&Slot::Time) {
            // Sala sin línea de tiempo (Storyboard): el hueco principal ocupa todo el centro.
            self.sm_cap_leave();
            self.draw_slot(ui, center, Slot::Main);
            self.right_handles(ui, full, right, split_y, gap);
            return;
        }
        let (view, tl) = if self.xs_horizontal {
            let h = (if sz.timeline > 0.0 { sz.timeline } else { def.tl_h.max(150.0) }).max(panelsize::PanelSizes::TIMELINE_MIN).min(center.height() * 0.6);
            (
                Rect::from_min_max(center.min, Pos2::new(center.max.x, center.max.y - h - gap)),
                Rect::from_min_max(Pos2::new(center.min.x, center.max.y - h), center.max),
            )
        } else {
            let w = sz.sheet.min(center.width() * 0.6);
            (
                Rect::from_min_max(center.min, Pos2::new(center.max.x - w - gap, center.max.y)),
                Rect::from_min_max(Pos2::new(center.max.x - w, center.min.y), center.max),
            )
        };
        if let Some(p) = pointer {
            self.hover_area = if sm_room {
                if right.contains(p) { 3 } else { 0 }
            } else if view.contains(p) {
                1
            } else if tl.contains(p) {
                2
            } else if right.contains(p) {
                3
            } else {
                0
            };
        }
        if sm_room {
            // Sala Stop Motion: cuatro huecos (X-Sheet, escenario, cámara y tira de fotos) en todo el centro.
            self.sm_dragon(ui, center);
        } else {
            self.draw_slot(ui, view, Slot::Main);
            // Al salir de la sala se cierra la cámara: la luz de la webcam no debe quedar encendida.
            self.sm_cap_leave();
            self.draw_slot(ui, tl, Slot::Time);
        }
        // Bordes arrastrables (van al final para quedar por encima de las áreas): toolbar, línea de tiempo u hoja, y columna derecha.
        use panelsize::{resize_handle, PanelSizes, Resize};
        let mut save = false;
        if tb_t > 0.99 {
            let bar = Rect::from_center_size(Pos2::new(below.min.x + sz.toolbar + gap * 0.5, below.center().y), Vec2::new(8.0, below.height()));
            match resize_handle(ui, "toolbar", bar, true) {
                Resize::Drag(p) => self.sizes.toolbar = p.x - below.min.x - gap * 0.5,
                Resize::Reset => {
                    self.sizes.toolbar = PanelSizes::default().toolbar;
                    save = true;
                }
                Resize::Done => save = true,
                Resize::None => {}
            }
        }
        if sm_room {
            // La sala Stop Motion trae sus propios bordes arrastrables.
        } else if self.xs_horizontal {
            let bar = Rect::from_center_size(Pos2::new(tl.center().x, tl.min.y - gap * 0.5), Vec2::new(tl.width(), 8.0));
            match resize_handle(ui, "timeline", bar, false) {
                Resize::Drag(p) => self.sizes.timeline = center.max.y - p.y - gap * 0.5,
                Resize::Reset => {
                    self.sizes.timeline = 0.0;
                    save = true;
                }
                Resize::Done => save = true,
                Resize::None => {}
            }
        } else {
            let bar = Rect::from_center_size(Pos2::new(tl.min.x - gap * 0.5, tl.center().y), Vec2::new(8.0, tl.height()));
            match resize_handle(ui, "sheet", bar, true) {
                Resize::Drag(p) => self.sizes.sheet = center.max.x - p.x - gap * 0.5,
                Resize::Reset => {
                    self.sizes.sheet = PanelSizes::default().sheet;
                    save = true;
                }
                Resize::Done => save = true,
                Resize::None => {}
            }
        }
        if self.right_handles(ui, full, right, split_y, gap) || save {
            self.save_panel_sizes();
        }
    }

    /// Bordes arrastrables de la columna derecha: su ancho y el reparto entre capas y propiedades.
    /// Devuelve true si hay que guardar los tamaños.
    fn right_handles(&mut self, ui: &mut egui::Ui, full: Rect, right: Rect, split_y: f32, gap: f32) -> bool {
        use panelsize::{resize_handle, PanelSizes, Resize};
        let mut save = false;
        let bar = Rect::from_center_size(Pos2::new(right.min.x - gap * 0.5, right.center().y), Vec2::new(8.0, right.height()));
        match resize_handle(ui, "right", bar, true) {
            Resize::Drag(p) => self.sizes.right = full.max.x - p.x - gap * 0.5,
            Resize::Reset => {
                self.sizes.right = PanelSizes::default().right;
                save = true;
            }
            Resize::Done => save = true,
            Resize::None => {}
        }
        let bar = Rect::from_center_size(Pos2::new(right.center().x, split_y + 1.0), Vec2::new(right.width(), 8.0));
        match resize_handle(ui, "layers", bar, false) {
            Resize::Drag(p) => self.sizes.layers = p.y - right.min.y,
            Resize::Reset => {
                self.sizes.layers = 0.0;
                save = true;
            }
            Resize::Done => save = true,
            Resize::None => {}
        }
        save
    }

    fn title_bar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::default().fill(BG).inner_margin(egui::Margin::symmetric(6, 0)).stroke(stk(1.0, LINE));
        egui::TopBottomPanel::top("top").frame(frame).exact_height(32.0).show_separator_line(false).show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.set_height(32.0);
                // Marca: isotipo y nombre (abre el menú de LeonToon)
                let (r, _) = ui.allocate_exact_size(Vec2::new(20.0, 30.0), Sense::hover());
                start::draw_iso(ui.painter(), r.center(), 8.0, ACCENT);
                let brand = egui::text::LayoutJob::default();
                let mut brand = brand;
                brand.append("Leon", 0.0, egui::TextFormat { font_id: display(13.5), color: TX, ..Default::default() });
                brand.append("Toon", 0.0, egui::TextFormat { font_id: display(13.5), color: MU, ..Default::default() });
                ui.menu_button(brand, |ui| {
                    ui.set_min_width(230.0);
                    if ui.button("Pantalla de inicio").clicked() {
                        ui.close_menu();
                        self.stage = start::Stage::Home;
                    }
                    if ui.button("Acerca de LeonToon").clicked() {
                        ui.close_menu();
                        self.show_about = true;
                    }
                    ui.separator();
                    ui.menu_button("Sistema", |ui| {
                        if ui.button("Abrir carpeta de configuración").clicked() {
                            ui.close_menu();
                            if let Some(d) = safety::data_dir() {
                                let _ = std::process::Command::new("explorer").arg(d).spawn();
                            }
                        }
                        if ui.button("Comprobar ffmpeg").clicked() {
                            ui.close_menu();
                            self.ffmpeg = render::find_ffmpeg();
                            let m = match &self.ffmpeg {
                                Some(p) => format!("ffmpeg encontrado: {}", p.display()),
                                None => "No encuentro ffmpeg: el vídeo MP4 lo necesita (gratuito, ffmpeg.org).".into(),
                            };
                            self.say(m);
                        }
                    });
                });                self.menus(ctx, ui);
                self.room_tabs(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Escena 1").size(11.5).color(MU));
                    let name = format!("{}{}", if self.dirty { "• " } else { "" }, self.file_name());
                    ui.label(RichText::new(name).size(11.5).color(TX));
                });
            });
        });
    }

    /// Pestañas de salas: clic elige, doble clic renombra, ✕ o clic central cierra,
    /// arrastrar reordena, y «+» añade una sala o cambia el conjunto.
    fn room_tabs(&mut self, ui: &mut egui::Ui) {
        ui.add_space(14.0);
        ui.spacing_mut().item_spacing.x = 2.0;
        let mut go = None;
        let mut close = None;
        let mut start_rename = None;
        let mut rects = Vec::new();
        for i in 0..self.project.rooms.len() {
            let on = i == self.room_idx;
            if let Some((ri, buf)) = &mut self.tab_rename {
                if *ri == i {
                    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(110.0).font(FontId::proportional(12.5)));
                    r.request_focus();
                    if r.lost_focus() {
                        let name = buf.trim().to_string();
                        self.project.rooms[i].name = if name.is_empty() { None } else { Some(name) };
                        self.tab_rename = None;
                    }
                    rects.push(r.rect);
                    continue;
                }
            }
            let name = self.tab_name(i);
            let g = ui.painter().layout_no_wrap(name.clone(), FontId::proportional(12.5), TX);
            let w = g.size().x + 24.0 + if on { 16.0 } else { 0.0 };
            let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click_and_drag());
            let r = r.translate(Vec2::new(0.0, 2.0));
            let hov = resp.hovered();
            if on {
                ui.painter().rect_filled(r, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, UI0);
                ui.painter().hline(r.min.x + 6.0..=r.max.x - 6.0, r.min.y + 1.0, stk(2.0, ACCENT));
            } else if hov {
                ui.painter().rect_filled(r, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, BG3);
            }
            ui.painter().galley(Pos2::new(r.min.x + 12.0, r.center().y - g.size().y / 2.0), g, if on || hov { TX } else { MU });
            let xr = Rect::from_center_size(Pos2::new(r.max.x - 12.0, r.center().y), Vec2::splat(14.0));
            if on {
                let xh = ui.rect_contains_pointer(xr);
                ui.painter().text(xr.center(), Align2::CENTER_CENTER, "x", FontId::proportional(11.0), if xh { ACCENT } else { INK3 });
            }
            let def = RoomKind::from_id(&self.project.rooms[i].kind).map(|k| k.def().desc).unwrap_or("");
            let resp = resp.on_hover_text(format!("{def}\nDoble clic: renombrar · Clic central o x: cerrar · Arrastrar: reordenar"));
            if resp.clicked() {
                let at_x = resp.interact_pointer_pos().map_or(false, |p| on && xr.contains(p));
                if at_x {
                    close = Some(i);
                } else {
                    go = Some(i);
                }
            }
            if resp.middle_clicked() {
                close = Some(i);
            }
            if resp.double_clicked() {
                start_rename = Some(i);
            }
            if resp.drag_started() {
                self.tab_drag = Some(i);
            }
            rects.push(r);
        }
        // Reordenar arrastrando
        if let Some(from) = self.tab_drag {
            if let Some(p) = ui.input(|i| i.pointer.hover_pos()) {
                if let Some(to) = rects.iter().position(|r| r.x_range().contains(p.x)) {
                    if to != from && ui.input(|i| i.pointer.any_down()) {
                        let t = self.project.rooms.remove(from);
                        self.project.rooms.insert(to, t);
                        if self.room_idx == from {
                            self.room_idx = to;
                        } else if from < self.room_idx && to >= self.room_idx {
                            self.room_idx -= 1;
                        } else if from > self.room_idx && to <= self.room_idx {
                            self.room_idx += 1;
                        }
                        self.tab_drag = Some(to);
                    }
                }
            }
            if !ui.input(|i| i.pointer.any_down()) {
                self.tab_drag = None;
            }
        }
        if let Some(i) = start_rename {
            self.tab_rename = Some((i, self.tab_name(i)));
        }
        if let Some(i) = close {
            self.close_room(i);
        } else if let Some(i) = go {
            if i != self.room_idx {
                self.set_room_idx(i);
            }
        }
        // «+»: añadir sala o cambiar el conjunto de salas
        ui.menu_button(RichText::new("+").size(16.0).color(MU), |ui| {
            ui.set_min_width(220.0);
            self.add_room_menu(ui);
        })
        .response
        .on_hover_text("Agregar sala");
    }

    fn add_room_menu(&mut self, ui: &mut egui::Ui) {
        for (group, kinds) in ROOM_GROUPS {
            ui.label(RichText::new(group).size(11.0).color(INK3));
            for k in kinds.iter() {
                if ui.button(k.def().name).on_hover_text(k.def().desc).clicked() {
                    ui.close_menu();
                    self.project.rooms.push(crate::model::RoomTab { kind: k.def().id.into(), name: None });
                    let i = self.project.rooms.len() - 1;
                    self.set_room_idx(i);
                }
            }
        }
        ui.separator();
        ui.menu_button("Conjunto de salas", |ui| self.room_set_menu(ui));
    }

    fn room_set_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Reemplaza las salas abiertas").size(11.0).color(INK3));
        for s in ROOM_SETS.iter() {
            if ui.button(s.name).clicked() {
                ui.close_menu();
                self.project.room_set = s.id.into();
                self.project.rooms = start::tabs_for_set(s.id);
                self.set_room_idx(0);
                self.say(format!("Conjunto de salas: {}", s.name));
            }
        }
    }

    fn close_room(&mut self, i: usize) {
        if self.project.rooms.len() <= 1 {
            self.say("Tiene que quedar al menos una sala abierta");
            return;
        }
        self.project.rooms.remove(i);
        let ni = if self.room_idx > i { self.room_idx - 1 } else { self.room_idx.min(self.project.rooms.len() - 1) };
        self.set_room_idx(ni);
    }

    fn menus(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let item = |ui: &mut egui::Ui, label: &str, key: &str| -> bool {
            let r = ui.add(egui::Button::new(label).shortcut_text(key)).clicked();
            if r {
                ui.close_menu();
            }
            r
        };
        ui.menu_button("Archivo", |ui| {
            ui.set_min_width(240.0);
            if item(ui, "Nuevo proyecto", "Ctrl+N") {
                self.stage = start::Stage::Home;
            }
            if item(ui, "Abrir…", "Ctrl+O") {
                self.request(ctx, Pending::Open);
            }
            ui.menu_button("Abrir reciente", |ui| {
                if self.recent.is_empty() {
                    ui.label(RichText::new("Nada todavía").color(INK3));
                }
                for r in self.recent.clone() {
                    let n = r.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    if ui.button(n).on_hover_text(r.display().to_string()).clicked() {
                        ui.close_menu();
                        self.request(ctx, Pending::OpenPath(r));
                    }
                }
            });
            ui.separator();
            if item(ui, "Guardar", "Ctrl+S") {
                self.save();
            }
            if item(ui, "Guardar como…", "Ctrl+Mayús+S") {
                self.save_as();
            }
            ui.separator();
            if item(ui, "Recuperar copia automática…", "") {
                self.request(ctx, Pending::Recover);
            }
            if item(ui, "Volver a la versión anterior guardada", "") {
                self.open_backup();
            }
            if item(ui, "Exportar…", "Ctrl+E") {
                self.goto_room_kind(RoomKind::Exportar);
            }
            ui.separator();
            if item(ui, "Salir", "Ctrl+Q") {
                self.request(ctx, Pending::Quit);
            }
        });
        ui.menu_button("Editar", |ui| {
            ui.set_min_width(240.0);
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Deshacer").shortcut_text("Ctrl+Z")).clicked() {
                ui.close_menu();
                self.do_undo();
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Rehacer").shortcut_text("Ctrl+Mayús+Z")).clicked() {
                ui.close_menu();
                self.do_redo();
            }
            ui.separator();
            if item(ui, "Copiar", "Ctrl+C") {
                if !(self.tool == Tool::Select && !self.obj_mode && self.picked_copy()) {
                    self.copy_drawing();
                }
            }
            if item(ui, "Pegar", "Ctrl+V") {
                if self.tool == Tool::Select && !self.obj_mode {
                    self.picked_paste();
                } else {
                    self.paste_drawing();
                }
            }
            ui.separator();
            if item(ui, "Seleccionar todo", "Ctrl+A") {
                self.obj_mode = false;
                self.tool = Tool::Select;
                self.select_all();
            }
        });
        ui.menu_button("Dibujo", |ui| {
            ui.set_min_width(240.0);
            if item(ui, "Nueva capa vectorial", "Mayús+N") {
                self.add_layer();
            }
            if item(ui, "Nueva capa de mapa de bits", "") {
                self.add_layer_kind(LayerKind::Bitmap);
            }
            ui.separator();
            if item(ui, "Limpiar cuadro", "") {
                self.clear_drawing();
            }
            if item(ui, "Voltear horizontal", "") {
                self.picked_flip(true);
            }
            if item(ui, "Voltear vertical", "") {
                self.picked_flip(false);
            }
        });
        ui.menu_button("Animación", |ui| {
            ui.set_min_width(260.0);
            if item(ui, "Insertar cuadro vacío", "Mayús+I") {
                self.add_key(false);
            }
            if item(ui, "Duplicar cuadro", "Mayús+D") {
                self.add_key(true);
            }
            if item(ui, "Eliminar cuadro", "Mayús+Supr") {
                self.delete_key();
            }
            if item(ui, "Alargar exposición", "F5") {
                self.insert_frame();
            }
            if item(ui, "Acortar exposición", "Mayús+F5") {
                self.remove_frame();
            }
            ui.separator();
            if item(ui, "Insertar clave", "K") {
                self.set_xkey(None);
            }
            if item(ui, "Clave siguiente", "]") {
                self.jump_xkey(true);
            }
            if item(ui, "Clave anterior", "[") {
                self.jump_xkey(false);
            }
            ui.separator();
            ui.menu_button("Celdas", |ui| {
                if ui.button("Tiempos ×2").clicked() {
                    ui.close_menu();
                    self.cell_op_pub(0);
                }
                if ui.button("Invertir orden").clicked() {
                    ui.close_menu();
                    self.cell_op_pub(1);
                }
                if ui.button("Ida y vuelta").clicked() {
                    ui.close_menu();
                    self.cell_op_pub(2);
                }
                if ui.button("Vaciar").clicked() {
                    ui.close_menu();
                    self.cell_op_pub(3);
                }
            });
            if item(ui, "Inicio del rango", "Alt+I") {
                self.mark_range(true);
            }
            if item(ui, "Fin del rango", "Alt+O") {
                self.mark_range(false);
            }
            if item(ui, "Quitar rango", "") {
                self.project.range = None;
            }
            ui.separator();
            ui.checkbox(&mut self.onion, "Papel cebolla   O");
            ui.checkbox(&mut self.loop_play, "Bucle al reproducir");
            if item(ui, "Reproducir", "Espacio") {
                self.toggle_play();
            }
        });
        ui.menu_button("Capa", |ui| {
            ui.set_min_width(240.0);
            if item(ui, "Agregar capa…", "Mayús+A") {
                let at = ui.ctx().input(|i| i.pointer.hover_pos()).unwrap_or_else(|| ui.ctx().screen_rect().center());
                self.add_menu = Some(at + Vec2::new(0.0, 24.0));
                ui.close_menu();
            }
            if item(ui, "Duplicar capa", "Ctrl+J") {
                self.duplicate_layer();
            }
            if item(ui, "Eliminar capa", "") {
                self.delete_layer();
            }
            if item(ui, "Precomponer", "Ctrl+Mayús+C") {
                self.precompose_current();
            }
            if item(ui, "Entrar en la capa", "Tab") {
                self.enter_precomp(self.cl);
            }
            if item(ui, "Salir de la precomposición", "Mayús+Tab") {
                self.exit_precomp();
            }
            if item(ui, "Deshacer precomposición", "") {
                self.unprecompose_layer(self.cl);
            }
            ui.separator();
            if item(ui, "Agrupar en colección", "Ctrl+G") {
                self.group_current();
            }
            if item(ui, "Desagrupar", "Ctrl+Mayús+G") {
                self.ungroup_current();
            }
            if item(ui, "Subir capa", "") {
                self.move_layer(true);
            }
            if item(ui, "Bajar capa", "") {
                self.move_layer(false);
            }
            ui.separator();
            if item(ui, "Capa 3D: activar / cambiar plano", "") {
                let i = self.cl;
                self.cycle_plane(i);
            }
            if item(ui, "Modo objeto / edición", "Tab") {
                self.toggle_mode();
            }
        });
        ui.menu_button("Ver", |ui| {
            ui.set_min_width(240.0);
            ui.checkbox(&mut self.show_toolbar, "Barra de herramientas   T");
            ui.checkbox(&mut self.show_n, "Barra lateral   N");
            if item(ui, if self.view3d { "Vista 2D" } else { "Vista 3D" }, "Ctrl+3") {
                self.toggle_view3d();
            }
            ui.separator();
            ui.menu_button("Guías", |ui| {
                for (i, n) in GUIDE_NAMES.iter().enumerate() {
                    ui.checkbox(&mut self.guides[i], *n);
                }
            });
            if item(ui, "Ajustar a la vista", "Inicio") {
                self.need_fit = true;
            }
            if item(ui, "Zoom 100 %", "Ctrl+1") {
                let f = 1.0 / self.zoom;
                self.zoom_by(f);
            }
        });
        ui.menu_button("Ventana", |ui| {
            ui.set_min_width(260.0);
            ui.menu_button("Nueva sala", |ui| self.add_room_menu(ui));
            ui.menu_button("Conjunto de salas", |ui| self.room_set_menu(ui));
            ui.separator();
            if item(ui, "Alternar línea de tiempo / hoja de exposición", "Ctrl+Tab") {
                self.xs_horizontal = !self.xs_horizontal;
            }
            ui.add_enabled(false, egui::Button::new("Vista 2D / 3D (pendiente)").shortcut_text("Ctrl+3"));
            ui.separator();
            if item(ui, "Pantalla de inicio", "") {
                self.stage = start::Stage::Home;
            }
        });
        ui.menu_button("Ayuda", |ui| {
            ui.set_min_width(220.0);
            if item(ui, "Atajos de teclado y guía", "F1") {
                self.show_help = true;
            }
            if item(ui, "Pantalla de inicio", "") {
                self.stage = start::Stage::Home;
            }
            if item(ui, "Acerca de LeonToon", "") {
                self.show_about = true;
            }
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::default().fill(BG).inner_margin(egui::Margin::symmetric(10, 0)).stroke(stk(1.0, LINE));
        egui::TopBottomPanel::bottom("status").frame(frame).exact_height(24.0).show_separator_line(false).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let fresh = self.now - self.status_time < 6.0;
                if fresh {
                    ui.label(RichText::new(&self.status).size(11.0).color(TX));
                } else {
                    for (k, d) in self.hints() {
                        if !k.is_empty() {
                            let g = ui.painter().layout_no_wrap(k.to_string(), FontId::proportional(10.5), TX);
                            let (r, _) = ui.allocate_exact_size(g.size() + Vec2::new(8.0, 4.0), Sense::hover());
                            ui.painter().rect_filled(r, 3.0, BG4);
                            ui.painter().galley(r.min + Vec2::new(4.0, 2.0), g, TX);
                        }
                        ui.label(RichText::new(d).size(11.0).color(MU));
                        ui.add_space(8.0);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("LeonToon {}  |  Windows", env!("CARGO_PKG_VERSION"))).size(11.0).color(MU));
                    ui.add_space(12.0);
                    ui.label(RichText::new(format!("Capa: {}  |  Cuadro {}", self.layer().name, self.frame + 1)).size(11.0).color(MU));
                });
            });
        });
    }

    /// Pistas de la barra de estado según dónde está el ratón (HINTS del prototipo).
    fn hints(&self) -> Vec<(&'static str, String)> {
        match self.hover_area {
            2 => vec![("", "Clic: ir al cuadro".into()), ("Espacio", "Reproducir".into()), (", .", "Cuadro anterior / siguiente".into()), ("Ctrl+rueda", "Zoom de la hoja".into())],
            3 => vec![("", "Clic: elegir capa".into()), ("Mayús+N", "Nueva capa vectorial".into())],
            _ => vec![
                ("", self.tool.name().to_string()),
                ("Tab", if self.obj_mode { "Modo edición".into() } else { "Modo objeto".into() }),
                ("Mayús+A", "Agregar capa".into()),
                ("Rueda", "Zoom".into()),
                ("Clic central", "Desplazar".into()),
                ("T / N", "Paneles".into()),
            ],
        }
    }
}
