//! Cámara simulada: un cuadro con un cuadrado que se mueve. Sirve para las pruebas automáticas y para probar la sala
//! sin cámara (`LEONTOON_FAKECAM=1`). Puede simular que la cámara se desconecta tras N cuadros.

use super::*;

#[derive(Clone, Debug)]
pub struct MockProvider {
    pub width: u32,
    pub height: u32,
    /// Tras estos cuadros, la cámara «se desconecta» (`frame` falla).
    pub fail_after: Option<u64>,
    /// Entrega JPEG (como una webcam MJPEG) en vez de RGB sin comprimir.
    pub jpeg: bool,
    /// Si es false, el dispositivo no declara ningún control.
    pub with_controls: bool,
    /// Carpeta con una secuencia de imágenes de referencia: la cámara las muestra en bucle (a 4 por segundo) en vez del degradado.
    pub frames_dir: Option<std::path::PathBuf>,
}

impl Default for MockProvider {
    fn default() -> Self {
        MockProvider { width: 1280, height: 720, fail_after: None, jpeg: true, with_controls: true, frames_dir: None }
    }
}

impl CaptureProvider for MockProvider {
    fn name(&self) -> &'static str {
        "simulada"
    }

    fn list(&self) -> Result<Vec<DeviceInfo>, String> {
        Ok(vec![DeviceInfo { id: "simulada:0".into(), name: if self.frames_dir.is_some() { "Cámara de prueba (secuencia de referencia)".into() } else { "Cámara simulada".into() }, provider: "simulada".into() }])
    }

    fn open(&self, id: &str) -> Result<Box<dyn CaptureDevice>, String> {
        if id != "simulada:0" {
            return Err(format!("no existe el dispositivo {id}"));
        }
        let mut files: Vec<std::path::PathBuf> = self
            .frames_dir
            .as_ref()
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png") || x.eq_ignore_ascii_case("jpg"))).collect())
            .unwrap_or_default();
        files.sort();
        Ok(Box::new(MockDevice { cfg: self.clone(), n: 0, brightness: 50, files, start: std::time::Instant::now() }))
    }
}

struct MockDevice {
    cfg: MockProvider,
    n: u64,
    brightness: i64,
    files: Vec<std::path::PathBuf>,
    start: std::time::Instant,
}

impl CaptureDevice for MockDevice {
    fn controls(&mut self) -> Vec<Control> {
        if !self.cfg.with_controls {
            return Vec::new();
        }
        vec![Control { key: "Brightness".into(), label: "Brillo".into(), min: 0, max: 100, step: 1, value: self.brightness, default: 50, boolean: false }]
    }

    fn frame(&mut self) -> Result<RawFrame, String> {
        if self.cfg.fail_after.is_some_and(|k| self.n >= k) {
            return Err("la cámara se desconectó".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
        self.n += 1;
        if !self.files.is_empty() {
            let i = (self.start.elapsed().as_millis() / 250) as usize % self.files.len();
            let mut img = image::open(&self.files[i]).map_err(|e| e.to_string())?.to_rgb8();
            let (w, h) = (img.width(), img.height());
            let lum = (self.brightness as f32 / 50.0).clamp(0.0, 2.0);
            if (lum - 1.0).abs() > 0.01 {
                for p in img.pixels_mut() {
                    for c in p.0.iter_mut() {
                        *c = (*c as f32 * lum).min(255.0) as u8;
                    }
                }
            }
            return Ok(RawFrame { width: w, height: h, data: img.into_raw(), jpeg: false });
        }
        let (w, h) = (self.cfg.width, self.cfg.height);
        let mut img = image::RgbImage::new(w, h);
        let lum = (self.brightness as f32 / 50.0).clamp(0.0, 2.0);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            *p = image::Rgb([((40.0 + 90.0 * u) * lum).min(255.0) as u8, ((60.0 + 80.0 * v) * lum).min(255.0) as u8, (120.0 * lum).min(255.0) as u8]);
        }
        // El cuadrado se desplaza con cada cuadro: sirve para ver el onion skin entre capturas.
        let side = h / 5;
        let x0 = (self.n * 7 % (w - side) as u64) as u32;
        for y in h / 3..h / 3 + side {
            for x in x0..x0 + side {
                img.put_pixel(x, y, image::Rgb([240, 200, 60]));
            }
        }
        if self.cfg.jpeg {
            let mut bytes = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
            Ok(RawFrame { width: w, height: h, data: bytes, jpeg: true })
        } else {
            Ok(RawFrame { width: w, height: h, data: img.into_raw(), jpeg: false })
        }
    }

    fn set_control(&mut self, key: &str, value: i64) -> Result<(), String> {
        if key != "Brightness" || !self.cfg.with_controls {
            return Err(format!("el control {key} no existe en este dispositivo"));
        }
        self.brightness = value.clamp(0, 100);
        Ok(())
    }
}
