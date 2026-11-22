//! Mide cuánto cuesta leer fotos grandes: decodificar un JPEG, hacer una miniatura y cuánta memoria ocupa sin comprimir.
//! Sirve para dimensionar la carga bajo demanda, las miniaturas y los proxies de Stop Motion (Issue #58).
//! No necesita cámara: crea fotos sintéticas con aspecto de foto (degradados y ruido).

use image::{imageops, ImageFormat, RgbImage};
use std::io::Cursor;
use std::time::Instant;

fn synthetic(w: u32, h: u32) -> RgbImage {
    let mut seed = 12345u32;
    RgbImage::from_fn(w, h, |x, y| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let n = ((seed >> 24) as i32 - 128) / 12;
        let r = (x as f32 / w as f32 * 255.0) as i32 + n;
        let g = (y as f32 / h as f32 * 255.0) as i32 + n;
        let b = (((x + y) as f32 / (w + h) as f32) * 255.0) as i32 + n;
        image::Rgb([r.clamp(0, 255) as u8, g.clamp(0, 255) as u8, b.clamp(0, 255) as u8])
    })
}

fn main() {
    println!("{:>12} {:>9} {:>11} {:>14} {:>16} {:>17}", "foto", "JPEG", "sin comprimir", "decodificar", "miniatura 320 px", "proxy 1280 px");
    for (name, w, h) in [("2 MP", 1920u32, 1080u32), ("12 MP", 4000, 3000), ("24 MP", 6000, 4000), ("45 MP", 8192, 5464)] {
        let img = synthetic(w, h);
        let mut jpg = Vec::new();
        img.write_to(&mut Cursor::new(&mut jpg), ImageFormat::Jpeg).unwrap();
        let t = Instant::now();
        let dec = image::load_from_memory_with_format(&jpg, ImageFormat::Jpeg).unwrap().to_rgb8();
        let dec_ms = t.elapsed().as_millis();
        let t = Instant::now();
        let _th = imageops::thumbnail(&dec, 320, (320.0 * h as f32 / w as f32) as u32);
        let th_ms = t.elapsed().as_millis();
        let t = Instant::now();
        let _px = imageops::resize(&dec, 1280, (1280.0 * h as f32 / w as f32) as u32, imageops::FilterType::Triangle);
        let px_ms = t.elapsed().as_millis();
        println!(
            "{:>12} {:>7.1}MB {:>9.0}MB {:>11} ms {:>13} ms {:>14} ms",
            name,
            jpg.len() as f32 / 1e6,
            (w * h * 3) as f32 / 1e6,
            dec_ms,
            th_ms,
            px_ms
        );
    }
}