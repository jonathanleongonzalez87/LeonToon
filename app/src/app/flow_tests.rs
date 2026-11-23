//! Pruebas del flujo completo (Issue #20): guardar y abrir un `.ltoon`, abrir proyectos de versiones anteriores,
//! archivos dañados, escrituras que fallan y exportaciones. Son más cercanas a lo que hace la persona que las pruebas
//! del modelo: pasan por `write_project` y `read_project` (los mismos de Guardar y Abrir) y por el exportador real.
//!
//! `app/testdata/` guarda proyectos **reales de la versión 0.1.0**, generados con aquel programa, para comprobar que
//! los proyectos viejos siguen abriendo sin perder nada.

use super::safety::{read_project, write_project};
use crate::model::{Drawing, Layer, LayerKind, Panel, Project, ShapeKind, Stroke, XKey};
use crate::paper;
use crate::raster::Raster;
use crate::render;
use crate::stopmotion::{Located, SeqFolder, SmRef};
use crate::vector::bezier::Node;
use crate::vector::edit::{self, Prop};
use crate::vector::{default_shape, Cap, Join, Profile, VStyle};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const V010_DEMO: &[u8] = include_bytes!("../../testdata/proyecto_v0.1.0_demo.ltoon");
const V010_COMPLETO: &[u8] = include_bytes!("../../testdata/proyecto_v0.1.0_completo.ltoon");

/// Carpeta temporal propia de cada prueba (se borra al terminar con `finish`).
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("leontoon_flujo_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn finish(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn stroke(points: Vec<[f32; 2]>, pressure: Vec<f32>, style: Option<VStyle>, color: [u8; 4], width: f32) -> Stroke {
    Stroke { kind: ShapeKind::Free, points, color, width, closed: false, fill: None, pressure, style, original: None, nodes: None }
}

fn curve(nodes: Vec<Node>, closed: bool, style: VStyle) -> Stroke {
    let mut s = stroke(Vec::new(), Vec::new(), Some(style), [30, 30, 120, 255], 14.0);
    s.closed = closed;
    s.nodes = Some(nodes);
    s.regen();
    s
}

/// Un proyecto con de todo: una capa de cada tipo, 3D, claves de animación, trazos con presión, estilo personalizado,
/// aspecto original, curvas Bézier abiertas y cerradas, mapa de bits, storyboard, rango y papel de estudio.
fn rich_project() -> Project {
    let mut p = Project::demo();
    p.background = [paper::PAPERS[8].rgb[0], paper::PAPERS[8].rgb[1], paper::PAPERS[8].rgb[2], 255];

    let custom = VStyle { profile: Profile::Custom, shape: default_shape(), cap: Cap::Square, join: Join::Bevel, ..VStyle::default() };
    let mut pressured = stroke(vec![[100.0, 300.0], [250.0, 260.0], [400.0, 330.0]], vec![0.2, 0.9, 0.5], Some(custom.clone()), [10, 20, 30, 255], 12.0);
    edit::set_prop(&mut pressured, &Prop::Width(22.0)); // guarda su aspecto original
    let open = curve(
        vec![
            Node { p: [500.0, 300.0], hin: [500.0, 300.0], hout: [540.0, 240.0], pr: 1.0, smooth: true },
            Node { p: [700.0, 320.0], hin: [660.0, 380.0], hout: [740.0, 260.0], pr: 0.6, smooth: true },
            Node::corner([850.0, 300.0]),
        ],
        false,
        custom.clone(),
    );
    let closed = curve(
        vec![Node::corner([200.0, 450.0]), Node::corner([420.0, 450.0]), Node::corner([420.0, 600.0]), Node::corner([200.0, 600.0])],
        true,
        VStyle { profile: Profile::TaperBoth, ..VStyle::default() },
    );
    let mut vec_layer = Layer::new("Vectorial");
    let d = Arc::new(Drawing { strokes: vec![pressured, open, closed], raster: None });
    vec_layer.keys.insert(0, d.clone());
    vec_layer.keys.insert(6, d);
    vec_layer.tracks.set_all(0, &XKey::default());
    vec_layer.tracks.set_all(8, &XKey { x: 30.0, r: 12.0, s: 1.1, ..XKey::default() });
    p.layers.push(vec_layer);

    let mut bitmap = Layer::new("Píxeles");
    bitmap.kind = LayerKind::Bitmap;
    let mut r = Raster::new(p.width, p.height);
    render::stamp_stroke(&mut r, &Stroke::shape(ShapeKind::Line, [100.0, 120.0], [500.0, 120.0], [30, 90, 220, 255], 24.0, None));
    bitmap.keys.insert(0, Arc::new(Drawing { strokes: Vec::new(), raster: Some(Arc::new(r)) }));
    p.layers.push(bitmap);

    for (i, k) in [LayerKind::Solid, LayerKind::Null, LayerKind::Text, LayerKind::Light, LayerKind::Camera, LayerKind::StopMotion, LayerKind::Collection].into_iter().enumerate() {
        let mut l = Layer::new(format!("Capa {k:?}"));
        l.kind = k;
        if k == LayerKind::StopMotion {
            l.stopmotion = Some(SmRef { id: "0123456789abcdef".into(), name: "Escena 1".into(), rel: Some("fotos/escena1".into()), abs: "C:/no/existe/escena1".into(), photos: 12, frames: 30, fps: 12, start: 0 });
        }
        l.solid = [40, 90, 60, 255];
        l.is3d = i % 2 == 0;
        l.plane = (i % 3) as u8;
        l.depth = i as f32 * 40.0 - 80.0;
        l.opacity = 0.8;
        p.layers.push(l);
    }
    p.storyboard.push(Panel { title: "Plano 1".into(), dur: 2.5, action: "Entra el personaje".into(), ..Default::default() });
    p.range = Some((1, 10));
    p.sanitize();
    p
}

fn value(p: &Project) -> serde_json::Value {
    serde_json::to_value(p).unwrap()
}

fn save_and_open(p: &Project, dir: &Path, name: &str) -> Project {
    let path = dir.join(name);
    write_project(p, &path, false).unwrap();
    read_project(&path).unwrap()
}

// ---------- Guardar y abrir ----------

#[test]
fn rich_project_survives_save_and_open_identically() {
    let dir = temp_dir("rico");
    let p = rich_project();
    let back = save_and_open(&p, &dir, "rico.ltoon");
    assert_eq!(value(&p), value(&back), "el proyecto cambió al guardar y abrir");

    // Los campos nuevos de cada fase siguen ahí, no solo «lo mismo en JSON».
    let v = &back.layers.iter().find(|l| l.name == "Vectorial").unwrap().keys[&0].strokes;
    assert_eq!(v[0].pressure, vec![0.2, 0.9, 0.5]);
    assert_eq!(v[0].style.as_ref().unwrap().profile, Profile::Custom);
    assert_eq!(v[0].style.as_ref().unwrap().shape, default_shape());
    assert_eq!(v[0].original.as_ref().map(|o| o.width), Some(12.0), "se conserva el aspecto original");
    assert!(v[1].nodes.as_ref().map_or(false, |n| n.len() == 3) && !v[1].points.is_empty(), "la curva abierta conserva sus nodos");
    assert!(v[2].closed && v[2].nodes.is_some());
    let kinds: Vec<LayerKind> = back.layers.iter().map(|l| l.kind).collect();
    for k in LayerKind::ALL {
        assert!(kinds.contains(&k), "falta una capa {k:?}");
    }
    let sm = back.layers.iter().find(|l| l.kind == LayerKind::StopMotion).unwrap().stopmotion.clone().unwrap();
    assert_eq!((sm.name.as_str(), sm.rel.as_deref(), sm.photos, sm.frames, sm.fps), ("Escena 1", Some("fotos/escena1"), 12, 30, 12), "la referencia de Stop Motion se conserva");
    let cam = back.layers.iter().find(|l| l.kind == LayerKind::Camera).unwrap();
    assert!((cam.depth - (4.0 * 40.0 - 80.0)).abs() < 1e-4 && cam.is3d);
    assert_eq!(back.range, Some((1, 10)));
    assert_eq!(back.storyboard.len(), p.storyboard.len());
    assert_eq!(back.storyboard.last().map(|s| s.title.as_str()), Some("Plano 1"));
    assert_eq!(paper::for_background(back.background).map(|p| p.id), Some("tmk"), "el papel de estudio se reconoce al abrir");
    let px = back.layers.iter().find(|l| l.kind == LayerKind::Bitmap).unwrap().keys[&0].pick([300.0, 120.0]);
    assert_eq!(px, Some([30, 90, 220, 255]), "el mapa de bits conserva sus píxeles");
    finish(&dir);
}

#[test]
fn saving_is_stable_and_keeps_a_previous_version() {
    let dir = temp_dir("estable");
    let p = rich_project();
    let path = dir.join("estable.ltoon");
    write_project(&p, &path, true).unwrap();
    let first = std::fs::read(&path).unwrap();
    // Abrir y volver a guardar no cambia ni un byte ni hace crecer el archivo.
    let again = read_project(&path).unwrap();
    write_project(&again, &path, true).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first, "guardar de nuevo cambió el archivo");
    assert_eq!(std::fs::read(path.with_extension("ltoon.bak")).unwrap(), first, "la copia anterior no coincide");
    assert!(!path.with_extension("ltoon.tmp").exists());
    finish(&dir);
}

#[test]
fn the_image_is_the_same_after_saving_and_opening() {
    let dir = temp_dir("imagen");
    let p = rich_project();
    let back = save_and_open(&p, &dir, "imagen.ltoon");
    for f in [0, 3, 6, 9] {
        let (a, b) = (render::render_frame(&p, f, 0.25).unwrap(), render::render_frame(&back, f, 0.25).unwrap());
        assert_eq!(a.data(), b.data(), "el fotograma {f} se ve distinto al abrir el proyecto");
    }
    finish(&dir);
}

// ---------- Proyectos de versiones anteriores ----------

fn raw(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

fn count_strokes(v: &serde_json::Value) -> usize {
    v["layers"].as_array().unwrap().iter().flat_map(|l| l["keys"].as_object().unwrap().values()).map(|d| d["strokes"].as_array().map_or(0, |s| s.len())).sum()
}

fn open_bytes(bytes: &[u8], name: &str) -> Project {
    let dir = temp_dir(name);
    let path = dir.join("viejo.ltoon");
    std::fs::write(&path, bytes).unwrap();
    let p = read_project(&path).unwrap();
    finish(&dir);
    p
}

#[test]
fn projects_saved_by_v0_1_0_open_without_losing_anything() {
    for (name, bytes) in [("demo", V010_DEMO), ("completo", V010_COMPLETO)] {
        let old = raw(bytes);
        let p = open_bytes(bytes, name);
        assert_eq!(p.layers.len(), old["layers"].as_array().unwrap().len(), "{name}: capas");
        assert_eq!(count_strokes(&serde_json::to_value(&p).unwrap()), count_strokes(&old), "{name}: trazos");
        assert_eq!((p.width, p.height, p.fps, p.length), (old["width"].as_u64().unwrap() as u32, old["height"].as_u64().unwrap() as u32, old["fps"].as_u64().unwrap() as u32, old["length"].as_u64().unwrap() as u32));
        for (new, old) in p.layers.iter().zip(old["layers"].as_array().unwrap()) {
            assert_eq!(new.name, old["name"].as_str().unwrap());
            assert_eq!(new.keys.len(), old["keys"].as_object().unwrap().len(), "{name}: claves de {}", new.name);
            assert_eq!(new.tracks.count(), old["xform"].as_object().map_or(0, |o| o.len()), "{name}: claves de animación de {}", new.name);
            // Los datos de 3D y el tipo de capa guardados en 0.1.0 se conservan tal cual.
            assert_eq!(new.is3d, old["is3d"].as_bool().unwrap_or(false));
            assert_eq!(new.depth, old["depth"].as_f64().unwrap_or(0.0) as f32);
            assert_eq!(serde_json::to_value(new.kind).unwrap(), old.get("kind").cloned().unwrap_or(serde_json::json!("Vector")));
        }
        // Los trazos viejos no ganan campos nuevos: se ven y se editan como siempre.
        for l in &p.layers {
            for d in l.keys.values() {
                for s in &d.strokes {
                    assert!(s.style.is_none() && s.pressure.is_empty() && s.nodes.is_none() && s.original.is_none(), "{name}: un trazo viejo cambió");
                }
            }
        }
        assert!(!p.palettes.is_empty(), "{name}: un proyecto viejo recibe sus paletas");
    }
}

#[test]
fn old_projects_keep_their_details_and_render() {
    let old = raw(V010_COMPLETO);
    let p = open_bytes(V010_COMPLETO, "detalles");
    assert_eq!(p.range, Some((2, 18)));
    let kinds: Vec<LayerKind> = p.layers.iter().map(|l| l.kind).collect();
    // Las capas Stop Motion y las colecciones no existían en 0.1.0: el resto de clases sí están.
    for k in LayerKind::ALL.into_iter().filter(|k| !matches!(k, LayerKind::StopMotion | LayerKind::Collection)) {
        assert!(kinds.contains(&k), "falta una capa {k:?} del proyecto viejo");
    }
    assert!(p.layers.iter().all(|l| l.stopmotion.is_none()), "un proyecto viejo no trae secuencias Stop Motion");
    assert_eq!(old["range"], serde_json::json!([2, 18]));
    let pm = render::render_frame(&p, 0, 0.25).unwrap();
    assert!(pm.pixels().iter().any(|px| px.alpha() > 0));
    // Un proyecto viejo, al guardarse con la versión nueva, no escribe campos que no usa y se vuelve a abrir igual.
    let dir = temp_dir("viejo_guardado");
    let path = dir.join("v.ltoon");
    write_project(&p, &path, false).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    for field in ["\"nodes\"", "\"original\"", "\"shape\"", "\"pressure\"", "\"style\""] {
        assert!(!text.contains(field), "el proyecto viejo ganó el campo {field}");
    }
    assert_eq!(value(&read_project(&path).unwrap()), value(&p));
    finish(&dir);
}

#[test]
fn files_with_missing_or_unknown_fields_still_open() {
    // Mínimo imprescindible: solo lo que tenía la primera versión.
    let minimal = br#"{"width":640,"height":360,"fps":12,"length":24,"background":[255,255,255,255],
        "layers":[{"name":"A","visible":true,"locked":false,"opacity":1.0,"keys":{}}],"palette":[]}"#;
    let p = open_bytes(minimal, "minimo");
    assert_eq!((p.width, p.fps), (640, 12));
    assert!(!p.palettes.is_empty() && p.layers[0].kind == LayerKind::Vector);
    // Un archivo de una versión futura puede traer campos desconocidos: se ignoran sin fallar.
    let mut v = raw(V010_DEMO);
    v["campo_del_futuro"] = serde_json::json!({"a": 1});
    v["layers"][0]["otro_campo"] = serde_json::json!([1, 2, 3]);
    let p = open_bytes(&serde_json::to_vec(&v).unwrap(), "futuro");
    assert_eq!(p.layers.len(), raw(V010_DEMO)["layers"].as_array().unwrap().len());
}

// ---------- Archivos dañados ----------

#[test]
fn damaged_files_give_a_clear_error_and_never_panic() {
    let dir = temp_dir("danados");
    let half = V010_DEMO[..V010_DEMO.len() / 2].to_vec();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("vacío", Vec::new()),
        ("texto", b"esto no es un proyecto".to_vec()),
        ("recortado a la mitad", half),
        ("otro JSON", b"[1, 2, 3]".to_vec()),
        ("sin lo imprescindible", br#"{"width": 0}"#.to_vec()),
        ("bytes sueltos", vec![0xff, 0xfe, 0x00, 0x13, 0x37]),
    ];
    for (name, bytes) in cases {
        let path = dir.join("danado.ltoon");
        std::fs::write(&path, &bytes).unwrap();
        let err = read_project(&path).expect_err(name);
        assert!(err.contains("no es un proyecto válido"), "{name}: mensaje poco claro: {err}");
    }
    // Un archivo que no existe también da un error, no un fallo.
    assert!(read_project(&dir.join("no_existe.ltoon")).is_err());
    finish(&dir);
}

#[test]
fn absurd_values_in_a_valid_file_are_repaired() {
    let mut v = raw(V010_DEMO);
    v["width"] = serde_json::json!(0);
    v["height"] = serde_json::json!(99999);
    v["fps"] = serde_json::json!(0);
    v["length"] = serde_json::json!(0);
    v["layers"][0]["opacity"] = serde_json::json!(7.5);
    let p = open_bytes(&serde_json::to_vec(&v).unwrap(), "absurdo");
    assert!(p.width >= 16 && p.height <= 8192 && p.fps >= 1 && p.length >= 1);
    assert!((0.0..=1.0).contains(&p.layers[0].opacity));
    // Y se puede dibujar y exportar con él sin fallar.
    assert!(render::render_frame(&p, 0, 0.1).is_some());
}

#[test]
fn a_failed_save_leaves_the_previous_file_intact() {
    let dir = temp_dir("escritura_fallida");
    let path = dir.join("p.ltoon");
    let p = rich_project();
    write_project(&p, &path, false).unwrap();
    let before = std::fs::read(&path).unwrap();
    // El archivo temporal no se puede crear (hay una carpeta con ese nombre): el guardado falla sin tocar nada.
    std::fs::create_dir_all(path.with_extension("ltoon.tmp")).unwrap();
    let mut q = p.clone();
    q.fps = 7;
    let err = write_project(&q, &path, true).expect_err("debería fallar");
    assert!(!err.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), before, "el archivo anterior cambió");
    assert_eq!(read_project(&path).unwrap().fps, p.fps);
    // Una carpeta que no existe y no se puede crear (el padre es un archivo) también da error.
    let blocker = dir.join("bloqueo");
    std::fs::write(&blocker, b"x").unwrap();
    assert!(write_project(&p, &blocker.join("dentro").join("p.ltoon"), false).is_err());
    finish(&dir);
}

// ---------- Exportación ----------

/// Proyecto con la hoja vacía del color de un papel de estudio, una línea vectorial roja y una línea de píxeles azul.
fn export_project() -> Project {
    let mut p = Project::demo();
    let tmk = &paper::PAPERS[8];
    p.background = [tmk.rgb[0], tmk.rgb[1], tmk.rgb[2], 255];
    p.layers.clear();
    let mut v = Layer::new("Vectorial");
    let red = Stroke::shape(ShapeKind::Line, [300.0, 200.0], [900.0, 200.0], [220, 30, 30, 255], 40.0, None);
    v.keys.insert(0, Arc::new(Drawing { strokes: vec![red], raster: None }));
    let mut b = Layer::new("Píxeles");
    b.kind = LayerKind::Bitmap;
    let mut r = Raster::new(p.width, p.height);
    render::stamp_stroke(&mut r, &Stroke::shape(ShapeKind::Line, [300.0, 500.0], [900.0, 500.0], [30, 90, 220, 255], 40.0, None));
    b.keys.insert(0, Arc::new(Drawing { strokes: Vec::new(), raster: Some(Arc::new(r)) }));
    p.layers = vec![v, b];
    p.length = 12;
    p
}

fn px(pm: &tiny_skia::Pixmap, x: f32, y: f32, scale: f32) -> [u8; 4] {
    let c = pm.pixel((x * scale) as u32, (y * scale) as u32).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn png_export_has_the_right_size_content_and_no_paper_grain() {
    let dir = temp_dir("png");
    let p = export_project();
    let path = dir.join("f.png");
    render::export_png_opts(&p, 0, 0.5, false, &path).unwrap();
    let pm = tiny_skia::Pixmap::load_png(&path).unwrap();
    assert_eq!((pm.width(), pm.height()), ((p.width as f32 * 0.5).round() as u32, (p.height as f32 * 0.5).round() as u32));
    let tmk = &paper::PAPERS[8];
    // La hoja vacía es del color exacto del papel: el grano es solo una ayuda de la vista y no se exporta.
    assert_eq!(px(&pm, 20.0, 20.0, 0.5), [tmk.rgb[0], tmk.rgb[1], tmk.rgb[2], 255]);
    let red = px(&pm, 600.0, 200.0, 0.5);
    assert!(red[0] > 200 && red[1] < 60 && red[2] < 60, "la línea vectorial no salió roja: {red:?}");
    let blue = px(&pm, 600.0, 500.0, 0.5);
    assert!(blue[2] > 180 && blue[0] < 70, "la línea de píxeles no salió azul: {blue:?}");
    // Con fondo transparente, lo vacío queda transparente y el trazo se conserva.
    let tp = dir.join("t.png");
    render::export_png_opts(&p, 0, 0.5, true, &tp).unwrap();
    let t = tiny_skia::Pixmap::load_png(&tp).unwrap();
    assert_eq!(px(&t, 20.0, 20.0, 0.5)[3], 0);
    assert!(px(&t, 600.0, 200.0, 0.5)[3] > 200);
    finish(&dir);
}

#[test]
fn image_sequence_export_writes_one_valid_png_per_frame() {
    let dir = temp_dir("secuencia");
    let p = export_project();
    let n = render::export_sequence_range(&p, &dir, 2, 5, 0.25, false).unwrap();
    assert_eq!(n, 4);
    for f in 3..=6 {
        let path = dir.join(format!("fotograma_{f:04}.png"));
        let pm = tiny_skia::Pixmap::load_png(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!((pm.width(), pm.height()), ((p.width as f32 * 0.25).round() as u32, (p.height as f32 * 0.25).round() as u32));
    }
    assert!(!dir.join("fotograma_0007.png").exists() && !dir.join("fotograma_0002.png").exists());
    finish(&dir);
}

#[test]
fn gif_export_has_one_frame_per_step_and_respects_the_max_width() {
    let dir = temp_dir("gif");
    let p = export_project();
    let path = dir.join("a.gif");
    render::export_gif_range(&p, &path, 0, 4, 320).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"GIF89a") || bytes.starts_with(b"GIF87a"), "no empieza como un GIF");
    let mut dec = gif::DecodeOptions::new();
    dec.set_color_output(gif::ColorOutput::RGBA);
    let mut r = dec.read_info(std::fs::File::open(&path).unwrap()).unwrap();
    let (w, h) = (r.width() as u32, r.height() as u32);
    assert!(w <= 320 && w >= 300, "ancho del GIF: {w}");
    assert!((w as f32 / h as f32 - p.width as f32 / p.height as f32).abs() < 0.02, "se deformó el GIF: {w}×{h}");
    let mut frames = 0;
    let mut red_seen = false;
    while let Some(f) = r.read_next_frame().unwrap() {
        frames += 1;
        let (fw, i) = (f.width as usize, (((200.0 * w as f32 / p.width as f32) as usize) * f.width as usize + (600.0 * w as f32 / p.width as f32) as usize) * 4);
        let _ = fw;
        red_seen |= f.buffer[i] > 180 && f.buffer[i + 1] < 90;
    }
    assert_eq!(frames, 5);
    assert!(red_seen, "la línea roja no aparece en el GIF");
    finish(&dir);
}

#[test]
fn sprite_sheet_export_lays_out_every_frame_and_describes_them() {
    let dir = temp_dir("hoja");
    let p = export_project();
    let path = dir.join("hoja.png");
    let n = render::export_sprite_sheet(&p, &path, 0, 3, 0.25, 2, false).unwrap();
    assert_eq!(n, 4);
    let pm = tiny_skia::Pixmap::load_png(&path).unwrap();
    let (cw, ch) = ((p.width as f32 * 0.25).round() as u32, (p.height as f32 * 0.25).round() as u32);
    assert_eq!((pm.width(), pm.height()), (cw * 2, ch * 2));
    let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(path.with_extension("json")).unwrap()).unwrap();
    assert_eq!(meta["frames"].as_array().unwrap().len(), 4);
    assert_eq!(meta["columns"], 2);
    finish(&dir);
}

#[test]
fn export_after_save_and_open_matches_the_original() {
    let dir = temp_dir("exportar_abierto");
    let p = rich_project();
    let back = save_and_open(&p, &dir, "p.ltoon");
    let (a, b) = (dir.join("a.png"), dir.join("b.png"));
    render::export_png_opts(&p, 0, 0.5, false, &a).unwrap();
    render::export_png_opts(&back, 0, 0.5, false, &b).unwrap();
    assert_eq!(tiny_skia::Pixmap::load_png(&a).unwrap().data(), tiny_skia::Pixmap::load_png(&b).unwrap().data());
    finish(&dir);
}

#[test]
fn mp4_export_works_when_ffmpeg_is_installed() {
    let Some(ffmpeg) = render::find_ffmpeg() else {
        eprintln!("ffmpeg no está instalado en este equipo: se omite la prueba de MP4");
        return;
    };
    let dir = temp_dir("mp4");
    let p = export_project();
    let path = dir.join("v.mp4");
    let n = render::export_video(&p, &ffmpeg, &path, 0, 5, 0.25).unwrap();
    assert_eq!(n, 6);
    assert!(std::fs::metadata(&path).map_or(false, |m| m.len() > 500), "el MP4 está vacío");
    finish(&dir);
}

/// Hallazgo de esta revisión: el exportador no dibuja las capas de color sólido (el visor sí). Cuando se corrija,
/// quitar `ignore` y esta prueba debe pasar.
#[test]
#[ignore = "pendiente (Issue #65): la exportación ignora las capas de color sólido"]
fn solid_color_layers_are_exported_like_in_the_viewer() {
    let mut p = Project::demo();
    p.layers.clear();
    let mut s = Layer::new("Color");
    s.kind = LayerKind::Solid;
    s.solid = [10, 200, 10, 255];
    p.layers.push(s);
    let pm = render::render_frame(&p, 0, 0.1).unwrap();
    assert_eq!(px(&pm, 100.0, 100.0, 0.1), [10, 200, 10, 255]);
}

// ---------- Stop Motion (Issue #59) ----------

#[test]
fn a_stop_motion_layer_with_a_real_folder_survives_saving_and_moving_the_project() {
    let root = temp_dir("stopmotion");
    let project_dir = root.join("proyecto");
    let jpeg = |seed: u8| {
        let mut out = Vec::new();
        let img = image::RgbImage::from_fn(64, 48, |x, y| image::Rgb([(x as u8).wrapping_add(seed), y as u8, seed]));
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90).encode_image(&img).unwrap();
        out
    };
    let mut folder = SeqFolder::create(&project_dir.join("fotos").join("escena1"), "Escena 1", 12).unwrap();
    folder.add_photo_bytes(&jpeg(1), None, None).unwrap();
    folder.add_photo_bytes(&jpeg(2), None, None).unwrap();
    folder.seq.push_item("s000001", 2).unwrap();
    folder.seq.push_item("s000002", 1).unwrap();
    folder.seq.push_item("s000001", 3).unwrap();
    folder.save().unwrap();

    let mut p = Project::demo();
    let mut layer = Layer::new("Stop Motion 1");
    layer.kind = LayerKind::StopMotion;
    layer.stopmotion = Some(SmRef::new(&folder, Some(&project_dir)));
    p.layers.insert(0, layer);
    let file = project_dir.join("peli.ltoon");
    write_project(&p, &file, false).unwrap();

    // El proyecto solo guarda la referencia, no las fotos: el archivo sigue siendo pequeño.
    let size = std::fs::metadata(&file).unwrap().len();
    assert!(size < 200_000, "el proyecto con fotos pesa {size} bytes: las fotos no deben ir dentro");
    assert!(!std::fs::read_to_string(&file).unwrap().contains("sha256:"), "los hashes de las fotos no van en el proyecto");

    // Se mueve el proyecto entero con su carpeta de fotos: se vuelve a encontrar.
    let moved = root.join("proyecto_movido");
    std::fs::rename(&project_dir, &moved).unwrap();
    let back = read_project(&moved.join("peli.ltoon")).unwrap();
    let r = back.layers[0].stopmotion.clone().expect("la capa conserva su referencia");
    assert_eq!(back.layers[0].kind, LayerKind::StopMotion);
    let Located::Found(dir) = r.locate(Some(&moved)) else { panic!("no se encontró la carpeta tras mover el proyecto") };
    let (opened, _) = SeqFolder::open(&dir).unwrap();
    assert_eq!((opened.seq.sources.len(), opened.seq.items.len(), opened.seq.total_frames()), (2, 3, 6));
    assert!(opened.verify(true).is_empty(), "las fotos están intactas tras mover el proyecto");
    assert!(render::render_frame(&back, 0, 0.1).is_some());
    finish(&root);
}

#[test]
fn drawing_on_a_stop_motion_layer_is_refused_and_the_layer_is_marked_as_pending() {
    use super::Tool;
    for t in [Tool::Brush, Tool::Vector, Tool::Pen, Tool::Eraser, Tool::Shape, Tool::Fill] {
        assert!(t.layer_block(LayerKind::StopMotion).is_some(), "{t:?} no debe dibujar en una capa Stop Motion");
    }
    assert!(!LayerKind::StopMotion.pending(), "desde #60 la capa se ve en la escena y se exporta");
    assert_eq!(LayerKind::StopMotion.label(), "Stop Motion");
    assert!(LayerKind::ALL.contains(&LayerKind::StopMotion));
}

// ---------- Fase 2 (#60): composición y exportación ----------

fn solid_png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    image::RgbImage::from_pixel(w, h, image::Rgb(rgb)).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

/// Proyecto de 100×50 a 12 fps con una capa Stop Motion de tres fotos de color (rojo ×2, verde ×1, azul ×3 cuadros).
fn stop_motion_project(root: &std::path::Path, photo: (u32, u32)) -> (Project, SeqFolder) {
    let mut folder = SeqFolder::create(&root.join("fotos"), "Colores", 12).unwrap();
    for (rgb, n) in [([255u8, 0, 0], 2u32), ([0, 255, 0], 1), ([0, 0, 255], 3)] {
        let id = folder.add_photo_bytes(&solid_png(photo.0, photo.1, rgb), None, None).unwrap().id;
        folder.seq.push_item(&id, n).unwrap();
    }
    folder.save().unwrap();
    let mut p = Project::blank(100, 50, 12, 6);
    p.background = [0, 0, 0, 255];
    p.layers.clear();
    let mut layer = Layer::new("Stop Motion 1");
    layer.kind = LayerKind::StopMotion;
    layer.stopmotion = Some(SmRef::new(&folder, None));
    p.layers.push(layer);
    (p, folder)
}

fn spx(pm: &tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = pm.pixel(x, y).unwrap();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

fn near(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| (*x as i32 - *y as i32).abs() <= 3)
}

#[test]
fn each_project_frame_shows_the_photo_that_belongs_to_it() {
    let root = temp_dir("sm_frames");
    let (p, _f) = stop_motion_project(&root, (100, 50));
    let want = [[255, 0, 0, 255], [255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [0, 0, 255, 255], [0, 0, 255, 255]];
    for (f, w) in want.iter().enumerate() {
        let pm = render::render_frame(&p, f as u32, 1.0).unwrap();
        assert!(near(spx(&pm, 50, 25), *w), "cuadro {f}: {:?}, se esperaba {w:?}", spx(&pm, 50, 25));
    }
    finish(&root);
}

#[test]
fn sequence_frames_are_mapped_with_the_project_speed_and_start() {
    let mut s = crate::stopmotion::Sequence::new("t", 12);
    s.sources.push(crate::stopmotion::SourceFrame { id: "a".into(), file: "x".into(), width: 1, height: 1, format: "png".into(), bytes: 1, hash: "h".into(), captured_at: None, device: None, exif: None });
    s.push_item("a", 2).unwrap();
    s.push_item("a", 1).unwrap();
    // Secuencia a 12 fps en un proyecto a 24: cada cuadro de la secuencia dura dos del proyecto.
    assert_eq!(s.item_for_project_frame(0, 24, 0), Some(0));
    assert_eq!(s.item_for_project_frame(0, 24, 3), Some(0));
    assert_eq!(s.item_for_project_frame(0, 24, 4), Some(1));
    assert_eq!(s.item_for_project_frame(0, 24, 6), None);
    // Misma velocidad y con desplazamiento: antes del inicio no hay foto.
    assert_eq!(s.item_for_project_frame(5, 12, 4), None);
    assert_eq!(s.item_for_project_frame(5, 12, 5), Some(0));
    assert_eq!(s.item_for_project_frame(5, 12, 7), Some(1));
    assert_eq!(s.item_for_project_frame(5, 12, 8), None);
    assert_eq!(s.item_for_project_frame(0, 0, 0), None);
}

#[test]
fn a_big_photo_is_fitted_without_distortion_and_leaves_bars() {
    let root = temp_dir("sm_fit");
    // Foto cuadrada en un cuadro 2:1: ocupa el centro (50×50) y deja barras negras a los lados.
    let (p, _f) = stop_motion_project(&root, (200, 200));
    let pm = render::render_frame(&p, 0, 1.0).unwrap();
    assert!(near(spx(&pm, 50, 25), [255, 0, 0, 255]));
    assert!(near(spx(&pm, 5, 25), [0, 0, 0, 255]), "barra izquierda: {:?}", spx(&pm, 5, 25));
    assert!(near(spx(&pm, 95, 25), [0, 0, 0, 255]));
    finish(&root);
}

#[test]
fn layer_opacity_and_visibility_apply_to_the_photo() {
    let root = temp_dir("sm_opacity");
    let (mut p, _f) = stop_motion_project(&root, (100, 50));
    p.layers[0].opacity = 0.5;
    let pm = render::render_frame(&p, 0, 1.0).unwrap();
    let c = spx(&pm, 50, 25);
    assert!((c[0] as i32 - 128).abs() <= 4 && c[1] == 0, "media opacidad sobre negro: {c:?}");
    p.layers[0].visible = false;
    let pm = render::render_frame(&p, 0, 1.0).unwrap();
    assert!(near(spx(&pm, 50, 25), [0, 0, 0, 255]));
    finish(&root);
}

#[test]
fn a_hidden_item_and_frames_outside_the_sequence_show_nothing() {
    let root = temp_dir("sm_hidden");
    let (mut p, mut f) = stop_motion_project(&root, (100, 50));
    f.seq.items[1].visible = false;
    f.save().unwrap();
    let pm = render::render_frame(&p, 2, 1.0).unwrap();
    assert!(near(spx(&pm, 50, 25), [0, 0, 0, 255]), "una posición oculta no se ve");
    p.layers[0].stopmotion.as_mut().unwrap().start = 4;
    let pm = render::render_frame(&p, 1, 1.0).unwrap();
    assert!(near(spx(&pm, 50, 25), [0, 0, 0, 255]), "antes de que empiece la secuencia");
    finish(&root);
}

#[test]
fn a_missing_folder_does_not_break_the_export() {
    let root = temp_dir("sm_missing");
    let (p, _f) = stop_motion_project(&root, (100, 50));
    std::fs::remove_dir_all(root.join("fotos")).unwrap();
    let pm = render::render_frame(&p, 0, 1.0).expect("la exportación no falla si falta la carpeta");
    assert!(near(spx(&pm, 50, 25), [0, 0, 0, 255]));
    finish(&root);
}

#[test]
fn rendering_never_modifies_the_originals_and_big_photos_are_read_reduced() {
    let root = temp_dir("sm_untouched");
    let (p, f) = stop_motion_project(&root, (3000, 1500));
    let before = f.verify(true);
    assert!(before.is_empty());
    let pm = render::render_frame(&p, 0, 1.0).unwrap();
    assert!(near(spx(&pm, 50, 25), [255, 0, 0, 255]));
    let (again, _) = SeqFolder::open(&f.dir).unwrap();
    assert!(again.verify(true).is_empty(), "los originales siguen intactos tras exportar");
    finish(&root);
}

#[test]
fn the_viewer_proxy_is_created_for_big_photos_and_the_original_is_untouched() {
    let root = temp_dir("sm_proxy");
    let (_p, f) = stop_motion_project(&root, (3000, 1500));
    let s = f.seq.sources[0].clone();
    let original = std::fs::read(f.dir.join(&s.file)).unwrap();
    let d = crate::stopmotion::load_proxy(&f.dir, &s, crate::stopmotion::PROXY_SIDE).unwrap();
    assert_eq!((d.w, d.h), (1280, 640));
    assert!(crate::stopmotion::proxy_path(&f.dir, &s.id).is_file(), "se creó el proxy");
    assert_eq!(std::fs::read(f.dir.join(&s.file)).unwrap(), original, "el original no cambió");
    finish(&root);
}

#[test]
fn the_project_does_not_save_the_runtime_folder_cache() {
    let root = temp_dir("sm_serde");
    let (mut p, _f) = stop_motion_project(&root, (100, 50));
    p.base_dir = Some(root.clone());
    let json = serde_json::to_string(&p).unwrap();
    assert!(!json.contains("base_dir") && !json.contains("\"sm\""), "la caché y la carpeta base no se guardan");
    finish(&root);
}

// ---------- Fase 3 (#61): la sala ----------

#[test]
fn the_stop_motion_room_is_registered_and_survives_saving() {
    use super::catalog::{room_set, RoomKind, ROOM_GROUPS};
    let k = RoomKind::from_id("stopmotion").expect("la sala existe por su id");
    assert_eq!(k, RoomKind::StopMotion);
    assert!(RoomKind::ALL.contains(&k));
    assert!(ROOM_GROUPS.iter().any(|(_, rooms)| rooms.contains(&k)), "sale en el menú «+ Nueva sala»");
    let set = room_set("stopmotion");
    assert_eq!(set.id, "stopmotion");
    assert!(set.rooms.contains(&k));
    let def = k.def();
    assert!(def.tl_horizontal && !def.story && def.tl_h >= 300.0, "la sala necesita espacio abajo para la secuencia");
    // Las salas no se repiten ni chocan por id.
    let mut ids: Vec<&str> = RoomKind::ALL.iter().map(|r| r.def().id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), RoomKind::ALL.len());
    // Un proyecto con la sala la conserva al guardar y abrir.
    let root = temp_dir("sala_sm");
    let mut p = Project::demo();
    p.rooms.push(crate::model::RoomTab { kind: "stopmotion".into(), name: Some("Mi sala".into()) });
    let file = root.join("p.ltoon");
    write_project(&p, &file, false).unwrap();
    let back = read_project(&file).unwrap();
    assert!(back.rooms.iter().any(|r| r.kind == "stopmotion" && r.name.as_deref() == Some("Mi sala")));
    finish(&root);
}

#[test]
fn editing_the_sequence_changes_the_summary_but_never_the_photos() {
    let root = temp_dir("sala_edit");
    let (p, mut f) = stop_motion_project(&root, (100, 50));
    let before: Vec<Vec<u8>> = f.seq.sources.iter().map(|s| std::fs::read(f.dir.join(&s.file)).unwrap()).collect();
    // Reordenar y quitar una posición cambia lo que se ve en cada cuadro, pero no los archivos.
    let ids: Vec<String> = f.seq.items.iter().map(|i| i.id.clone()).collect();
    f.seq.move_items(&ids[2..3], 0); // el azul pasa a ser el primero
    f.seq.remove_items(&ids[1..2]); // se quita el verde
    f.save().unwrap();
    let want = [[0, 0, 255, 255], [0, 0, 255, 255], [0, 0, 255, 255], [255, 0, 0, 255], [255, 0, 0, 255], [0, 0, 0, 255]];
    for (fr, w) in want.iter().enumerate() {
        let pm = render::render_frame(&p, fr as u32, 1.0).unwrap();
        assert!(near(spx(&pm, 50, 25), *w), "cuadro {fr}: {:?}", spx(&pm, 50, 25));
    }
    let after: Vec<Vec<u8>> = f.seq.sources.iter().map(|s| std::fs::read(f.dir.join(&s.file)).unwrap()).collect();
    assert_eq!(before, after, "las fotos originales no cambian al editar la secuencia");
    // Un fotograma vacío no pinta nada y deshacer lo quita.
    let mut h = crate::stopmotion::History::default();
    h.record(&f.seq);
    f.seq.insert_blank(0, 2);
    f.save().unwrap();
    let pm = render::render_frame(&p, 0, 1.0).unwrap();
    assert!(near(spx(&pm, 50, 25), [0, 0, 0, 255]), "el fotograma vacío se ve vacío");
    assert!(h.undo(&mut f.seq));
    f.save().unwrap();
    let pm = render::render_frame(&p, 0, 1.0).unwrap();
    assert!(near(spx(&pm, 50, 25), [0, 0, 255, 255]), "tras deshacer vuelve la foto");
    finish(&root);
}

/// Capa con un rectángulo gris que cubre toda la cámara.
fn gray_sheet(name: &str, gray: u8) -> Layer {
    let mut l = Layer::new(name);
    let r = Stroke::shape(ShapeKind::Rect, [0.0, 0.0], [1280.0, 720.0], [0, 0, 0, 0], 1.0, Some([gray, gray, gray, 255]));
    l.keys.insert(0, Arc::new(Drawing { strokes: vec![r], ..Default::default() }));
    l
}

fn center_pixel(p: &Project) -> [u8; 3] {
    let pm = crate::render::render_frame(p, 0, 0.25).expect("se dibuja");
    let (w, h) = (pm.width(), pm.height());
    let px = pm.pixel(w / 2, h / 2).expect("hay pixel");
    [px.red(), px.green(), px.blue()]
}

#[test]
fn el_modo_de_fusion_cambia_lo_que_se_exporta() {
    let mut p = Project::blank(1280, 720, 24, 4);
    p.background = [200, 100, 50, 255];
    p.layers = vec![gray_sheet("gris", 128)];
    let normal = center_pixel(&p);
    assert_eq!(normal, [128, 128, 128], "normal: tapa el fondo");
    p.layers[0].blend = crate::model::Blend::Multiply;
    let m = center_pixel(&p);
    // Multiplicar: fondo × gris / 255 (200→100, 100→50, 50→25).
    assert!((m[0] as i32 - 100).abs() <= 2 && (m[1] as i32 - 50).abs() <= 2 && (m[2] as i32 - 25).abs() <= 2, "multiplicar dio {m:?}");
    p.layers[0].blend = crate::model::Blend::Difference;
    let d = center_pixel(&p);
    assert!((d[0] as i32 - 72).abs() <= 2 && (d[1] as i32 - 28).abs() <= 2 && (d[2] as i32 - 78).abs() <= 2, "diferencia dio {d:?}");
}

#[test]
fn modo_de_fusion_y_vinculo_se_guardan_y_las_capas_antiguas_se_abren_normales() {
    let mut p = Project::blank(1280, 720, 24, 4);
    p.layers = vec![gray_sheet("hija", 10), gray_sheet("madre", 20)];
    p.layers[0].blend = crate::model::Blend::Screen;
    assert!(p.set_parent(0, Some(1)));
    let back: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(back.layers[0].blend, crate::model::Blend::Screen);
    let parent_uid = back.layers[0].parent.expect("conserva el vínculo");
    assert_eq!(back.layers[1].uid, parent_uid);
    // Un proyecto sin esos campos: normal y sin vínculos.
    let mut v: serde_json::Value = serde_json::to_value(&p).unwrap();
    for l in v["layers"].as_array_mut().unwrap() {
        l.as_object_mut().unwrap().remove("blend");
        l.as_object_mut().unwrap().remove("parent");
        l.as_object_mut().unwrap().remove("uid");
    }
    let old: Project = serde_json::from_value(v).unwrap();
    assert!(old.layers.iter().all(|l| l.blend == crate::model::Blend::Normal && l.parent.is_none() && l.uid == 0));
}

#[test]
fn una_coleccion_se_compone_como_unidad_con_su_opacidad_y_su_visibilidad() {
    let mut p = Project::blank(1280, 720, 24, 4);
    p.background = [200, 100, 50, 255];
    // Orden de arriba abajo: colección, y debajo sus dos capas (una gris encima de otra más clara).
    p.layers = vec![Layer::new("Colección"), gray_sheet("clara", 220), gray_sheet("gris", 100)];
    p.layers[0].kind = LayerKind::Collection;
    p.layers[0].uid = 1;
    p.layers[1].group = Some(1);
    p.layers[2].group = Some(1);
    p.normalize_groups();
    // Sin opacidad propia, la colección deja ver lo mismo que las capas sueltas: manda la de más arriba (la clara).
    assert_eq!(center_pixel(&p), [220, 220, 220]);
    // Con la mitad de opacidad, se mezcla el conjunto ya compuesto con el fondo (no cada capa por separado).
    p.layers[0].opacity = 0.5;
    let m = center_pixel(&p);
    assert!((m[0] as i32 - 210).abs() <= 2 && (m[1] as i32 - 160).abs() <= 2 && (m[2] as i32 - 135).abs() <= 2, "opacidad de la colección: {m:?}");
    // Con la colección oculta no se ve ninguna de sus capas.
    p.layers[0].visible = false;
    assert_eq!(center_pixel(&p), [200, 100, 50]);
}

#[test]
fn la_pertenencia_a_una_coleccion_se_guarda_y_se_repara() {
    let mut p = Project::blank(1280, 720, 24, 4);
    p.layers = vec![gray_sheet("a", 10), gray_sheet("b", 20)];
    let ci = p.group_layer(1).expect("se agrupa");
    assert_eq!(p.layers[ci].kind, LayerKind::Collection);
    assert_eq!(p.members_of(ci), vec![ci + 1]);
    let back: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    let c2 = back.layers.iter().position(|l| l.kind == LayerKind::Collection).unwrap();
    assert_eq!(back.members_of(c2).len(), 1);
    assert_eq!(back.collection_of(c2 + 1), Some(c2));
    // Un archivo cuya capa apunta a una colección que no existe la deja suelta.
    let mut q = back.clone();
    q.layers.retain(|l| l.kind != LayerKind::Collection);
    q.sanitize();
    assert!(q.layers.iter().all(|l| l.group.is_none()));
}
