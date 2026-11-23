//! Interfaz de LeonToon: estado de la aplicación, salas y ciclo principal.

mod catalog;
mod canvas;
mod dialogs;
mod edit;
mod icons;
mod palette;
mod panels;
mod rooms;
mod safety;
mod start;
mod select;
mod shell;
mod storyboard;
mod abrui;
mod brushui;
mod vectorui;
mod strokeui;
mod penui;
#[cfg(test)]
mod flow_tests;
mod papers;
mod panelsize;
mod context;
mod layersel;
mod montage;
mod smcap;
mod smdragon;
mod slots;
mod smroom;
mod smui;
mod smview;
mod hud;
mod view3d;
mod timebar;
mod timeline;

pub use safety::install_panic_hook;

use crate::model::{Drawing, Interp, Layer, LayerKind, Project, Rgba, ShapeKind, Stroke, XKey};
use crate::tracks::Prop;
use crate::render;
use eframe::egui::{
    self, Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, RichText, Sense, Shape, Stroke as EStroke,
    StrokeKind as EStrokeKind, Vec2,
};
use catalog::*;
use icons::*;
use std::path::{Path, PathBuf};

// ---------- Colores de la interfaz (prototipo de interfaz v0.3 de LeonToon) ----------
const UI0: Color32 = Color32::from_rgb(0x14, 0x13, 0x12);
const BG: Color32 = Color32::from_rgb(0x1a, 0x19, 0x18);
const BG2: Color32 = Color32::from_rgb(0x21, 0x1f, 0x1e);
const BG3: Color32 = Color32::from_rgb(0x2a, 0x28, 0x26);
const BG4: Color32 = Color32::from_rgb(0x34, 0x31, 0x2e);
const LINE: Color32 = Color32::from_rgb(0x2e, 0x2b, 0x29);
const LINE_STRONG: Color32 = Color32::from_rgb(0x3a, 0x36, 0x33);
const TX: Color32 = Color32::from_rgb(0xf2, 0xe8, 0xda);
const MU: Color32 = Color32::from_rgb(0xb5, 0xa8, 0x98);
const INK3: Color32 = Color32::from_rgb(0x85, 0x7a, 0x6e);
const ACCENT: Color32 = Color32::from_rgb(0xfd, 0x67, 0x24);
const ACCENT2: Color32 = Color32::from_rgb(0xff, 0x8a, 0x52);
const ACCENT_INK: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
const VIEWER_BG: Color32 = Color32::from_rgb(0x1a, 0x19, 0x18);
const SEL_BG: Color32 = Color32::from_rgb(0x44, 0x2b, 0x1f);
const CELL_SEL: Color32 = Color32::from_rgb(0x5a, 0x33, 0x20);
const KEY_GOLD: Color32 = Color32::from_rgb(0xff, 0xcc, 0x44);/// Colores del papel cebolla del prototipo v0.3: anterior en ámbar, siguiente en azul
/// (el original usa rojo 255,0,0 y verde 55,120,55 en prefs.xml).
const ONION_BEFORE: Color32 = Color32::from_rgb(0xfd, 0x67, 0x24);
const ONION_AFTER: Color32 = Color32::from_rgb(0x5f, 0x8f, 0xb4);
/// Opacidad del papel cebolla: del más cercano al más lejano (prefs.xml del original).
const ONION_MAX_OPACITY: f32 = 0.8;
const ONION_MIN_OPACITY: f32 = 0.2;
/// Hasta 10 dibujos por lado (0x1404dbbc0 reserva 10 pesos y activa la máscara 0x3ff).
const ONION_LIMIT: u32 = 10;
/// Alcance máximo de los deslizadores del papel cebolla en el prototipo v0.3 (0 a 12).
const ONION_UI_MAX: u32 = 12;
/// Peso por distancia del original (0x1404dbbc0): empieza en 1.0 y baja 1/10 por dibujo.
fn onion_slot_weight(d: u32) -> f32 {
    (1.0 - (d.max(1) - 1) as f32 / ONION_LIMIT as f32).max(0.0)
}
/// Opacidad del dibujo a distancia d: entre la mínima y la máxima (0x1404e1200) según su peso.
fn onion_original_opacity(d: u32) -> f32 {
    ONION_MIN_OPACITY + (ONION_MAX_OPACITY - ONION_MIN_OPACITY) * onion_slot_weight(d)
}

/// Niveles de zoom del visor del original (tabla de textos en 0x140c1b840…0x140c1b9a0, en %).
const ZOOM_LADDER: [f32; 50] = [
    1.0, 1.25, 1.6, 2.0, 2.5, 3.3, 4.0, 5.0, 6.3, 8.0, 10.0, 12.5, 16.0, 20.0, 25.0, 32.0, 40.0, 50.0, 63.0, 80.0, 100.0,
    125.0, 160.0, 200.0, 250.0, 320.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3200.0,
    4000.0, 5000.0, 6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0, 25000.0, 32000.0, 40000.0, 50000.0, 63000.0,
    80000.0,
];
/// Siguiente nivel de zoom de la escalera (dir = +1 acercar, -1 alejar), desde un zoom cualquiera.
fn zoom_ladder_step(zoom: f32, dir: i32) -> f32 {
    let pct = zoom * 100.0;
    let next = if dir > 0 {
        ZOOM_LADDER.iter().copied().find(|z| *z > pct * 1.001)
    } else {
        ZOOM_LADDER.iter().rev().copied().find(|z| *z < pct * 0.999)
    };
    next.unwrap_or(pct) / 100.0
}
/// Xsheet: alto de fila 17 px a zoom 10, zoom de 5 a 16 (0x140106d90 y prefs.xml XSHEET_MIN/MAX_ZOOM).
const XS_ROW_BASE: f32 = 17.0;
/// Una muesca de la rueda en puntos de egui; el original da un paso cada 2×WHEEL_ZOOM_THRESHOLD = 100 unidades Qt (una muesca = 120).
const WHEEL_NOTCH: f32 = 40.0;
const XS_ZOOM_DEFAULT: f32 = 10.0;
const XS_ZOOM_MIN: f32 = 5.0;
const XS_ZOOM_MAX: f32 = 16.0;

fn c32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}
fn rgba(c: Color32) -> Rgba {
    c.to_srgba_unmultiplied()
}
fn stk(w: f32, c: Color32) -> EStroke {
    EStroke::new(w, c)
}
fn with_alpha(c: Color32, a: f32) -> Color32 {
    let [r, g, b, al] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (al as f32 * a.clamp(0.0, 1.0)).round() as u8)
}
/// Curva de salida suave para los paneles que entran y salen (0,18 s, como el prototipo).
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}
/// Avance 0…1 de un panel que se muestra u oculta con movimiento fluido.
fn panel_t(ctx: &egui::Context, id: &str, on: bool) -> f32 {
    ctx.animate_bool_with_time_and_easing(egui::Id::new(("panel_t", id)), on, 0.18, ease_out_cubic)
}
fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

enum Gesture {
    Paint { stroke: Stroke, smooth: [f32; 2], last_raw: [f32; 2] },
    Shape { kind: ShapeKind, start: [f32; 2], stroke: Stroke },
    Erase { last: [f32; 2] },
    Move { last: [f32; 2] },
    /// Recuadro de selección (en coordenadas del dibujo).
    Marquee { start: [f32; 2], cur: [f32; 2], add: bool },
    /// Escalar lo elegido: punto fijo y último punto.
    SelScale { anchor: [f32; 2], last: [f32; 2], keep: bool },
    /// Girar lo elegido alrededor de su centro.
    SelRotate { center: [f32; 2], last: f32 },
    Animate { start: [f32; 2], base: XKey, mode: u8 },
    Pick,
    /// Pluma: arrastre tras el clic que puso el nodo idx.
    PenNode { idx: usize },
    /// Edición de nodos: arrastre del nodo idx (part 0) o de su tirador (1 entrada, 2 salida).
    NodeDrag { idx: usize, part: u8 },
}

#[derive(Clone)]
enum Pending {
    New,
    Open,
    OpenPath(PathBuf),
    Recover,
    Quit,
}

struct Shot {
    path: PathBuf,
    frames: u32,
    requested: bool,
}

/// Ajustes "ligeros" del proyecto, para poder deshacer cambios hechos en los paneles.
#[derive(Clone, PartialEq)]
struct Meta {
    size: (u32, u32, u32, u32),
    bg: Rgba,
    palettes: Vec<crate::model::NamedPalette>,
    layers: Vec<(String, bool, bool, f32, crate::tracks::Tracks, (u32, Option<u32>, bool, bool, u8, crate::model::Blend))>,
    sb: Vec<[String; 6]>,
    durs: Vec<u32>,
    range: Option<(u32, u32)>,
}

fn meta_of(p: &Project) -> Meta {
    Meta {
        size: (p.width, p.height, p.fps, p.length),
        bg: p.background,
        palettes: p.palettes.clone(),
        layers: p.layers.iter().map(|l| (l.name.clone(), l.visible, l.locked, l.opacity, l.tracks.clone(), (l.in_f, l.out_f, l.solo, l.shy, l.label, l.blend))).collect(),
        sb: p
            .storyboard
            .iter()
            .map(|s| [s.title.clone(), s.action.clone(), s.dialogue.clone(), s.camera.clone(), s.trans.clone(), s.notes.clone()])
            .collect(),
        durs: p.storyboard.iter().map(|s| (s.dur * 1000.0).round() as u32).collect(),
        range: p.range,
    }
}

fn apply_meta(p: &mut Project, m: &Meta) {
    (p.width, p.height, p.fps, p.length) = m.size;
    p.background = m.bg;
    p.palettes = m.palettes.clone();
    p.range = m.range;
    for (l, (n, v, lk, o, x, bar)) in p.layers.iter_mut().zip(&m.layers) {
        l.name = n.clone();
        l.visible = *v;
        l.locked = *lk;
        l.opacity = *o;
        l.tracks = x.clone();
        (l.in_f, l.out_f, l.solo, l.shy, l.label, l.blend) = *bar;
    }
    for ((s, t), d) in p.storyboard.iter_mut().zip(&m.sb).zip(&m.durs) {
        [s.title, s.action, s.dialogue, s.camera, s.trans, s.notes] = t.clone();
        s.dur = *d as f32 / 1000.0;
    }
}

/// Opciones de la sala Render.
struct RenderOpts {
    a: u32,
    b: u32,
    scale: f32,
    transparent: bool,
    columns: u32,
    message: String,
}

pub struct LeonApp {
    project: Project,
    path: Option<PathBuf>,
    dirty: bool,
    undo: Vec<Project>,
    redo: Vec<Project>,
    meta_skip: bool,
    last_meta_change: u64,
    frame_no: u64,

    /// Pestaña de sala activa (índice en project.rooms).
    room_idx: usize,
    /// Arranque: bienvenida, inicio o trabajo.
    stage: start::Stage,
    splash_t0: Option<f64>,
    puma: Option<egui::TextureHandle>,
    recent: Vec<PathBuf>,
    /// Modo objeto (true) o modo edición (false); Tab cambia.
    obj_mode: bool,
    show_toolbar: bool,
    show_n: bool,
    n_tab: u8,
    /// Editor elegido en el panel de propiedades (None = el de la sala).
    /// Editor elegido en cada hueco de cada sala (lo que no está aquí usa el del preset de la sala). Se guarda en editores.json.
    slot_editors: std::collections::HashMap<(RoomKind, Slot), Editor>,
    /// Hueco que se está dibujando (para que los editores con cabecera propia sepan de qué hueco es el selector).
    cur_slot: Slot,
    /// Ventana emergente abierta en el visor: 1 mesa de luz, 2 guías, 3 papel cebolla.
    hud_pop: u8,
    /// Mesa de luz: gris del fondo (0 negro…100 blanco), cuadrícula de transparencia, barra de registro.
    light_gray: f32,
    show_alpha: bool,
    pegbar: bool,
    /// Mesa de luz (prototipo v0.3): luz 0…100, temperatura 0…100 (cálida→fría),
    /// tamaño y contraste de la cuadrícula de transparencia, opacidad de la vista.
    light_amt: f32,
    light_temp: f32,
    chk_size: f32,
    chk_con: f32,
    view_op: f32,
    /// Menú «Agregar capa» (Mayús+A): posición en pantalla donde se abrió.
    add_menu: Option<Pos2>,
    /// Vista 3D (planos en profundidad): activa, modo (0 cámara, 1 usuario, 2 superior, 3 lateral…),
    /// giro, proyección ortográfica, desplazamiento y zoom.
    /// Pincel de píxeles: forma de la punta (dureza, espaciado, flujo, redondez, ángulo),
    /// trazo en curso, su textura de vista previa y la imagen de muestra del panel N.
    brush: crate::brush::BrushParams,
    brush_live: Option<crate::brush::LiveStroke>,
    /// Estilo del Lápiz vectorial (extremos, uniones, perfil de grosor y presión). Es independiente de los pinceles de píxeles.
    vstyle: crate::vector::VStyle,
    /// Biblioteca de pinceles vectoriales (independiente de la de píxeles), el pincel elegido y el estado de su panel.
    vlib: crate::vector::library::VLibrary,
    vpreset: Option<String>,
    vui: vectorui::VUi,
    /// Edición de trazos vectoriales ya dibujados (panel contextual de la Selección).
    sui: strokeui::StrokeUi,
    /// Curva de la Pluma en curso y nodo elegido al editar una curva.
    /// Texturas de grano de los papeles de estudio (se suben una vez).
    paper_tex: std::collections::HashMap<crate::paper::Grain, egui::TextureHandle>,
    /// Escala del grano (porcentaje) y semilla de su variante.
    paper_scale: f32,
    paper_seed: u32,
    /// Tamaños de los paneles elegidos por la persona (se guardan) y los que se usan ahora según la ventana.
    sizes: panelsize::PanelSizes,
    sz: panelsize::PanelSizes,
    /// Secuencias Stop Motion abiertas (por identificador) y sus miniaturas como texturas.
    sm: std::collections::HashMap<String, smui::SmView>,
    sm_thumbs: std::collections::HashMap<(String, String), egui::TextureHandle>,
    /// Fotos de Stop Motion para el visor: cargador en segundo plano, texturas (con el último cuadro en que se usaron) y las que fallaron.
    sm_loader: crate::stopmotion::Loader,
    /// Miniaturas del Frame Browser (hilo aparte) y estado de la interfaz de la sala Stop Motion.
    sm_tloader: crate::stopmotion::Loader,
    sm_ui: smroom::SmUi,
    sm_tex: std::collections::HashMap<crate::stopmotion::Key, (egui::TextureHandle, u64)>,
    sm_failed: std::collections::HashSet<crate::stopmotion::Key>,
    pen_path: Option<penui::PenPath>,
    sel_node: Option<usize>,
    /// Última presión leída del lápiz (1 si no hay lápiz).
    pen_pressure: f32,
    /// Imágenes cargadas para la punta y para la textura del pincel (nombre, máscara).
    brush_tip_img: Option<brushui::BrushImg>,
    brush_tex_img: Option<brushui::BrushImg>,
    /// Preset del que viene el pincel en uso (categoría y nombre) y editor de presets del panel N.
    brush_preset: Option<crate::presets::PresetKey>,
    preset_ui: brushui::PresetUi,
    /// Pinceles eliminados o renombrados que hay que quitar de la biblioteca del usuario, y su ventana.
    brush_lib_removed: Vec<crate::presets::PresetKey>,
    lib_window: Option<crate::presets::Library>,
    /// Ventanas de importar y exportar pinceles de Photoshop (.abr) y su informe.
    abr_import: Option<abrui::ImportUi>,
    abr_export: Option<abrui::ExportUi>,
    abr_report: Option<abrui::ReportUi>,
    /// Máscaras ya decodificadas de las imágenes guardadas en el proyecto.
    brush_img_cache: std::collections::HashMap<String, std::sync::Arc<crate::brush::Gray>>,
    brush_tex: Option<egui::TextureHandle>,
    brush_prev: Option<(String, egui::TextureHandle)>,
    view3d: bool,
    v3: u8,
    yaw: f32,
    pitch: f32,
    ortho: bool,
    pan3: Vec2,
    zoom3: f32,
    giz: Option<view3d::Giz>,
    giz_drag: Option<view3d::GizDrag>,
    stroke_opacity: f32,
    pressure: bool,
    shape_kind: ShapeKind,
    hover_area: u8,
    tab_drag: Option<usize>,
    tab_rename: Option<(usize, String)>,
    tool: Tool,
    color: Color32,
    fill_color: Color32,
    fill_shapes: bool,
    brush_size: f32,
    pencil_size: f32,
    eraser_size: f32,
    shape_width: f32,
    smoothing: f32,

    cl: usize,
    frame: u32,
    playing: bool,
    play_accum: f64,
    loop_play: bool,
    /// Sala Stop Motion: reproducir solo un tramo (por ejemplo, las últimas fotos). Se borra al parar.
    sm_play_range: Option<(u32, u32)>,
    /// Botón maestro «Ocultar tímidas»: las capas tímidas no salen en las listas (el dibujo no cambia).
    hide_shy: bool,
    /// Selección múltiple de capas (Ctrl y Mayús + clic) y la última capa pulsada sin Mayús (ancla del rango).
    lsel: Vec<usize>,
    lanchor: usize,
    /// Línea de tiempo de montaje (columnas, filas desplegadas, zoom).
    mt: montage::MtState,
    /// Selección de celdas en el Xsheet: (capa, desde, hasta).
    sel: Option<(usize, u32, u32)>,
    xs_horizontal: bool,

    onion: bool,
    onion_before: u32,
    onion_after: u32,
    onion_opacity: f32,
    onion_falloff: u8,

    guides: [bool; 6],

    zoom: f32,
    pan: Vec2,
    need_fit: bool,
    cursor_canvas: Option<[f32; 2]>,
    tb_ppf: Option<f32>,
    tb_start: f32,
    tb_drag: u8,
    /// Zoom del Xsheet (5–16, 10 = normal).
    xs_zoom: f32,
    /// Rueda acumulada para dar un paso de zoom por muesca.
    wheel_acc: f32,

    sb_sel: usize,
    sb_zoom: f32,
    sb_detail: bool,
    sb_last: Option<[f32; 2]>,
    sb_erase: bool,
    sb_onion: bool,
    sb_stroke: Option<Stroke>,
    sb_drag: Option<usize>,
    render_opts: RenderOpts,

    gesture: Option<Gesture>,
    pre: Option<Project>,
    changed: bool,
    space_down: bool,
    space_panned: bool,
    /// Trazos elegidos con la herramienta Selección, y de qué capa y dibujo son.
    picked: Vec<usize>,
    picked_owner: Option<(usize, u32)>,
    pal_sel: usize,
    swatch_sel: Option<usize>,
    swatch_rename: Option<(usize, String)>,
    ffmpeg: Option<PathBuf>,

    clipboard: Option<Drawing>,
    status: String,
    status_time: f64,
    now: f64,

    show_help: bool,
    show_about: bool,
    show_new: bool,
    new_w: u32,
    new_h: u32,
    new_fps: u32,
    new_secs: f32,
    pending: Option<Pending>,
    allow_close: bool,
    renaming: Option<(usize, String, bool)>,
    last_scroll_frame: Option<u32>,
    last_title: String,
    last_autosave: f64,
    shot: Option<Shot>,
    safety: safety::Safety,
    /// Solo para pruebas: provoca un fallo interno en ese fotograma (LEONTOON_PRUEBA_FALLO=n).
    test_panic_frame: u64,
}

impl LeonApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, open: Option<PathBuf>) -> Self {
        setup_style(&cc.egui_ctx);
        start::init_iso(&cc.egui_ctx);
        let safety = safety::Safety::start(screenshot.is_none());
        let mut project = Project::demo();
        project.room_set = "trad".into();
        project.rooms = start::tabs_for_set("trad");
        // Para capturas de la vista 3D: LEONTOON_CAMARA=1 añade una cámara; LEONTOON_V3=0..7 elige la vista.
        if std::env::var("LEONTOON_CAMARA").is_ok() {
            let mut c = Layer::new("Cámara 1");
            c.kind = LayerKind::Camera;
            c.is3d = true;
            c.depth = -crate::model::CAM_DIST;
            project.layers.insert(0, c);
        }
        // Para capturas de capas de mapa de bits: LEONTOON_BITMAP=1 añade una con trazos en los cuadros 1 y 3.
        if std::env::var("LEONTOON_BITMAP").is_ok() {
            let mut l = Layer::new("Píxeles");
            l.kind = LayerKind::Bitmap;
            if std::env::var("LEONTOON_BRUSH").is_ok() {
                // Tres pinceles del motor: duro, difuso con poco flujo y plano girado (caligráfico).
                use crate::brush::{BrushParams as P, BrushSpec as S, Control, LiveStroke, Pen, TexKind};
                let d = P::default();
                let specs = [
                    // 1 duro, 2 difuso con poco flujo, 3 plano girado, 4 presión (tamaño y flujo), 5 variaciones al azar
                    S::new(22.0, 1.0, [230, 60, 160], d, 1),
                    S::new(34.0, 1.0, [30, 120, 230], P { hardness: 0.1, flow: 0.25, spacing: 6.0, ..d }, 2),
                    S::new(30.0, 1.0, [20, 150, 80], P { roundness: 0.25, angle: 40.0, spacing: 40.0, ..d }, 3),
                    S::new(38.0, 1.0, [240, 150, 20], P { size_control: Control::Pressure, min_size: 0.08, flow_control: Control::Pressure, spacing: 8.0, ..d }, 4),
                    S::new(30.0, 1.0, [140, 60, 200], P { size_jitter: 0.6, angle_jitter: 1.0, roundness: 0.5, roundness_jitter: 0.7, min_roundness: 0.15, flow_jitter: 0.5, spacing: 45.0, ..d }, 5),
                    // Fase 3: dispersión, textura de papel, doble pincel, color, bordes húmedos
                    S::new(14.0, 1.0, [60, 60, 70], P { scatter: 2.5, scatter_count: 4, count_jitter: 0.4, spacing: 35.0, size_jitter: 0.5, ..d }, 6),
                    S::new(44.0, 1.0, [80, 50, 30], P { texture: TexKind::Paper, texture_depth: 0.9, spacing: 12.0, hardness: 0.7, ..d }, 7),
                    S::new(46.0, 1.0, [30, 90, 70], P { dual: true, dual_size: 0.18, dual_count: 18, spacing: 14.0, ..d }, 8),
                    S::new(34.0, 1.0, [240, 130, 60], P { hue_jitter: 0.6, bri_jitter: 0.3, spacing: 70.0, size_jitter: 0.3, ..d }, 9).with_color2([60, 140, 240]),
                    S::new(54.0, 0.9, [40, 110, 200], P { wet_edges: 1.0, hardness: 0.35, spacing: 8.0, flow: 0.5, ..d }, 10),
                ];
                let mut r = crate::raster::Raster::new(project.width, project.height);
                for (i, s) in specs.into_iter().enumerate() {
                    let mut ls = LiveStroke::new(project.width, project.height, s);
                    let y = 70.0 + i as f32 * 58.0;
                    for k in 0..=70 {
                        let t = k as f32 / 70.0;
                        let pressure = (t * std::f32::consts::PI).sin().clamp(0.03, 1.0);
                        ls.push_with([280.0 + t * 560.0, y + (t * 9.0).sin() * 24.0], Pen { pressure, tilt: 1.0 });
                    }
                    ls.finish_into(&mut r);
                }
                l.keys.insert(0, std::sync::Arc::new(Drawing { strokes: vec![], raster: Some(std::sync::Arc::new(r)) }));
                project.layers.insert(0, l);
            } else {
            for (f, dy) in [(0u32, 0.0f32), (2, 120.0)] {
                let mut r = crate::raster::Raster::new(project.width, project.height);
                for (i, c) in [[230, 60, 160, 255], [30, 120, 230, 255]].into_iter().enumerate() {
                    let y = 220.0 + dy + i as f32 * 90.0;
                    let s = Stroke { kind: crate::model::ShapeKind::Free, points: vec![[300.0, y], [450.0, y - 60.0], [600.0, y + 40.0], [800.0, y - 30.0]], color: c, width: 28.0, closed: false, fill: None, pressure: Vec::new(), style: None, original: None, nodes: None };
                    crate::render::stamp_stroke(&mut r, &s);
                }
                l.keys.insert(f, std::sync::Arc::new(Drawing { strokes: vec![], raster: Some(std::sync::Arc::new(r)) }));
            }
            project.layers.insert(0, l);
            }
        }
        // Para capturas de la biblioteca vectorial: LEONTOON_VPRESET=categoría/nombre elige un pincel; LEONTOON_VFAV=id marca un favorito.
        // Para capturas del lápiz vectorial: LEONTOON_VSTROKES=1 añade una capa vectorial con trazos de distinto estilo.
        if std::env::var("LEONTOON_VSTROKES").is_ok() {
            use crate::vector::{Cap, Join, Profile, VStyle};
            let mut l = Layer::new("Trazos vectoriales");
            let wave = |y: f32, amp: f32, n: usize| -> Vec<[f32; 2]> { (0..=n).map(|i| { let t = i as f32 / n as f32; [250.0 + t * 640.0, y + (t * 7.0).sin() * amp] }).collect() };
            let ramp = |n: usize| -> Vec<f32> { (0..=n).map(|i| ((i as f32 / n as f32) * std::f32::consts::PI).sin().clamp(0.04, 1.0)).collect() };
            let mk = |points: Vec<[f32; 2]>, pressure: Vec<f32>, width: f32, color: [u8; 4], style: Option<VStyle>| Stroke { kind: crate::model::ShapeKind::Free, points, color, width, closed: false, fill: None, pressure, style, original: None, nodes: None };
            let d = VStyle::default();
            let mut strokes = vec![
                // 1 de siempre (sin estilo)
                mk(wave(120.0, 18.0, 60), vec![], 12.0, [20, 20, 24, 255], None),
                // 2 afilado en ambos extremos
                mk(wave(200.0, 18.0, 60), vec![], 26.0, [200, 60, 50, 255], Some(VStyle { profile: Profile::TaperBoth, taper: 0.3, ..d.clone() })),
                // 3 grosor con la presión (sube y baja)
                mk(wave(285.0, 18.0, 60), ramp(60), 34.0, [30, 100, 190, 255], Some(VStyle { min_width: 0.06, ..d.clone() })),
                // 4 afilado al final con extremo plano
                mk(wave(370.0, 18.0, 60), vec![], 26.0, [40, 140, 80, 255], Some(VStyle { profile: Profile::TaperEnd, cap: Cap::Flat, taper: 0.5, ..d.clone() })),
                // 5 zigzag con extremo cuadrado e unión de inglete
                mk(vec![[250.0, 450.0], [350.0, 410.0], [450.0, 460.0], [560.0, 405.0], [660.0, 465.0], [780.0, 412.0]], vec![], 18.0, [120, 60, 160, 255], Some(VStyle { cap: Cap::Square, join: Join::Miter, ..d.clone() })),
                // 6 el mismo zigzag con unión de bisel y extremo plano
                mk(vec![[250.0, 520.0], [350.0, 480.0], [450.0, 530.0], [560.0, 475.0], [660.0, 535.0], [780.0, 482.0]], vec![], 18.0, [230, 140, 20, 255], Some(VStyle { cap: Cap::Flat, join: Join::Bevel, ..d })),
            ];
            // 7 (solo con LEONTOON_VCURVE=1): curva Bézier editable con nodos suaves y una esquina.
            if std::env::var("LEONTOON_VCURVE").is_ok() {
                use crate::vector::bezier::Node;
                let n = |p: [f32; 2], hin: [f32; 2], hout: [f32; 2], smooth: bool| Node { p, hin, hout, pr: 1.0, smooth };
                let nodes = vec![
                    n([300.0, 560.0], [300.0, 560.0], [340.0, 470.0], true),
                    n([520.0, 520.0], [470.0, 590.0], [570.0, 450.0], true),
                    n([700.0, 600.0], [700.0, 600.0], [700.0, 600.0], false),
                    n([860.0, 500.0], [800.0, 640.0], [860.0, 500.0], true),
                ];
                let mut s = mk(vec![], vec![], 16.0, [30, 30, 120, 255], Some(VStyle { profile: Profile::TaperBoth, taper: 0.25, ..VStyle::default() }));
                s.nodes = Some(nodes);
                strokes.push(s);
                // 8: curva cerrada con perfil afilado (se afila en el punto donde se cerró, arriba).
                let (cx, cy, r, k) = (1010.0f32, 330.0f32, 90.0f32, 0.5523f32 * 90.0);
                let ring = vec![
                    n([cx, cy - r], [cx - k, cy - r], [cx + k, cy - r], true),
                    n([cx + r, cy], [cx + r, cy - k], [cx + r, cy + k], true),
                    n([cx, cy + r], [cx + k, cy + r], [cx - k, cy + r], true),
                    n([cx - r, cy], [cx - r, cy + k], [cx - r, cy - k], true),
                ];
                let mut c = mk(vec![], vec![], 30.0, [150, 40, 90, 255], Some(VStyle { profile: Profile::TaperBoth, taper: 0.4, ..VStyle::default() }));
                c.nodes = Some(ring);
                c.closed = true;
                strokes.push(c);
            }
            for s in &mut strokes {
                s.fix_attrs();
            }
            l.keys.insert(0, std::sync::Arc::new(Drawing { strokes, raster: None }));
            project.layers.insert(0, l);
        }
        // Para capturas de las colecciones: LEONTOON_COLECCION=1 mete las dos capas de arriba en una colección.
        if std::env::var("LEONTOON_COLECCION").is_ok() && project.layers.len() >= 3 {
            if let Some(ci) = project.group_layer(1) {
                project.move_to_collection(0, ci);
                // Con LEONTOON_COLECCION=2, además, esa colección se mete en otra (anidado).
                if std::env::var("LEONTOON_COLECCION").as_deref() == Ok("2") {
                    project.group_layer(ci);
                }
            }
        }
        // Para capturas de los modos de fusión y los vínculos: LEONTOON_FUSION=1 pone la capa de arriba en Multiplicar y la vincula a la segunda.
        if std::env::var("LEONTOON_FUSION").is_ok() && project.layers.len() >= 2 {
            project.layers[0].blend = crate::model::Blend::Multiply;
            project.layers[1].blend = crate::model::Blend::Difference;
            project.set_parent(0, Some(1));
            project.layers[1].tracks.set(Prop::Position, 0, [80.0, 40.0], crate::model::Interp::Linear);
        }
        // Para capturas de la capa Stop Motion: LEONTOON_STOPMOTION=<carpeta> crea ahí una secuencia con fotos de ejemplo y una capa que la usa;
        // con LEONTOON_SMFALTA=1 la capa apunta a una carpeta que no existe (para ver el aviso).
        if let Ok(dir) = std::env::var("LEONTOON_STOPMOTION") {
            let dir = std::path::PathBuf::from(dir);
            if let Ok(mut f) = crate::stopmotion::SeqFolder::create(&dir, "Escena de prueba", 12) {
                for i in 0..14u32 {
                    // Una bola blanca que avanza de foto en foto sobre un fondo que cambia de color: se nota el movimiento.
                    let (cx, cy) = (30.0 + i as f32 * 20.0, 90.0 - (i as f32 * 0.5).sin() * 40.0);
                    let img = image::RgbImage::from_fn(320, 180, |x, y| {
                        let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                        if d < 18.0 { image::Rgb([250, 250, 250]) } else { image::Rgb([(40 + i * 12) as u8, (120 + y / 3) as u8, (200 - i * 9) as u8]) }
                    });
                    let mut bytes = Vec::new();
                    if image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85).encode_image(&img).is_ok() {
                        let _ = f.append_photo(&bytes, None);
                    }
                }
            }
            if let Ok((f, _)) = crate::stopmotion::SeqFolder::open(&dir) {
                let mut l = Layer::new("Stop Motion 1");
                l.kind = LayerKind::StopMotion;
                let mut r = crate::stopmotion::SmRef::new(&f, None);
                if std::env::var("LEONTOON_SMFALTA").is_ok() {
                    r.abs = dir.join("carpeta_que_no_existe").to_string_lossy().to_string();
                }
                l.stopmotion = Some(r);
                project.layers.insert(0, l);
            }
        }
        // Para pruebas y capturas: LEONTOON_SALA=<nombre de sala> abre esa sala.
        let room_idx = std::env::var("LEONTOON_SALA")
            .ok()
            .and_then(|s| {
                let k = RoomKind::ALL.into_iter().find(|r| r.def().name.eq_ignore_ascii_case(&s) || r.def().id == s)?;
                if !project.rooms.iter().any(|t| t.kind == k.def().id) {
                    project.rooms.push(crate::model::RoomTab { kind: k.def().id.into(), name: None });
                }
                project.rooms.iter().position(|t| t.kind == k.def().id)
            })
            .unwrap_or(0);
        let stage = start::initial_stage(screenshot.is_some());
        let len = project.length;
        let mut app = LeonApp {
            project,
            path: None,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            meta_skip: false,
            last_meta_change: 0,
            frame_no: 0,
            room_idx,
            stage,
            splash_t0: None,
            puma: None,
            recent: start::load_recent(),
            obj_mode: false,
            show_toolbar: true,
            show_n: true,
            n_tab: std::env::var("LEONTOON_NTAB").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
            slot_editors: match std::env::var("LEONTOON_TAM") {
                Ok(_) => Default::default(),
                Err(_) => safety::data_dir().map(|d| slots::load(&d)).unwrap_or_default(),
            },
            cur_slot: Slot::Main,
            // Variables de prueba para capturas: LEONTOON_POP=1|2|3, LEONTOON_ALPHA=1, LEONTOON_ADDMENU=1.
            hud_pop: std::env::var("LEONTOON_POP").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
            light_gray: 10.0,
            show_alpha: std::env::var("LEONTOON_ALPHA").is_ok(),
            pegbar: true,
            light_amt: 55.0,
            light_temp: 30.0,
            chk_size: 20.0,
            chk_con: 12.0,
            view_op: 100.0,
            add_menu: if std::env::var("LEONTOON_ADDMENU").is_ok() { Some(Pos2::new(560.0, 260.0)) } else { None },
            brush: Default::default(),
            brush_live: None,
            vstyle: Default::default(),
            vlib: if screenshot.is_some() { Default::default() } else { safety::data_dir().map(|d| crate::vector::library::load(&d)).unwrap_or_default() },
            vpreset: None,
            vui: Default::default(),
            sui: Default::default(),
            paper_tex: Default::default(),
            sizes: match std::env::var("LEONTOON_TAM") {
                Ok(spec) => panelsize::PanelSizes::from_spec(&spec),
                Err(_) if screenshot.is_some() => Default::default(),
                Err(_) => safety::data_dir().map(|d| panelsize::load(&d)).unwrap_or_default(),
            },
            sz: Default::default(),
            sm: Default::default(),
            sm_thumbs: Default::default(),
            sm_loader: crate::stopmotion::Loader::new(),
            sm_tloader: crate::stopmotion::Loader::thumbs(),
            sm_ui: smroom::SmUi::from_env(),
            sm_tex: Default::default(),
            sm_failed: Default::default(),
            paper_scale: crate::paper::SCALE_DEFAULT,
            paper_seed: std::env::var("LEONTOON_SEMILLA").ok().and_then(|v| v.parse().ok()).unwrap_or(1),
            pen_path: None,
            sel_node: None,
            pen_pressure: 1.0,
            brush_tip_img: None,
            brush_tex_img: None,
            brush_preset: None,
            preset_ui: Default::default(),
            brush_img_cache: Default::default(),
            brush_lib_removed: Vec::new(),
            lib_window: None,
            abr_import: None,
            abr_export: None,
            abr_report: None,
            brush_tex: None,
            brush_prev: None,
            view3d: false,
            v3: std::env::var("LEONTOON_V3").ok().and_then(|v| v.parse().ok()).unwrap_or(1),
            yaw: match std::env::var("LEONTOON_V3").ok().and_then(|v| v.parse::<u8>().ok()) { Some(2) | Some(4) | Some(7) => 0.0, Some(3) => -90.0, Some(5) => 180.0, Some(6) => 90.0, _ => -32.0 },
            pitch: match std::env::var("LEONTOON_V3").ok().and_then(|v| v.parse::<u8>().ok()) { Some(2) => 85.0, Some(7) => -85.0, Some(3) | Some(4) | Some(5) | Some(6) => 0.0, _ => 16.0 },
            ortho: false,
            pan3: Vec2::ZERO,
            zoom3: 1.0,
            giz: None,
            giz_drag: None,
            stroke_opacity: 1.0,
            pressure: true,
            shape_kind: ShapeKind::Ellipse,
            hover_area: 0,
            tab_drag: None,
            tab_rename: None,
            tool: match std::env::var("LEONTOON_TOOL").as_deref() { Ok("lapiz") => Tool::Vector, Ok("pluma") => Tool::Pen, Ok("seleccion") => Tool::Select, Ok("borrador") => Tool::Eraser, _ => Tool::Brush },
            color: Color32::from_rgb(20, 20, 24),
            fill_color: Color32::from_rgb(253, 216, 53),
            fill_shapes: false,
            brush_size: 8.0,
            pencil_size: 2.0,
            eraser_size: 24.0,
            shape_width: 4.0,
            smoothing: 0.35,
            cl: 0,
            frame: 0,
            playing: false,
            play_accum: 0.0,
            loop_play: false,
            sm_play_range: None,
            hide_shy: false,
            lsel: Vec::new(),
            lanchor: 0,
            mt: Default::default(),
            sel: None,
            xs_horizontal: false,
            onion: true,
            onion_before: 2,
            onion_after: 2,
            onion_opacity: 0.4,
            onion_falloff: 0,
            guides: [false; 6],
            zoom: 0.5,
            pan: Vec2::ZERO,
            need_fit: true,
            cursor_canvas: None,
            tb_ppf: None,
            tb_start: 0.0,
            tb_drag: 0,
            xs_zoom: XS_ZOOM_DEFAULT,
            wheel_acc: 0.0,
            sb_sel: 0,
            sb_zoom: 260.0,
            sb_detail: false,
            sb_last: None,
            sb_erase: false,
            sb_onion: true,
            sb_stroke: None,
            sb_drag: None,
            render_opts: RenderOpts { a: 1, b: len, scale: 1.0, transparent: false, columns: 8, message: "Listo.".into() },
            gesture: None,
            pre: None,
            changed: false,
            space_down: false,
            space_panned: false,
            picked: Vec::new(),
            picked_owner: None,
            pal_sel: 2,
            swatch_sel: None,
            swatch_rename: None,
            ffmpeg: None,
            clipboard: None,
            status: "LeonToon listo · funciona sin conexión. Pulsa F1 para ver la guía rápida.".into(),
            status_time: 0.0,
            now: 0.0,
            show_help: false,
            show_about: false,
            show_new: false,
            new_w: 1280,
            new_h: 720,
            new_fps: 24,
            new_secs: 2.0,
            pending: None,
            allow_close: false,
            renaming: None,
            last_scroll_frame: None,
            last_title: String::new(),
            last_autosave: 0.0,
            shot: screenshot.map(|path| Shot { path, frames: 0, requested: false }),
            safety,
            test_panic_frame: std::env::var("LEONTOON_PRUEBA_FALLO").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
        };
        app.set_room_idx(room_idx);
        app.status = "LeonToon listo · funciona sin conexión. Pulsa F1 para ver la guía rápida.".into();
        app.adopt_project_brush();
        // Para capturas: LEONTOON_PRESET="Categoría/Nombre" aplica ese pincel guardado; LEONTOON_USERPRESETS=1
        // añade dos pinceles propios al proyecto (sin tocar la biblioteca del usuario).
        if std::env::var("LEONTOON_USERPRESETS").is_ok() {
            let base = app.brush_setup_now();
            let mut a = base.clone();
            a.params.hardness = 0.5;
            a.size = 24.0;
            crate::presets::save_new(&mut app.project.brush_presets, "Boceto suave", "Mis pinceles", a);
            let mut b = base;
            b.params.roundness = 0.4;
            b.params.angle = 30.0;
            crate::presets::save_new(&mut app.project.brush_presets, "Caligrafía fina", "Letras", b);
        }
        if let Ok(v) = std::env::var("LEONTOON_PRESET") {
            if let Some((c, n)) = v.split_once('/') {
                if let Some(p) = crate::presets::find(&app.project.brush_presets, &(c.to_string(), n.to_string())) {
                    app.apply_preset(&p);
                }
            }
        }
        if let Ok(v) = std::env::var("LEONTOON_VFAV") {
            for id in v.split(';') {
                app.vlib.toggle_favorite(id);
            }
        }
        if let Some(p) = std::env::var("LEONTOON_VPRESET").ok().and_then(|v| app.vlib.find(&v)) {
            app.apply_vpreset(&p);
        }
        // Para capturas de la edición de trazos: LEONTOON_VPICK=0,1,2 elige trazos del dibujo actual.
        if let Ok(v) = std::env::var("LEONTOON_VPICK") {
            app.picked = v.split(',').filter_map(|n| n.trim().parse().ok()).collect();
            app.picked_owner = app.layer().key_at_or_before(app.frame).map(|k| (app.cl, k));
        }
        // Capturas de los papeles de estudio: LEONTOON_PAPEL=<id> elige uno y LEONTOON_LUZ=<0-100> fija la luz de la mesa.
        if let Some(pp) = std::env::var("LEONTOON_PAPEL").ok().and_then(|v| crate::paper::PAPERS.iter().find(|p| p.id == v)) {
            app.project.background = [pp.rgb[0], pp.rgb[1], pp.rgb[2], 255];
        }
        if let Some(s) = std::env::var("LEONTOON_ESCALA").ok().and_then(|v| v.parse::<f32>().ok()) {
            app.paper_scale = s;
        }
        if let Some(l) = std::env::var("LEONTOON_LUZ").ok().and_then(|v| v.parse::<f32>().ok()) {
            app.light_amt = l;
        }
        // Capturas: LEONTOON_CUADRO=<n> (desde 0) pone ese cuadro y LEONTOON_ONION=1 activa el papel cebolla.
        if let Some(n) = std::env::var("LEONTOON_CUADRO").ok().and_then(|v| v.parse::<u32>().ok()) {
            app.frame = n.min(app.project.length.saturating_sub(1));
        }
        if std::env::var("LEONTOON_ONION").is_ok() {
            app.onion = true;
        }
        // LEONTOON_VNODE=<n> elige ese nodo de la curva elegida (captura de la edición de nodos).
        if let Some(n) = std::env::var("LEONTOON_VNODE").ok().and_then(|v| v.parse().ok()) {
            app.sel_node = Some(n);
        }
        // LEONTOON_PENDEMO=1: curva de la Pluma en curso (captura).
        if std::env::var("LEONTOON_PENDEMO").is_ok() {
            use crate::vector::bezier::Node;
            let k = |p: [f32; 2], o: [f32; 2]| Node { p, hin: [2.0 * p[0] - o[0], 2.0 * p[1] - o[1]], hout: o, pr: 1.0, smooth: true };
            app.pen_path = Some(penui::PenPath { nodes: vec![k([300.0, 400.0], [350.0, 330.0]), k([520.0, 380.0], [570.0, 450.0]), Node::corner([720.0, 330.0]), k([860.0, 420.0], [900.0, 480.0])], owner: (app.cl, app.frame) });
        }
        if let Ok(p) = std::env::var("LEONTOON_ABR") {
            app.abr_screenshot_demo(std::path::Path::new(&p));
        }
        if let Some(p) = open {
            app.load_from(&p);
        }
        app
    }

    // ---------- Utilidades ----------

    fn say(&mut self, s: impl Into<String>) {
        self.status = s.into();
        self.status_time = self.now.max(0.001);
    }

    fn push_undo(&mut self, p: Project) {
        self.undo.push(p);
        if self.undo.len() > 150 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    fn checkpoint(&mut self) {
        // Un cambio del proyecto: Ctrl+Z vuelve a deshacer el proyecto, no la secuencia.
        self.sm_ui.last_was_seq = false;
        let p = self.project.clone();
        self.push_undo(p);
        self.meta_skip = true;
    }

    fn clamp_state(&mut self) {
        self.project.sanitize();
        self.cl = self.cl.min(self.project.layers.len() - 1);
        self.frame = self.frame.min(self.project.length - 1);
        self.sb_sel = self.sb_sel.min(self.project.storyboard.len().saturating_sub(1));
    }

    fn do_undo(&mut self) {
        if self.sm_history(true) {
            return;
        }
        self.finish_gesture();
        self.pen_path = None;
        if let Some(p) = self.undo.pop() {
            let brush = self.brush_data();
            let cur = std::mem::replace(&mut self.project, p);
            self.restore_brush_data(brush);
            self.redo.push(cur);
            self.dirty = true;
            self.meta_skip = true;
            self.clamp_state();
            self.say("Deshacer");
        } else {
            self.say("No hay nada que deshacer");
        }
    }

    fn do_redo(&mut self) {
        if self.sm_history(false) {
            return;
        }
        self.finish_gesture();
        self.pen_path = None;
        if let Some(p) = self.redo.pop() {
            let brush = self.brush_data();
            let cur = std::mem::replace(&mut self.project, p);
            self.restore_brush_data(brush);
            self.undo.push(cur);
            self.dirty = true;
            self.meta_skip = true;
            self.clamp_state();
            self.say("Rehacer");
        } else {
            self.say("No hay nada que rehacer");
        }
    }

    fn layer(&self) -> &Layer {
        &self.project.layers[self.cl]
    }

    fn file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sin título".into())
    }

    /// Tipo de la sala activa.
    fn room_kind(&self) -> RoomKind {
        self.project
            .rooms
            .get(self.room_idx)
            .and_then(|t| RoomKind::from_id(&t.kind))
            .unwrap_or(RoomKind::Layout)
    }

    /// Nombre visible de una pestaña (el propio si se renombró).
    fn tab_name(&self, i: usize) -> String {
        match self.project.rooms.get(i) {
            Some(t) => t
                .name
                .clone()
                .unwrap_or_else(|| RoomKind::from_id(&t.kind).map_or("Sala", |k| k.def().name).to_string()),
            None => String::new(),
        }
    }

    /// Activa la pestaña `i`: herramienta y orientación de la línea de tiempo de esa sala.
    fn set_room_idx(&mut self, i: usize) {
        if self.project.rooms.is_empty() {
            self.project.rooms = start::tabs_for_set("trad");
        }
        let i = i.min(self.project.rooms.len() - 1);
        self.finish_gesture();
        self.playing = false;
        self.room_idx = i;
        let def = self.room_kind().def();
        self.xs_horizontal = def.tl_horizontal;
        self.hud_pop = std::env::var("LEONTOON_POP").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        if let Some(t) = def.tool {
            self.obj_mode = false;
            self.tool = t;
        }
        self.need_fit = true;
        // Como el prototipo: la sala Composición 3D abre la vista 3D y las demás la 2D.
        self.view3d = def.id == "comp";
        self.giz_drag = None;
        self.say(format!("Sala {}: {}", self.tab_name(i), def.desc));
    }

    /// Abre la primera sala de un tipo (o la añade si no está).
    fn goto_room_kind(&mut self, k: RoomKind) {
        let id = k.def().id;
        let i = match self.project.rooms.iter().position(|t| t.kind == id) {
            Some(i) => i,
            None => {
                self.project.rooms.push(crate::model::RoomTab { kind: id.into(), name: None });
                self.project.rooms.len() - 1
            }
        };
        self.set_room_idx(i);
    }

    /// Opacidad del papel cebolla a distancia d (1..n): la del más cercano es la elegida en el
    /// panel (prototipo v0.3, 40 % por defecto); la caída sigue los pesos del original.
    fn onion_weight(&self, d: u32, _n: u32) -> f32 {
        let rel = match self.onion_falloff {
            1 => 0.55f32.powi(d as i32 - 1),
            2 => 1.0,
            _ => onion_original_opacity(d) / ONION_MAX_OPACITY,
        };
        (self.onion_opacity * rel).clamp(0.0, 1.0)
    }

    // ---------- Archivos ----------

    fn autosave_path() -> Option<PathBuf> {
        Some(safety::data_dir()?.join("autosave.ltoon"))
    }

    fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(p) => self.save_to(&p),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) -> bool {
        let dialog = rfd::FileDialog::new()
            .set_title("Guardar proyecto")
            .add_filter("Proyecto LeonToon", &["ltoon"])
            .set_file_name("mi_animacion.ltoon");
        match dialog.save_file() {
            Some(mut p) => {
                if p.extension().is_none() {
                    p.set_extension("ltoon");
                }
                self.save_to(&p)
            }
            None => false,
        }
    }

    fn save_to(&mut self, p: &Path) -> bool {
        // Las secuencias Stop Motion guardan su carpeta relativa a la del proyecto: se recalcula al guardar en otro sitio.
        let pdir = p.parent().map(|d| d.to_path_buf());
        for l in &mut self.project.layers {
            if let Some(r) = l.stopmotion.as_mut() {
                r.refresh_paths(pdir.as_deref());
            }
        }
        match safety::write_project(&self.project, p, true) {
            Ok(()) => {
                self.path = Some(p.to_path_buf());
                self.project.base_dir = pdir.clone();
                self.dirty = false;
                self.remember_recent(p);
                self.say(format!("Guardado y verificado: {}", p.display()));
                true
            }
            Err(e) => {
                safety::log_line(&format!("No se pudo guardar {}: {e}", p.display()));
                self.say(format!("No se pudo guardar: {e}"));
                false
            }
        }
    }

    fn load_from(&mut self, p: &Path) -> bool {
        match safety::read_project(p) {
            Ok(proj) => {
                self.project = proj;
                self.path = Some(p.to_path_buf());
                self.project.base_dir = p.parent().map(|d| d.to_path_buf());
                self.after_load();
                self.remember_recent(p);
                self.say(format!("Abierto: {}", p.display()));
                true
            }
            Err(e) => {
                self.say(format!("No se pudo abrir el archivo: {e}"));
                false
            }
        }
    }

    fn after_load(&mut self) {
        self.sm.clear();
        self.sm_thumbs.clear();
        self.sm_tex.clear();
        self.sm_failed.clear();
        self.sm_ui = Default::default();
        self.adopt_project_brush();
        self.undo.clear();
        self.redo.clear();
        self.dirty = false;
        self.cl = 0;
        self.frame = 0;
        self.sel = None;
        self.sb_sel = 0;
        self.sb_detail = false;
        self.picked.clear();
        self.pal_sel = self.pal_sel.min(self.project.palettes.len().saturating_sub(1));
        self.playing = false;
        self.gesture = None;
        self.pre = None;
        self.need_fit = true;
        self.meta_skip = true;
        self.render_opts.a = 1;
        self.render_opts.b = self.project.length;
        if self.project.rooms.is_empty() {
            if self.project.room_set.is_empty() {
                self.project.room_set = "trad".into();
            }
            self.project.rooms = start::tabs_for_set(&self.project.room_set.clone());
        }
        self.set_room_idx(0);
        self.stage = start::Stage::Work;
    }

    fn perform(&mut self, ctx: &egui::Context, a: Pending) {
        match a {
            Pending::New => self.show_new = true,
            Pending::Open => {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Abrir proyecto")
                    .add_filter("Proyecto LeonToon", &["ltoon"])
                    .pick_file()
                {
                    self.load_from(&p);
                }
            }
            Pending::OpenPath(p) => {
                self.load_from(&p);
            }
            Pending::Recover => match Self::autosave_path() {
                Some(p) if p.exists() => {
                    if self.load_from(&p) {
                        self.path = None;
                        self.dirty = true;
                        self.say("Copia automática recuperada. Guárdala con «Guardar como…».");
                    }
                }
                _ => self.say("No hay ninguna copia automática."),
            },
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn request(&mut self, ctx: &egui::Context, a: Pending) {
        if self.dirty {
            self.pending = Some(a);
        } else {
            self.perform(ctx, a);
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = format!("{}{} — LeonToon", if self.dirty { "• " } else { "" }, self.file_name());
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        shot.frames += 1;
        ctx.request_repaint();
        if !shot.requested && shot.frames >= 120 {
            shot.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let img = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = img {
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let path = shot.path.clone();
            match render::save_rgba_png(img.size[0] as u32, img.size[1] as u32, &bytes, &path) {
                Ok(()) => println!("Captura guardada en {}", path.display()),
                Err(e) => eprintln!("No se pudo guardar la captura: {e}"),
            }
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if shot.frames > 900 {
            eprintln!("La captura no llegó a tiempo");
            std::process::exit(1);
        }
    }
}

impl eframe::App for LeonApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Si algo falla dentro de un fotograma, no se pierde nada: se guarda y se sigue.
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.update_inner(ctx))).is_ok();
        if !ok {
            self.recover_from_panic(ctx);
        }
    }
}

impl LeonApp {
    fn update_inner(&mut self, ctx: &egui::Context) {
        self.frame_no += 1;
        self.now = ctx.input(|i| i.time);
        self.handle_screenshot(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }

        let meta_before = meta_of(&self.project);
        self.meta_skip = false;

        self.validate_pick();
        if self.stage == start::Stage::Splash {
            self.splash(ctx);
            return;
        }
        self.read_pen(ctx);
        self.handle_shortcuts(ctx);
        self.advance_playback(ctx);
        self.shell(ctx);
        if self.stage == start::Stage::Home {
            self.home(ctx);
        }
        self.dialogs(ctx);
        self.safety_dialogs(ctx);
        canvas::prune_raster_tex(ctx.cumulative_pass_nr());
        self.sync_brush_to_project();

        // Los cambios hechos en los paneles (nombre, opacidad, claves, notas…) también se pueden deshacer.
        if !self.meta_skip {
            let now_meta = meta_of(&self.project);
            if now_meta != meta_before {
                if self.last_meta_change + 1 != self.frame_no {
                    let mut snap = self.project.clone();
                    apply_meta(&mut snap, &meta_before);
                    self.push_undo(snap);
                }
                self.last_meta_change = self.frame_no;
                self.dirty = true;
            }
        }
        self.project.length = self.project.length.max(1);
        self.project.fps = self.project.fps.max(1);
        self.frame = self.frame.min(self.project.length - 1);
        self.cl = self.cl.min(self.project.layers.len().saturating_sub(1));
        self.autosave();
        self.update_title(ctx);
        ctx.request_repaint_after(std::time::Duration::from_secs(20));
    }
}

#[cfg(test)]
mod tests {
    /// El motor de dibujo lo decide la capa activa: el lápiz y la pluma vectoriales no se mezclan con las capas de
    /// mapa de bits, y el Pincel de píxeles no se usa como motor vectorial.
    #[test]
    fn tools_and_layer_kinds_never_mix() {
        use crate::model::LayerKind::*;
        // Lápiz y Pluma vectoriales: solo en capas vectoriales.
        for t in [Tool::Vector, Tool::Pen] {
            assert!(t.layer_block(Vector).is_none(), "{t:?} en capa vectorial");
            assert!(t.layer_block(Bitmap).unwrap().contains("capas vectoriales"), "{t:?} en capa de mapa de bits");
        }
        // El Pincel y el Borrador trabajan en las dos clases, cada una con su motor.
        for t in [Tool::Brush, Tool::Eraser] {
            assert!(t.layer_block(Vector).is_none() && t.layer_block(Bitmap).is_none(), "{t:?}");
        }
        // Las formas y el relleno siguen como estaban en las dos.
        assert!(Tool::Shape.layer_block(Bitmap).is_none() && Tool::Shape.layer_block(Vector).is_none());
        // Las capas que no guardan dibujos (cámara, luz, nulo, texto) no admiten ninguna herramienta de dibujo.
        for k in [Camera, Light, Null, Text, StopMotion] {
            for t in [Tool::Brush, Tool::Vector, Tool::Pen, Tool::Eraser, Tool::Shape, Tool::Fill] {
                assert!(t.layer_block(k).is_some(), "{t:?} no debe dibujar en {k:?}");
            }
        }
        // Las herramientas que solo miran o seleccionan no se bloquean.
        for t in [Tool::Select, Tool::Hand, Tool::Zoom, Tool::Picker, Tool::RotateObj] {
            for k in [Vector, Bitmap, Camera, Solid] {
                assert!(t.layer_block(k).is_none(), "{t:?} en {k:?}");
            }
        }
        // Una capa de color sólido no queda bloqueada.
        assert!(Tool::Vector.layer_block(Solid).is_none());
    }
    use super::*;

    #[test]
    fn zoom_ladder_matches_original_steps() {
        assert!((zoom_ladder_step(1.0, 1) - 1.25).abs() < 1e-4);
        assert!((zoom_ladder_step(1.0, -1) - 0.8).abs() < 1e-4);
        assert!((zoom_ladder_step(0.45, -1) - 0.40).abs() < 1e-4);
        assert!((zoom_ladder_step(800.0, 1) - 800.0).abs() < 1e-3);
    }

    #[test]
    fn onion_opacity_follows_original_weights() {
        assert!((onion_original_opacity(1) - 0.8).abs() < 1e-5);
        assert!((onion_original_opacity(2) - 0.74).abs() < 1e-5);
        assert!((onion_original_opacity(10) - 0.26).abs() < 1e-5);
    }
}
