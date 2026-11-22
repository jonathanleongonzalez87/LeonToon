//! Prueba aislada de la Fase 0 de Stop Motion (Issue #58). No forma parte de LeonToon.
//!
//! Qué hace: lista las cámaras que ve Windows, muestra los formatos que ofrece la elegida, mide cuántos cuadros por
//! segundo da el live view en cada formato y guarda una foto a la máxima resolución en JPEG y en PNG, con los tiempos.
//!
//! Uso:  cargo run --release -- [índice de cámara] [carpeta de salida]

use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{ApiBackend, CameraFormat, CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::{query, Camera};
use nokhwa::utils::FrameFormat;
use std::time::{Duration, Instant};

/// Decodifica un cuadro a RGB. Las cámaras que dan MJPEG entregan un JPEG ya comprimido por la cámara: se decodifica con
/// la biblioteca de imágenes (no hace falta ninguna dependencia de C). Los demás formatos (NV12, YUYV) los convierte nokhwa.
fn to_rgb(frame: &nokhwa::Buffer) -> Option<image::RgbImage> {
    if frame.source_frame_format() == FrameFormat::MJPEG {
        image::load_from_memory_with_format(frame.buffer(), image::ImageFormat::Jpeg).ok().map(|i| i.to_rgb8())
    } else {
        frame.decode_image::<RgbFormat>().ok()
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let index: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(0);
    let out_dir = std::path::PathBuf::from(args.next().unwrap_or_else(|| "salida".into()));
    std::fs::create_dir_all(&out_dir).expect("no se pudo crear la carpeta de salida");

    println!("== Cámaras que ve el sistema ==");
    match query(ApiBackend::Auto) {
        Ok(list) if list.is_empty() => {
            println!("(ninguna)");
            return;
        }
        Ok(list) => {
            for c in &list {
                println!("  [{}] {}  ·  {}  ·  {}", c.index(), c.human_name(), c.description(), c.misc());
            }
        }
        Err(e) => {
            println!("No se pudo consultar las cámaras: {e}");
            return;
        }
    }

    println!("\n== Formatos de la cámara {index} ==");
    let probe = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestResolution);
    let mut cam = match Camera::new(CameraIndex::Index(index), probe) {
        Ok(c) => c,
        Err(e) => {
            println!("No se pudo abrir la cámara: {e}");
            return;
        }
    };
    let mut formats: Vec<CameraFormat> = cam.compatible_camera_formats().unwrap_or_default();
    formats.sort_by_key(|f| (std::cmp::Reverse(f.width() * f.height()), std::cmp::Reverse(f.frame_rate())));
    for f in formats.iter().take(14) {
        println!("  {}×{} a {} fps  ({:?})", f.width(), f.height(), f.frame_rate(), f.format());
    }
    println!("  … {} formatos en total", formats.len());
    // Controles que la cámara declara de verdad (la interfaz no debe mostrar los que no existan).
    println!("\n== Controles que declara la cámara {index} ==");
    match cam.camera_controls() {
        Ok(list) if !list.is_empty() => {
            for c in list {
                println!("  {:?}: {:?}", c.control(), c.description());
            }
        }
        Ok(_) => println!("  (ninguno)"),
        Err(e) => println!("  no se pudieron consultar: {e}"),
    }
    drop(cam);

    // Live view: unos 3 segundos en cada uno de los formatos más grandes (uno por formato de píxel).
    println!("\n== Live view (cuadros por segundo reales, 3 s cada uno) ==");
    let mut seen = Vec::new();
    for f in formats.iter() {
        if seen.contains(&(f.format(), f.width())) || seen.len() >= 4 {
            continue;
        }
        seen.push((f.format(), f.width()));
        let req = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(*f));
        let Ok(mut c) = Camera::new(CameraIndex::Index(index), req) else {
            println!("  {}×{} {:?}: no se pudo abrir", f.width(), f.height(), f.format());
            continue;
        };
        if c.open_stream().is_err() {
            println!("  {}×{} {:?}: no se pudo iniciar el flujo", f.width(), f.height(), f.format());
            continue;
        }
        let t0 = Instant::now();
        let (mut n, mut decode_ms) = (0u32, 0u128);
        while t0.elapsed() < Duration::from_secs(3) {
            let Ok(frame) = c.frame() else { break };
            let d = Instant::now();
            let _ = to_rgb(&frame);
            decode_ms += d.elapsed().as_millis();
            n += 1;
        }
        let secs = t0.elapsed().as_secs_f32();
        println!(
            "  {}×{} {:?}: {:.1} cuadros/s con decodificación a RGB ({} cuadros; {:.1} ms por cuadro decodificado)",
            f.width(),
            f.height(),
            f.format(),
            n as f32 / secs,
            n,
            decode_ms as f32 / n.max(1) as f32
        );
        let _ = c.stop_stream();
    }

    // Foto a la máxima resolución: se prefiere MJPEG (la propia cámara entrega un JPEG) y, si no hay, el mayor formato.
    println!("\n== Foto a la máxima resolución ==");
    let best = formats.iter().find(|f| f.format() == FrameFormat::MJPEG).or(formats.first()).copied().expect("la cámara no ofrece formatos");
    println!("  formato elegido: {}×{} a {} fps ({:?})", best.width(), best.height(), best.frame_rate(), best.format());
    let req = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(best));
    let mut c = Camera::new(CameraIndex::Index(index), req).expect("no se pudo abrir la cámara");
    c.open_stream().expect("no se pudo iniciar el flujo");
    // Los primeros cuadros suelen salir oscuros mientras la cámara ajusta la exposición.
    for _ in 0..20 {
        let _ = c.frame();
    }
    let t0 = Instant::now();
    let frame = c.frame().expect("no llegó ningún cuadro");
    let got = t0.elapsed();
    let raw = frame.buffer().to_vec();
    let t1 = Instant::now();
    let img = to_rgb(&frame).expect("no se pudo decodificar el cuadro");
    let dec = t1.elapsed();
    println!("  {}×{}  (cuadro recibido en {} ms, {} KB tal como lo entrega la cámara, decodificado en {} ms)", img.width(), img.height(), got.as_millis(), raw.len() / 1000, dec.as_millis());
    let size = |p: &std::path::Path| std::fs::metadata(p).map(|m| m.len() as f64 / 1_000_000.0).unwrap_or(0.0);
    if frame.source_frame_format() == FrameFormat::MJPEG {
        // El original sin volver a comprimir: los bytes JPEG de la cámara, tal cual.
        let orig = out_dir.join("foto_original_de_la_camara.jpg");
        std::fs::write(&orig, &raw).expect("no se pudo guardar el JPEG original");
        println!("  JPEG original de la cámara (sin recomprimir): {:.2} MB  ({})", size(&orig), orig.display());
    }
    let jpg = out_dir.join("foto_recodificada.jpg");
    let png = out_dir.join("foto.png");
    let t = Instant::now();
    img.save(&jpg).expect("no se pudo guardar el JPEG");
    let tj = t.elapsed();
    let t = Instant::now();
    img.save(&png).expect("no se pudo guardar el PNG");
    let tp = t.elapsed();
    println!("  JPEG recodificado: {:.2} MB en {} ms  ({})", size(&jpg), tj.as_millis(), jpg.display());
    println!("  PNG:  {:.2} MB en {} ms  ({})", size(&png), tp.as_millis(), png.display());
    let _ = c.stop_stream();
}