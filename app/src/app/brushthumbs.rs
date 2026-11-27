//! Muestras de trazo de la lista de pinceles de píxeles (Issue #45): se pintan con el mismo motor que la escena,
//! con los parámetros de cada pincel, y se calculan por turnos para no congelar la interfaz.
//!
//! Solo viven en memoria: no se guardan en el proyecto ni en la biblioteca de pinceles.

use crate::brush::{preview, BrushSpec, Gray, TexKind};
use crate::presets::{BrushSetup, PresetKey};
use crate::raster::Raster;
use eframe::egui;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Tamaño de la muestra, en píxeles. En la lista se dibuja a la mitad (como la de los pinceles vectoriales), para
/// que se vea nítida en pantallas con escala.
pub(super) const THUMB_W: u32 = 140;
pub(super) const THUMB_H: u32 = 44;
/// Los pinceles más grandes se reducen para que el trazo quepa en la muestra.
const MAX_SIZE: f32 = THUMB_H as f32 * 0.55;
/// Tiempo por fotograma para calcular muestras (siempre se calcula al menos una).
const FRAME_BUDGET: Duration = Duration::from_millis(4);

/// Lo que queda por gastar en muestras en este fotograma.
pub(super) struct Budget {
    limit: Duration,
    spent: Duration,
    made: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Budget::new(FRAME_BUDGET)
    }
}

impl Budget {
    pub(super) fn new(limit: Duration) -> Self {
        Budget { limit, spent: Duration::ZERO, made: 0 }
    }

    pub(super) fn allows(&self) -> bool {
        self.made == 0 || self.spent < self.limit
    }

    /// Hace un cálculo y descuenta lo que tarda.
    pub(super) fn run<T>(&mut self, work: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let out = work();
        self.spent += start.elapsed();
        self.made += 1;
        out
    }
}

/// Pinta la muestra de un pincel. `tip` y `texture` son las imágenes que pide su configuración, si se pudieron leer;
/// si falta alguna, se pinta como al aplicar el pincel (punta redonda o sin textura).
pub(super) fn render_thumb(setup: &BrushSetup, tip: Option<Arc<Gray>>, texture: Option<Arc<Gray>>) -> Raster {
    let s = setup.clone().clamped();
    let mut params = s.params;
    if tip.is_none() {
        params.use_tip_image = false;
    }
    if texture.is_none() && params.texture == TexKind::Image {
        params.texture = TexKind::None;
    }
    // Los pinceles que varían el color se muestran con dos colores, igual que la muestra del pincel en uso.
    let colorful = params.hue_jitter > 0.0 || params.sat_jitter > 0.0 || params.bri_jitter > 0.0 || params.fg_bg_jitter > 0.0;
    let (c1, c2) = if colorful { ([240, 130, 60], [60, 150, 240]) } else { ([232, 232, 232], [232, 232, 232]) };
    let spec = BrushSpec::new(s.size.min(MAX_SIZE), s.opacity, c1, params, 7).with_color2(c2).with_images(tip, texture);
    preview(spec, THUMB_W, THUMB_H)
}

/// Caché de muestras. Cada entrada recuerda la configuración con la que se pintó: si el pincel cambia, se rehace.
#[derive(Default)]
pub(super) struct Thumbs {
    map: HashMap<PresetKey, (BrushSetup, egui::TextureHandle)>,
}

impl Thumbs {
    /// La muestra guardada y si corresponde a la configuración actual.
    pub(super) fn get(&self, key: &PresetKey, setup: &BrushSetup) -> Option<(&egui::TextureHandle, bool)> {
        self.map.get(key).map(|(s, t)| (t, s == setup))
    }

    pub(super) fn put(&mut self, ctx: &egui::Context, key: PresetKey, setup: BrushSetup, r: &Raster) {
        let img = egui::ColorImage::from_rgba_premultiplied([r.w as usize, r.h as usize], &r.data);
        let tex = ctx.load_texture("brush_thumb", img, egui::TextureOptions::LINEAR);
        self.map.insert(key, (setup, tex));
    }

    /// Olvida las muestras de pinceles que ya no están en la lista.
    pub(super) fn retain(&mut self, keep: impl Fn(&PresetKey) -> bool) {
        self.map.retain(|k, _| keep(k));
    }

    pub(super) fn clear(&mut self) {
        self.map.clear();
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brush::BrushParams;
    use crate::model::Project;
    use crate::presets;

    fn key(n: usize) -> PresetKey {
        ("Prueba".to_string(), format!("Pincel {n}"))
    }

    #[test]
    fn cada_pincel_incluido_tiene_una_muestra_distinta() {
        let all = presets::builtin();
        let samples: Vec<Vec<u8>> = all.iter().map(|p| render_thumb(&p.setup, None, None).data.to_vec()).collect();
        for (i, a) in samples.iter().enumerate() {
            assert!(a.iter().any(|v| *v != 0), "«{}» sale en blanco", all[i].name);
            for (j, b) in samples.iter().enumerate().skip(i + 1) {
                assert!(a != b, "«{}» y «{}» se ven igual", all[i].name, all[j].name);
            }
        }
    }

    #[test]
    fn la_punta_la_textura_y_la_dispersion_cambian_la_muestra() {
        let plain = BrushSetup { size: 18.0, ..BrushSetup::default() };
        let base = render_thumb(&plain, None, None).data.to_vec();
        // Media punta pintada y media vacía.
        let half = Arc::new(Gray { w: 16, h: 16, data: (0..256).map(|i| if i % 16 < 6 { 255 } else { 0 }).collect() });

        let tip = BrushSetup { params: BrushParams { use_tip_image: true, ..plain.params }, ..plain.clone() };
        assert!(render_thumb(&tip, Some(half.clone()), None).data.to_vec() != base, "punta de imagen");
        assert!(render_thumb(&tip, None, None).data.to_vec() == base, "sin la imagen se ve la punta redonda");

        let tex = BrushSetup { params: BrushParams { texture: TexKind::Image, texture_depth: 1.0, ..plain.params }, ..plain.clone() };
        assert!(render_thumb(&tex, None, Some(half)).data.to_vec() != base, "textura");

        let scattered = BrushSetup { params: BrushParams { scatter: 2.0, scatter_count: 3, ..plain.params }, ..plain.clone() };
        assert!(render_thumb(&scattered, None, None).data.to_vec() != base, "dispersión");
    }

    #[test]
    fn sin_presupuesto_se_calcula_una_muestra_por_turno() {
        let mut budget = Budget::new(Duration::ZERO);
        assert!(budget.allows());
        budget.run(|| std::thread::sleep(Duration::from_millis(1)));
        assert!(!budget.allows(), "la siguiente muestra espera al próximo fotograma");
    }

    #[test]
    fn la_muestra_caduca_al_cambiar_el_pincel_y_se_olvida_al_borrarlo() {
        let ctx = egui::Context::default();
        let mut thumbs = Thumbs::default();
        let a = BrushSetup::default();
        thumbs.put(&ctx, key(1), a.clone(), &render_thumb(&a, None, None));
        thumbs.put(&ctx, key(2), a.clone(), &render_thumb(&a, None, None));
        assert!(thumbs.get(&key(1), &a).unwrap().1);

        let b = BrushSetup { params: BrushParams { hardness: 0.1, ..a.params }, ..a.clone() };
        assert!(!thumbs.get(&key(1), &b).unwrap().1, "el pincel cambió");
        thumbs.put(&ctx, key(1), b.clone(), &render_thumb(&b, None, None));
        assert!(thumbs.get(&key(1), &b).unwrap().1);
        assert_eq!(thumbs.len(), 2, "actualizar no acumula muestras");

        thumbs.retain(|k| *k == key(1));
        assert!(thumbs.get(&key(2), &a).is_none());
    }

    #[test]
    fn las_muestras_no_cambian_lo_que_se_guarda() {
        let ctx = egui::Context::default();
        let project = Project::demo();
        let before = serde_json::to_string(&project).unwrap();
        let mut thumbs = Thumbs::default();
        for p in presets::builtin() {
            thumbs.put(&ctx, p.key(), p.setup.clone(), &render_thumb(&p.setup, None, None));
        }
        assert_eq!(serde_json::to_string(&project).unwrap(), before);
    }
}
