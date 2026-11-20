//! Pinceles de Photoshop (`.abr`): lectura de las versiones 1, 2 y 6.x, y escritura de la versión 2.
//!
//! Qué se lee de verdad:
//! - **v6.x (Photoshop CS y posteriores):** las formas muestreadas (sección `samp`, con compresión RLE o sin ella)
//!   y los pinceles con sus parámetros (sección `desc`, descriptores de Photoshop). Probado con un archivo real.
//! - **v1 y v2 (clásicos):** formas muestreadas y pinceles calculados (diámetro, dureza, espaciado, ángulo, redondez).
//!   Implementado según la especificación publicada; sin archivos reales a mano para probarlo.
//! - Los patrones (`patt`) y la jerarquía de carpetas (`phry`) no se importan, y se avisa.
//!
//! Nada se descarta en silencio: cada propiedad que no se puede representar se anota en `notes` del pincel
//! y se resume en `warnings` del archivo.

pub mod reader;
mod write;

use crate::brush::{BrushParams, Control, Gray};
use crate::presets::BrushSetup;
use reader::{descriptor, unpackbits, Res, R, V};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub use write::export_v2;

/// Lado máximo de una punta importada (igual que el de las imágenes del pincel).
const MAX_TIP_SIDE: u32 = 2048;
/// Máximo de pinceles que se importan de un archivo.
const MAX_PRESETS: usize = 5000;
/// Tope de bytes que se leen de una sección (contra archivos que mienten sobre su largo).
const MAX_SECTION: u64 = 512 * 1024 * 1024;

/// Un pincel leído de un archivo ABR.
#[derive(Clone, Debug)]
pub struct Imported {
    pub name: String,
    pub setup: BrushSetup,
    /// Forma de la punta, si el pincel usa una imagen.
    pub tip: Option<Gray>,
    pub tip_name: String,
    /// Lo que de este pincel no se pudo representar.
    pub notes: Vec<String>,
}

/// Todo lo leído de un archivo ABR.
#[derive(Clone, Debug, Default)]
pub struct AbrFile {
    pub version: (u16, u16),
    pub brushes: Vec<Imported>,
    /// Avisos del archivo entero y resumen de lo no compatible.
    pub warnings: Vec<String>,
    /// Cuántas formas muestreadas trae el archivo (con o sin pincel que las use).
    pub tips_found: usize,
}

// ---------------- Secciones ----------------

/// Partes del archivo que interesan.
#[derive(Default)]
struct Sections {
    version: u16,
    minor: u16,
    samp: Vec<u8>,
    desc: Vec<u8>,
    patt_bytes: u64,
    other: Vec<String>,
}

fn io(e: std::io::Error) -> String {
    format!("No se pudo leer el archivo: {e}")
}

/// Recorre las secciones `8BIM` de un ABR v6+ leyendo solo `samp` y `desc` (los patrones pueden pesar cientos de MB).
fn scan_sections<F: Read + Seek>(f: &mut F, total: u64, version: u16, minor: u16) -> Res<Sections> {
    let mut s = Sections { version, minor, ..Default::default() };
    let mut pos: u64 = 4;
    loop {
        if pos + 12 > total {
            break;
        }
        f.seek(SeekFrom::Start(pos)).map_err(io)?;
        let mut head = [0u8; 12];
        f.read_exact(&mut head).map_err(io)?;
        if &head[0..4] != b"8BIM" {
            // Algunos archivos dejan un byte de relleno entre secciones.
            if pos + 13 <= total && head[1..5] == *b"8BIM" {
                pos += 1;
                continue;
            }
            s.other.push(format!("datos sin reconocer al final del archivo (byte {pos})"));
            break;
        }
        let key = String::from_utf8_lossy(&head[4..8]).into_owned();
        let len = u32::from_be_bytes([head[8], head[9], head[10], head[11]]) as u64;
        let body = pos + 12;
        if body + len > total {
            return Err(format!("La sección «{key}» dice ocupar más de lo que tiene el archivo: está recortado o dañado"));
        }
        match key.as_str() {
            "samp" | "desc" => {
                if len > MAX_SECTION {
                    return Err(format!("La sección «{key}» es demasiado grande ({} MB)", len / 1_000_000));
                }
                let mut buf = vec![0u8; len as usize];
                f.read_exact(&mut buf).map_err(io)?;
                if key == "samp" {
                    s.samp = buf;
                } else {
                    s.desc = buf;
                }
            }
            "patt" => s.patt_bytes = len,
            other => s.other.push(other.to_string()),
        }
        pos = body + len;
    }
    Ok(s)
}

// ---------------- Formas muestreadas ----------------

/// Imagen de una forma: rectángulo, profundidad, compresión y datos. Devuelve la máscara (255 = pintura).
fn read_pixels(r: &mut R) -> Res<Gray> {
    let top = r.i32()? as i64;
    let left = r.i32()? as i64;
    let bottom = r.i32()? as i64;
    let right = r.i32()? as i64;
    let depth = r.u16()?;
    let comp = r.u8()?;
    let (w, h) = (right - left, bottom - top);
    if w <= 0 || h <= 0 || w > 8192 || h > 8192 {
        return Err(format!("Forma con tamaño imposible ({w}×{h})"));
    }
    let (w, h) = (w as usize, h as usize);
    let bpp = match depth {
        8 => 1,
        16 => 2,
        d => return Err(format!("Profundidad de color {d} no admitida en las formas")),
    };
    let row_bytes = w * bpp;
    let raw: Vec<u8> = match comp {
        0 => r.bytes(row_bytes * h)?.to_vec(),
        1 => {
            let mut lens = Vec::with_capacity(h);
            for _ in 0..h {
                lens.push(r.u16()? as usize);
            }
            let mut out = Vec::with_capacity(row_bytes * h);
            for l in lens {
                out.extend(unpackbits(r.bytes(l)?, row_bytes));
            }
            out
        }
        c => return Err(format!("Compresión {c} no reconocida en las formas")),
    };
    // En 16 bits se queda con el byte alto de cada valor.
    let data: Vec<u8> = if bpp == 2 { raw.chunks_exact(2).map(|c| c[0]).collect() } else { raw };
    Ok(Gray { w: w as u32, h: h as u32, data })
}

/// Sección `samp` de un ABR v6.x: formas con identificador (UUID).
fn parse_samp(data: &[u8], minor: u16, warns: &mut Vec<String>) -> HashMap<String, Gray> {
    let mut out = HashMap::new();
    let mut r = R::new(data);
    let header: usize = if minor <= 1 { 47 } else { 301 };
    let mut bad = 0;
    while r.left() >= 4 {
        let Ok(len) = r.u32() else { break };
        let len = len as usize;
        if len == 0 {
            continue;
        }
        let Ok(entry) = r.bytes(len) else {
            warns.push("La última forma del archivo está cortada: se ignora".into());
            break;
        };
        let pad = (4 - len % 4) % 4;
        let _ = r.skip(pad.min(r.left()));
        let mut e = R::new(entry);
        let res: Res<(String, Gray)> = (|| {
            let key = e.pascal()?;
            e.skip(header.saturating_sub(1 + key.len()))?;
            Ok((key, read_pixels(&mut e)?))
        })();
        match res {
            Ok((k, g)) => {
                out.insert(k, g);
            }
            Err(_) => bad += 1,
        }
    }
    if bad > 0 {
        warns.push(format!("{bad} forma(s) del archivo no se pudieron leer y se ignoran"));
    }
    out
}

// ---------------- Pinceles de la sección desc ----------------

fn pct(v: Option<&V>) -> Option<f64> {
    v.and_then(|v| v.num())
}

/// Control de una propiedad dinámica de Photoshop: 0 apagado, 2 presión del lápiz, 3 inclinación.
fn control_of(dynamic: &V, what: &str, notes: &mut Vec<String>) -> Control {
    let t = dynamic.get("bVTy").and_then(|v| v.num()).unwrap_or(0.0) as i32;
    match t {
        0 => Control::Off,
        2 => Control::Pressure,
        3 => Control::Tilt,
        1 => {
            notes.push(format!("{what}: el control «Fundido» no existe en LeonToon"));
            Control::Off
        }
        4 => {
            notes.push(format!("{what}: el control «Rueda del lápiz» no existe en LeonToon"));
            Control::Off
        }
        5 | 6 | 7 => {
            notes.push(format!("{what}: el control por rotación o dirección del lápiz no existe en LeonToon"));
            Control::Off
        }
        _ => Control::Off,
    }
}

/// Convierte un `brushPreset` de la sección `desc` en un pincel de LeonToon.
fn brush_from_preset(p: &V, tips: &HashMap<String, Gray>) -> Imported {
    let mut notes: Vec<String> = Vec::new();
    let name = p.get("Nm  ").and_then(|v| v.text()).unwrap_or("Pincel").to_string();
    let mut params = BrushParams::default();
    let mut setup = BrushSetup::default();
    let mut tip: Option<Gray> = None;
    let mut tip_name = String::new();

    if let Some(b) = p.get("Brsh") {
        let class = b.class().unwrap_or("");
        if let Some(d) = pct(b.get("Dmtr")) {
            setup.size = d.clamp(1.0, 1000.0) as f32;
            if d > 1000.0 {
                notes.push(format!("Diámetro de {d:.0} px: se limita a 1000 px"));
            }
        }
        if let Some(a) = pct(b.get("Angl")) {
            params.angle = a as f32;
        }
        if let Some(r) = pct(b.get("Rndn")) {
            params.roundness = (r / 100.0) as f32;
        }
        if let Some(s) = pct(b.get("Spcn")) {
            params.spacing = s as f32;
        }
        if class == "computedBrush" {
            if let Some(h) = pct(b.get("Hrdn")) {
                params.hardness = (h / 100.0) as f32;
            }
        } else if class == "sampledBrush" {
            tip_name = b.get("Nm  ").and_then(|v| v.text()).unwrap_or("").to_string();
            match b.get("sampledData").and_then(|v| v.text()) {
                Some(id) => match tips.get(id) {
                    Some(g) if g.w > MAX_TIP_SIDE || g.h > MAX_TIP_SIDE => {
                        notes.push(format!("La forma mide {}×{} px y supera los {MAX_TIP_SIDE} px: se usa una punta redonda", g.w, g.h));
                    }
                    Some(g) => {
                        let mut g = g.clone();
                        flip(&mut g, b.get("flipX").and_then(|v| v.bool()).unwrap_or(false), b.get("flipY").and_then(|v| v.bool()).unwrap_or(false));
                        tip = Some(g);
                        params.use_tip_image = true;
                    }
                    None => notes.push("La forma de la punta no está en el archivo: se usa una punta redonda".into()),
                },
                None => notes.push("El pincel no indica qué forma usa: se usa una punta redonda".into()),
            }
        } else if !class.is_empty() {
            notes.push(format!("Tipo de punta «{class}» desconocido: se usa una punta redonda"));
        }
    }

    // ---- Dinámica de forma ----
    if p.get("useTipDynamics").and_then(|v| v.bool()).unwrap_or(false) {
        if let Some(sz) = p.get("szVr") {
            params.size_control = control_of(sz, "Tamaño", &mut notes);
            params.size_jitter = (pct(sz.get("jitter")).unwrap_or(0.0) / 100.0) as f32;
            if let Some(m) = pct(p.get("minimumDiameter")).or_else(|| pct(sz.get("Mnm "))) {
                params.min_size = (m / 100.0) as f32;
            }
        }
        if let Some(a) = p.get("angleDynamics") {
            params.angle_jitter = (pct(a.get("jitter")).unwrap_or(0.0) / 100.0) as f32;
            if control_of(a, "Ángulo", &mut notes) != Control::Off {
                notes.push("Ángulo: el control por lápiz no existe, solo la variación al azar".into());
            }
        }
        if let Some(rd) = p.get("roundnessDynamics") {
            params.roundness_jitter = (pct(rd.get("jitter")).unwrap_or(0.0) / 100.0) as f32;
            if let Some(m) = pct(p.get("minimumRoundness")) {
                params.min_roundness = (m / 100.0) as f32;
            }
            if control_of(rd, "Redondez", &mut notes) != Control::Off {
                notes.push("Redondez: el control por lápiz no existe, solo la variación al azar".into());
            }
        }
    }

    // ---- Dispersión ----
    if p.get("useScatter").and_then(|v| v.bool()).unwrap_or(false) {
        if let Some(sd) = p.get("scatterDynamics") {
            params.scatter = (pct(sd.get("jitter")).unwrap_or(0.0) / 100.0) as f32;
            if control_of(sd, "Dispersión", &mut notes) != Control::Off {
                notes.push("Dispersión: el control por lápiz no existe".into());
            }
        }
        params.scatter_both = p.get("bothAxes").and_then(|v| v.bool()).unwrap_or(false);
        if let Some(c) = pct(p.get("Cnt ")) {
            params.scatter_count = c.clamp(1.0, 16.0) as u8;
            if c > 16.0 {
                notes.push(format!("Cantidad de {c:.0} marcas: se limita a 16"));
            }
        }
        if let Some(cd) = p.get("countDynamics") {
            params.count_jitter = (pct(cd.get("jitter")).unwrap_or(0.0) / 100.0) as f32;
        }
    }

    // ---- Opciones de la herramienta: flujo, opacidad y suavizado con que se guardó el pincel ----
    if let Some(t) = p.get("toolOptions") {
        if let Some(f) = pct(t.get("flow")) {
            params.flow = (f / 100.0).clamp(0.01, 1.0) as f32;
        }
        if let Some(o) = pct(t.get("Opct")) {
            setup.opacity = (o / 100.0).clamp(0.05, 1.0) as f32;
        }
        if let Some(s) = t.get("smoothing").and_then(|v| v.bool()) {
            setup.smoothing = if s { 0.35 } else { 0.0 };
        }
    }

    // ---- Transferencia ----
    if p.get("usePaintDynamics").and_then(|v| v.bool()).unwrap_or(false) {
        if let Some(o) = p.get("opVr") {
            params.opacity_control = control_of(o, "Opacidad", &mut notes);
            params.opacity_jitter = (pct(o.get("jitter")).unwrap_or(0.0) / 100.0) as f32;
            params.opacity_min = (pct(o.get("Mnm ")).unwrap_or(0.0) / 100.0) as f32;
        }
        if let Some(f) = p.get("prVr") {
            params.flow_control = control_of(f, "Flujo", &mut notes);
            params.flow_jitter = (pct(f.get("jitter")).unwrap_or(0.0) / 100.0) as f32;
            params.flow_min = (pct(f.get("Mnm ")).unwrap_or(0.0) / 100.0) as f32;
        }
    }

    // ---- Color (tono, saturación y brillo van en la raíz del pincel; primer plano y fondo, en clVr) ----
    if p.get("useColorDynamics").and_then(|v| v.bool()).unwrap_or(false) {
        params.hue_jitter = (pct(p.get("H   ")).unwrap_or(0.0) / 100.0) as f32;
        params.sat_jitter = (pct(p.get("Strt")).unwrap_or(0.0) / 100.0) as f32;
        params.bri_jitter = (pct(p.get("Brgh")).unwrap_or(0.0) / 100.0) as f32;
        if let Some(c) = p.get("clVr") {
            params.fg_bg_jitter = (pct(c.get("jitter")).unwrap_or(0.0) / 100.0) as f32;
            if control_of(c, "Color", &mut notes) != Control::Off {
                notes.push("Color: el control por lápiz no existe, solo la variación al azar".into());
            }
        }
        if pct(p.get("purity")).map_or(false, |v| v != 0.0) {
            notes.push("Pureza del color: no existe en LeonToon".into());
        }
    }

    // ---- Bordes húmedos ----
    if p.get("Wtdg").and_then(|v| v.bool()).unwrap_or(false) {
        params.wet_edges = 1.0;
        notes.push("Bordes húmedos: Photoshop no guarda la cantidad; se usa el máximo".into());
    }

    // ---- Lo que no existe en LeonToon ----
    if p.get("useTexture").and_then(|v| v.bool()).unwrap_or(false) {
        let pat = p.get("Txtr").and_then(|t| t.get("Nm  ")).and_then(|v| v.text()).unwrap_or("sin nombre");
        notes.push(format!("Textura de patrón «{pat}»: los patrones de Photoshop no se importan"));
    }
    if p.get("brushProjection").and_then(|v| v.bool()).unwrap_or(false) {
        notes.push("Proyección del pincel: no existe en LeonToon".into());
    }
    if p.get("useBrushPose").and_then(|v| v.bool()).unwrap_or(false) {
        notes.push("Postura del pincel: no existe en LeonToon".into());
    }
    if p.get("useDualBrush").and_then(|v| v.bool()).unwrap_or(false) || p.get("dualBrush").and_then(|d| d.get("useDualBrush")).and_then(|v| v.bool()).unwrap_or(false) {
        notes.push("Doble pincel: no se importa (la segunda punta de Photoshop es una forma muestreada)".into());
    }
    if p.get("Nose").and_then(|v| v.bool()).unwrap_or(false) {
        notes.push("Ruido: no existe en LeonToon".into());
    }
    if p.get("Rpt ").and_then(|v| v.bool()).unwrap_or(false) {
        notes.push("Aerógrafo (acumulación): no existe en LeonToon".into());
    }

    setup.params = params;
    // Los valores quedan dentro de los rangos de LeonToon (por ejemplo, redondez mínima de 5 %).
    Imported { name, setup: setup.clamped(), tip, tip_name, notes }
}

/// Voltea una forma en horizontal y/o vertical.
fn flip(g: &mut Gray, fx: bool, fy: bool) {
    let (w, h) = (g.w as usize, g.h as usize);
    if fx {
        for y in 0..h {
            g.data[y * w..(y + 1) * w].reverse();
        }
    }
    if fy {
        for y in 0..h / 2 {
            for x in 0..w {
                g.data.swap(y * w + x, (h - 1 - y) * w + x);
            }
        }
    }
}

// ---------------- Versiones 1 y 2 ----------------

/// Formas y pinceles calculados de un ABR v1 o v2.
fn parse_classic(data: &[u8], version: u16, warns: &mut Vec<String>) -> Res<(Vec<Imported>, usize)> {
    let mut r = R::new(data);
    r.skip(2)?; // versión
    let count = r.u16()? as usize;
    let mut out = Vec::new();
    let mut tips = 0;
    for i in 0..count {
        let ty = match r.u16() {
            Ok(t) => t,
            Err(_) => {
                warns.push(format!("El archivo termina en el pincel {} de {count}", i + 1));
                break;
            }
        };
        let size = r.u32()? as usize;
        let body = match r.bytes(size) {
            Ok(b) => b,
            Err(_) => {
                warns.push(format!("El pincel {} está cortado: se ignora", i + 1));
                break;
            }
        };
        let mut b = R::new(body);
        let res: Res<Imported> = (|| match ty {
            1 => {
                // Calculado: misc, espaciado, diámetro, redondez, dureza, ángulo.
                b.skip(4)?;
                let spacing = b.u16()? as f32;
                let diameter = b.u16()? as f32;
                let roundness = b.u16()? as f32;
                let hardness = b.u16()? as f32;
                let angle = b.u16()? as f32;
                let params = BrushParams { spacing, roundness: roundness / 100.0, hardness: hardness / 100.0, angle, ..Default::default() };
                let setup = BrushSetup { size: diameter.clamp(1.0, 1000.0), params, ..Default::default() };
                Ok(Imported { name: format!("Pincel redondo {}", i + 1), setup, tip: None, tip_name: String::new(), notes: vec![] })
            }
            2 => {
                b.skip(4)?; // misc
                let spacing = b.u16()? as f32;
                let name = if version >= 2 { b.unicode()? } else { format!("Muestra {}", i + 1) };
                b.skip(1)?; // suavizado
                b.skip(8)?; // límites (4 × u16)
                let g = read_pixels(&mut b)?;
                let setup = BrushSetup { size: (g.w.max(g.h) as f32).clamp(1.0, 1000.0), params: BrushParams { spacing, use_tip_image: true, ..Default::default() }, ..Default::default() };
                Ok(Imported { name: if name.is_empty() { format!("Muestra {}", i + 1) } else { name }, tip_name: String::new(), setup, tip: Some(g), notes: vec![] })
            }
            t => Err(format!("tipo de pincel {t} desconocido")),
        })();
        match res {
            Ok(im) => {
                if im.tip.is_some() {
                    tips += 1;
                }
                out.push(im);
            }
            Err(e) => warns.push(format!("Pincel {} no se pudo leer ({e}): se ignora", i + 1)),
        }
    }
    Ok((out, tips))
}

// ---------------- Entrada principal ----------------

/// Lee un archivo ABR. Si no lo es, o está dañado, devuelve un error que dice por qué.
pub fn read_abr(path: &Path) -> Res<AbrFile> {
    let mut f = std::fs::File::open(path).map_err(io)?;
    let total = f.metadata().map_err(io)?.len();
    if total < 8 {
        return Err("El archivo está vacío o es demasiado pequeño para ser un ABR".into());
    }
    let mut head = [0u8; 4];
    f.read_exact(&mut head).map_err(io)?;
    let version = u16::from_be_bytes([head[0], head[1]]);
    let minor = u16::from_be_bytes([head[2], head[3]]);
    match version {
        1 | 2 => {
            if total > 256 * 1024 * 1024 {
                return Err("El archivo es demasiado grande para un ABR clásico".into());
            }
            let mut all = Vec::with_capacity(total as usize);
            f.seek(SeekFrom::Start(0)).map_err(io)?;
            f.read_to_end(&mut all).map_err(io)?;
            read_abr_bytes(&all)
        }
        6..=12 => {
            let s = scan_sections(&mut f, total, version, minor)?;
            Ok(build_v6(s))
        }
        _ => Err(format!(
            "Este archivo no es un ABR de Photoshop: empieza con la versión {version}.{minor}, y los ABR son versión 1, 2 o 6 en adelante. La extensión .abr la usan también otros programas para sus propios datos."
        )),
    }
}

/// Lee un ABR completo desde la memoria (versiones 1, 2 y 6+).
pub fn read_abr_bytes(data: &[u8]) -> Res<AbrFile> {
    if data.len() < 8 {
        return Err("El archivo está vacío o es demasiado pequeño para ser un ABR".into());
    }
    let (version, minor) = (u16::from_be_bytes([data[0], data[1]]), u16::from_be_bytes([data[2], data[3]]));
    match version {
        1 | 2 => {
            let mut warnings = Vec::new();
            let (brushes, tips) = parse_classic(data, version, &mut warnings)?;
            let mut file = AbrFile { version: (version, 0), brushes, warnings, tips_found: tips };
            if file.brushes.is_empty() {
                file.warnings.push("El archivo no trae ningún pincel".into());
            }
            Ok(file)
        }
        6..=12 => {
            let mut cur = std::io::Cursor::new(data);
            let s = scan_sections(&mut cur, data.len() as u64, version, minor)?;
            Ok(build_v6(s))
        }
        _ => Err(format!("Este archivo no es un ABR de Photoshop: empieza con la versión {version}.{minor}")),
    }
}

fn build_v6(s: Sections) -> AbrFile {
    let mut warnings: Vec<String> = Vec::new();
    if s.version > 6 {
        warnings.push(format!("Versión {}.{}: es más nueva de las que se han probado; puede que algunos datos no se lean", s.version, s.minor));
    }
    if s.samp.is_empty() && s.desc.is_empty() {
        warnings.push("El archivo no trae formas ni pinceles".into());
    }
    let tips = parse_samp(&s.samp, s.minor, &mut warnings);
    let tips_found = tips.len();
    let mut brushes: Vec<Imported> = Vec::new();
    if s.desc.len() > 4 {
        let mut r = R::new(&s.desc);
        let parsed: Res<V> = (|| {
            r.skip(4)?; // versión del descriptor
            descriptor(&mut r, 0)
        })();
        match parsed {
            Ok(root) => {
                if let Some(list) = root.get("Brsh").and_then(|v| v.list()) {
                    for item in list.iter().take(MAX_PRESETS) {
                        if item.class() == Some("brushPreset") {
                            brushes.push(brush_from_preset(item, &tips));
                        }
                    }
                    if list.len() > MAX_PRESETS {
                        warnings.push(format!("El archivo trae {} pinceles: solo se leen los primeros {MAX_PRESETS}", list.len()));
                    }
                }
            }
            Err(e) => warnings.push(format!("No se pudieron leer los parámetros de los pinceles ({e}): se importan solo las formas")),
        }
    }
    // Sin descriptores (o ilegibles), cada forma se vuelve un pincel con los valores por defecto.
    if brushes.is_empty() && !tips.is_empty() {
        let mut keys: Vec<&String> = tips.keys().collect();
        keys.sort();
        for (i, k) in keys.into_iter().enumerate() {
            let g = tips[k].clone();
            let setup = BrushSetup { size: (g.w.max(g.h) as f32).clamp(1.0, 1000.0), params: BrushParams { use_tip_image: true, ..Default::default() }, ..Default::default() };
            brushes.push(Imported { name: format!("Forma {}", i + 1), setup, tip: Some(g), tip_name: k.to_string(), notes: vec!["Sin parámetros en el archivo: solo se importó la forma".into()] });
        }
        warnings.push("El archivo no trae parámetros de pinceles: cada forma se importa con valores por defecto".into());
    }
    if s.patt_bytes > 0 {
        let size = if s.patt_bytes < 1_000_000 { "menos de 1 MB".to_string() } else { format!("{} MB", (s.patt_bytes + 500_000) / 1_000_000) };
        warnings.push(format!("El archivo trae patrones de Photoshop ({size}): no se importan, y los pinceles con textura de patrón pierden la textura"));
    }
    if s.other.iter().any(|o| o == "phry") {
        warnings.push("La organización en carpetas del archivo no se importa".into());
    }
    AbrFile { version: (s.version, s.minor), brushes, warnings, tips_found }
}

#[cfg(test)]
mod tests;
