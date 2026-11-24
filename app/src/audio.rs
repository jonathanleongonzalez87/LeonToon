//! Audio (Issue #136 y #15, fases A2 y M8a de docs/decodificadores-video-audio.md): leer archivos de audio, calcular su onda, mezclar las
//! capas de audio de una composición y reproducirlas por el dispositivo de salida.
//!
//! Se decodifica con `symphonia` (WAV, MP3, FLAC, OGG/Vorbis y AAC/M4A, en Rust puro) y se reproduce con `cpal` (WASAPI en Windows).
//! El reloj de la reproducción es el del audio: la imagen se ajusta al sonido. No depende de la interfaz.

use crate::model::{AudioParams, Layer, LayerKind, Project};
use crate::tracks::{Prop, Tracks};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Un archivo de audio ya decodificado: muestras en coma flotante, entrelazadas por canales.
pub struct Clip {
    pub rate: u32,
    pub channels: u16,
    pub data: Vec<f32>,
}

impl Clip {
    pub fn frames(&self) -> usize {
        self.data.len() / self.channels.max(1) as usize
    }

    pub fn secs(&self) -> f32 {
        self.frames() as f32 / self.rate.max(1) as f32
    }

    /// El par izquierda y derecha del cuadro de muestras `i` (un canal suena por los dos lados).
    fn stereo(&self, i: usize) -> (f32, f32) {
        let ch = self.channels.max(1) as usize;
        let at = |k: usize| self.data.get(i * ch + k).copied().unwrap_or(0.0);
        if ch == 1 {
            (at(0), at(0))
        } else {
            (at(0), at(1))
        }
    }
}

/// Lee un archivo de audio entero.
pub fn decode(path: &Path) -> Result<Clip, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path).map_err(|e| format!("No se pudo abrir el audio: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|_| "Ese archivo no es un audio que LeonToon sepa leer (WAV, MP3, FLAC, OGG o AAC/M4A)".to_string())?;
    let mut format = probed.format;
    let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).ok_or("El archivo no trae pista de audio")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|_| "No se pudo preparar el decodificador de ese audio".to_string())?;
    let (mut rate, mut channels) = (track.codec_params.sample_rate.unwrap_or(0), track.codec_params.channels.map_or(0, |c| c.count() as u16));
    let mut data: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("Error al leer el audio: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let spec = *buf.spec();
                rate = spec.rate;
                channels = spec.channels.count() as u16;
                let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
                sb.copy_interleaved_ref(buf);
                data.extend_from_slice(sb.samples());
            }
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("Error al decodificar el audio: {e}")),
        }
    }
    if rate == 0 || channels == 0 || data.is_empty() {
        return Err("El audio está vacío".into());
    }
    Ok(Clip { rate, channels, data })
}

/// Cuántos picos por segundo se guardan para dibujar la onda.
const PEAKS_PER_SEC: f32 = 100.0;

/// Un clip con su onda (pico máximo de cada centésima de segundo, de 0 a 1).
#[derive(Clone)]
pub struct Entry {
    pub clip: Arc<Clip>,
    pub peaks: Vec<f32>,
}

impl Entry {
    fn new(clip: Clip) -> Entry {
        let per = (clip.rate as f32 / PEAKS_PER_SEC).max(1.0) as usize;
        let ch = clip.channels.max(1) as usize;
        let peaks = clip.data.chunks(per * ch).map(|c| c.iter().fold(0.0f32, |m, v| m.max(v.abs())).min(1.0)).collect();
        Entry { clip: Arc::new(clip), peaks }
    }

    /// El pico más alto entre los segundos `t0` y `t1` del clip.
    pub fn peak(&self, t0: f32, t1: f32) -> f32 {
        let a = ((t0.max(0.0)) * PEAKS_PER_SEC) as usize;
        let b = (((t1.max(t0)) * PEAKS_PER_SEC).ceil() as usize + 1).min(self.peaks.len());
        if a >= b {
            return 0.0;
        }
        self.peaks[a..b].iter().fold(0.0f32, |m, v| m.max(*v))
    }
}

/// Los audios ya leídos, por ruta.
#[derive(Default, Clone)]
pub struct Cache {
    map: HashMap<PathBuf, Entry>,
}

impl Cache {
    pub fn get(&self, p: &Path) -> Option<&Entry> {
        self.map.get(p)
    }

    /// El audio de esa ruta: del caché o leyéndolo.
    pub fn load(&mut self, p: &Path) -> Result<&Entry, String> {
        if !self.map.contains_key(p) {
            let e = Entry::new(decode(p)?);
            self.map.insert(p.to_path_buf(), e);
        }
        Ok(&self.map[p])
    }
}

/// Un audio colocado en el tiempo, con todo lo que lo modifica: ganancia, paneo, ecualizador, efectos y silencio (cada uno fijo o
/// animado con claves).
#[derive(Clone, Debug)]
pub struct Item {
    pub path: PathBuf,
    /// Segundo en que empieza a sonar y cuánto suena.
    pub start: f32,
    pub len: f32,
    /// Segundos del principio del archivo que se recortan.
    pub offset: f32,
    pub params: AudioParams,
    /// Las claves de la capa (en cuadros de su composición).
    pub tracks: Tracks,
    /// Cuadros que hay que restar al cuadro de la composición principal para llegar a los de la composición de la capa.
    pub base_frames: u32,
    pub fps: f32,
    pub solo: bool,
}

impl Item {
    /// El valor de una propiedad en el segundo `t` de la composición principal (con las claves interpoladas entre cuadros).
    pub fn value(&self, p: Prop, t: f32) -> f32 {
        if !self.tracks.has(p) {
            return self.params.get(p);
        }
        let f = (t * self.fps - self.base_frames as f32).max(0.0);
        let a = f.floor() as u32;
        let (va, _) = self.tracks.value_at(p, a);
        if p == Prop::Mute {
            return va[0];
        }
        let (vb, _) = self.tracks.value_at(p, a + 1);
        va[0] + (vb[0] - va[0]) * (f - a as f32)
    }

    /// ¿Está en silencio (Mute con claves) en el segundo `t`?
    pub fn muted(&self, t: f32) -> bool {
        self.value(Prop::Mute, t) > 0.5
    }

    /// La ganancia en veces en el segundo `t` (la de los dB): lo que escala la onda dibujada.
    pub fn gain_lin(&self, t: f32) -> f32 {
        10f32.powf(self.value(Prop::Gain, t) / 20.0)
    }

    /// Lo que suena a esta altura, en veces, con el silencio.
    pub fn level(&self, t: f32) -> f32 {
        if self.muted(t) {
            0.0
        } else {
            self.gain_lin(t)
        }
    }

    fn uses_fx(&self) -> bool {
        self.params.reverb > 0.001 || self.params.echo > 0.001 || self.tracks.has(Prop::Reverb) || self.tracks.has(Prop::Echo)
    }
}

/// Los audios de las capas de audio de la composición principal y de las precomposiciones que contiene, con su tiempo en segundos
/// desde el principio. Una capa apagada (M, el altavoz) no suena; si alguna capa está en solo, solo suenan las que lo están.
pub fn collect(p: &Project) -> Vec<Item> {
    let mut out = Vec::new();
    walk(p, &p.layers, 0, p.length, 0, &mut out);
    if out.iter().any(|i| i.solo) {
        out.retain(|i| i.solo);
    }
    out
}

fn walk(p: &Project, layers: &[Layer], base: u32, comp_len: u32, depth: u8, out: &mut Vec<Item>) {
    if depth > 4 {
        return;
    }
    let fps = p.fps.max(1) as f32;
    for l in layers {
        if !l.visible {
            continue;
        }
        match l.kind {
            LayerKind::Audio => {
                let Some(a) = l.audio.as_ref() else { continue };
                let Some(path) = a.resolve(p.base_dir.as_deref()) else { continue };
                let (first, last) = (l.in_f, l.out_frame(comp_len));
                if last < first {
                    continue;
                }
                out.push(Item {
                    path,
                    start: (base + first) as f32 / fps,
                    len: (last - first + 1) as f32 / fps,
                    offset: a.offset.max(0.0),
                    params: a.params,
                    tracks: l.tracks.clone(),
                    base_frames: base,
                    fps,
                    solo: l.solo,
                });
            }
            LayerKind::Precomp => {
                if let Some(c) = l.precomp.and_then(|id| p.comp(id)) {
                    walk(p, &c.layers, base + l.in_f, c.length, depth + 1, out);
                }
            }
            _ => {}
        }
    }
}

// ---------- Procesado ----------

/// Un filtro de segundo orden (biquad) con su memoria, para el ecualizador.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
    on: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Band {
    LowShelf,
    Peak,
    HighShelf,
}

impl Biquad {
    /// Coeficientes del «Audio EQ Cookbook»; con ganancia casi nula el filtro se salta.
    fn set(&mut self, band: Band, fs: f32, f0: f32, gain_db: f32) {
        if gain_db.abs() < 0.05 {
            self.on = false;
            return;
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = std::f32::consts::TAU * f0 / fs;
        let (sin, cos) = w0.sin_cos();
        let (b0, b1, b2, a0, a1, a2);
        match band {
            Band::Peak => {
                let alpha = sin / (2.0 * 0.8);
                b0 = 1.0 + alpha * a;
                b1 = -2.0 * cos;
                b2 = 1.0 - alpha * a;
                a0 = 1.0 + alpha / a;
                a1 = -2.0 * cos;
                a2 = 1.0 - alpha / a;
            }
            Band::LowShelf | Band::HighShelf => {
                let alpha = sin / 2.0 * ((a + 1.0 / a) * (1.0 / 1.0 - 1.0) + 2.0).sqrt();
                let k = 2.0 * a.sqrt() * alpha;
                if band == Band::LowShelf {
                    b0 = a * ((a + 1.0) - (a - 1.0) * cos + k);
                    b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos);
                    b2 = a * ((a + 1.0) - (a - 1.0) * cos - k);
                    a0 = (a + 1.0) + (a - 1.0) * cos + k;
                    a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos);
                    a2 = (a + 1.0) + (a - 1.0) * cos - k;
                } else {
                    b0 = a * ((a + 1.0) + (a - 1.0) * cos + k);
                    b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos);
                    b2 = a * ((a + 1.0) + (a - 1.0) * cos - k);
                    a0 = (a + 1.0) - (a - 1.0) * cos + k;
                    a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos);
                    a2 = (a + 1.0) - (a - 1.0) * cos - k;
                }
            }
        }
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
        self.on = true;
    }

    fn run(&mut self, x: f32) -> f32 {
        if !self.on {
            return x;
        }
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Una línea de retardo circular.
struct Delay {
    buf: Vec<f32>,
    pos: usize,
}

impl Delay {
    fn new(n: usize) -> Delay {
        Delay { buf: vec![0.0; n.max(1)], pos: 0 }
    }
}

/// Eco: una copia retardada que se realimenta.
struct Echo {
    line: [Delay; 2],
}

impl Echo {
    fn new(rate: u32) -> Echo {
        let n = (0.31 * rate as f32) as usize;
        Echo { line: [Delay::new(n), Delay::new(n + (0.013 * rate as f32) as usize)] }
    }

    fn run(&mut self, ch: usize, x: f32, mix: f32) -> f32 {
        let d = &mut self.line[ch];
        let wet = d.buf[d.pos];
        d.buf[d.pos] = x + 0.4 * wet;
        d.pos = (d.pos + 1) % d.buf.len();
        x + mix * wet
    }
}

/// Reverberación de Schroeder: cuatro filtros de peine en paralelo y dos pasatodo en serie, por canal.
struct Reverb {
    comb: [[Delay; 4]; 2],
    damp: [[f32; 4]; 2],
    all: [[Delay; 2]; 2],
}

impl Reverb {
    fn new(rate: u32) -> Reverb {
        let k = rate as f32 / 44_100.0;
        let mk = |n: usize, spread: usize| Delay::new(((n + spread) as f32 * k) as usize);
        let combs = |s: usize| [mk(1116, s), mk(1188, s), mk(1277, s), mk(1356, s)];
        let alls = |s: usize| [mk(556, s), mk(441, s)];
        Reverb { comb: [combs(0), combs(23)], damp: [[0.0; 4]; 2], all: [alls(0), alls(11)] }
    }

    fn run(&mut self, ch: usize, x: f32, mix: f32) -> f32 {
        let mut sum = 0.0;
        for i in 0..4 {
            let d = &mut self.comb[ch][i];
            let out = d.buf[d.pos];
            self.damp[ch][i] = out * 0.8 + self.damp[ch][i] * 0.2;
            d.buf[d.pos] = x * 0.015 + self.damp[ch][i] * 0.84;
            d.pos = (d.pos + 1) % d.buf.len();
            sum += out;
        }
        for i in 0..2 {
            let d = &mut self.all[ch][i];
            let buffered = d.buf[d.pos];
            let out = -sum + buffered;
            d.buf[d.pos] = sum + buffered * 0.5;
            d.pos = (d.pos + 1) % d.buf.len();
            sum = out;
        }
        x + mix * sum * 3.0
    }
}

/// Procesa el audio ya colocado de un elemento: ecualizador, efectos y luego ganancia y paneo, por bloques de 10 ms en los que los
/// valores se evalúan en el centro (las ganancias se suavizan entre bloques para que no haya chasquidos). `t0` es el segundo en que
/// empieza `buf` (estéreo entrelazado).
fn process(buf: &mut [f32], it: &Item, rate: u32, t0: f32) {
    let fs = rate as f32;
    let block = (rate / 100).max(64) as usize;
    let mut eq = [[Biquad::default(); 2]; 3];
    let mut echo = it.uses_fx().then(|| Echo::new(rate));
    let mut reverb = it.uses_fx().then(|| Reverb::new(rate));
    let (mut gl_prev, mut gr_prev) = (None::<f32>, None::<f32>);
    for (bi, chunk) in buf.chunks_mut(block * 2).enumerate() {
        let t = t0 + (bi * block + block / 2) as f32 / fs;
        // Ganancia y paneo (potencia constante: en el centro no cambia nada).
        let level = it.level(t);
        let angle = (it.value(Prop::Pan, t).clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
        let (gl, gr) = (level * angle.cos() * std::f32::consts::SQRT_2, level * angle.sin() * std::f32::consts::SQRT_2);
        let (gl0, gr0) = (gl_prev.unwrap_or(gl), gr_prev.unwrap_or(gr));
        // Ecualizador: graves (repisa a 200 Hz), medios (campana a 1 kHz) y agudos (repisa a 4 kHz).
        let (lo, mid, hi) = (it.value(Prop::EqLo, t), it.value(Prop::EqMid, t), it.value(Prop::EqHi, t));
        for ch in 0..2 {
            eq[0][ch].set(Band::LowShelf, fs, 200.0, lo);
            eq[1][ch].set(Band::Peak, fs, 1000.0, mid);
            eq[2][ch].set(Band::HighShelf, fs, 4000.0, hi);
        }
        let (mix_r, mix_e) = (it.value(Prop::Reverb, t).clamp(0.0, 1.0), it.value(Prop::Echo, t).clamp(0.0, 1.0));
        let n = chunk.len() / 2;
        for k in 0..n {
            let a = k as f32 / n as f32;
            let gains = [gl0 + (gl - gl0) * a, gr0 + (gr - gr0) * a];
            for ch in 0..2 {
                let mut x = chunk[k * 2 + ch];
                for band in eq.iter_mut() {
                    x = band[ch].run(x);
                }
                if let Some(e) = echo.as_mut() {
                    x = e.run(ch, x, mix_e);
                }
                if let Some(r) = reverb.as_mut() {
                    x = r.run(ch, x, mix_r);
                }
                chunk[k * 2 + ch] = x * gains[ch];
            }
        }
        gl_prev = Some(gl);
        gr_prev = Some(gr);
    }
}

/// Cuánto se alarga el sonido de un elemento con eco o reverberación más allá de su barra (la cola).
const FX_TAIL_SECS: f32 = 2.5;

/// Mezcla los audios en estéreo (muestras entrelazadas izquierda y derecha) a la frecuencia `rate`, desde el segundo `from` y
/// durante `total` segundos, con la ganancia, el paneo, el ecualizador y los efectos de cada uno. Lo que no se ha leído se omite.
pub fn mix(items: &[Item], cache: &Cache, rate: u32, from: f32, total: f32) -> Vec<f32> {
    let rate = rate.max(1);
    let frames = (total.max(0.0) * rate as f32).ceil() as usize;
    let mut out = vec![0.0f32; frames * 2];
    for it in items {
        let Some(e) = cache.get(&it.path) else { continue };
        let clip = &e.clip;
        let n0 = ((it.start - from) * rate as f32).ceil().max(0.0) as usize;
        let n1 = (((it.start + it.len - from) * rate as f32).floor().max(0.0) as usize).min(frames);
        if n0 >= n1 {
            continue;
        }
        let tail = if it.uses_fx() { (FX_TAIL_SECS * rate as f32) as usize } else { 0 };
        let end = (n1 + tail).min(frames);
        let mut buf = vec![0.0f32; (end - n0) * 2];
        let frames_clip = clip.frames();
        for n in n0..n1 {
            let local = (n as f32 / rate as f32 + from - it.start + it.offset) as f64;
            let sp = local * clip.rate as f64;
            let i = sp as usize;
            if i >= frames_clip {
                break;
            }
            let frac = (sp - i as f64) as f32;
            let (l0, r0) = clip.stereo(i);
            let (l1, r1) = if i + 1 < frames_clip { clip.stereo(i + 1) } else { (l0, r0) };
            buf[(n - n0) * 2] = l0 + (l1 - l0) * frac;
            buf[(n - n0) * 2 + 1] = r0 + (r1 - r0) * frac;
        }
        process(&mut buf, it, rate, from + n0 as f32 / rate as f32);
        for (k, v) in buf.iter().enumerate() {
            out[n0 * 2 + k] += *v;
        }
    }
    // La mezcla no se recorta aquí: así el medidor ve la sobremodulación real (por encima de 0 dBFS). El recorte lo hacen el
    // dispositivo de salida (`build_stream`) y `write_wav`, que es donde de verdad se pierde la señal.
    out
}
/// Guarda una mezcla estéreo como WAV de 16 bits.
pub fn write_wav(path: &Path, rate: u32, stereo: &[f32]) -> Result<(), String> {
    let data_len = (stereo.len() * 2) as u32;
    let mut out: Vec<u8> = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&2u16.to_le_bytes()); // canales
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for v in stereo {
        out.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    std::fs::write(path, out).map_err(|e| e.to_string())
}

// ---------- Reproducción ----------

struct Shared {
    mix: Arc<Vec<f32>>,
    /// Posición en cuadros de muestras estéreo.
    pos: usize,
    playing: bool,
}

/// El dispositivo de salida de audio con su corriente abierta. Suena lo que se le dé con `play_from`.
pub struct Player {
    _stream: cpal::Stream,
    shared: Arc<Mutex<Shared>>,
    pub rate: u32,
}

fn build_stream<T>(device: &cpal::Device, cfg: &cpal::StreamConfig, shared: Arc<Mutex<Shared>>) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    use cpal::traits::DeviceTrait;
    let ch = cfg.channels.max(1) as usize;
    device
        .build_output_stream(
            cfg,
            move |data: &mut [T], _| {
                let Ok(mut s) = shared.try_lock() else {
                    data.iter_mut().for_each(|d| *d = T::from_sample(0.0));
                    return;
                };
                for frame in data.chunks_mut(ch) {
                    let (l, r) = if s.playing && s.pos * 2 + 1 < s.mix.len() {
                        let i = s.pos * 2;
                        let v = (s.mix[i], s.mix[i + 1]);
                        s.pos += 1;
                        v
                    } else {
                        s.playing = false;
                        (0.0, 0.0)
                    };
                    for (k, d) in frame.iter_mut().enumerate() {
                        let v = if ch == 1 { (l + r) * 0.5 } else if k == 0 { l } else if k == 1 { r } else { 0.0 };
                        *d = T::from_sample(v.clamp(-1.0, 1.0));
                    }
                }
            },
            |e| eprintln!("Error de audio: {e}"),
            None,
        )
        .map_err(|e| format!("No se pudo abrir la salida de audio: {e}"))
}

impl Player {
    pub fn open() -> Result<Player, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let device = cpal::default_host().default_output_device().ok_or("No hay dispositivo de salida de audio")?;
        let supported = device.default_output_config().map_err(|e| format!("No se pudo leer la salida de audio: {e}"))?;
        let rate = supported.sample_rate().0;
        let cfg: cpal::StreamConfig = supported.config();
        let shared = Arc::new(Mutex::new(Shared { mix: Arc::new(Vec::new()), pos: 0, playing: false }));
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &cfg, shared.clone())?,
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &cfg, shared.clone())?,
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &cfg, shared.clone())?,
            other => return Err(format!("Formato de audio no admitido: {other:?}")),
        };
        stream.play().map_err(|e| format!("No se pudo iniciar el audio: {e}"))?;
        Ok(Player { _stream: stream, shared, rate })
    }

    /// Empieza a sonar la mezcla (estéreo a la frecuencia del dispositivo) desde el segundo `secs`.
    pub fn play_from(&self, mix: Arc<Vec<f32>>, secs: f32) {
        if let Ok(mut s) = self.shared.lock() {
            s.pos = ((secs.max(0.0) * self.rate as f32) as usize).min(mix.len() / 2);
            s.mix = mix;
            s.playing = true;
        }
    }

    pub fn stop(&self) {
        if let Ok(mut s) = self.shared.lock() {
            s.playing = false;
        }
    }

    /// Por dónde va el sonido, en segundos: el reloj de la reproducción.
    pub fn secs(&self) -> f32 {
        self.shared.lock().map_or(0.0, |s| s.pos as f32 / self.rate as f32)
    }

    pub fn playing(&self) -> bool {
        self.shared.lock().map_or(false, |s| s.playing)
    }

    /// Por dónde va el sonido, en cuadros de muestras estéreo.
    pub fn frames(&self) -> usize {
        self.shared.lock().map_or(0, |s| s.pos)
    }
}

// ---------- Medidor (VU) ----------

/// Los pilotos de cada canal: apagado con silencio, verde con sonido, amarillo cerca del tope (−3 a 0 dBFS) y rojo al pasarse.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lamp {
    Off,
    Green,
    Yellow,
    Red,
}

/// El medidor de la mezcla final, por canal (0 = izquierdo, 1 = derecho). La aguja sigue el promedio suave (balística de VU,
/// unos 300 ms) y el piloto y el número de pico siguen el mismo pico, que se sostiene un instante (`PEAK_HOLD`) y luego cae: lo que dice el piloto es siempre lo que dice el número.
#[derive(Clone, Copy, Default, Debug)]
pub struct Meter {
    /// Nivel eficaz suavizado (lineal, 1.0 = fondo de escala).
    pub vu: [f32; 2],
    /// Pico reciente (lineal), sostenido un instante y con caída después; lo usan el piloto y el número.
    pub peak: [f32; 2],
    /// Segundos que le quedan al pico sostenido.
    pub hold: [f32; 2],
}

impl Meter {
    /// Cuánto se sostiene un pico antes de empezar a caer.
    pub const PEAK_HOLD: f32 = 0.6;
    /// Constante de tiempo de la aguja.
    pub const VU_TIME: f32 = 0.3;
    /// Por debajo de esto (−50 dBFS) se considera silencio.
    pub const SILENCE: f32 = 0.003_162_3;
    /// A partir de aquí (−3 dBFS) el piloto pasa a amarillo.
    pub const WARN: f32 = 0.707_9;

    /// Lee las muestras estéreo `from..to` (en cuadros) de la mezcla, que acaban de sonar.
    pub fn feed(&mut self, mix: &[f32], from: usize, to: usize, rate: u32) {
        let to = to.min(mix.len() / 2);
        let from = from.min(to);
        let n = to - from;
        if n == 0 {
            return;
        }
        let dt = n as f32 / rate.max(1) as f32;
        let a = 1.0 - (-dt / Self::VU_TIME).exp();
        for ch in 0..2 {
            let (mut sq, mut pk) = (0.0f32, 0.0f32);
            for i in from..to {
                let v = mix[i * 2 + ch];
                sq += v * v;
                pk = pk.max(v.abs());
            }
            let rms = (sq / n as f32).sqrt();
            self.vu[ch] += a * (rms - self.vu[ch]);
            self.hold_peak(ch, pk, dt);
        }
    }

    /// Un pico nuevo mayor lo sustituye y reinicia la espera; si no, se espera y luego cae.
    fn hold_peak(&mut self, ch: usize, pk: f32, dt: f32) {
        if pk >= self.peak[ch] {
            self.peak[ch] = pk;
            self.hold[ch] = Self::PEAK_HOLD;
        } else if self.hold[ch] > 0.0 {
            self.hold[ch] -= dt;
        } else {
            self.peak[ch] *= (-dt / 0.25).exp();
        }
        if self.peak[ch] < 1e-5 {
            self.peak[ch] = 0.0;
        }
    }

    /// Sin sonido: la aguja y el piloto vuelven al reposo.
    pub fn idle(&mut self, dt: f32) {
        for ch in 0..2 {
            self.vu[ch] *= (-dt / 0.12).exp();
            if self.vu[ch] < 1e-5 {
                self.vu[ch] = 0.0;
            }
            self.hold_peak(ch, 0.0, dt);
        }
    }

    /// Borra el pico sostenido (clic en el medidor).
    pub fn clear_clip(&mut self) {
        self.peak = [0.0; 2];
        self.hold = [0.0; 2];
    }

    /// Rojo al pasar de 0 dBFS, amarillo desde −3 dBFS, verde con señal (desde −50 dBFS) y apagado en silencio.
    pub fn lamp(&self, ch: usize) -> Lamp {
        if self.peak[ch] > 1.0 {
            Lamp::Red
        } else if self.peak[ch] >= Self::WARN {
            Lamp::Yellow
        } else if self.peak[ch] >= Self::SILENCE {
            Lamp::Green
        } else {
            Lamp::Off
        }
    }

    /// La lectura de la aguja en dBFS eficaces, de −21 (reposo) a +3.
    pub fn needle_db(&self, ch: usize) -> f32 {
        if self.vu[ch] <= 1e-6 {
            return -21.0;
        }
        (20.0 * self.vu[ch].log10()).clamp(-21.0, 3.0)
    }

    /// El pico reciente en dBFS (−∞ se da como −99).
    pub fn peak_db(&self, ch: usize) -> f32 {
        if self.peak[ch] <= 1e-5 {
            -99.0
        } else {
            20.0 * self.peak[ch].log10()
        }
    }

    /// Qué tan encendido está el piloto, de 0 a 1: crece con el pico (de −50 a +3 dBFS).
    pub fn lamp_intensity(&self, ch: usize) -> f32 {
        ((self.peak_db(ch) + 50.0) / 53.0).clamp(0.0, 1.0)
    }

    /// El nivel eficaz en porcentaje de la escala completa (100 % = 0 dBFS).
    pub fn percent(&self, ch: usize) -> f32 {
        self.vu[ch] * 100.0
    }

    pub fn is_moving(&self) -> bool {
        (0..2).any(|c| self.vu[c] > 0.0 || self.peak[c] > 0.0)
    }
}

/// Todo el estado de audio de la aplicación.
#[derive(Default)]
pub struct State {
    pub cache: Cache,
    pub player: Option<Player>,
    /// La mezcla de la composición, con la firma de los audios que la forman.
    pub mix: Option<(u64, Arc<Vec<f32>>)>,
    /// ¿Está sonando con la reproducción?
    pub on: bool,
    /// Último cuadro visto durante la reproducción (para detectar el salto del bucle).
    pub last_frame: u32,
    /// Un aviso de problema con el audio ya dado (para no repetirlo en cada reproducción).
    pub warned: bool,
    /// Audios que no se pudieron leer (no se reintenta en cada fotograma).
    pub failed: std::collections::HashSet<PathBuf>,
    /// El medidor de la mezcla y hasta dónde se ha leído (en cuadros de muestras).
    pub meter: Meter,
    pub meter_pos: usize,
    /// Solo para capturas: deja el medidor fijo en el valor pedido (`LEONTOON_VU`).
    pub meter_frozen: bool,
}

/// Una firma de los audios y su colocación: cambia si cambia cualquier cosa que afecte a la mezcla.
pub fn signature(items: &[Item], rate: u32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    rate.hash(&mut h);
    for it in items {
        (&it.path, it.start.to_bits(), it.len.to_bits(), it.offset.to_bits(), it.base_frames).hash(&mut h);
        // Los valores y las claves también cambian el sonido.
        format!("{:?}{:?}", it.params, it.tracks).hash(&mut h);
    }
    h.finish()
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Un WAV de prueba de 16 bits: una onda cuadrada de amplitud `amp` y `secs` segundos.
    fn wav_bytes(rate: u32, channels: u16, secs: f32, amp: f32) -> Vec<u8> {
        let n = (rate as f32 * secs) as usize;
        let mut pcm: Vec<u8> = Vec::new();
        for i in 0..n {
            let v = if (i / 50) % 2 == 0 { amp } else { -amp };
            for _ in 0..channels {
                pcm.extend_from_slice(&((v * 32767.0) as i16).to_le_bytes());
            }
        }
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
        out.extend_from_slice(&(channels * 2).to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        out.extend_from_slice(&pcm);
        out
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("leontoon_audio_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_wav_is_decoded_with_its_rate_channels_and_length() {
        let d = temp("decode");
        let f = d.join("tono.wav");
        std::fs::write(&f, wav_bytes(22050, 2, 0.5, 0.5)).unwrap();
        let c = decode(&f).unwrap();
        assert_eq!((c.rate, c.channels), (22050, 2));
        assert!((c.secs() - 0.5).abs() < 0.01, "{}", c.secs());
        assert!(c.data.iter().all(|v| v.abs() <= 0.51));
        // Un archivo que no es audio da un error claro, sin fallar.
        std::fs::write(d.join("nada.wav"), b"esto no es audio").unwrap();
        assert!(decode(&d.join("nada.wav")).is_err());
        assert!(decode(&d.join("no_existe.wav")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_waveform_follows_the_loudness_over_time() {
        let d = temp("peaks");
        // Un segundo silencioso seguido de uno fuerte.
        let mut quiet = wav_bytes(8000, 1, 1.0, 0.1);
        let loud = wav_bytes(8000, 1, 1.0, 0.9);
        quiet.extend_from_slice(&loud[44..]);
        let n = (quiet.len() - 44) as u32;
        quiet[40..44].copy_from_slice(&n.to_le_bytes());
        quiet[4..8].copy_from_slice(&(36 + n).to_le_bytes());
        let f = d.join("dos.wav");
        std::fs::write(&f, quiet).unwrap();
        let mut cache = Cache::default();
        let e = cache.load(&f).unwrap();
        assert!(e.peak(0.0, 0.9) < 0.15);
        assert!(e.peak(1.1, 1.9) > 0.85);
        assert!((e.clip.secs() - 2.0).abs() < 0.01);
        let _ = std::fs::remove_dir_all(&d);
    }

    fn item(path: &Path, start: f32, len: f32) -> Item {
        Item { path: path.to_path_buf(), start, len, offset: 0.0, params: AudioParams::default(), tracks: Tracks::default(), base_frames: 0, fps: 24.0, solo: false }
    }

    /// La energía (valor absoluto máximo) de un tramo de la mezcla, en segundos.
    fn peak(m: &[f32], rate: u32, a: f32, b: f32, ch: usize) -> f32 {
        let (i0, i1) = ((a * rate as f32) as usize, ((b * rate as f32) as usize).min(m.len() / 2));
        (i0..i1).fold(0.0f32, |x, i| x.max(m[i * 2 + ch].abs()))
    }

    fn tone_file(name: &str, hz: f32, secs: f32, amp: f32) -> (PathBuf, PathBuf) {
        let d = temp(name);
        let rate = 16_000u32;
        let n = (rate as f32 * secs) as usize;
        let mut pcm: Vec<u8> = Vec::new();
        for i in 0..n {
            let v = (i as f32 / rate as f32 * hz * std::f32::consts::TAU).sin() * amp;
            pcm.extend_from_slice(&((v * 32767.0) as i16).to_le_bytes());
        }
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        out.extend_from_slice(&pcm);
        let f = d.join("tono.wav");
        std::fs::write(&f, out).unwrap();
        (d, f)
    }

    #[test]
    fn mixing_places_each_audio_at_its_time_and_trims_the_start_of_the_file() {
        let d = temp("mix");
        let f = d.join("tono.wav");
        std::fs::write(&f, wav_bytes(8000, 1, 1.0, 0.5)).unwrap();
        let mut cache = Cache::default();
        cache.load(&f).unwrap();
        let rate = 8000;
        // Empieza en el segundo 1 y dura medio segundo.
        let items = vec![item(&f, 1.0, 0.5)];
        let m = mix(&items, &cache, rate, 0.0, 3.0);
        assert_eq!(m.len(), 3 * rate as usize * 2);
        assert_eq!(peak(&m, rate, 0.0, 0.99, 0), 0.0, "antes de empezar, silencio");
        assert!(peak(&m, rate, 1.0, 1.49, 0) > 0.4);
        assert_eq!(peak(&m, rate, 1.52, 3.0, 0), 0.0, "pasada su duración, silencio");
        // Mezclando desde el segundo 1 (como al exportar un tramo) el audio empieza en el principio de la mezcla.
        let tail = mix(&items, &cache, rate, 1.0, 1.0);
        assert!(tail[..200].iter().any(|v| v.abs() > 0.2));
        // El recorte del principio: con 0,8 s recortados solo quedan 0,2 s del archivo (la onda cuadrada sigue por 1,0 s).
        let mut trimmed = item(&f, 0.0, 5.0);
        trimmed.offset = 0.8;
        let m = mix(&[trimmed], &cache, rate, 0.0, 2.0);
        assert!(peak(&m, rate, 0.0, 0.19, 0) > 0.4 && peak(&m, rate, 0.25, 2.0, 0) == 0.0, "suena solo el final del archivo");
        // Dos audios se suman y nunca pasan de 1.
        let two = vec![item(&f, 0.0, 1.0), item(&f, 0.0, 1.0)];
        assert!(mix(&two, &cache, rate, 0.0, 1.0).iter().all(|v| v.abs() <= 1.0));
        // El WAV de la mezcla se vuelve a leer igual.
        let out = d.join("mezcla.wav");
        let m = mix(&items, &cache, rate, 0.0, 3.0);
        write_wav(&out, rate, &m).unwrap();
        let back = decode(&out).unwrap();
        assert_eq!((back.rate, back.channels), (rate, 2));
        assert!((back.secs() - 3.0).abs() < 0.01);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn gain_in_decibels_scales_the_level_and_can_be_automated_with_keys() {
        let (d, f) = tone_file("gain", 440.0, 2.0, 0.5);
        let mut cache = Cache::default();
        cache.load(&f).unwrap();
        let rate = 16_000;
        let mut it = item(&f, 0.0, 2.0);
        let base = peak(&mix(&[it.clone()], &cache, rate, 0.0, 2.0), rate, 0.1, 1.9, 0);
        it.params.gain_db = -6.0;
        let m = mix(&[it.clone()], &cache, rate, 0.0, 2.0);
        let half = peak(&m, rate, 0.1, 1.9, 0);
        assert!((half / base - 0.501).abs() < 0.03, "-6 dB es la mitad: {}", half / base);
        // Una rampa de ganancia con claves: de -60 dB en el cuadro 0 a 0 dB en el cuadro 24 (1 s).
        it.params.gain_db = 0.0;
        it.tracks.set(Prop::Gain, 0, [-60.0, 0.0], crate::model::Interp::Linear);
        it.tracks.set(Prop::Gain, 24, [0.0, 0.0], crate::model::Interp::Linear);
        let m = mix(&[it.clone()], &cache, rate, 0.0, 2.0);
        assert!(peak(&m, rate, 0.05, 0.15, 0) < base * 0.01, "al principio casi no suena");
        assert!(peak(&m, rate, 1.2, 1.9, 0) > base * 0.95, "después de la rampa suena entero");
        assert!(peak(&m, rate, 0.45, 0.55, 0) > base * 0.01 && peak(&m, rate, 0.45, 0.55, 0) < base * 0.2, "a mitad de la rampa, entre medias");
        // La onda dibujada usa el mismo valor.
        assert!((it.value(Prop::Gain, 0.5) + 30.0).abs() < 1.0 && (it.gain_lin(1.0) - 1.0).abs() < 0.01);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn pan_sends_the_sound_to_one_side_and_mute_silences_it_by_keys() {
        let (d, f) = tone_file("pan", 440.0, 2.0, 0.5);
        let mut cache = Cache::default();
        cache.load(&f).unwrap();
        let rate = 16_000;
        let mut it = item(&f, 0.0, 2.0);
        let c = mix(&[it.clone()], &cache, rate, 0.0, 2.0);
        assert!((peak(&c, rate, 0.1, 1.0, 0) - peak(&c, rate, 0.1, 1.0, 1)).abs() < 0.02, "al centro suena igual por los dos lados");
        it.params.pan = -1.0;
        let l = mix(&[it.clone()], &cache, rate, 0.0, 2.0);
        assert!(peak(&l, rate, 0.1, 1.0, 0) > 0.5 && peak(&l, rate, 0.1, 1.0, 1) < 0.001, "todo a la izquierda");
        it.params.pan = 1.0;
        let r = mix(&[it.clone()], &cache, rate, 0.0, 2.0);
        assert!(peak(&r, rate, 0.1, 1.0, 1) > 0.5 && peak(&r, rate, 0.1, 1.0, 0) < 0.001, "todo a la derecha");
        // Mute con claves: calla del cuadro 24 (1 s) en adelante, de golpe.
        it.params.pan = 0.0;
        it.tracks.set(Prop::Mute, 0, [0.0, 0.0], crate::model::Interp::Step);
        it.tracks.set(Prop::Mute, 24, [1.0, 0.0], crate::model::Interp::Step);
        let m = mix(&[it.clone()], &cache, rate, 0.0, 2.0);
        assert!(peak(&m, rate, 0.1, 0.9, 0) > 0.4);
        assert!(peak(&m, rate, 1.05, 1.9, 0) < 0.001, "en silencio desde el segundo 1");
        assert!(it.muted(1.5) && !it.muted(0.5));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_equalizer_boosts_or_cuts_each_band() {
        let rate = 16_000;
        // Un tono grave (100 Hz), uno medio (1 kHz) y uno agudo (6 kHz): cada banda del EQ afecta sobre todo a su tono.
        for (hz, prop, name) in [(100.0, Prop::EqLo, "graves"), (1000.0, Prop::EqMid, "medios"), (6000.0, Prop::EqHi, "agudos")] {
            let (d, f) = tone_file(&format!("eq_{name}"), hz, 2.0, 0.2);
            let mut cache = Cache::default();
            cache.load(&f).unwrap();
            let it0 = item(&f, 0.0, 2.0);
            let base = peak(&mix(&[it0.clone()], &cache, rate, 0.0, 2.0), rate, 0.5, 1.9, 0);
            let mut up = it0.clone();
            up.params.set(prop, 12.0);
            let boosted = peak(&mix(&[up], &cache, rate, 0.0, 2.0), rate, 0.5, 1.9, 0);
            let mut down = it0.clone();
            down.params.set(prop, -12.0);
            let cut = peak(&mix(&[down], &cache, rate, 0.0, 2.0), rate, 0.5, 1.9, 0);
            assert!(boosted > base * 2.5, "+12 dB en {name} sube su tono: {boosted} contra {base}");
            assert!(cut < base * 0.4, "-12 dB en {name} lo baja: {cut} contra {base}");
            let _ = std::fs::remove_dir_all(&d);
        }
        // Una banda no toca a las lejanas: subir los graves casi no mueve un tono agudo.
        let (d, f) = tone_file("eq_lejos", 6000.0, 2.0, 0.2);
        let mut cache = Cache::default();
        cache.load(&f).unwrap();
        let base = peak(&mix(&[item(&f, 0.0, 2.0)], &cache, rate, 0.0, 2.0), rate, 0.5, 1.9, 0);
        let mut it = item(&f, 0.0, 2.0);
        it.params.eq_lo = 12.0;
        let lo = peak(&mix(&[it], &cache, rate, 0.0, 2.0), rate, 0.5, 1.9, 0);
        assert!((lo / base - 1.0).abs() < 0.25, "los graves no cambian un agudo: {}", lo / base);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn echo_and_reverb_add_a_tail_after_the_end_of_the_audio() {
        let (d, f) = tone_file("fx", 440.0, 0.5, 0.5);
        let mut cache = Cache::default();
        cache.load(&f).unwrap();
        let rate = 16_000;
        let seco = mix(&[item(&f, 0.0, 0.5)], &cache, rate, 0.0, 3.0);
        assert!(peak(&seco, rate, 0.7, 3.0, 0) == 0.0, "sin efectos no queda nada después del final");
        let mut e = item(&f, 0.0, 0.5);
        e.params.echo = 0.8;
        let m = mix(&[e], &cache, rate, 0.0, 3.0);
        assert!(peak(&m, rate, 0.6, 1.2, 0) > 0.05, "el eco repite el sonido después de su final");
        let mut r = item(&f, 0.0, 0.5);
        r.params.reverb = 1.0;
        let m = mix(&[r], &cache, rate, 0.0, 3.0);
        assert!(peak(&m, rate, 0.55, 1.5, 0) > 0.005, "la reverberación deja una cola");
        // Con el eco animado a 0 (claves), no hay cola.
        let mut z = item(&f, 0.0, 0.5);
        z.tracks.set(Prop::Echo, 0, [0.0, 0.0], crate::model::Interp::Linear);
        let m = mix(&[z], &cache, rate, 0.0, 3.0);
        assert!(peak(&m, rate, 0.7, 3.0, 0) < 0.001);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn audio_layers_are_collected_with_their_properties_time_trim_mute_and_solo() {
        use crate::model::{AudioRef, Layer};
        let mut p = Project::blank(1280, 720, 24, 100);
        let exe = std::env::current_exe().unwrap().to_string_lossy().to_string();
        let audio_layer = |name: &str, in_f: u32, out_f: Option<u32>| {
            let mut l = Layer::new(name);
            l.kind = LayerKind::Audio;
            l.audio = Some(AudioRef { path: exe.clone(), name: name.into(), ..Default::default() });
            l.in_f = in_f;
            l.out_f = out_f;
            l
        };
        let mut a = audio_layer("voz", 24, Some(71));
        a.audio.as_mut().unwrap().offset = 0.5;
        a.audio.as_mut().unwrap().params.gain_db = -3.0;
        a.audio.as_mut().unwrap().params.pan = 0.25;
        a.tracks.set(Prop::EqHi, 30, [4.0, 0.0], crate::model::Interp::Linear);
        let mut muted = audio_layer("muda", 0, None);
        muted.visible = false;
        p.layers = vec![a, muted];
        let items = collect(&p);
        assert_eq!(items.len(), 1, "la capa en mute (M) no suena");
        let it = &items[0];
        assert_eq!((it.start, it.len, it.offset), (1.0, 2.0, 0.5));
        assert_eq!((it.params.gain_db, it.params.pan), (-3.0, 0.25));
        assert!(it.tracks.has(Prop::EqHi) && (it.value(Prop::EqHi, 2.0) - 4.0).abs() < 1e-3);
        // Solo: si una capa de audio está en solo, solo suenan las que lo están.
        let mut other = audio_layer("otra", 0, Some(10));
        other.solo = true;
        p.layers.push(other);
        let items = collect(&p);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].len, 11.0 / 24.0);
        // Dentro de una precomposición que empieza en el cuadro 48, el tiempo se suma y las claves siguen siendo de su composición.
        let mut inner = audio_layer("dentro", 0, Some(23));
        inner.tracks.set(Prop::Gain, 24, [-12.0, 0.0], crate::model::Interp::Linear);
        p.layers = vec![Layer::new("vacía")];
        p.layers[0].kind = LayerKind::Precomp;
        p.comps.push(crate::model::Composition { id: 1, name: "Animatic".into(), layers: vec![inner], length: 24, panel: false });
        p.layers[0].precomp = Some(1);
        p.layers[0].in_f = 48;
        let items = collect(&p);
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].start, items[0].len, items[0].base_frames), (2.0, 1.0, 48));
        assert!((items[0].value(Prop::Gain, 3.0) + 12.0).abs() < 1e-3, "el cuadro 24 de la composición es el segundo 3 de la principal");
        // Una capa con una ruta que no existe se ignora.
        p.comps[0].layers[0].audio.as_mut().unwrap().path = "C:/no/existe.wav".into();
        assert!(collect(&p).is_empty());
    }
    #[test]
    fn the_player_clock_advances_while_sound_plays_when_there_is_an_output_device() {
        let Ok(pl) = Player::open() else {
            eprintln!("sin dispositivo de salida de audio: se omite la prueba de reproducción");
            return;
        };
        // Un segundo de silencio: lo que se prueba es el reloj, no el sonido.
        let mix = Arc::new(vec![0.0f32; pl.rate as usize * 2]);
        pl.play_from(mix, 0.0);
        std::thread::sleep(std::time::Duration::from_millis(400));
        let t = pl.secs();
        assert!(t > 0.1 && t < 0.9, "el reloj avanza con el sonido: {t}");
        pl.stop();
        let frozen = pl.secs();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(pl.secs(), frozen, "al parar, el reloj se detiene");
    }
    #[test]
    fn el_medidor_apaga_en_silencio_enciende_verde_amarillo_y_rojo_con_retencion() {
        let rate = 1000;
        let tone = |amp: f32, n: usize| -> Vec<f32> { (0..n * 2).map(|i| if (i / 2) % 2 == 0 { amp } else { -amp }).collect() };
        let mut m = Meter::default();
        // Silencio: piloto apagado y aguja en reposo.
        m.feed(&tone(0.0, 100), 0, 100, rate);
        assert_eq!(m.lamp(0), Lamp::Off);
        assert_eq!(m.needle_db(0), -21.0);
        // Sonido normal: verde, y la aguja sube con el promedio (no de golpe).
        let quiet = tone(0.25, 1000);
        m.feed(&quiet, 0, 100, rate);
        assert_eq!(m.lamp(0), Lamp::Green);
        let early = m.needle_db(0);
        m.feed(&quiet, 100, 1000, rate);
        assert!(m.needle_db(0) > early, "la aguja sigue subiendo con el tiempo");
        assert!((m.needle_db(0) - 20.0 * 0.25f32.log10()).abs() < 1.0, "se acerca al nivel eficaz de la señal (−12 dBFS)");
        // Cerca del tope (entre −3 y 0 dBFS): amarillo.
        m.feed(&tone(0.85, 100), 0, 100, rate);
        assert_eq!(m.lamp(0), Lamp::Yellow);
        // Justo por debajo de 0 dBFS sigue amarillo; solo por encima de 0 dBFS es rojo.
        m.feed(&tone(0.999, 100), 0, 100, rate);
        assert_eq!(m.lamp(1), Lamp::Yellow);
        m.feed(&tone(1.4, 100), 0, 100, rate);
        assert_eq!(m.lamp(1), Lamp::Red);
        // El pico se sostiene un instante (la lámpara y el número coinciden) y luego cae: el rojo no se queda pegado.
        m.idle(0.3);
        assert_eq!(m.lamp(1), Lamp::Red, "dentro de la espera");
        assert!(m.peak_db(1) > 0.0, "el número dice lo mismo que el piloto");
        m.idle(Meter::PEAK_HOLD);
        for _ in 0..10 {
            m.idle(0.1);
        }
        assert_ne!(m.lamp(1), Lamp::Red, "pasada la espera, el piloto sigue a la señal");
        m.feed(&tone(1.4, 100), 0, 100, rate);
        m.clear_clip();
        assert_eq!(m.lamp(1), Lamp::Off, "un clic borra el pico");
        m.feed(&tone(1.4, 100), 0, 100, rate);
        for _ in 0..40 {
            m.idle(0.1);
        }
        assert_eq!(m.lamp(0), Lamp::Off);
        assert!(!m.is_moving());
    }

    #[test]
    fn cada_canal_se_mide_por_separado() {
        let mut m = Meter::default();
        let mix: Vec<f32> = (0..2000).map(|i| if i % 2 == 0 { 0.5 } else { 0.0 }).collect();
        m.feed(&mix, 0, 1000, 1000);
        assert_eq!(m.lamp(0), Lamp::Green);
        assert_eq!(m.lamp(1), Lamp::Off);
    }
    #[test]
    fn the_mix_keeps_levels_above_full_scale_so_the_meter_can_show_overmodulation() {
        let (d, f) = tone_file("over", 440.0, 1.0, 0.9);
        let mut cache = Cache::default();
        cache.load(&f).unwrap();
        let rate = 16_000;
        let mut it = item(&f, 0.0, 1.0);
        it.params.gain_db = 9.0;
        let m = mix(&[it], &cache, rate, 0.0, 1.0);
        assert!(peak(&m, rate, 0.1, 0.9, 0) > 1.5, "la mezcla sobrepasa 0 dBFS");
        let mut meter = Meter::default();
        meter.feed(&m, 0, m.len() / 2, rate);
        assert_eq!(meter.lamp(0), Lamp::Red);
        assert!(meter.needle_db(0) > -3.0 && meter.lamp_intensity(0) > 0.95 && meter.percent(0) > 90.0);
        // Al exportar a WAV se recorta a 0 dBFS.
        let out = d.join("over.wav");
        write_wav(&out, rate, &m).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }
}