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
    /// Herramientas del Live View: aumento (1 = el cuadro entero), desplazamiento en el cuadro, cuadrículas, enfoque
    /// asistido y «ver la foto anterior» en lugar de la cámara.
    zoom: f32,
    pan: (f32, f32),
    thirds: bool,
    cross: bool,
    peaking: bool,
    flip: bool,
    /// Cuántas fotos reproducir con «Últimas».
    pub(super) last_n: u32,
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
            zoom: 1.0,
            pan: (0.0, 0.0),
            thirds: false,
            cross: false,
            peaking: false,
            flip: false,
            last_n: 12,
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

/// Texto pequeño sobre fondo oscuro para rotular el escenario. `pos` es la esquina de arriba a la izquierda, o la de
/// abajo a la izquierda si `from_bottom`.
fn badge(p: &egui::Painter, pos: Pos2, from_bottom: bool, text: String, color: Color32) {
    let galley = p.layout_no_wrap(text, FontId::proportional(12.0), color);
    let top = if from_bottom { Pos2::new(pos.x, pos.y - galley.size().y) } else { pos };
    p.rect_filled(Rect::from_min_size(top, galley.size()).expand2(Vec2::new(5.0, 2.0)), 4.0, Color32::from_black_alpha(170));
    p.galley(top, galley, color);
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
            if let Some(mut l) = s.take_live() {
                if c.peaking {
                    capture::peaking(&mut l.rgba, l.width as usize, l.height as usize);
                }
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
        // L: ver la foto anterior en lugar de la cámara (y volver), como el botón del panel de la derecha.
        if !ctx.wants_keyboard_input() && ctx.input_mut(|i| i.consume_key(none, Key::L)) {
            self.sm_ui.cap.flip = !self.sm_ui.cap.flip;
            return true;
        }
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

    /// El Live View: ocupa todo el escenario de la sala. Muestra lo que ve la cámara con la foto anterior encima, las
    /// cuadrículas y las ayudas de enfoque que se hayan activado, y la foto pendiente de revisar. La rueda del ratón
    /// acerca, arrastrar desplaza y doble clic vuelve al cuadro entero.
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
        let stage = rect.shrink(8.0);
        let ar_of = |w: u32, h: u32| w.max(1) as f32 / h.max(1) as f32;
        let fit = |ar: f32| -> Rect {
            let size = if stage.width() / stage.height().max(1.0) > ar { Vec2::new(stage.height() * ar, stage.height()) } else { Vec2::new(stage.width(), stage.width() / ar) };
            Rect::from_center_size(stage.center(), size)
        };
        let mut onion_tex = None;
        if (self.sm_ui.cap.onion || self.sm_ui.cap.flip) && self.sm_ui.cap.held.is_none() {
            if let Some(r) = seq_ref.as_ref().filter(|_| ready) {
                onion_tex = self.cap_onion(&ctx, r);
            }
        }
        let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let mut zoom_pan: Option<(f32, (f32, f32))> = None;
        let c = &self.sm_ui.cap;
        let img = if let Some(h) = &c.held {
            Some((h.tex.id(), h.photo.width, h.photo.height, true))
        } else {
            c.tex.as_ref().map(|t| (t.id(), t.size_vec2().x as u32, t.size_vec2().y as u32, false))
        };
        let mut status_extra = String::new();
        if let Some((tid, w, h, is_held)) = img {
            let r = fit(ar_of(w, h));
            let p = ui.painter();
            if is_held {
                p.image(tid, r, full_uv, Color32::WHITE);
                badge(p, stage.left_top() + Vec2::new(4.0, 4.0), false, format!("Foto pendiente · {}×{} · {}", c.held.as_ref().map_or(0, |h| h.photo.width), c.held.as_ref().map_or(0, |h| h.photo.height), c.held.as_ref().map_or(String::new(), |h| kb(h.photo.bytes.len()))), ACCENT);
            } else {
                let zoom = c.zoom.clamp(1.0, 8.0);
                let half = 0.5 / zoom;
                let lim = 0.5 - half;
                let (px, py) = (c.pan.0.clamp(-lim, lim), c.pan.1.clamp(-lim, lim));
                let uv = Rect::from_min_max(Pos2::new(0.5 + px - half, 0.5 + py - half), Pos2::new(0.5 + px + half, 0.5 + py + half));
                p.image(tid, r, uv, Color32::WHITE);
                if let Some(o) = onion_tex {
                    p.image(o, r, uv, Color32::WHITE.gamma_multiply(if c.flip { 1.0 } else if c.onion { c.onion_alpha } else { 0.0 }));
                }
                if c.flip {
                    badge(p, stage.left_top() + Vec2::new(4.0, 26.0), false, "Foto anterior (L para volver a la cámara)".into(), ACCENT);
                }
                let line = Color32::from_white_alpha(120);
                if c.thirds {
                    for k in 1..=2 {
                        let f = k as f32 / 3.0;
                        p.vline(r.min.x + r.width() * f, r.y_range(), stk(1.0, line));
                        p.hline(r.x_range(), r.min.y + r.height() * f, stk(1.0, line));
                    }
                }
                if c.cross {
                    let m = r.center();
                    p.vline(m.x, r.y_range(), stk(1.0, line));
                    p.hline(r.x_range(), m.y, stk(1.0, line));
                    p.circle_stroke(m, 18.0, stk(1.0, line));
                }
                if zoom > 1.0 {
                    status_extra = format!(" · aumento {zoom:.1}×");
                }
                let resp = ui.interact(r, egui::Id::new("cap_stage"), Sense::click_and_drag());
                let (mut nz, mut npan) = (zoom, (px, py));
                if resp.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0 {
                        nz = (zoom * (1.0 + scroll * 0.002)).clamp(1.0, 8.0);
                    }
                }
                if zoom > 1.0 && resp.dragged() {
                    let d = resp.drag_delta();
                    npan.0 -= d.x / (r.width() * zoom);
                    npan.1 -= d.y / (r.height() * zoom);
                }
                if resp.double_clicked() {
                    nz = 1.0;
                }
                if nz <= 1.001 {
                    nz = 1.0;
                    npan = (0.0, 0.0);
                } else {
                    let l = 0.5 - 0.5 / nz;
                    npan = (npan.0.clamp(-l, l), npan.1.clamp(-l, l));
                }
                if (nz - c.zoom).abs() > f32::EPSILON || npan != c.pan {
                    zoom_pan = Some((nz, npan));
                }
            }
        } else if c.session.is_some() {
            ui.painter().text(stage.center(), Align2::CENTER_CENTER, "Esperando la imagen de la cámara…", FontId::proportional(13.0), INK3);
        }
        // Estado de la cámara (arriba a la izquierda) y último aviso (abajo a la izquierda).
        let status = match (&c.session, c.full) {
            (Some(s), Some((w, h))) => format!("● {} · foto {w}×{h} · {:.0} cuadros/s{status_extra}", s.device.name, c.fps),
            (Some(s), None) => format!("Abriendo {}…", s.device.name),
            _ => "Sin cámara conectada".into(),
        };
        if c.held.is_none() {
            badge(ui.painter(), stage.left_top() + Vec2::new(4.0, 4.0), false, status, TX);
        }
        let line = c.problem.as_ref().map(|p| (format!("⚠ {p}"), ACCENT2)).or_else(|| (!c.info.is_empty()).then(|| (c.info.clone(), MU)));
        if let Some((t, col)) = line {
            badge(ui.painter(), stage.left_bottom() + Vec2::new(4.0, -4.0), true, t, col);
        }
        if let Some((z, p)) = zoom_pan {
            self.sm_ui.cap.zoom = z;
            self.sm_ui.cap.pan = p;
        }
        // Sin cámara: elegir y conectar.
        if self.sm_ui.cap.session.is_none() && self.sm_ui.cap.held.is_none() {
            let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(stage.center(), Vec2::new(stage.width().min(420.0), 140.0))).layout(egui::Layout::top_down(egui::Align::Center)));
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
    }

    /// Barra bajo el escenario: transporte y reproducción arriba; captura abajo.
    pub(super) fn sm_capture_bar(&mut self, ui: &mut egui::Ui) {
        use super::icons::{draw_first, draw_last, draw_loop, draw_next, draw_pause, draw_play, draw_prev, icon_button};
        let rect = ui.max_rect();
        let Some(r) = self.sm_room_layer().and_then(|li| self.project.layers[li].stopmotion.clone()) else { return };
        let ready = self.sm.contains_key(&r.id);
        let n = self.project.length;
        let row1 = Rect::from_min_size(rect.min + Vec2::new(8.0, 4.0), Vec2::new(rect.width() - 16.0, 28.0));
        let row2 = Rect::from_min_size(rect.min + Vec2::new(8.0, 38.0), Vec2::new(rect.width() - 16.0, 28.0));
        let mut r1 = ui.new_child(egui::UiBuilder::new().max_rect(row1).layout(egui::Layout::left_to_right(egui::Align::Center)));
        r1.spacing_mut().item_spacing.x = 4.0;
        {
            let ui = &mut r1;
            if icon_button(ui, 24.0, false, "Primer cuadro (Inicio)", draw_first).clicked() {
                self.playing = false;
                self.frame = self.project.range.map_or(0, |r| r.0);
            }
            if icon_button(ui, 24.0, false, "Cuadro anterior ( , )", draw_prev).clicked() {
                self.step(-1);
            }
            let playing = self.playing;
            let play = icon_button(ui, 24.0, playing, "Reproducir / pausa (Espacio). Con el Live View abierto, reproduce lo capturado y vuelve a la cámara al parar", move |p, r, c| {
                if playing {
                    draw_pause(p, r, c)
                } else {
                    draw_play(p, r, c)
                }
            });
            if play.clicked() {
                self.sm_play_range = None;
                self.toggle_play();
            }
            if icon_button(ui, 24.0, false, "Cuadro siguiente ( . )", draw_next).clicked() {
                self.step(1);
            }
            if icon_button(ui, 24.0, false, "Último cuadro (Fin)", draw_last).clicked() {
                self.playing = false;
                self.frame = self.project.range.map_or(n - 1, |r| r.1);
            }
            if icon_button(ui, 24.0, self.loop_play, "Bucle", draw_loop).clicked() {
                self.loop_play = !self.loop_play;
            }
            ui.add_space(6.0);
            ui.label(RichText::new(format!("{} / {}", self.frame + 1, n)).size(12.5).color(TX).strong());
            ui.add(egui::DragValue::new(&mut self.project.fps).range(1..=120).suffix(" fps"));
            ui.add_space(6.0);
            let mut last_n = self.sm_ui.cap.last_n;
            ui.add(egui::DragValue::new(&mut last_n).range(2..=500).prefix("últimas ")).on_hover_text("Cuántas fotos reproduce el botón «▶ Últimas»");
            self.sm_ui.cap.last_n = last_n;
            if ui.button(format!("▶ Últimas {last_n}")).on_hover_text("Reproduce solo las últimas fotos de la secuencia, para revisar el movimiento que acabas de hacer").clicked() {
                let pfps = self.project.fps;
                let range = self.sm.get(&r.id).and_then(|v| {
                    let s = &v.folder.seq;
                    let items = s.items.len();
                    (items > 0).then(|| {
                        let a = super::smroom::project_frame_of(s, &r, pfps, items.saturating_sub(last_n as usize));
                        let b = super::smroom::project_frame_of(s, &r, pfps, items).saturating_sub(1).max(a);
                        (a, b.min(n.saturating_sub(1)))
                    })
                });
                if range.is_some() {
                    self.sm_play_range = range;
                    self.playing = false;
                    self.toggle_play();
                }
            }
        }
        let mut r2 = ui.new_child(egui::UiBuilder::new().max_rect(row2).layout(egui::Layout::left_to_right(egui::Align::Center)));
        r2.spacing_mut().item_spacing.x = 6.0;
        let ui = &mut r2;
        let connected = self.sm_ui.cap.session.is_some();
        let busy = self.sm_ui.cap.want.is_some() || self.playing;
        let live = self.sm_ui.cap.live;
        if ui.add(egui::SelectableLabel::new(live, "● Live View")).on_hover_text("Muestra lo que ve la cámara en el escenario y permite capturar").clicked() {
            self.sm_ui.cap.live = !live;
            if !live && !connected {
                self.sm_cap_connect();
            }
        }
        if self.sm_ui.cap.held.is_some() {
            if ui.add(egui::Button::new(RichText::new("✔ Incorporar (Intro)").strong())).clicked() {
                self.cap_accept();
            }
            if ui.button("↻ Repetir").on_hover_text("Descarta esta foto y captura otra").clicked() {
                self.sm_ui.cap.held = None;
                self.cap_request(Want::Review);
            }
            if ui.button("✕ Descartar (Esc)").clicked() {
                self.sm_ui.cap.held = None;
            }
        } else {
            ui.add_enabled_ui(connected && ready && !busy, |ui| {
                let shoot = egui::Button::new(RichText::new("● Capturar y avanzar (Intro / F5)").strong().color(Color32::WHITE)).fill(Color32::from_rgb(0xa8, 0x2a, 0x2a)).min_size(Vec2::new(210.0, 26.0));
                if ui.add(shoot).on_hover_text("Guarda la foto al final de la secuencia y lleva el cabezal a ella").clicked() {
                    self.cap_request(Want::Add);
                }
                if ui.button("Capturar y revisar").on_hover_text("Deja la foto pendiente para incorporarla, repetirla o descartarla").clicked() {
                    self.cap_request(Want::Review);
                }
            });
        }
        let c = &self.sm_ui.cap;
        if let Some(t) = c.problem.as_ref().map(|p| (format!("⚠ {p}"), ACCENT2)).or_else(|| (!c.info.is_empty()).then(|| (c.info.clone(), MU))) {
            ui.add(egui::Label::new(RichText::new(t.0).size(11.5).color(t.1)).truncate());
        }
    }

    fn cap_device_combo(&mut self, ui: &mut egui::Ui) {
        if !self.sm_ui.cap.listed {
            self.cap_refresh_list();
        }
        let c = &mut self.sm_ui.cap;
        let label = c.chosen.as_ref().and_then(|id| c.devices.iter().find(|d| &d.id == id)).map_or("(ninguna)".to_string(), |d| d.name.clone());
        egui::ComboBox::from_id_salt("cap_dev").selected_text(label).width(150.0).show_ui(ui, |ui| {
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

    /// Columna derecha de la sala: la cámara, sus controles, el papel cebolla y las herramientas del Live View.
    pub(super) fn sm_cam_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.sm_cap_poll(&ctx);
        egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("sm_cam").show(ui, |ui| {
            panel_header(ui, "Cámara");
            ui.horizontal_wrapped(|ui| {
                self.cap_device_combo(ui);
            });
            ui.horizontal_wrapped(|ui| {
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
                muted(ui, format!("La foto sale a {w}×{h} ({}); el Live View es una vista reducida", s.device.name));
            }
            // Controles: solo los que la cámara declara.
            let controls = c.controls.clone();
            if !controls.is_empty() {
                ui.add_space(4.0);
                panel_header(ui, "Controles de la cámara");
                let mut changed: Option<(String, i64)> = None;
                ui.spacing_mut().slider_width = (ui.available_width() - 70.0).max(80.0);
                for ctl in &controls {
                    ui.label(RichText::new(&ctl.label).size(11.5).color(MU));
                    let mut v = self.sm_ui.cap.control_value(&ctl.key, ctl.value);
                    if ctl.boolean {
                        let mut b = v != 0;
                        if ui.checkbox(&mut b, "Activado").changed() {
                            v = b as i64;
                            changed = Some((ctl.key.clone(), v));
                        }
                    } else if ui.add(egui::Slider::new(&mut v, ctl.min..=ctl.max).step_by(ctl.step as f64)).changed() {
                        changed = Some((ctl.key.clone(), v));
                    }
                }
                if let Some((k, v)) = changed {
                    self.sm_ui.cap.set_control(&k, v);
                }
            } else if self.sm_ui.cap.session.is_some() && self.sm_ui.cap.full.is_some() {
                muted(ui, "Esta cámara no declara controles que se puedan cambiar.");
            }
            ui.add_space(4.0);
            panel_header(ui, "Captura y papel cebolla");
            let c = &mut self.sm_ui.cap;
            ui.checkbox(&mut c.direct, "Animación directa").on_hover_text("Intro y F5 capturan y avanzan sin pedir revisión. Si lo quitas, la foto queda pendiente para incorporarla, repetirla o descartarla");
            ui.checkbox(&mut c.onion, "Foto anterior encima").on_hover_text("Onion skin en vivo: la última foto de la secuencia sobre lo que ve la cámara");
            ui.spacing_mut().slider_width = (ui.available_width() - 20.0).max(80.0);
            ui.add_enabled(c.onion, egui::Slider::new(&mut c.onion_alpha, 0.1..=0.9).show_value(false)).on_hover_text("Opacidad de la foto anterior sobre el Live View");
            ui.add_space(4.0);
            panel_header(ui, "Herramientas del Live View");
            ui.horizontal(|ui| {
                for (t, z) in [("Ajustar", 1.0f32), ("2×", 2.0), ("4×", 4.0)] {
                    if ui.selectable_label((c.zoom - z).abs() < 0.01, t).clicked() {
                        c.zoom = z;
                        c.pan = (0.0, 0.0);
                    }
                }
            });
            muted(ui, "Rueda para acercar, arrastrar para mover y doble clic para ajustar.");
            ui.checkbox(&mut c.thirds, "Cuadrícula de tercios");
            ui.checkbox(&mut c.cross, "Cruz de centro");
            ui.checkbox(&mut c.peaking, "Enfoque asistido").on_hover_text("Marca en magenta los bordes nítidos para ver qué está enfocado. Solo afecta a la vista, no a la foto");
            ui.checkbox(&mut c.flip, "Ver la foto anterior (L)").on_hover_text("Cambia entre la cámara y la última foto sin tocar nada más");
        });
    }
}

impl CapState {
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
