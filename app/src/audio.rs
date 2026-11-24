//! Audio (Issue #136 y #15, fases A2 y M8a de docs/decodificadores-video-audio.md): leer archivos de audio, calcular su onda, mezclar las
//! capas de audio de una composición y reproducirlas por el dispositivo de salida.
//!
//! Se decodifica con `symphonia` (WAV, MP3, FLAC, OGG/Vorbis y AAC/M4A, en Rust puro) y se reproduce con `cpal` (WASAPI en Windows).
//! El reloj de la reproducción es el del audio: la imagen se ajusta al sonido. No depende de la interfaz.

use crate::model::{Layer, LayerKind, Project};
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
#[derive(Default)]
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

/// Un audio colocado en el tiempo: dónde empieza, cuánto suena y con qué volumen.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub path: PathBuf,
    pub start: f32,
    pub len: f32,
    pub gain: f32,
}

/// Los audios de las capas de audio de la composición principal y de las precomposiciones que contiene, con su tiempo en segundos
/// desde el principio. Una capa apagada (el ojo) no suena; su opacidad es el volumen.
pub fn collect(p: &Project) -> Vec<Item> {
    let mut out = Vec::new();
    walk(p, &p.layers, 0, p.length, 0, &mut out);
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
                let Some(path) = l.audio.as_ref().and_then(|a| a.resolve(p.base_dir.as_deref())) else { continue };
                let (a, b) = (l.in_f, l.out_frame(comp_len));
                if b < a {
                    continue;
                }
                out.push(Item { path, start: (base + a) as f32 / fps, len: (b - a + 1) as f32 / fps, gain: l.opacity.clamp(0.0, 1.0) });
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

/// Mezcla los audios en estéreo (muestras entrelazadas izquierda y derecha) a la frecuencia `rate`, desde el segundo `from` y
/// durante `total` segundos. Lo que no se ha leído se omite.
pub fn mix(items: &[Item], cache: &Cache, rate: u32, from: f32, total: f32) -> Vec<f32> {
    let rate = rate.max(1);
    let frames = (total.max(0.0) * rate as f32).ceil() as usize;
    let mut out = vec![0.0f32; frames * 2];
    for it in items {
        let Some(e) = cache.get(&it.path) else { continue };
        let clip = &e.clip;
        let n0 = ((it.start - from) * rate as f32).ceil().max(0.0) as usize;
        let n1 = (((it.start + it.len - from) * rate as f32).floor().max(0.0) as usize).min(frames);
        let frames_clip = clip.frames();
        for n in n0..n1 {
            let local = (n as f32 / rate as f32 + from - it.start) as f64;
            let sp = local * clip.rate as f64;
            let i = sp as usize;
            if i >= frames_clip {
                break;
            }
            let frac = (sp - i as f64) as f32;
            let (l0, r0) = clip.stereo(i);
            let (l1, r1) = if i + 1 < frames_clip { clip.stereo(i + 1) } else { (l0, r0) };
            out[n * 2] += (l0 + (l1 - l0) * frac) * it.gain;
            out[n * 2 + 1] += (r0 + (r1 - r0) * frac) * it.gain;
        }
    }
    for v in &mut out {
        *v = v.clamp(-1.0, 1.0);
    }
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
                        *d = T::from_sample(v);
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
}

/// Una firma de los audios y su colocación: cambia si cambia cualquier cosa que afecte a la mezcla.
pub fn signature(items: &[Item], rate: u32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    rate.hash(&mut h);
    for it in items {
        (&it.path, it.start.to_bits(), it.len.to_bits(), it.gain.to_bits()).hash(&mut h);
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

    #[test]
    fn mixing_places_each_audio_at_its_time_with_its_volume() {
        let d = temp("mix");
        let f = d.join("tono.wav");
        std::fs::write(&f, wav_bytes(8000, 1, 1.0, 0.5)).unwrap();
        let mut cache = Cache::default();
        cache.load(&f).unwrap();
        let rate = 8000;
        // Empieza en el segundo 1, dura medio segundo y suena a la mitad de volumen.
        let items = vec![Item { path: f.clone(), start: 1.0, len: 0.5, gain: 0.5 }];
        let m = mix(&items, &cache, rate, 0.0, 3.0);
        assert_eq!(m.len(), 3 * rate as usize * 2);
        let energy = |a: f32, b: f32| m[(a * rate as f32) as usize * 2..(b * rate as f32) as usize * 2].iter().fold(0.0f32, |x, v| x.max(v.abs()));
        assert_eq!(energy(0.0, 0.99), 0.0, "antes de empezar, silencio");
        assert!((energy(1.0, 1.49) - 0.25).abs() < 0.02, "volumen: {}", energy(1.0, 1.49));
        assert_eq!(energy(1.52, 3.0), 0.0, "pasada su duración, silencio");
        // Mezclando desde el segundo 1 (como al exportar un tramo) el audio empieza en el principio de la mezcla.
        let tail = mix(&items, &cache, rate, 1.0, 1.0);
        assert!(tail[..200].iter().any(|v| v.abs() > 0.2));
        // Dos audios se suman y nunca pasan de 1.
        let two = vec![Item { path: f.clone(), start: 0.0, len: 1.0, gain: 1.0 }, Item { path: f.clone(), start: 0.0, len: 1.0, gain: 1.0 }];
        assert!(mix(&two, &cache, rate, 0.0, 1.0).iter().all(|v| v.abs() <= 1.0));
        // El WAV de la mezcla se vuelve a leer igual.
        let out = d.join("mezcla.wav");
        write_wav(&out, rate, &m).unwrap();
        let back = decode(&out).unwrap();
        assert_eq!((back.rate, back.channels), (rate, 2));
        assert!((back.secs() - 3.0).abs() < 0.01);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn audio_layers_are_collected_with_their_time_even_inside_a_precomposition() {
        use crate::model::{AudioRef, Layer};
        let mut p = Project::blank(1280, 720, 24, 100);
        let audio_layer = |name: &str, in_f: u32, out_f: Option<u32>| {
            let mut l = Layer::new(name);
            l.kind = LayerKind::Audio;
            l.audio = Some(AudioRef { path: "C:/x/voz.wav".into(), rel: None, name: name.into() });
            // Para que se resuelva sin existir en disco, se prueba la lógica de colocación con una ruta que existe.
            l.in_f = in_f;
            l.out_f = out_f;
            l
        };
        let exe = std::env::current_exe().unwrap();
        let mut a = audio_layer("voz", 24, Some(71));
        a.audio.as_mut().unwrap().path = exe.to_string_lossy().to_string();
        let mut muted = audio_layer("muda", 0, None);
        muted.audio.as_mut().unwrap().path = exe.to_string_lossy().to_string();
        muted.visible = false;
        p.layers = vec![a, muted];
        let items = collect(&p);
        assert_eq!(items.len(), 1, "la capa apagada no suena");
        assert_eq!((items[0].start, items[0].len, items[0].gain), (1.0, 2.0, 1.0));
        // Dentro de una precomposición que empieza en el cuadro 48, el tiempo se suma.
        let mut inner = audio_layer("dentro", 0, Some(23));
        inner.audio.as_mut().unwrap().path = exe.to_string_lossy().to_string();
        p.layers = vec![Layer::new("vacía")];
        p.layers[0].kind = LayerKind::Precomp;
        p.comps.push(crate::model::Composition { id: 1, name: "Animatic".into(), layers: vec![inner], length: 24, panel: false });
        p.layers[0].precomp = Some(1);
        p.layers[0].in_f = 48;
        let items = collect(&p);
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].start, items[0].len), (2.0, 1.0));
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
}