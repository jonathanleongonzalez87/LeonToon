//! Proyecto de muestra de Stop Motion: una secuencia real de plastilina (una bola roja y otra naranja), fotografiada con una
//! cámara. Las fotos van dentro del programa y se copian a la carpeta de datos la primera vez que se abre el ejemplo, porque
//! una secuencia Stop Motion vive en una carpeta aparte (el proyecto solo guarda la referencia).

use crate::model::{Layer, LayerKind, Project};
use crate::stopmotion::{SeqFolder, SmRef};
use std::path::Path;

pub const NOMBRE: &str = "Plastilina";

/// Copia las fotos a `dir` (si aún no están) y devuelve el proyecto con una capa Stop Motion que las usa.
pub fn build(dir: &Path) -> Result<Project, String> {
    let n = crate::demo_sm_fotos::FOTOS.len();
    let folder = match SeqFolder::open(dir) {
        Ok((f, _)) if f.seq.sources.len() == n => f,
        _ => {
            // Una carpeta a medias (o de otra versión) se rehace entera.
            if dir.exists() {
                std::fs::remove_dir_all(dir).map_err(|e| format!("no se pudo preparar la carpeta del ejemplo: {e}"))?;
            }
            let mut f = SeqFolder::create(dir, NOMBRE, 24)?;
            for bytes in crate::demo_sm_fotos::FOTOS.iter() {
                f.append_photo(bytes, None)?;
            }
            f
        }
    };
    let r = SmRef::new(&folder, None);
    let frames = (r.frames as u32).max(1);
    let mut p = Project::blank(1280, 720, 24, frames);
    let mut l = Layer::new("Stop Motion 1");
    l.kind = LayerKind::StopMotion;
    l.stopmotion = Some(r);
    p.layers = vec![l];
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_copies_its_photos_once_and_opens_a_stop_motion_layer() {
        let dir = std::env::temp_dir().join(format!("leontoon_muestra_sm_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = build(&dir).unwrap();
        assert_eq!(p.layers.len(), 1);
        assert_eq!(p.layers[0].kind, LayerKind::StopMotion);
        assert_eq!(p.length as usize, crate::demo_sm_fotos::FOTOS.len());
        // La segunda vez reutiliza la carpeta: no se duplican las fotos.
        let again = build(&dir).unwrap();
        assert_eq!(again.length, p.length);
        let (f, _) = SeqFolder::open(&dir).unwrap();
        assert_eq!(f.seq.sources.len(), crate::demo_sm_fotos::FOTOS.len());
        assert!(f.verify(true).is_empty(), "las fotos copiadas están intactas");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
