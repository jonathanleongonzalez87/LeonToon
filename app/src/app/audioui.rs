//! Audio en la interfaz (Issue #136, fase A2): importar un audio como capa, dibujar su onda y hacerlo sonar con la reproducción.
//!
//! El reloj de la reproducción es el del audio: mientras suena, la imagen se ajusta al punto en que va el sonido.

use super::*;
use crate::audio;
use crate::model::{AudioRef, Layer};
use std::path::Path;
use std::sync::Arc;

/// Lo máximo que se mezcla de una vez, en segundos (ocho minutos de estéreo son unos 180 MB).
const MAX_MIX_SECS: f32 = 480.0;

/// Dibuja la onda de un audio dentro de su barra: una línea vertical por cada par de píxeles, centrada, con el pico de ese tramo.
/// `secs_per_px` es cuántos segundos del audio cubre cada píxel; `skip` son los segundos del principio del clip que quedan fuera.
pub(super) fn paint_wave(painter: &egui::Painter, bar: Rect, e: &audio::Entry, secs_per_px: f32, skip: f32, color: Color32) {
    let clip = painter.clip_rect().intersect(bar);
    if clip.width() <= 0.0 || secs_per_px <= 0.0 {
        return;
    }
    let mid = bar.center().y;
    let half = (bar.height() * 0.5 - 2.0).max(1.0);
    let mut x = (clip.min.x - bar.min.x).max(0.0).floor();
    let end = (clip.max.x - bar.min.x).min(bar.width());
    while x < end {
        let t0 = skip + x * secs_per_px;
        let p = e.peak(t0, t0 + 2.0 * secs_per_px);
        let h = (p * half).max(0.5);
        painter.line_segment([Pos2::new(bar.min.x + x, mid - h), Pos2::new(bar.min.x + x, mid + h)], stk(1.5, color));
        x += 2.0;
    }
}

impl LeonApp {
    /// Elige un archivo de audio y lo pone como capa de audio en la composición actual, a partir del cuadro actual.
    pub(super) fn import_audio(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Importar audio").add_filter("Audio", &["wav", "mp3", "flac", "ogg", "m4a", "aac", "mp4"]).pick_file() else { return };
        self.import_audio_file(&path);
    }

    pub(super) fn import_audio_file(&mut self, path: &Path) {
        let secs = match self.audio.cache.load(path) {
            Ok(e) => e.clip.secs(),
            Err(e) => {
                self.say(e);
                return;
            }
        };
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Audio".into());
        let fps = self.project.fps.max(1) as f32;
        let frames = ((secs * fps).ceil() as u32).max(1);
        let rel = self.project.base_dir.as_ref().and_then(|b| path.strip_prefix(b).ok()).map(|r| r.to_string_lossy().to_string());
        self.checkpoint();
        self.lsel.clear();
        let mut l = Layer::new(name.clone());
        l.kind = LayerKind::Audio;
        l.audio = Some(AudioRef { path: path.to_string_lossy().to_string(), rel, name: name.clone() });
        l.in_f = self.frame;
        l.out_f = Some(self.frame + frames - 1);
        self.project.layers.insert(0, l);
        self.cl = 0;
        self.sel = None;
        self.project.length = self.project.length.max(self.frame + frames).min(100_000);
        self.say(format!("Audio «{name}» importado: {secs:.1} s. Su barra se mueve y se recorta en la línea de tiempo; la opacidad es el volumen y el ojo lo silencia"));
    }

    /// Lee los audios de las capas de audio que se ven (para dibujar su onda), una sola vez cada uno.
    pub(super) fn audio_prepare(&mut self) {
        let paths: Vec<std::path::PathBuf> = self
            .project
            .layers
            .iter()
            .filter(|l| l.kind == LayerKind::Audio)
            .filter_map(|l| l.audio.as_ref().and_then(|a| a.resolve(self.project.base_dir.as_deref())))
            .filter(|p| self.audio.cache.get(p).is_none() && !self.audio.failed.contains(p))
            .collect();
        for p in paths {
            if let Err(e) = self.audio.cache.load(&p) {
                self.audio.failed.insert(p);
                self.say(e);
            }
        }
    }

    /// Empieza a sonar la mezcla de los audios del proyecto desde el cuadro actual. Sin audios o sin dispositivo, la reproducción
    /// sigue sin sonido.
    pub(super) fn audio_start(&mut self) {
        let root = self.project.root_clone();
        let items = audio::collect(&root);
        if items.is_empty() {
            self.audio.on = false;
            return;
        }
        for it in &items {
            if self.audio.cache.get(&it.path).is_none() {
                if let Err(e) = self.audio.cache.load(&it.path) {
                    self.say(e);
                }
            }
        }
        if self.audio.player.is_none() {
            match audio::Player::open() {
                Ok(p) => self.audio.player = Some(p),
                Err(e) => {
                    if !self.audio.warned {
                        self.audio.warned = true;
                        self.say(format!("{e}: se reproduce sin sonido"));
                    }
                    return;
                }
            }
        }
        let rate = self.audio.player.as_ref().map_or(48_000, |p| p.rate);
        let sig = audio::signature(&items, rate);
        let mix = match &self.audio.mix {
            Some((s, m)) if *s == sig => m.clone(),
            _ => {
                let total = items.iter().map(|i| i.start + i.len).fold(0.0f32, f32::max).min(MAX_MIX_SECS);
                let m = Arc::new(audio::mix(&items, &self.audio.cache, rate, 0.0, total));
                self.audio.mix = Some((sig, m.clone()));
                m
            }
        };
        let fps = self.project.fps.max(1) as f32;
        if let Some(p) = &self.audio.player {
            p.play_from(mix, self.frame as f32 / fps);
        }
        self.audio.on = true;
        self.audio.last_frame = self.frame;
    }

    pub(super) fn audio_stop(&mut self) {
        if let Some(p) = &self.audio.player {
            p.stop();
        }
        self.audio.on = false;
    }

    /// Durante la reproducción: la imagen sigue al sonido (si se separan más de dos cuadros, se corrige) y, al saltar el bucle al
    /// principio, el sonido vuelve a empezar.
    pub(super) fn audio_follow(&mut self) {
        let fps = self.project.fps.max(1) as f32;
        let Some((want, sounding)) = self.audio.player.as_ref().map(|p| ((p.secs() * fps).floor() as i64, p.playing())) else { return };
        if !sounding {
            self.audio.last_frame = self.frame;
            return;
        }
        if self.frame + 3 < self.audio.last_frame {
            self.audio_start();
            return;
        }
        let (a, b) = self.play_bounds();
        if (want - self.frame as i64).abs() > 2 {
            self.frame = want.clamp(a as i64, b as i64) as u32;
        }
        self.audio.last_frame = self.frame;
    }

    /// Mezcla de audio de un tramo del proyecto (cuadros `a` a `b`) como WAV, para unirla al vídeo al exportar. `None` si no hay audio.
    pub(super) fn audio_mixdown_wav(&mut self, root: &Project, a: u32, b: u32, path: &Path) -> Option<Result<(), String>> {
        let items = audio::collect(root);
        if items.is_empty() {
            return None;
        }
        for it in &items {
            if self.audio.cache.get(&it.path).is_none() {
                if let Err(e) = self.audio.cache.load(&it.path) {
                    return Some(Err(e));
                }
            }
        }
        let fps = root.fps.max(1) as f32;
        let (from, total) = (a as f32 / fps, (b + 1 - a) as f32 / fps);
        let rate = 48_000;
        let m = audio::mix(&items, &self.audio.cache, rate, from, total.min(MAX_MIX_SECS));
        Some(audio::write_wav(path, rate, &m))
    }
}
