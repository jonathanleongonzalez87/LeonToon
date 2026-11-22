//! Captura de fotos desde cámaras (Issue #62, Fase 4 del épico #57).
//!
//! Arquitectura: un **proveedor** (`CaptureProvider`) sabe listar y abrir los dispositivos de un tipo (webcams UVC, o una
//! cámara simulada para pruebas); un **dispositivo** (`CaptureDevice`) entrega cuadros y declara sus controles. Una
//! `Session` abre el dispositivo en su propio hilo, así que la interfaz nunca espera a la cámara: recibe el Live View
//! (reducido, solo el cuadro más reciente) y, al capturar, la foto **a la resolución del dispositivo**, independiente del
//! Live View. Añadir otra cámara después (Canon, Nikon…) es escribir otro proveedor; nada más cambia.
//!
//! Este módulo no depende de la interfaz ni de la secuencia.
// Parte de esta interfaz la usan las pruebas o las fases siguientes.
#![allow(dead_code)]

mod mock;
mod webcam;
#[cfg(test)]
mod tests;

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

pub use mock::MockProvider;
pub use webcam::WebcamProvider;

/// Lado máximo del Live View: es solo para ver la escena, no la foto final.
pub const LIVE_WIDTH: u32 = 640;

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceInfo {
    /// `proveedor:dispositivo`, por ejemplo `webcam:1`.
    pub id: String,
    pub name: String,
    pub provider: String,
}

/// Un control que el dispositivo declara de verdad (brillo, enfoque…). La interfaz solo muestra los que existen.
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    pub key: String,
    pub label: String,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub value: i64,
    pub default: i64,
    /// Interruptor (0 o 1) en vez de deslizador.
    pub boolean: bool,
}

/// Un cuadro tal como lo entrega la cámara.
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    /// JPEG ya comprimido por la cámara (`jpeg`), o píxeles RGB sin comprimir.
    pub data: Vec<u8>,
    pub jpeg: bool,
}

/// Un dispositivo abierto. Vive en el hilo de la sesión.
pub trait CaptureDevice {
    fn controls(&mut self) -> Vec<Control>;
    /// Espera el siguiente cuadro. Un error significa que la cámara dejó de responder (por ejemplo, se desconectó).
    fn frame(&mut self) -> Result<RawFrame, String>;
    fn set_control(&mut self, key: &str, value: i64) -> Result<(), String>;
}

pub trait CaptureProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn list(&self) -> Result<Vec<DeviceInfo>, String>;
    fn open(&self, id: &str) -> Result<Box<dyn CaptureDevice>, String>;
}

/// Todos los proveedores disponibles. `LEONTOON_FAKECAM=1` usa solo la cámara simulada (para probar sin tocar ninguna cámara real).
pub fn providers() -> Vec<Arc<dyn CaptureProvider>> {
    if std::env::var("LEONTOON_FAKECAM").is_ok() {
        return vec![Arc::new(MockProvider::default())];
    }
    vec![Arc::new(WebcamProvider)]
}

/// Lista los dispositivos de todos los proveedores. Un proveedor que falla no impide a los demás; sus errores se devuelven aparte.
pub fn list_all(providers: &[Arc<dyn CaptureProvider>]) -> (Vec<DeviceInfo>, Vec<String>) {
    let (mut devices, mut errors) = (Vec::new(), Vec::new());
    for p in providers {
        match p.list() {
            Ok(d) => devices.extend(d),
            Err(e) => errors.push(format!("{}: {e}", p.name())),
        }
    }
    (devices, errors)
}

/// El proveedor al que pertenece un dispositivo (por el prefijo de su id).
pub fn provider_of(providers: &[Arc<dyn CaptureProvider>], id: &str) -> Option<Arc<dyn CaptureProvider>> {
    let prefix = id.split(':').next().unwrap_or("");
    providers.iter().find(|p| p.name() == prefix).cloned()
}

/// Cuadro reducido para el Live View (RGBA opaco).
pub struct LiveFrame {
    pub seq: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Tamaño real del cuadro de la cámara (la foto final tiene este).
    pub full: (u32, u32),
}

/// Una foto capturada: el archivo tal como lo dio la cámara (JPEG) o un PNG si la cámara no comprime.
pub struct CapturedPhoto {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub png: bool,
}

pub enum Event {
    /// El dispositivo se abrió y declara estos controles.
    Opened { controls: Vec<Control>, size: (u32, u32) },
    Photo(CapturedPhoto),
    /// La cámara falló o se desconectó: la sesión terminó.
    Failed(String),
    /// Algo no salió bien pero la cámara sigue (un control que no se pudo cambiar, una foto que no se pudo codificar).
    Warning(String),
}

enum Cmd {
    Capture,
    SetControl(String, i64),
    Stop,
}

/// Una cámara abierta en su hilo.
pub struct Session {
    pub device: DeviceInfo,
    tx: Sender<Cmd>,
    pub events: Receiver<Event>,
    live: Arc<Mutex<Option<LiveFrame>>>,
}

impl Session {
    pub fn start(provider: Arc<dyn CaptureProvider>, device: DeviceInfo) -> Session {
        let (tx, rx) = channel::<Cmd>();
        let (etx, events) = channel::<Event>();
        let live = Arc::new(Mutex::new(None));
        let (live2, id) = (live.clone(), device.id.clone());
        std::thread::spawn(move || run(provider, &id, rx, etx, live2));
        Session { device, tx, events, live }
    }

    /// Pide una foto: llegará como `Event::Photo` con el próximo cuadro completo.
    pub fn capture(&self) {
        let _ = self.tx.send(Cmd::Capture);
    }

    pub fn set_control(&self, key: &str, value: i64) {
        let _ = self.tx.send(Cmd::SetControl(key.to_string(), value));
    }

    /// El cuadro de Live View más reciente, si hay uno nuevo desde la última vez.
    pub fn take_live(&self) -> Option<LiveFrame> {
        self.live.lock().ok().and_then(|mut g| g.take())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Stop);
    }
}

fn decode_rgb(f: &RawFrame) -> Option<image::RgbImage> {
    if f.jpeg {
        image::load_from_memory_with_format(&f.data, image::ImageFormat::Jpeg).ok().map(|i| i.to_rgb8())
    } else {
        image::RgbImage::from_raw(f.width, f.height, f.data.clone())
    }
}

/// Reduce un cuadro RGB a `LIVE_WIDTH` de ancho (por promedio de bloques) y lo pasa a RGBA.
fn live_from(rgb: &image::RgbImage, seq: u64) -> LiveFrame {
    let (w, h) = rgb.dimensions();
    let k = (w / LIVE_WIDTH).max(1);
    let (nw, nh) = (w / k, h / k);
    let src = rgb.as_raw();
    let mut rgba = Vec::with_capacity((nw * nh * 4) as usize);
    for y in 0..nh {
        for x in 0..nw {
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for dy in 0..k {
                for dx in 0..k {
                    let i = (((y * k + dy) * w + x * k + dx) * 3) as usize;
                    r += src[i] as u32;
                    g += src[i + 1] as u32;
                    b += src[i + 2] as u32;
                }
            }
            let n = k * k;
            rgba.extend_from_slice(&[(r / n) as u8, (g / n) as u8, (b / n) as u8, 255]);
        }
    }
    LiveFrame { seq, width: nw, height: nh, rgba, full: (w, h) }
}

fn photo_from(f: RawFrame) -> Result<CapturedPhoto, String> {
    if f.jpeg {
        // El JPEG original de la cámara, sin volver a comprimir.
        return Ok(CapturedPhoto { bytes: f.data, width: f.width, height: f.height, png: false });
    }
    let img = image::RgbImage::from_raw(f.width, f.height, f.data).ok_or("el cuadro de la cámara no tiene el tamaño esperado")?;
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).map_err(|e| format!("no se pudo codificar la foto: {e}"))?;
    Ok(CapturedPhoto { bytes, width: f.width, height: f.height, png: true })
}

fn run(provider: Arc<dyn CaptureProvider>, id: &str, rx: Receiver<Cmd>, tx: Sender<Event>, live: Arc<Mutex<Option<LiveFrame>>>) {
    let mut dev = match provider.open(id) {
        Ok(d) => d,
        Err(e) => {
            let _ = tx.send(Event::Failed(format!("No se pudo abrir la cámara: {e}")));
            return;
        }
    };
    let controls = dev.controls();
    let mut opened = false;
    let (mut n, mut want) = (0u64, false);
    loop {
        while let Ok(c) = rx.try_recv() {
            match c {
                Cmd::Stop => return,
                Cmd::Capture => want = true,
                Cmd::SetControl(k, v) => {
                    if let Err(e) = dev.set_control(&k, v) {
                        let _ = tx.send(Event::Warning(e));
                    }
                }
            }
        }
        let frame = match dev.frame() {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.send(Event::Failed(format!("La cámara dejó de responder: {e}")));
                return;
            }
        };
        n += 1;
        if !opened {
            opened = true;
            let _ = tx.send(Event::Opened { controls: controls.clone(), size: (frame.width, frame.height) });
        }
        // El Live View solo se prepara si la interfaz ya recogió el anterior: así nunca se acumula retraso.
        let wanted = live.lock().map(|g| g.is_none()).unwrap_or(false);
        if wanted {
            if let Some(rgb) = decode_rgb(&frame) {
                if let Ok(mut g) = live.lock() {
                    *g = Some(live_from(&rgb, n));
                }
            }
        }
        if want {
            want = false;
            let ev = match photo_from(frame) {
                Ok(p) => Event::Photo(p),
                Err(e) => Event::Warning(e),
            };
            let _ = tx.send(ev);
        }
    }
}
