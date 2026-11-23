//! Dibujo "fuera de pantalla" para exportar: PNG, secuencias, hoja de sprites y GIF animado.

use crate::model::{Drawing, Layer, LayerKind, Project, Rgba, Stroke, XKey};
use crate::raster::Raster;
use std::path::Path;
use tiny_skia::{BlendMode, Color, FillRule, FilterQuality, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, PixmapRef, Transform};

/// Pinta un trazo en los píxeles de una capa de mapa de bits (mismo aspecto que el trazo vectorial).
pub fn stamp_stroke(r: &mut Raster, s: &Stroke) {
    let d = Drawing { strokes: vec![s.clone()], raster: None };
    r.edit(|pm| draw_drawing(pm, &d, Transform::identity()));
}

/// Borra con un pincel redondo de diámetro `width` a lo largo del segmento a→b.
pub fn erase_segment(r: &mut Raster, a: [f32; 2], b: [f32; 2], width: f32) {
    r.edit(|pm| {
        let mut p = Paint::default();
        p.set_color(Color::BLACK);
        p.blend_mode = BlendMode::Clear;
        p.anti_alias = true;
        if a == b {
            if let Some(path) = PathBuilder::from_circle(a[0], a[1], (width * 0.5).max(0.5)) {
                pm.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
            }
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(a[0], a[1]);
        pb.line_to(b[0], b[1]);
        if let Some(path) = pb.finish() {
            let st = tiny_skia::Stroke { width: width.max(1.0), line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
            pm.stroke_path(&path, &p, &st, Transform::identity(), None);
        }
    });
}

fn paint(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(c[0], c[1], c[2], c[3]));
    p.anti_alias = true;
    p
}

/// Transformación de una capa (giro y escala alrededor del centro de la cámara) y luego la escala de salida.
pub fn layer_transform(t: &XKey, w: f32, h: f32, scale: f32) -> Transform {
    let (cx, cy) = (w / 2.0 + t.ax, h / 2.0 + t.ay);
    Transform::from_translate(-cx, -cy)
        .post_scale(t.s, t.s)
        .post_rotate(t.r)
        .post_translate(cx + t.x, cy + t.y)
        .post_scale(scale, scale)
}

pub fn draw_drawing(pm: &mut Pixmap, d: &Drawing, tr: Transform) {
    // Capas de mapa de bits: los píxeles van debajo de los trazos.
    if let Some(r) = &d.raster {
        if let Some(pr) = PixmapRef::from_bytes(&r.data, r.w, r.h) {
            let pp = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
            pm.draw_pixmap(0, 0, pr, &pp, tr, None);
        }
    }
    for s in &d.strokes {
        if !s.is_plain() && !s.points.is_empty() {
            // Trazo vectorial de grosor variable: los polígonos se rellenan juntos (unión, sin doble mezcla).
            if let (Some(f), true) = (s.fill, s.points.len() >= 3) {
                let mut pb = PathBuilder::new();
                pb.move_to(s.points[0][0], s.points[0][1]);
                for p in &s.points[1..] {
                    pb.line_to(p[0], p[1]);
                }
                pb.close();
                if let Some(path) = pb.finish() {
                    pm.fill_path(&path, &paint(f), FillRule::Winding, tr, None);
                }
            }
            let mut pb = PathBuilder::new();
            for poly in s.pieces() {
                if poly.len() < 3 {
                    continue;
                }
                pb.move_to(poly[0][0], poly[0][1]);
                for v in &poly[1..] {
                    pb.line_to(v[0], v[1]);
                }
                pb.close();
            }
            if let Some(path) = pb.finish() {
                pm.fill_path(&path, &paint(s.color), FillRule::Winding, tr, None);
            }
            continue;
        }
        if s.points.len() == 1 {
            let p = s.points[0];
            if let Some(path) = PathBuilder::from_circle(p[0], p[1], (s.width * 0.5).max(0.25)) {
                pm.fill_path(&path, &paint(s.color), FillRule::Winding, tr, None);
            }
            continue;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(s.points[0][0], s.points[0][1]);
        for p in &s.points[1..] {
            pb.line_to(p[0], p[1]);
        }
        if s.closed {
            pb.close();
        }
        let Some(path) = pb.finish() else { continue };
        if let Some(f) = s.fill {
            pm.fill_path(&path, &paint(f), FillRule::Winding, tr, None);
        }
        let stroke = tiny_skia::Stroke {
            width: s.width.max(0.1),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint(s.color), &stroke, tr, None);
    }
}

/// El modo de fusión de una capa en tiny-skia.
fn blend_mode(b: crate::model::Blend) -> BlendMode {
    use crate::model::Blend;
    match b {
        Blend::Normal => BlendMode::SourceOver,
        Blend::Multiply => BlendMode::Multiply,
        Blend::Screen => BlendMode::Screen,
        Blend::Overlay => BlendMode::Overlay,
        Blend::Darken => BlendMode::Darken,
        Blend::Lighten => BlendMode::Lighten,
        Blend::Difference => BlendMode::Difference,
        Blend::Add => BlendMode::Plus,
    }
}

pub fn render_frame_opts(p: &Project, frame: u32, scale: f32, transparent: bool) -> Option<Pixmap> {
    let (fw, fh) = (p.width as f32, p.height as f32);
    let w = ((fw * scale).round() as u32).max(1);
    let h = ((fh * scale).round() as u32).max(1);
    let mut pm = Pixmap::new(w, h)?;
    if !transparent {
        let b = p.background;
        pm.fill(Color::from_rgba8(b[0], b[1], b[2], b[3]));
    }
    let any_solo = p.layers.iter().any(|l| l.solo);
    for (li, layer) in p.layers.iter().enumerate().rev() {
        if p.collection_of(li).is_some() {
            continue; // lo dibuja su colección
        }
        if layer.kind == LayerKind::Collection {
            draw_group(&mut pm, p, li, frame, scale, any_solo)?;
        } else {
            draw_layer(&mut pm, p, li, frame, scale, any_solo);
        }
    }
    Some(pm)
}

/// Dibuja una colección: sus capas (y las colecciones que contiene, a cualquier profundidad) se componen aparte y el conjunto se
/// mezcla con su opacidad y su modo de fusión.
fn draw_group(pm: &mut Pixmap, p: &Project, ci: usize, frame: u32, scale: f32, any_solo: bool) -> Option<()> {
    let layer = &p.layers[ci];
    if !layer.visible || !layer.active_at(frame) {
        return Some(());
    }
    let op = (layer.opacity * p.effective_at(ci, frame).o).clamp(0.0, 1.0);
    if op <= 0.0 {
        return Some(());
    }
    let mut gp = Pixmap::new(pm.width(), pm.height())?;
    for m in p.members_of(ci).into_iter().rev() {
        if p.layers[m].kind == LayerKind::Collection {
            draw_group(&mut gp, p, m, frame, scale, any_solo)?;
        } else {
            draw_layer(&mut gp, p, m, frame, scale, any_solo);
        }
    }
    let pp = PixmapPaint { opacity: op, blend_mode: blend_mode(layer.blend), ..Default::default() };
    pm.draw_pixmap(0, 0, gp.as_ref(), &pp, Transform::identity(), None);
    Some(())
}

/// Dibuja una capa (con su transformación, la de sus madres y la de su colección) sobre `pm`.
fn draw_layer(pm: &mut Pixmap, p: &Project, li: usize, frame: u32, scale: f32, any_solo: bool) -> Option<()> {
    let layer = &p.layers[li];
    let (fw, fh) = (p.width as f32, p.height as f32);
    let (w, h) = (pm.width(), pm.height());
    let t = p.effective_at(li, frame);
    let op = (layer.opacity * t.o).clamp(0.0, 1.0);
    if !p.shows(li, frame, any_solo) || op <= 0.0 {
        return Some(());
    }
    if layer.kind == LayerKind::StopMotion {
        draw_stopmotion(pm, p, layer, frame, &t, scale, op, blend_mode(layer.blend));
        return Some(());
    }
    if let Some(d) = layer.drawing_at(frame) {
        let mut lp = Pixmap::new(w, h)?;
        draw_drawing(&mut lp, d, layer_transform(&t, fw, fh, scale));
        let pp = PixmapPaint { opacity: op, blend_mode: blend_mode(layer.blend), ..Default::default() };
        pm.draw_pixmap(0, 0, lp.as_ref(), &pp, Transform::identity(), None);
    }
    Some(())
}
/// Dibuja la foto que toca en este cuadro de una capa Stop Motion, con el mismo cálculo de cuadros, ajuste y transformación
/// que el visor. Se lee el **original** (reducido al tamaño que hace falta) y nunca se modifica. Si la carpeta, la foto o la
/// secuencia faltan, la capa no dibuja nada (la exportación no falla por eso).
fn draw_stopmotion(pm: &mut Pixmap, p: &Project, layer: &Layer, frame: u32, t: &XKey, scale: f32, op: f32, blend: BlendMode) {
    let Some(r) = &layer.stopmotion else { return };
    let Some(folder) = p.sm.folder(r, p.base_dir.as_deref()) else { return };
    let Some(idx) = folder.seq.item_for_project_frame(r.start, p.fps, frame) else { return };
    let item = &folder.seq.items[idx];
    let Some(src) = folder.seq.source(&item.source).filter(|_| item.visible) else { return };
    if src.width == 0 || src.height == 0 {
        return;
    }
    let (fw, fh) = (p.width as f32, p.height as f32);
    let fit = (fw / src.width as f32).min(fh / src.height as f32);
    // La foto se reduce a lo que se va a ver: no hace falta pasar 24 megapíxeles para un fotograma de 1280 px.
    let max_side = (fw.max(fh) * scale * t.s.abs()).ceil().clamp(16.0, 8192.0) as u32;
    let Some(photo) = p.sm.photo(&folder, src, max_side) else { return };
    let k = photo.pixmap.width() as f32 / src.width as f32;
    let (rw, rh) = (src.width as f32 * fit, src.height as f32 * fit);
    let fit_t = Transform::from_translate((fw - rw) / 2.0, (fh - rh) / 2.0).pre_scale(fit / k, fit / k);
    let full = layer_transform(t, fw, fh, scale).pre_concat(fit_t);
    let paint = PixmapPaint { opacity: op, quality: FilterQuality::Bicubic, blend_mode: blend, ..Default::default() };
    pm.draw_pixmap(0, 0, photo.pixmap.as_ref(), &paint, full, None);
}

pub fn render_frame(p: &Project, frame: u32, scale: f32) -> Option<Pixmap> {
    render_frame_opts(p, frame, scale, false)
}

pub fn export_png_opts(p: &Project, frame: u32, scale: f32, transparent: bool, path: &Path) -> Result<(), String> {
    let pm = render_frame_opts(p, frame, scale, transparent).ok_or("No se pudo crear la imagen")?;
    pm.save_png(path).map_err(|e| e.to_string())
}

/// Exporta los fotogramas a..=b como fotograma_0001.png, … en una carpeta.
pub fn export_sequence_range(p: &Project, dir: &Path, a: u32, b: u32, scale: f32, transparent: bool) -> Result<u32, String> {
    let mut n = 0;
    for f in a..=b.min(p.length - 1) {
        let path = dir.join(format!("fotograma_{:04}.png", f + 1));
        export_png_opts(p, f, scale, transparent, &path)?;
        n += 1;
    }
    Ok(n)
}

/// Hoja de sprites: todos los fotogramas a..=b en una cuadrícula, más un .json con dónde está cada uno.
pub fn export_sprite_sheet(
    p: &Project,
    path: &Path,
    a: u32,
    b: u32,
    scale: f32,
    columns: u32,
    transparent: bool,
) -> Result<u32, String> {
    let b = b.min(p.length - 1);
    let n = b.saturating_sub(a) + 1;
    let cols = columns.clamp(1, n.max(1));
    let rows = n.div_ceil(cols);
    let cw = ((p.width as f32 * scale).round() as u32).max(1);
    let ch = ((p.height as f32 * scale).round() as u32).max(1);
    if (cw as u64 * cols as u64) > 16384 || (ch as u64 * rows as u64) > 16384 {
        return Err("La hoja sería demasiado grande: baja la escala o el rango".into());
    }
    let mut sheet = Pixmap::new(cw * cols, ch * rows).ok_or("No se pudo crear la imagen")?;
    let mut frames = Vec::new();
    for (i, f) in (a..=b).enumerate() {
        let pm = render_frame_opts(p, f, scale, transparent).ok_or("No se pudo crear la imagen")?;
        let (x, y) = ((i as u32 % cols) * cw, (i as u32 / cols) * ch);
        sheet.draw_pixmap(x as i32, y as i32, pm.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        frames.push(serde_json::json!({ "frame": f + 1, "x": x, "y": y, "w": cw, "h": ch }));
    }
    sheet.save_png(path).map_err(|e| e.to_string())?;
    let meta = serde_json::json!({ "image": path.file_name().map(|s| s.to_string_lossy().to_string()),
        "fps": p.fps, "frameWidth": cw, "frameHeight": ch, "columns": cols, "frames": frames });
    std::fs::write(path.with_extension("json"), serde_json::to_string_pretty(&meta).unwrap_or_default())
        .map_err(|e| e.to_string())?;
    Ok(n)
}

pub fn export_gif_range(p: &Project, path: &Path, a: u32, b: u32, max_width: u32) -> Result<(), String> {
    let scale = (max_width as f32 / p.width as f32).min(1.0);
    let w = ((p.width as f32 * scale).round() as u32).clamp(1, 65535);
    let h = ((p.height as f32 * scale).round() as u32).clamp(1, 65535);
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = gif::Encoder::new(std::io::BufWriter::new(file), w as u16, h as u16, &[]).map_err(|e| e.to_string())?;
    enc.set_repeat(gif::Repeat::Infinite).map_err(|e| e.to_string())?;
    let delay = ((100.0 / p.fps as f32).round() as u16).max(2);
    for f in a..=b.min(p.length - 1) {
        let pm = render_frame(p, f, scale).ok_or("No se pudo crear la imagen")?;
        let mut rgba: Vec<u8> = Vec::with_capacity((pm.width() * pm.height() * 4) as usize);
        for px in pm.pixels() {
            let c = px.demultiply();
            rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), 255]);
        }
        let mut frame = gif::Frame::from_rgba_speed(pm.width() as u16, pm.height() as u16, &mut rgba, 10);
        frame.delay = delay;
        enc.write_frame(&frame).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Busca ffmpeg: en el PATH, junto al programa, o en carpetas típicas de descargas.
pub fn find_ffmpeg() -> Option<std::path::PathBuf> {
    let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    if let Some(paths) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&paths) {
            let c = d.join(exe);
            if c.is_file() {
                return Some(c);
            }
        }
    }
    if let Ok(me) = std::env::current_exe() {
        if let Some(c) = me.parent().map(|d| d.join(exe)).filter(|c| c.is_file()) {
            return Some(c);
        }
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let home = std::path::PathBuf::from(home);
    for base in [home.join("Downloads"), home.join("Descargas"), home.join("ffmpeg"), std::path::PathBuf::from("C:\\ffmpeg")] {
        if let Some(c) = search_dir(&base, exe, 3) {
            return Some(c);
        }
    }
    None
}

fn search_dir(dir: &Path, exe: &str, depth: u32) -> Option<std::path::PathBuf> {
    let c = dir.join(exe);
    if c.is_file() {
        return Some(c);
    }
    if depth == 0 {
        return None;
    }
    let mut dirs: Vec<std::path::PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    // Las carpetas de ffmpeg primero
    dirs.sort_by_key(|p| !p.to_string_lossy().to_lowercase().contains("ffmpeg"));
    dirs.into_iter().take(60).find_map(|d| search_dir(&d, exe, depth - 1))
}

/// Vídeo MP4 (H.264) de los fotogramas a..=b, usando ffmpeg.
pub fn export_video(p: &Project, ffmpeg: &Path, path: &Path, a: u32, b: u32, scale: f32) -> Result<u32, String> {
    let tmp = std::env::temp_dir().join(format!("leontoon_video_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let b = b.min(p.length - 1);
    let mut n = 0;
    for f in a..=b {
        export_png_opts(p, f, scale, false, &tmp.join(format!("f_{:05}.png", n)))?;
        n += 1;
    }
    let mut cmd = std::process::Command::new(ffmpeg);
    cmd.args(["-y", "-loglevel", "error", "-framerate"])
        .arg(p.fps.to_string())
        .arg("-i")
        .arg(tmp.join("f_%05d.png"))
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2"])
        .arg(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // sin ventana de consola
    }
    let out = cmd.output().map_err(|e| format!("No se pudo iniciar ffmpeg: {e}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let out = out?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("ffmpeg falló: {}", err.lines().last().unwrap_or("error desconocido")));
    }
    Ok(n)
}

/// Guarda píxeles RGBA (de la captura de la ventana) como PNG.
pub fn save_rgba_png(w: u32, h: u32, rgba: &[u8], path: &Path) -> Result<(), String> {
    let mut pm = Pixmap::new(w, h).ok_or("Tamaño de imagen no válido")?;
    let data = pm.data_mut();
    if data.len() != rgba.len() {
        return Err("Tamaño de datos incorrecto".into());
    }
    // La captura es opaca, así que no hace falta premultiplicar.
    for (dst, src) in data.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        dst.copy_from_slice(&[src[0], src[1], src[2], 255]);
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    pm.save_png(path).map_err(|e| e.to_string())
}
