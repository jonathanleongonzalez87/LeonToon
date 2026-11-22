//! Tamaños de los paneles que la persona puede cambiar arrastrando su borde (Issue #44): barra de herramientas (T),
//! barra lateral (N), columna de capas y propiedades, reparto entre capas y propiedades, y línea de tiempo u hoja de
//! exposición. Los tamaños tienen mínimo y máximo, se ajustan al tamaño de la ventana sin tocar lo guardado, y se
//! recuerdan entre sesiones en `interfaz.json` (carpeta de datos de LeonToon). Doble clic en un borde lo restablece.

use super::*;
use serde::{Deserialize, Serialize};

/// Ancho mínimo que debe conservar la zona de dibujo, en píxeles.
const MIN_VIEWER: f32 = 360.0;

/// Tamaños elegidos por la persona. `0` en `timeline` y `layers` significa «el tamaño por defecto».
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct PanelSizes {
    /// Ancho de la barra de herramientas (T).
    pub toolbar: f32,
    /// Ancho de la columna de capas y propiedades.
    pub right: f32,
    /// Ancho de la barra lateral (N).
    pub n: f32,
    /// Alto de la línea de tiempo (0 = el de la sala).
    pub timeline: f32,
    /// Ancho de la hoja de exposición cuando se ve al lado del visor.
    pub sheet: f32,
    /// Alto de la lista de capas dentro de la columna derecha (0 = según el número de capas).
    pub layers: f32,
}

impl Default for PanelSizes {
    fn default() -> Self {
        PanelSizes { toolbar: 44.0, right: 268.0, n: 236.0, timeline: 0.0, sheet: 330.0, layers: 0.0 }
    }
}

impl PanelSizes {
    pub const TOOLBAR: (f32, f32) = (44.0, 92.0);
    pub const RIGHT: (f32, f32) = (220.0, 560.0);
    pub const N: (f32, f32) = (200.0, 520.0);
    pub const TIMELINE_MIN: f32 = 110.0;
    pub const SHEET: (f32, f32) = (220.0, 700.0);
    pub const LAYERS_MIN: f32 = 90.0;

    /// Valores en rango; un número roto (NaN o infinito) vuelve al valor por defecto.
    pub fn sanitized(mut self) -> Self {
        let d = PanelSizes::default();
        let f = |v: f32, lo: f32, hi: f32, def: f32| if v.is_finite() { v.clamp(lo, hi) } else { def };
        self.toolbar = f(self.toolbar, Self::TOOLBAR.0, Self::TOOLBAR.1, d.toolbar);
        self.right = f(self.right, Self::RIGHT.0, Self::RIGHT.1, d.right);
        self.n = f(self.n, Self::N.0, Self::N.1, d.n);
        self.sheet = f(self.sheet, Self::SHEET.0, Self::SHEET.1, d.sheet);
        self.timeline = if self.timeline.is_finite() && self.timeline > 0.0 { self.timeline.clamp(Self::TIMELINE_MIN, 2000.0) } else { 0.0 };
        self.layers = if self.layers.is_finite() && self.layers > 0.0 { self.layers.clamp(Self::LAYERS_MIN, 2000.0) } else { 0.0 };
        self
    }

    /// Los tamaños con los que se dibuja ahora: los elegidos, recortados para que la zona de dibujo siga siendo usable en
    /// la ventana actual. No cambia lo guardado: al agrandar la ventana vuelve el tamaño elegido.
    pub fn effective(&self, win: Vec2) -> PanelSizes {
        let mut s = self.sanitized();
        s.right = s.right.min((win.x - s.toolbar - MIN_VIEWER).max(Self::RIGHT.0));
        s.sheet = s.sheet.min((win.x * 0.6).max(Self::SHEET.0));
        // Los controles flotantes del visor (zoom, 2D/3D, mesa de luz, guías y papel cebolla) necesitan unos 880 px.
        s.n = s.n.min((win.x - s.right - s.toolbar - 880.0).max(Self::N.0));
        if s.timeline > 0.0 {
            s.timeline = s.timeline.min((win.y * 0.6).max(Self::TIMELINE_MIN));
        }
        if s.layers > 0.0 {
            s.layers = s.layers.min((win.y - 260.0).max(Self::LAYERS_MIN));
        }
        s
    }

    /// Lee `key=valor,key=valor` (para capturas de prueba): toolbar, right, n, timeline, sheet, layers.
    pub fn from_spec(spec: &str) -> PanelSizes {
        let mut s = PanelSizes::default();
        for part in spec.split(',') {
            let Some((k, v)) = part.split_once('=') else { continue };
            let Ok(v) = v.trim().parse::<f32>() else { continue };
            match k.trim() {
                "toolbar" => s.toolbar = v,
                "right" => s.right = v,
                "n" => s.n = v,
                "timeline" => s.timeline = v,
                "sheet" => s.sheet = v,
                "layers" => s.layers = v,
                _ => {}
            }
        }
        s.sanitized()
    }
}

const FILE: &str = "interfaz.json";

/// Lee los tamaños guardados; si no hay archivo o está dañado, los de por defecto (nunca falla).
pub(super) fn load(dir: &Path) -> PanelSizes {
    std::fs::read_to_string(dir.join(FILE)).ok().and_then(|s| serde_json::from_str::<PanelSizes>(&s).ok()).unwrap_or_default().sanitized()
}

/// Guarda los tamaños (primero en un archivo temporal, para no dejarlos a medias).
pub(super) fn save(dir: &Path, sizes: &PanelSizes) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&sizes.sanitized()).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join(FILE)).map_err(|e| e.to_string())
}

/// Qué pasó con un borde arrastrable en este fotograma.
pub(super) enum Resize {
    None,
    /// Se está arrastrando: posición del puntero.
    Drag(Pos2),
    /// Se soltó tras arrastrar (hay que guardar).
    Done,
    /// Doble clic: volver al tamaño por defecto.
    Reset,
}

/// Borde arrastrable. `vertical`: es una línea vertical (cambia un ancho); si no, horizontal (cambia un alto).
/// El área sensible es `rect`; al pasar el cursor se marca con una línea y cambia el puntero.
pub(super) fn resize_handle(ui: &mut egui::Ui, salt: &str, rect: Rect, vertical: bool) -> Resize {
    let resp = ui.interact(rect, ui.id().with(("resize", salt)), Sense::click_and_drag());
    let active = resp.hovered() || resp.dragged();
    if active {
        ui.ctx().set_cursor_icon(if vertical { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical });
        let c = rect.center();
        let line = if vertical { [Pos2::new(c.x, rect.min.y), Pos2::new(c.x, rect.max.y)] } else { [Pos2::new(rect.min.x, c.y), Pos2::new(rect.max.x, c.y)] };
        ui.painter().line_segment(line, stk(2.0, with_alpha(ACCENT, if resp.dragged() { 0.9 } else { 0.55 })));
    }
    if resp.double_clicked() {
        return Resize::Reset;
    }
    if resp.dragged() {
        if let Some(p) = resp.interact_pointer_pos() {
            return Resize::Drag(p);
        }
    }
    if resp.drag_stopped() {
        return Resize::Done;
    }
    Resize::None
}

impl LeonApp {
    /// Guarda los tamaños en el equipo (las capturas de prueba no tocan el archivo).
    pub(super) fn save_panel_sizes(&mut self) {
        if self.shot.is_some() {
            return;
        }
        if let Some(dir) = safety::data_dir() {
            if let Err(e) = save(&dir, &self.sizes) {
                safety::log_line(&format!("No se pudieron guardar los tamaños de los paneles: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_old_fixed_layout() {
        let d = PanelSizes::default();
        assert_eq!((d.toolbar, d.right, d.n, d.sheet), (44.0, 268.0, 236.0, 330.0));
        assert_eq!((d.timeline, d.layers), (0.0, 0.0));
        assert_eq!(d.sanitized(), d, "los valores por defecto están dentro de los límites");
    }

    #[test]
    fn sizes_stay_inside_their_limits() {
        let s = PanelSizes { toolbar: 5.0, right: 9999.0, n: -3.0, timeline: 10.0, sheet: 50.0, layers: 5.0 }.sanitized();
        assert_eq!(s.toolbar, PanelSizes::TOOLBAR.0);
        assert_eq!(s.right, PanelSizes::RIGHT.1);
        assert_eq!(s.n, PanelSizes::N.0);
        assert_eq!(s.timeline, PanelSizes::TIMELINE_MIN);
        assert_eq!(s.sheet, PanelSizes::SHEET.0);
        assert_eq!(s.layers, PanelSizes::LAYERS_MIN);
        let broken = PanelSizes { toolbar: f32::NAN, right: f32::INFINITY, n: f32::NEG_INFINITY, timeline: f32::NAN, sheet: f32::NAN, layers: f32::NAN }.sanitized();
        assert_eq!(broken, PanelSizes::default());
    }

    #[test]
    fn the_drawing_area_always_keeps_room_in_a_small_window() {
        // Ventana mínima del programa (960×620) con todo al máximo.
        let all_max = PanelSizes { toolbar: 92.0, right: 560.0, n: 520.0, timeline: 2000.0, sheet: 700.0, layers: 2000.0 };
        let e = all_max.effective(Vec2::new(960.0, 620.0));
        assert!(960.0 - e.toolbar - e.right >= 360.0 - 0.5, "queda poco espacio para dibujar: {e:?}");
        assert_eq!(e.n, PanelSizes::N.0, "en la ventana mínima la barra N queda en su mínimo");
        // En una ventana de 1800 px, la barra N deja sitio a los controles flotantes del visor.
        let wide = all_max.effective(Vec2::new(1800.0, 1000.0));
        assert!(1800.0 - wide.right - wide.toolbar - wide.n >= 880.0 - 0.5, "{wide:?}");
        assert!(e.timeline <= 620.0 * 0.6 + 0.5 && e.layers <= 620.0 - 260.0 + 0.5);
        // Lo guardado no cambia: en una ventana grande vuelve el tamaño elegido.
        assert_eq!(all_max.effective(Vec2::new(3000.0, 2000.0)).right, 560.0);
        // Con los tamaños normales, una ventana normal no recorta nada.
        let d = PanelSizes::default();
        assert_eq!(d.effective(Vec2::new(1440.0, 900.0)), d);
    }

    #[test]
    fn sizes_are_remembered_between_sessions_and_a_damaged_file_is_harmless() {
        let dir = std::env::temp_dir().join(format!("leontoon_paneles_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = PanelSizes { toolbar: 60.0, right: 340.0, n: 300.0, timeline: 260.0, sheet: 400.0, layers: 180.0 };
        save(&dir, &s).unwrap();
        assert_eq!(load(&dir), s);
        assert!(!dir.join("interfaz.json.tmp").exists());
        // Un archivo dañado o de otra versión: valores por defecto o los que se puedan leer, sin fallar.
        std::fs::write(dir.join("interfaz.json"), "esto no es json").unwrap();
        assert_eq!(load(&dir), PanelSizes::default());
        std::fs::write(dir.join("interfaz.json"), r#"{"right": 400.0, "campo_futuro": true}"#).unwrap();
        let partial = load(&dir);
        assert_eq!((partial.right, partial.n), (400.0, 236.0));
        std::fs::write(dir.join("interfaz.json"), r#"{"right": 99999.0, "n": null}"#).unwrap();
        assert!(load(&dir).right <= PanelSizes::RIGHT.1);
        assert_eq!(load(&dir.join("no_existe")), PanelSizes::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn spec_for_screenshots_parses_and_ignores_garbage() {
        let s = PanelSizes::from_spec("toolbar=60, right=340,n=300,timeline=260,sheet=400,layers=180");
        assert_eq!(s, PanelSizes { toolbar: 60.0, right: 340.0, n: 300.0, timeline: 260.0, sheet: 400.0, layers: 180.0 });
        assert_eq!(PanelSizes::from_spec("basura,right=abc,otro=3"), PanelSizes::default());
    }

    // ---- El borde arrastrable con un puntero simulado (egui sin ventana) ----

    fn frame(ctx: &egui::Context, t: f64, events: Vec<egui::Event>, bar: Rect, vertical: bool) -> Resize {
        let mut raw = egui::RawInput::default();
        raw.time = Some(t);
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        raw.events = events;
        let mut out = Resize::None;
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| out = resize_handle(ui, "prueba", bar, vertical));
        });
        out
    }

    fn mv(p: Pos2) -> egui::Event {
        egui::Event::PointerMoved(p)
    }

    fn click(p: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
    }

    #[test]
    fn dragging_a_border_reports_the_pointer_and_releasing_asks_to_save() {
        let ctx = egui::Context::default();
        let bar = Rect::from_center_size(Pos2::new(400.0, 300.0), Vec2::new(8.0, 400.0));
        let start = Pos2::new(400.0, 300.0);
        assert!(matches!(frame(&ctx, 0.0, vec![mv(start)], bar, true), Resize::None));
        assert!(matches!(frame(&ctx, 0.05, vec![], bar, true), Resize::None));
        frame(&ctx, 0.10, vec![click(start, true)], bar, true);
        let mut last = None;
        for (i, x) in [405.0, 420.0, 450.0, 480.0].into_iter().enumerate() {
            if let Resize::Drag(p) = frame(&ctx, 0.15 + i as f64 * 0.05, vec![mv(Pos2::new(x, 300.0))], bar, true) {
                last = Some(p);
            }
        }
        let p = last.expect("arrastrar el borde debería dar la posición del puntero");
        assert!((p.x - 480.0).abs() < 1.0, "posición: {p:?}");
        assert!(matches!(frame(&ctx, 0.5, vec![click(Pos2::new(480.0, 300.0), false)], bar, true), Resize::Done), "al soltar hay que guardar");
    }

    #[test]
    fn double_click_on_a_border_asks_for_the_default_size() {
        let ctx = egui::Context::default();
        let bar = Rect::from_center_size(Pos2::new(300.0, 200.0), Vec2::new(400.0, 8.0));
        let at = Pos2::new(300.0, 200.0);
        frame(&ctx, 0.0, vec![mv(at)], bar, false);
        frame(&ctx, 0.05, vec![], bar, false);
        frame(&ctx, 0.10, vec![click(at, true)], bar, false);
        frame(&ctx, 0.15, vec![click(at, false)], bar, false);
        frame(&ctx, 0.20, vec![click(at, true)], bar, false);
        let r = frame(&ctx, 0.25, vec![click(at, false)], bar, false);
        assert!(matches!(r, Resize::Reset), "doble clic debe restablecer");
    }

    #[test]
    fn a_pointer_far_from_the_border_does_nothing() {
        let ctx = egui::Context::default();
        let bar = Rect::from_center_size(Pos2::new(400.0, 300.0), Vec2::new(8.0, 400.0));
        let far = Pos2::new(100.0, 300.0);
        frame(&ctx, 0.0, vec![mv(far)], bar, true);
        frame(&ctx, 0.05, vec![], bar, true);
        frame(&ctx, 0.10, vec![click(far, true)], bar, true);
        for (i, x) in [120.0, 200.0, 300.0].into_iter().enumerate() {
            assert!(matches!(frame(&ctx, 0.15 + i as f64 * 0.05, vec![mv(Pos2::new(x, 300.0))], bar, true), Resize::None));
        }
        assert!(matches!(frame(&ctx, 0.4, vec![click(Pos2::new(300.0, 300.0), false)], bar, true), Resize::None));
    }
}