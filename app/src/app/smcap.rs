//! Captura con cámara en la sala Stop Motion (Issue #62): Live View en el visor, capturar con revisión o directo
//! («capturar y avanzar»), animación directa con onion skin en vivo y la lista de dispositivos de captura.
//!
//! La cámara vive en su propio hilo (`capture::Session`): la interfaz solo recoge el último cuadro del Live View y la
//! foto cuando llega. La foto se escribe en la carpeta de la secuencia con la misma escritura segura de siempre; si la
//! cámara se desconecta o falla, se avisa y la secuencia no cambia.

use super::shell::{muted, panel_header};
use super::*;
use crate::capture::{self, CapturedPhoto, Control, DeviceInfo, Event, Session};
use crate::stopmotion::{Device, SmRef};
use std::sync::Arc;

/// Qué se hará con la próxima foto que llegue.
#[derive(Clone, Copy, PartialEq)]
enum Want {
    /// Incorporarla a la secuencia y avanzar.
    Add,
    /// Dejarla pendiente para revisarla (incorporar, repetir o descartar).
    Review,
    /// Solo comprobar que la cámara captura (no se guarda).
    Test,
}

/// Foto capturada que aún no está en la secuencia.
struct Held {
    photo: CapturedPhoto,
    tex: egui::TextureHandle,
}

pub(super) struct CapState {
    /// El visor muestra el Live View.
    pub live: bool,
    providers: Vec<Arc<dyn capture::CaptureProvider>>,
    devices: Vec<DeviceInfo>,
    list_errors: Vec<String>,
    listed: bool,
    chosen: Option<String>,
    session: Option<Session>,
    controls: Vec<Control>,
    full: Option<(u32, u32)>,
    tex: Option<egui::TextureHandle>,
    frames: u32,
    fps: f32,
    fps_t: f64,
    /// Último aviso o error (la cámara falló, un control no se pudo cambiar…).
    problem: Option<String>,
    info: String,
    want: Option<Want>,
    held: Option<Held>,
    /// Animación directa: el atajo captura y avanza sin pedir revisión.
    direct: bool,
    onion: bool,
    onion_alpha: f32,
    onion_tex: Option<(String, egui::TextureHandle)>,
    shots: u32,
    /// Conectar la cámara elegida al abrir el Live View (solo para capturas de pantalla).
    auto: bool,
}

impl Default for CapState {
    fn default() -> Self {
        CapState {
            live: false,
            providers: Vec::new(),
            devices: Vec::new(),
            list_errors: Vec::new(),
            listed: false,
            chosen: None,
            session: None,
            controls: Vec::new(),
            full: None,
            tex: None,
            frames: 0,
            fps: 0.0,
            fps_t: 0.0,
            problem: None,
            info: String::new(),
            want: None,
            held: None,
            direct: true,
            onion: true,
            onion_alpha: 0.5,
            onion_tex: None,
            shots: 0,
            auto: false,
        }
    }
}

impl CapState {
    /// Para capturas de pantalla: LEONTOON_LIVE=1 abre el Live View al arrancar.
    pub(super) fn from_env() -> CapState {
        let live = std::env::var("LEONTOON_LIVE").is_ok();
        CapState { live, auto: live, ..Default::default() }
    }
}

fn kb(n: usize) -> String {
    if n >= 1_048_576 { format!("{:.1} MB", n as f32 / 1_048_576.0) } else { format!("{} KB", n / 1000) }
}

impl LeonApp {
    fn cap_refresh_list(&mut self) {
        let c = &mut self.sm_ui.cap;
        if c.providers.is_empty() {
            c.providers = capture::providers();
        }
        let (devices, errors) = capture::list_all(&c.providers);
        c.devices = devices;
        c.list_errors = errors;
        c.listed = true;
        if c.chosen.as_ref().map_or(true, |id| !c.devices.iter().any(|d| &d.id == id)) {
            c.chosen = c.devices.first().map(|d| d.id.clone());
        }
    }

    pub(super) fn sm_cap_connect(&mut self) {
        if !self.sm_ui.cap.listed {
            self.cap_refresh_list();
        }
        let c = &mut self.sm_ui.cap;
        let Some(dev) = c.chosen.as_ref().and_then(|id| c.devices.iter().find(|d| &d.id == id)).cloned() else {
            c.problem = Some("No hay ninguna cámara para conectar. Enchúfala y pulsa «Buscar cámaras».".into());
            return;
        };
        let Some(p) = capture::provider_of(&c.providers, &dev.id) else {
            c.problem = Some(format!("No hay un proveedor para {}", dev.id));
            return;
        };
        c.session = Some(Session::start(p, dev));
        c.problem = None;
        c.controls.clear();
        c.full = None;
        c.tex = None;
        c.info.clear();
    }

    /// Cierra la cámara (la luz de la webcam se apaga).
    pub(super) fn sm_cap_close(&mut self) {
        let c = &mut self.sm_ui.cap;
        c.session = None;
        c.want = None;
        c.tex = None;
        c.controls.clear();
    }

    /// Recoge lo que la cámara tenga listo: el Live View, la foto pedida y los avisos.
    fn sm_cap_poll(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let mut photo: Option<CapturedPhoto> = None;
        {
            let c = &mut self.sm_ui.cap;
            let Some(s) = c.session.as_ref() else { return };
            let mut failed: Option<String> = None;
            while let Ok(ev) = s.events.try_recv() {
                match ev {
                    Event::Opened { controls, size } => {
                        c.controls = controls;
                        c.full = Some(size);
                    }
                    Event::Photo(p) => photo = Some(p),
                    Event::Failed(e) => failed = Some(e),
                    Event::Warning(w) => c.problem = Some(w),
                }
            }
            if let Some(l) = s.take_live() {
                let img = egui::ColorImage::from_rgba_unmultiplied([l.width as usize, l.height as usize], &l.rgba);
                match c.tex.as_mut() {
                    Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                    None => c.tex = Some(ctx.load_texture("cap_live", img, egui::TextureOptions::LINEAR)),
                }
                c.full = Some(l.full);
                c.frames += 1;
            }
            if now - c.fps_t >= 1.0 {
                c.fps = c.frames as f32 / (now - c.fps_t).max(0.001) as f32;
                c.frames = 0;
                c.fps_t = now;
            }
            if let Some(e) = failed {
                // La cámara se perdió: la secuencia no se toca; la foto pendiente (si hay) sigue disponible.
                c.problem = Some(format!("{e}. La secuencia no ha cambiado."));
                c.session = None;
                c.want = None;
            }
        }
        if let Some(p) = photo {
            self.sm_cap_photo(ctx, p);
        }
        if self.sm_ui.cap.session.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn sm_cap_photo(&mut self, ctx: &egui::Context, photo: CapturedPhoto) {
        let want = self.sm_ui.cap.want.take().unwrap_or(Want::Review);
        match want {
            Want::Test => {
                self.sm_ui.cap.info = format!("Prueba correcta: foto de {}×{} ({}, {}). No se guardó.", photo.width, photo.height, if photo.png { "PNG" } else { "JPEG" }, kb(photo.bytes.len()));
            }
            Want::Add => {
                self.sm_cap_commit(photo);
            }
            Want::Review => match image::load_from_memory(&photo.bytes) {
                Ok(img) => {
                    let small = img.thumbnail(1280, 1280).to_rgba8();
                    let ci = egui::ColorImage::from_rgba_unmultiplied([small.width() as usize, small.height() as usize], small.as_raw());
                    let tex = ctx.load_texture("cap_held", ci, egui::TextureOptions::LINEAR);
                    self.sm_ui.cap.held = Some(Held { photo, tex });
                }
                Err(e) => self.sm_ui.cap.problem = Some(format!("La foto recibida no se pudo abrir: {e}")),
            },
        }
    }

    /// Pone la foto en la carpeta de la secuencia y al final de la línea de tiempo, y lleva el cabezal a ella.
    fn sm_cap_commit(&mut self, photo: CapturedPhoto) -> bool {
        let Some(li) = self.sm_room_layer() else { return false };
        let Some(r) = self.project.layers[li].stopmotion.clone() else {
            self.sm_ui.cap.problem = Some("La capa Stop Motion no tiene secuencia: créala antes de capturar.".into());
            return false;
        };
        let Some(dev) = self.sm_ui.cap.session.as_ref().map(|s| s.device.clone()).or_else(|| self.sm_ui.cap.chosen.as_ref().and_then(|id| self.sm_ui.cap.devices.iter().find(|d| &d.id == id).cloned())) else {
            return false;
        };
        let pdir = self.project_dir();
        let (pfps, plen) = (self.project.fps, self.project.length);
        let Some(v) = self.sm.get_mut(&r.id) else {
            self.sm_ui.cap.problem = Some("La carpeta de la secuencia no está disponible: no se guardó la foto.".into());
            return false;
        };
        let device = Some(Device { name: dev.name.clone(), provider: dev.provider.clone() });
        let item = match v.folder.capture_to_end(&mut v.hist, &photo.bytes, device) {
            Ok(i) => i,
            Err(e) => {
                self.sm_ui.cap.problem = Some(format!("No se pudo guardar la foto: {e}. La secuencia no ha cambiado."));
                return false;
            }
        };
        v.folder.log(&format!("Captura con {} ({}×{})", dev.name, photo.width, photo.height));
        let idx = v.folder.seq.index_of(&item).unwrap_or(0);
        let frame = super::smroom::project_frame_of(&v.folder.seq, &r, pfps, idx);
        let need = r.start + (v.folder.seq.total_frames() as u64 * pfps as u64).div_ceil(v.folder.seq.fps.max(1) as u64) as u32;
        v.last = format!("Foto capturada con {}", dev.name);
        self.sm_ui.last_was_seq = true;
        self.sm_ui.sel_one(item);
        self.sm_after_edit(&r.id, pdir);
        if need > plen {
            self.project.length = need.clamp(1, 100_000);
        }
        self.frame = frame.min(self.project.length.saturating_sub(1));
        self.playing = false;
        self.sm_ui.cap.shots += 1;
        self.sm_ui.cap.info = format!("Foto {} guardada: {}×{}, {}", self.sm_ui.cap.shots, photo.width, photo.height, kb(photo.bytes.len()));
        self.sm_ui.cap.problem = None;
        true
    }

    fn cap_request(&mut self, want: Want) {
        let c = &mut self.sm_ui.cap;
        if c.session.is_none() || c.want.is_some() {
            return;
        }
        c.want = Some(want);
        if let Some(s) = &c.session {
            s.capture();
        }
    }

    /// Atajos de la captura: F5 e Intro capturan (o incorporan la foto pendiente); Esc descarta la pendiente.
    /// Valen en toda la sala mientras el Live View está abierto. Devuelve true si se usó alguno.
    pub(super) fn sm_cap_keys(&mut self, ctx: &egui::Context) -> bool {
        if self.room_kind() != RoomKind::StopMotion || !self.sm_ui.cap.live {
            return false;
        }
        use egui::{Key, Modifiers};
        let none = Modifiers::NONE;
        let go = ctx.input_mut(|i| i.consume_key(none, Key::F5) || i.consume_key(none, Key::Enter));
        if go {
            if self.sm_ui.cap.held.is_some() {
                self.cap_accept();
            } else {
                let w = if self.sm_ui.cap.direct { Want::Add } else { Want::Review };
                self.cap_request(w);
            }
            return true;
        }
        if self.sm_ui.cap.held.is_some() && ctx.input_mut(|i| i.consume_key(none, Key::Escape) || i.consume_key(none, Key::Backspace)) {
            self.sm_ui.cap.held = None;
            return true;
        }
        false
    }

    fn cap_accept(&mut self) {
        if let Some(h) = self.sm_ui.cap.held.take() {
            if !self.sm_cap_commit_ref(&h.photo) {
                // No se pudo guardar: la foto sigue pendiente para volver a intentarlo o descartarla.
                self.sm_ui.cap.held = Some(h);
            }
        }
    }

    fn sm_cap_commit_ref(&mut self, photo: &CapturedPhoto) -> bool {
        self.sm_cap_commit(CapturedPhoto { bytes: photo.bytes.clone(), width: photo.width, height: photo.height, png: photo.png })
    }

    /// Textura del onion skin: la última foto de la secuencia (se vuelve a cargar solo cuando cambia).
    fn cap_onion(&mut self, ctx: &egui::Context, r: &SmRef) -> Option<egui::TextureId> {
        let (src, dir) = {
            let v = self.sm.get(&r.id)?;
            let it = v.folder.seq.items.iter().rev().find(|i| !i.source.is_empty())?;
            (v.folder.seq.source(&it.source)?.clone(), v.folder.dir.clone())
        };
        let key = format!("{}/{}", r.id, src.id);
        if self.sm_ui.cap.onion_tex.as_ref().map(|t| &t.0) != Some(&key) {
            let d = crate::stopmotion::load_proxy(&dir, &src, crate::stopmotion::PROXY_SIDE).ok()?;
            let img = egui::ColorImage::from_rgba_unmultiplied([d.w as usize, d.h as usize], &d.rgba);
            self.sm_ui.cap.onion_tex = Some((key, ctx.load_texture("cap_onion", img, egui::TextureOptions::LINEAR)));
        }
        self.sm_ui.cap.onion_tex.as_ref().map(|t| t.1.id())
    }

    /// Cierra la cámara al salir de la sala (la luz de la webcam no debe quedar encendida).
    pub(super) fn sm_cap_leave(&mut self) {
        if self.sm_ui.cap.session.is_some() {
            self.sm_cap_close();
        }
    }

    /// El visor de la sala mientras el Live View está abierto.
    pub(super) fn sm_live_view(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if std::mem::take(&mut self.sm_ui.cap.auto) {
            self.sm_cap_connect();
        }
        self.sm_cap_poll(&ctx);
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 6.0, Color32::from_rgb(0x10, 0x10, 0x12));
        let seq_ref = self.sm_room_layer().and_then(|li| self.project.layers[li].stopmotion.clone());
        let ready = seq_ref.as_ref().map_or(false, |r| self.sm.contains_key(&r.id));
        let bar_h = 40.0;
        let stage = Rect::from_min_max(rect.min + Vec2::new(8.0, 30.0), rect.max - Vec2::new(8.0, bar_h + 8.0));
        // Cabecera: estado de la cámara, opciones y salida.
        let head = Rect::from_min_size(rect.min + Vec2::new(8.0, 4.0), Vec2::new(rect.width() - 16.0, 24.0));
        let mut hui = ui.new_child(egui::UiBuilder::new().max_rect(head).layout(egui::Layout::left_to_right(egui::Align::Center)));
        hui.spacing_mut().item_spacing.x = 8.0;
        let mut exit = false;
        let status = {
            let c = &self.sm_ui.cap;
            match (&c.session, c.full) {
                (Some(s), Some((w, h))) => format!("● {} · foto {w}×{h} · {:.0} cuadros/s", s.device.name, c.fps),
                (Some(s), None) => format!("Abriendo {}…", s.device.name),
                _ => "Sin cámara conectada".into(),
            }
        };
        hui.label(RichText::new(status).size(12.0).color(TX));
        hui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Salir del Live View").clicked() {
                exit = true;
            }
            let c = &mut self.sm_ui.cap;
            ui.add(egui::Slider::new(&mut c.onion_alpha, 0.1..=0.9).show_value(false)).on_hover_text("Opacidad de la foto anterior sobre el Live View");
            ui.checkbox(&mut c.onion, "Foto anterior").on_hover_text("Onion skin en vivo: la última foto de la secuencia sobre lo que ve la cámara");
            ui.checkbox(&mut c.direct, "Animación directa").on_hover_text("Intro y F5 capturan y avanzan sin pedir revisión. Si lo quitas, la foto queda pendiente para incorporarla, repetirla o descartarla");
        });
        // Escenario: foto pendiente, o Live View con la foto anterior encima.
        let ar_of = |w: u32, h: u32| w.max(1) as f32 / h.max(1) as f32;
        let fit = |ar: f32| -> Rect {
            let size = if stage.width() / stage.height().max(1.0) > ar { Vec2::new(stage.height() * ar, stage.height()) } else { Vec2::new(stage.width(), stage.width() / ar) };
            Rect::from_center_size(stage.center(), size)
        };
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let mut onion_tex = None;
        if self.sm_ui.cap.onion && self.sm_ui.cap.held.is_none() {
            if let Some(r) = seq_ref.as_ref().filter(|_| ready) {
                onion_tex = self.cap_onion(&ctx, r);
            }
        }
        let c = &self.sm_ui.cap;
        if let Some(h) = &c.held {
            let r = fit(ar_of(h.photo.width, h.photo.height));
            ui.painter().image(h.tex.id(), r, uv, Color32::WHITE);
            ui.painter().text(stage.left_top() + Vec2::new(8.0, 8.0), Align2::LEFT_TOP, format!("Foto pendiente · {}×{} · {}", h.photo.width, h.photo.height, kb(h.photo.bytes.len())), FontId::proportional(12.0), ACCENT);
        } else if let Some(t) = &c.tex {
            let sz = t.size_vec2();
            let r = fit(ar_of(sz.x as u32, sz.y as u32));
            ui.painter().image(t.id(), r, uv, Color32::WHITE);
            if let Some(o) = onion_tex {
                ui.painter().image(o, r, uv, Color32::WHITE.gamma_multiply(c.onion_alpha));
            }
        } else if c.session.is_some() {
            ui.painter().text(stage.center(), Align2::CENTER_CENTER, "Esperando la imagen de la cámara…", FontId::proportional(13.0), INK3);
        }
        // Sin cámara: elegir y conectar.
        if self.sm_ui.cap.session.is_none() && self.sm_ui.cap.held.is_none() {
            let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(stage.center(), Vec2::new(stage.width().min(420.0), 140.0))).layout(egui::Layout::top_down(egui::Align::Center)));
            if let Some(p) = &self.sm_ui.cap.problem {
                cui.label(RichText::new(format!("⚠ {p}")).size(12.0).color(ACCENT2));
            }
            if !ready {
                muted(&mut cui, "La capa Stop Motion necesita una secuencia (carpeta) para guardar las fotos.");
            }
            cui.horizontal(|ui| {
                self.cap_device_combo(ui);
                if ui.button("Conectar").clicked() {
                    self.sm_cap_connect();
                }
            });
        }
        // Barra de captura.
        let bar = Rect::from_min_size(Pos2::new(rect.min.x + 8.0, rect.max.y - bar_h - 4.0), Vec2::new(rect.width() - 16.0, bar_h));
        let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)));
        bui.spacing_mut().item_spacing.x = 6.0;
        let connected = self.sm_ui.cap.session.is_some();
        let busy = self.sm_ui.cap.want.is_some();
        if self.sm_ui.cap.held.is_some() {
            if bui.add(egui::Button::new(RichText::new("✔ Incorporar (Intro)").strong())).clicked() {
                self.cap_accept();
            }
            if bui.button("↻ Repetir").on_hover_text("Descarta esta foto y captura otra").clicked() {
                self.sm_ui.cap.held = None;
                self.cap_request(Want::Review);
            }
            if bui.button("✕ Descartar (Esc)").clicked() {
                self.sm_ui.cap.held = None;
            }
        } else {
            bui.add_enabled_ui(connected && ready && !busy, |ui| {
                if ui.add(egui::Button::new(RichText::new("● Capturar y avanzar (Intro / F5)").strong())).on_hover_text("Guarda la foto al final de la secuencia y lleva el cabezal a ella").clicked() {
                    self.cap_request(Want::Add);
                }
                if ui.button("Capturar y revisar").on_hover_text("Deja la foto pendiente para incorporarla, repetirla o descartarla").clicked() {
                    self.cap_request(Want::Review);
                }
            });
            if connected && bui.button("Desconectar").clicked() {
                self.sm_cap_close();
            }
        }
        let c = &self.sm_ui.cap;
        let line = c.problem.as_ref().map(|p| (format!("⚠ {p}"), ACCENT2)).or_else(|| (!c.info.is_empty()).then(|| (c.info.clone(), MU)));
        if let Some((t, col)) = line {
            bui.label(RichText::new(t).size(11.5).color(col));
        }
        if exit {
            self.sm_ui.cap.live = false;
        }
    }

    fn cap_device_combo(&mut self, ui: &mut egui::Ui) {
        if !self.sm_ui.cap.listed {
            self.cap_refresh_list();
        }
        let c = &mut self.sm_ui.cap;
        let label = c.chosen.as_ref().and_then(|id| c.devices.iter().find(|d| &d.id == id)).map_or("(ninguna)".to_string(), |d| d.name.clone());
        egui::ComboBox::from_id_salt("cap_dev").selected_text(label).width(210.0).show_ui(ui, |ui| {
            for d in c.devices.clone() {
                let sel = c.chosen.as_ref() == Some(&d.id);
                if ui.selectable_label(sel, format!("{} ({})", d.name, d.provider)).clicked() {
                    c.chosen = Some(d.id);
                }
            }
        });
        if ui.small_button("Buscar cámaras").clicked() {
            self.cap_refresh_list();
        }
    }

    /// Pestaña «Captura» del panel inferior: dispositivos de captura, prueba de captura, controles y errores.
    pub(super) fn sm_cap_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.sm_cap_poll(&ctx);
        panel_header(ui, "Dispositivos de captura");
        ui.horizontal_wrapped(|ui| {
            self.cap_device_combo(ui);
            let connected = self.sm_ui.cap.session.is_some();
            if connected {
                if ui.button("Desconectar").clicked() {
                    self.sm_cap_close();
                }
                if ui.button("Probar captura").on_hover_text("Saca una foto para comprobar que funciona; no se guarda").clicked() {
                    self.cap_request(Want::Test);
                }
            } else if ui.button("Conectar").clicked() {
                self.sm_cap_connect();
            }
            let live = self.sm_ui.cap.live;
            if ui.selectable_label(live, "● Live View").on_hover_text("Muestra lo que ve la cámara en el visor y permite capturar").clicked() {
                self.sm_ui.cap.live = !live;
                if !live && self.sm_ui.cap.session.is_none() {
                    self.sm_cap_connect();
                }
            }
        });
        let c = &self.sm_ui.cap;
        if c.devices.is_empty() && c.listed {
            muted(ui, "No se encontró ninguna cámara. Comprueba que está enchufada y que Windows le da permiso a las aplicaciones de escritorio.");
        }
        for e in &c.list_errors {
            ui.label(RichText::new(format!("⚠ {e}")).size(11.5).color(ACCENT2));
        }
        if let Some(p) = &c.problem {
            ui.label(RichText::new(format!("⚠ {p}")).size(11.5).color(ACCENT2));
        }
        if !c.info.is_empty() {
            muted(ui, c.info.clone());
        }
        if let (Some(s), Some((w, h))) = (&c.session, c.full) {
            muted(ui, format!("{} · la foto sale a {w}×{h}; el Live View solo es una vista reducida", s.device.name));
        }
        // Controles: solo los que la cámara declara.
        let controls = c.controls.clone();
        if !controls.is_empty() {
            ui.add_space(4.0);
            panel_header(ui, "Controles de la cámara");
            let mut changed: Option<(String, i64)> = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for ctl in &controls {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&ctl.label).size(11.5).color(MU));
                        let mut v = self.sm_ui.cap.control_value(&ctl.key, ctl.value);
                        if ctl.boolean {
                            let mut b = v != 0;
                            if ui.checkbox(&mut b, "").changed() {
                                v = b as i64;
                                changed = Some((ctl.key.clone(), v));
                            }
                        } else if ui.add(egui::Slider::new(&mut v, ctl.min..=ctl.max).step_by(ctl.step as f64)).changed() {
                            changed = Some((ctl.key.clone(), v));
                        }
                    });
                }
            });
            if let Some((k, v)) = changed {
                self.sm_ui.cap.set_control(&k, v);
            }
        } else if self.sm_ui.cap.session.is_some() && self.sm_ui.cap.full.is_some() {
            muted(ui, "Esta cámara no declara controles que se puedan cambiar.");
        }
    }
}

impl CapState {
    pub(super) fn session_is_none(&self) -> bool {
        self.session.is_none()
    }

    fn control_value(&self, key: &str, fallback: i64) -> i64 {
        self.controls.iter().find(|c| c.key == key).map_or(fallback, |c| c.value)
    }

    fn set_control(&mut self, key: &str, value: i64) {
        if let Some(c) = self.controls.iter_mut().find(|c| c.key == key) {
            c.value = value;
        }
        if let Some(s) = &self.session {
            s.set_control(key, value);
        }
    }
}
