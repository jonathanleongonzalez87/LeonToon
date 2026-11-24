//! Arranque de LeonToon (prototipo v0.3): pantalla de bienvenida con la ilustración del puma
//! y barra de carga, y pantalla de inicio para crear un archivo según el tipo de animación
//! (cada tipo abre su conjunto de salas), abrir recientes o pedir ayuda.

use super::*;
use crate::model::RoomTab;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Stage {
    Splash,
    Home,
    Work,
}

/// Mensajes de la barra de carga (SPMSG del prototipo).
const SPLASH_MSG: [&str; 6] = [
    "Iniciando",
    "Preparando la mesa de luz",
    "Cargando pinceles",
    "Afilando lápices",
    "Ordenando la hoja de exposición",
    "Listo",
];
/// Duración de la carga y pausa final, en segundos (prototipo: 3400 ms + 350 ms).
const SPLASH_SECS: f64 = 3.4;
const SPLASH_HOLD: f64 = 0.35;

const PUMA: &[u8] = include_bytes!("../../assets/puma.webp");

/// Con --screenshot se va directo al trabajo; LEONTOON_INICIO=bienvenida|inicio fuerza otra pantalla.
pub(crate) fn initial_stage(screenshot: bool) -> Stage {
    match std::env::var("LEONTOON_INICIO").ok().as_deref() {
        Some("bienvenida") | Some("splash") => Stage::Splash,
        Some("inicio") | Some("home") => Stage::Home,
        Some(_) => Stage::Work,
        None if screenshot => Stage::Work,
        None => Stage::Splash,
    }
}

pub(crate) fn tabs_for_set(id: &str) -> Vec<RoomTab> {
    room_set(id).rooms.iter().map(|k| RoomTab { kind: k.def().id.into(), name: None }).collect()
}

fn recent_path() -> Option<PathBuf> {
    Some(safety::data_dir()?.join("recientes.json"))
}

pub(crate) fn load_recent() -> Vec<PathBuf> {
    recent_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.is_file())
        .take(8)
        .collect()
}

/// El icono del programa (hexágono del isotipo), dibujado.
/// Textura del isotipo oficial (blanco sobre transparente) para teñirlo del color que se pida.
static ISO_TEX: std::sync::OnceLock<egui::TextureId> = std::sync::OnceLock::new();

/// Carga el isotipo del kit de UI del prototipo (isotipo en 128 px, RGBA sin comprimir).
pub(crate) fn init_iso(ctx: &egui::Context) {
    const RGBA: &[u8] = include_bytes!("../../assets/iso_128.rgba");
    let ci = egui::ColorImage::from_rgba_unmultiplied([128, 128], RGBA);
    let h = ctx.load_texture("iso", ci, egui::TextureOptions::LINEAR);
    let _ = ISO_TEX.set(h.id());
    // La textura vive todo el programa: se deja la referencia sin soltar.
    std::mem::forget(h);
}

pub(crate) fn draw_iso(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    if let Some(id) = ISO_TEX.get() {
        let rect = Rect::from_center_size(c, Vec2::splat(r * 3.0));
        p.image(*id, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), col);
        return;
    }
    let pts: Vec<Pos2> = (0..6)
        .map(|i| {
            let a = std::f32::consts::FRAC_PI_3 * i as f32 - std::f32::consts::FRAC_PI_2;
            c + Vec2::new(a.cos(), a.sin()) * r
        })
        .collect();
    p.add(Shape::closed_line(pts, stk(r * 0.16, col)));
    // Cabeza de puma muy simple: frente y hocico
    p.add(Shape::convex_polygon(
        vec![c + Vec2::new(-r * 0.42, -r * 0.3), c + Vec2::new(r * 0.42, -r * 0.3), c + Vec2::new(r * 0.18, r * 0.42), c + Vec2::new(-r * 0.18, r * 0.42)],
        col,
        EStroke::NONE,
    ));
}

impl LeonApp {
    pub(super) fn remember_recent(&mut self, p: &Path) {
        let p = p.to_path_buf();
        if self.path.as_deref().map_or(false, |q| q.starts_with(std::env::temp_dir())) {
            return;
        }
        self.recent.retain(|q| *q != p);
        self.recent.insert(0, p);
        self.recent.truncate(8);
        if let Some(rp) = recent_path() {
            let _ = std::fs::write(rp, serde_json::to_string_pretty(&self.recent).unwrap_or_default());
        }
    }

    fn puma_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.puma.is_none() {
            if let Ok(img) = image::load_from_memory_with_format(PUMA, image::ImageFormat::WebP) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let ci = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                self.puma = Some(ctx.load_texture("puma", ci, egui::TextureOptions::LINEAR));
            }
        }
        self.puma.clone()
    }

    /// Pantalla de bienvenida: ilustración, nombre, «El León Solitario», porcentaje y mensajes.
    pub(super) fn splash(&mut self, ctx: &egui::Context) {
        let t0 = *self.splash_t0.get_or_insert(self.now);
        let p = ((self.now - t0) / SPLASH_SECS).clamp(0.0, 1.0);
        let e = 1.0 - (1.0 - p).powi(2);
        let done = self.now - t0 >= SPLASH_SECS + SPLASH_HOLD;
        let tex = self.puma_texture(ctx);
        let mut skip = false;
        egui::CentralPanel::default().frame(egui::Frame::default().fill(UI0)).show(ctx, |ui| {
            let r = ui.max_rect();
            let p0 = ui.painter();
            // Fondo radial suave
            for (k, a) in [(0.55, 30u8), (0.35, 26), (0.2, 22)] {
                p0.circle_filled(r.center(), r.width().min(r.height()) * k, Color32::from_rgba_unmultiplied(0x2b, 0x27, 0x24, a));
            }
            let w = (r.width() * 0.6).min(760.0);
            let (iw, ih) = tex.as_ref().map_or((1536.0, 1024.0), |t| (t.size()[0] as f32, t.size()[1] as f32));
            let card = Rect::from_center_size(r.center(), Vec2::new(w, w * ih / iw));
            paint_splash_art(p0, card, tex.as_ref(), Some((e, SPLASH_MSG[((e * (SPLASH_MSG.len() - 1) as f64 + 1e-4).floor() as usize).min(SPLASH_MSG.len() - 1)])));            // Botón «Omitir» abajo a la derecha
            let skip_r = Rect::from_min_size(r.max - Vec2::new(84.0, 50.0), Vec2::new(64.0, 30.0));
            let resp = ui.interact(skip_r, ui.id().with("skip"), Sense::click());
            p0.rect_stroke(skip_r, 15.0, stk(1.0, if resp.hovered() { ACCENT } else { LINE_STRONG }), EStrokeKind::Inside);
            p0.text(skip_r.center(), Align2::CENTER_CENTER, "Omitir", FontId::proportional(12.0), if resp.hovered() { TX } else { MU });
            skip = resp.clicked() || ui.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter));
        });
        if done || skip {
            self.stage = Stage::Home;
        }
        ctx.request_repaint();
    }

    /// Pantalla de inicio sobre la ventana: nuevo archivo por tipo de animación, recientes y ayuda.
    pub(super) fn home(&mut self, ctx: &egui::Context) {
        let tex = self.puma_texture(ctx);
        let screen = ctx.screen_rect();
        // Velo oscuro que además bloquea los clics a la ventana de detrás.
        let veil = egui::Area::new(egui::Id::new("home_veil")).fixed_pos(screen.min).order(egui::Order::Middle).show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha(130));
            resp
        });
        let w = 720.0f32.min(screen.width() * 0.94);
        let mut action: Option<u8> = None;
        let mut pick_set: Option<&'static str> = None;
        let mut pick_recent: Option<PathBuf> = None;
        // El arte del splash conserva su proporción y va FUERA del recuadro, encima de él; si la ventana es baja, se reduce
        // para que el recuadro de listas quepa entero.
        const LISTS_H: f32 = 330.0;
        const GAP: f32 = 14.0;
        let (iw, ih) = tex.as_ref().map_or((1536.0, 1024.0), |t| (t.size()[0] as f32, t.size()[1] as f32));
        let art_w = (screen.width() * 0.6).min(760.0).min(((screen.height() * 0.97 - LISTS_H - GAP).max(140.0)) * iw / ih);
        let art_h = art_w * ih / iw;
        let inner = egui::Area::new(egui::Id::new("home_box"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let (ar, _) = ui.allocate_exact_size(Vec2::new(w.max(art_w), art_h), Sense::hover());
                paint_splash_art(ui.painter(), Rect::from_center_size(ar.center(), Vec2::new(art_w, art_h)), tex.as_ref(), None);
                ui.add_space(GAP);
                egui::Frame::default()
                    .fill(BG2)
                    .stroke(stk(1.0, LINE_STRONG))
                    .corner_radius(12.0)
                    .shadow(egui::Shadow { offset: [0, 18], blur: 50, spread: 0, color: Color32::from_black_alpha(150) })
                    .show(ui, |ui| {
                        ui.set_width(w);
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        // Dos columnas
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            let cw = w * 1.1 / 2.1;
                            ui.vertical(|ui| {
                                ui.set_width(cw);
                                egui::Frame::default().inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
                                    ui.set_width(cw - 28.0);
                                    ui.label(RichText::new("Nuevo archivo según el tipo de animación").size(11.5).color(MU));
                                    ui.add_space(6.0);
                                    for s in ROOM_SETS.iter() {
                                        if home_item(ui, s.name, &format!("{} salas", s.rooms.len())).clicked() {
                                            pick_set = Some(s.id);
                                        }
                                    }
                                });
                            });
                            let (sep, _) = ui.allocate_exact_size(Vec2::new(1.0, 230.0), Sense::hover());
                            ui.painter().rect_filled(sep, 0.0, LINE);
                            ui.vertical(|ui| {
                                ui.set_width(w - cw - 1.0);
                                egui::Frame::default().inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
                                    ui.set_width(w - cw - 29.0);
                                    ui.label(RichText::new("Recientes").size(11.5).color(MU));
                                    ui.add_space(6.0);
                                    if home_item(ui, "Ejemplo: pelota que bota", "demo").clicked() {
                                        action = Some(1);
                                    }
                                    if home_item(ui, "Ejemplo: Stop Motion con plastilina", "demo").clicked() {
                                        action = Some(5);
                                    }
                                    if home_item(ui, "Ejemplo: Animatic con bocetos (A y B)", "demo").clicked() {
                                        action = Some(6);
                                    }
                                    for r in self.recent.clone().into_iter().take(2) {
                                        let name = r.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                        if home_item(ui, &name, "").on_hover_text(r.display().to_string()).clicked() {
                                            pick_recent = Some(r);
                                        }
                                    }
                                    if home_item(ui, "Abrir otro archivo…", "Ctrl+O").clicked() {
                                        action = Some(2);
                                    }
                                    ui.add_space(10.0);
                                    ui.label(RichText::new("Ayuda").size(11.5).color(MU));
                                    ui.add_space(6.0);
                                    if home_item(ui, "Manual de LeonToon", "F1").clicked() {
                                        action = Some(3);
                                    }
                                    if home_item(ui, "Novedades de la versión", "").clicked() {
                                        action = Some(4);
                                    }
                                });
                            });
                        });
                    });
            });
        // Un clic fuera del recuadro de listas cierra la pantalla de inicio.
        let outside = veil.inner.clicked() && !inner.response.rect.contains(ctx.input(|i| i.pointer.interact_pos().unwrap_or_default()));
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        if let Some(id) = pick_set {
            self.new_of_set(id);
        } else if let Some(r) = pick_recent {
            self.stage = Stage::Work;
            self.request(ctx, Pending::OpenPath(r));
        } else {
            match action {
                Some(1) => {
                    self.project = Project::demo();
                    self.project.room_set = "trad".into();
                    self.project.rooms = tabs_for_set("trad");
                    self.path = None;
                    self.after_load();
                    self.say("Ejemplo abierto: una pelota que bota. Pulsa Espacio para reproducir.");
                }
                Some(2) => {
                    self.stage = Stage::Work;
                    self.request(ctx, Pending::Open);
                }
                Some(5) => self.open_stopmotion_demo(),
                Some(6) => self.open_animatic_demo(),
                Some(3) => {
                    self.stage = Stage::Work;
                    self.show_help = true;
                }
                Some(4) => {
                    self.stage = Stage::Work;
                    self.show_about = true;
                }
                _ => {
                    if outside || esc {
                        self.stage = Stage::Work;
                    }
                }
            }
        }
    }

    /// Ejemplo de Stop Motion: las fotos de la secuencia se copian a la carpeta de datos y se abre la sala Stop Motion.
    pub(crate) fn open_stopmotion_demo(&mut self) {
        let Some(dir) = safety::data_dir().map(|d| d.join("muestras").join("stopmotion")) else {
            self.say("No se encontró dónde guardar las fotos del ejemplo.");
            return;
        };
        match crate::demo_sm::build(&dir) {
            Ok(mut p) => {
                p.room_set = "stopmotion".into();
                p.rooms = tabs_for_set("stopmotion");
                self.project = p;
                self.path = None;
                self.after_load();
                self.stage = Stage::Work;
                self.goto_room_kind(RoomKind::StopMotion);
                self.say("Ejemplo abierto: una secuencia de plastilina. Pulsa Espacio para reproducirla.");
            }
            Err(e) => self.say(format!("No se pudo abrir el ejemplo: {e}")),
        }
    }

    /// Ejemplo de animatic: la historia de la pelota con bocetos en mapa de bits (Personaje A y Personaje B), con el animatic creado.
    pub(crate) fn open_animatic_demo(&mut self) {
        let audio_dir = safety::data_dir().map(|d| d.join("muestras").join("animatic")).unwrap_or_else(|| std::env::temp_dir().join("leontoon_muestras_animatic"));
        let mut p = crate::demo_animatic::build_with_audio(&audio_dir);
        p.room_set = "animatic".into();
        p.rooms = tabs_for_set("animatic");
        self.project = p;
        self.path = None;
        self.after_load();
        self.stage = Stage::Work;
        self.goto_room_kind(RoomKind::Animatic);
        self.say("Ejemplo abierto: la historia de la pelota en bocetos. El Personaje A es la pelota roja y el B la azul; Tab y Mayús+Tab entran y salen del animatic.");
    }

    /// Archivo nuevo del tipo elegido: escena vacía con su conjunto de salas.
    fn new_of_set(&mut self, id: &str) {
        if self.dirty {
            // Primero se ofrece guardar; el tipo se aplica al proyecto nuevo del diálogo.
            self.say("Guarda o descarta los cambios antes de crear un archivo nuevo.");
            self.stage = Stage::Work;
            self.pending = Some(Pending::New);
            return;
        }
        let mut p = Project::blank(1280, 720, 24, 48);
        p.room_set = id.into();
        p.rooms = tabs_for_set(id);
        if id == "story" || id == "animatic" {
            p.storyboard = (1..=3).map(crate::model::Panel::new).collect();
        }
        self.project = p;
        self.path = None;
        self.after_load();
        self.say(format!("Nuevo archivo: {}. Las salas se crearon para ese tipo de animación.", room_set(id).name));
    }
}

/// La ilustración del splash con su franja (nombre, lema y, mientras carga, porcentaje, mensaje y barra). La usan la
/// pantalla de bienvenida y la de inicio, para que el arte conserve siempre el mismo aspecto.
fn paint_splash_art(p0: &egui::Painter, card: Rect, tex: Option<&egui::TextureHandle>, progress: Option<(f64, &str)>) {
    let (w, h) = (card.width(), card.height());
    if let Some(t) = tex {
        p0.image(t.id(), card, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    } else {
        p0.rect_filled(card, 12.0, BG3);
    }
    // Franja inferior con nombre, porcentaje y mensaje (como .sp-strip)
    let strip = Rect::from_min_size(Pos2::new(card.min.x + w * 0.0768, card.min.y + h * 0.824), Vec2::new(w * 0.8464, h * 0.14));
    let dark = Color32::from_rgb(0x3b, 0x2b, 0x1f);
    let warm = Color32::from_rgb(0x8a, 0x5a, 0x35);
    let pad = strip.width() * 0.034;
    let name_size = w * 0.036;
    let g1 = p0.layout_no_wrap("Leon".into(), FontId::proportional(name_size), dark);
    let base = Pos2::new(strip.min.x + pad, strip.min.y + strip.height() * 0.18);
    let lw = g1.size().x;
    p0.galley(base, g1, dark);
    p0.text(base + Vec2::new(lw, 0.0), Align2::LEFT_TOP, "Toon", FontId::proportional(name_size), warm);
    p0.text(base + Vec2::new(0.0, name_size * 1.25), Align2::LEFT_TOP, "El León Solitario", FontId::proportional(w * 0.015), warm);
    if let Some((e, msg)) = progress {
        let right = Pos2::new(strip.max.x - pad, strip.min.y + strip.height() * 0.2);
        p0.text(right, Align2::RIGHT_TOP, format!("{:.0} %", e * 100.0), FontId::proportional(w * 0.016), dark);
        p0.text(right + Vec2::new(0.0, w * 0.022), Align2::RIGHT_TOP, msg, FontId::proportional(w * 0.0135), warm);
        let bar = Rect::from_min_max(Pos2::new(strip.min.x + pad, strip.max.y - strip.height() * 0.14 - w * 0.0035), Pos2::new(strip.max.x - pad, strip.max.y - strip.height() * 0.14));
        p0.rect_filled(bar, 2.0, Color32::from_rgba_unmultiplied(59, 43, 31, 30));
        p0.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * e as f32, bar.height())), 2.0, ACCENT);
    }
    p0.text(
        Pos2::new(card.max.x - w * 0.03, card.min.y + h * 0.02),
        Align2::RIGHT_TOP,
        format!("v{}", env!("CARGO_PKG_VERSION")),
        FontId::proportional(w * 0.013),
        Color32::from_rgba_unmultiplied(242, 232, 218, 150),
    );
}

fn home_item(ui: &mut egui::Ui, label: &str, small: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 25.0), Sense::click());
    let hov = resp.hovered();
    if hov {
        ui.painter().rect_filled(r, 4.0, ACCENT);
    }
    let fg = if hov { ACCENT_INK } else { TX };
    ui.painter().text(r.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(13.0), fg);
    if !small.is_empty() {
        ui.painter().text(r.right_center() - Vec2::new(6.0, 0.0), Align2::RIGHT_CENTER, small, FontId::proportional(11.0), if hov { ACCENT_INK } else { INK3 });
    }
    resp
}
