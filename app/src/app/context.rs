//! Contexto de los editores (fase D de docs/arquitectura-editores.md, Issue #83).
//!
//! Cada editor declara qué necesita del contexto (una capa Stop Motion con secuencia, una capa de dibujo, claves de
//! movimiento…). Si no se cumple, el editor no se queda en blanco ni falla: explica qué falta y ofrece el botón para
//! arreglarlo; y el menú de tipo de editor atenúa los que no encajan y dice por qué.
//!
//! La decisión (`gap_of`) es una función pura sobre unos pocos datos del proyecto, para poder probarla sin ventana.

use super::smui::SmState;
use super::*;

/// Qué necesita un editor del contexto para tener algo que mostrar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Need {
    Nothing,
    /// Una capa Stop Motion (con o sin secuencia).
    StopMotionLayer,
    /// Una capa Stop Motion con su secuencia abierta.
    StopMotionSequence,
    /// La capa actual es de dibujo (vectorial o mapa de bits).
    DrawingLayer,
    /// La capa actual tiene claves de movimiento.
    TransformKeys,
}

impl Editor {
    pub(crate) fn needs(self) -> Need {
        match self {
            Editor::SmXSheet => Need::StopMotionSequence,
            Editor::Camera => Need::StopMotionLayer,
            Editor::Substitution => Need::DrawingLayer,
            Editor::Curves => Need::TransformKeys,
            _ => Need::Nothing,
        }
    }
}

/// Lo que se puede hacer desde el aviso para que el editor tenga contexto.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum Fix {
    AddStopMotionLayer,
    CreateSequence(usize),
    OpenSequence(usize),
    Relocate(usize),
    AddVectorLayer,
    GoToLayer(usize),
    AutoKey,
}

/// Lo que falta en el contexto: el mensaje y los botones que lo arreglan.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct Gap {
    pub msg: String,
    pub fixes: Vec<(String, Fix)>,
}

/// Datos de una capa que importan para decidir.
pub(super) struct LayerFact {
    pub name: String,
    pub kind: LayerKind,
    pub has_seq: bool,
    pub has_keys: bool,
}

/// Todo lo que hace falta saber del proyecto para decidir qué falta.
pub(super) struct Facts {
    pub layers: Vec<LayerFact>,
    pub current: usize,
    /// La capa Stop Motion con la que trabaja la sala (la actual si lo es, o la primera).
    pub sm_layer: Option<usize>,
    /// Aviso si la carpeta de la secuencia de esa capa no se encuentra.
    pub missing: Option<String>,
}

/// Qué le falta al contexto para cumplir `need` (`None` = no falta nada).
pub(super) fn gap_of(need: Need, f: &Facts) -> Option<Gap> {
    let go = |label: &str, fix: Fix| (label.to_string(), fix);
    match need {
        Need::Nothing => None,
        Need::StopMotionLayer | Need::StopMotionSequence => {
            let Some(li) = f.sm_layer else {
                return Some(Gap { msg: "Este editor trabaja con una capa Stop Motion y el proyecto todavía no tiene ninguna.".into(), fixes: vec![go("Agregar una capa Stop Motion", Fix::AddStopMotionLayer)] });
            };
            if need == Need::StopMotionLayer {
                return None;
            }
            let l = &f.layers[li];
            if !l.has_seq {
                Some(Gap { msg: format!("La capa «{}» todavía no tiene secuencia. Elige la carpeta donde se guardarán las fotos.", l.name), fixes: vec![go("Crear una secuencia nueva…", Fix::CreateSequence(li)), go("Usar una secuencia que ya existe…", Fix::OpenSequence(li))] })
            } else {
                f.missing.as_ref().map(|m| Gap { msg: m.clone(), fixes: vec![go("Buscar la carpeta…", Fix::Relocate(li))] })
            }
        }
        Need::DrawingLayer => {
            let cur = f.layers.get(f.current)?;
            if matches!(cur.kind, LayerKind::Vector | LayerKind::Bitmap) {
                return None;
            }
            let msg = format!("Este editor trabaja con capas de dibujo (vectorial o mapa de bits) y la capa «{}» es de tipo {}.", cur.name, cur.kind.label());
            let fix = match f.layers.iter().position(|l| matches!(l.kind, LayerKind::Vector | LayerKind::Bitmap)) {
                Some(i) => go(&format!("Ir a la capa «{}»", f.layers[i].name), Fix::GoToLayer(i)),
                None => go("Agregar una capa vectorial", Fix::AddVectorLayer),
            };
            Some(Gap { msg, fixes: vec![fix] })
        }
        Need::TransformKeys => {
            let cur = f.layers.get(f.current)?;
            (!cur.has_keys).then(|| Gap {
                msg: format!("La capa «{}» no tiene claves de movimiento. Con la animación automática activada, mover, girar o escalar la capa en modo objeto las crea.", cur.name),
                fixes: vec![go("Activar animación automática", Fix::AutoKey)],
            })
        }
    }
}

impl LeonApp {
    fn context_facts(&mut self, need: Need) -> Facts {
        let layers = self.project.layers.iter().map(|l| LayerFact { name: l.name.clone(), kind: l.kind, has_seq: l.stopmotion.is_some(), has_keys: !l.xform.is_empty() }).collect();
        let sm_layer = self.sm_room_layer();
        let missing = if need == Need::StopMotionSequence {
            sm_layer.and_then(|li| self.project.layers[li].stopmotion.clone()).and_then(|r| match self.sm_state(&r) {
                SmState::Missing(m) => Some(m),
                SmState::Ready => None,
            })
        } else {
            None
        };
        Facts { layers, current: self.cl, sm_layer, missing }
    }

    /// Qué le falta al contexto actual para el requisito `need`.
    pub(super) fn context_gap(&mut self, need: Need) -> Option<Gap> {
        if need == Need::Nothing {
            return None;
        }
        let facts = self.context_facts(need);
        gap_of(need, &facts)
    }

    fn apply_fix(&mut self, fix: Fix) {
        match fix {
            Fix::AddStopMotionLayer => self.add_layer_kind(LayerKind::StopMotion),
            Fix::CreateSequence(li) => {
                self.cl = li;
                self.sm_create();
            }
            Fix::OpenSequence(li) => {
                self.cl = li;
                self.sm_open_existing();
            }
            Fix::Relocate(li) => {
                if let Some(r) = self.project.layers.get(li).and_then(|l| l.stopmotion.clone()) {
                    self.cl = li;
                    self.sm_relocate(&r);
                }
            }
            Fix::AddVectorLayer => self.add_layer(),
            Fix::GoToLayer(i) => self.cl = i.min(self.project.layers.len().saturating_sub(1)),
            Fix::AutoKey => {
                if !self.obj_mode {
                    self.toggle_mode();
                }
                self.tool = Tool::Select;
            }
        }
    }

    /// Pinta el aviso de lo que falta con sus botones y aplica el que se pulse.
    pub(super) fn gap_ui(&mut self, ui: &mut egui::Ui, gap: Gap) {
        let mut chosen = None;
        ui.add_space(10.0);
        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(&gap.msg).size(12.0).color(MU));
            ui.add_space(4.0);
            for (label, fix) in &gap.fixes {
                if ui.button(label).clicked() {
                    chosen = Some(fix.clone());
                }
            }
        });
        if let Some(f) = chosen {
            self.apply_fix(f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(name: &str, kind: LayerKind, has_seq: bool, has_keys: bool) -> LayerFact {
        LayerFact { name: name.into(), kind, has_seq, has_keys }
    }
    fn facts(layers: Vec<LayerFact>, current: usize, missing: Option<&str>) -> Facts {
        let sm_layer = if layers[current].kind == LayerKind::StopMotion { Some(current) } else { layers.iter().position(|l| l.kind == LayerKind::StopMotion) };
        Facts { layers, current, sm_layer, missing: missing.map(String::from) }
    }

    #[test]
    fn sin_capa_stop_motion_ofrece_agregarla() {
        let f = facts(vec![layer("Dibujo", LayerKind::Vector, false, false)], 0, None);
        let g = gap_of(Need::StopMotionSequence, &f).unwrap();
        assert_eq!(g.fixes[0].1, Fix::AddStopMotionLayer);
        assert!(gap_of(Need::StopMotionLayer, &f).is_some());
    }

    #[test]
    fn capa_stop_motion_sin_secuencia_ofrece_crearla_o_abrirla() {
        let f = facts(vec![layer("Dibujo", LayerKind::Vector, false, false), layer("Fotos", LayerKind::StopMotion, false, false)], 0, None);
        // La capa de la sala es la primera Stop Motion aunque la actual sea otra.
        let g = gap_of(Need::StopMotionSequence, &f).unwrap();
        assert!(g.msg.contains("Fotos"));
        assert_eq!(g.fixes.iter().map(|x| x.1.clone()).collect::<Vec<_>>(), vec![Fix::CreateSequence(1), Fix::OpenSequence(1)]);
        assert!(gap_of(Need::StopMotionLayer, &f).is_none(), "para la cámara basta con la capa");
    }

    #[test]
    fn carpeta_que_falta_ofrece_buscarla_y_con_todo_bien_no_falta_nada() {
        let f = facts(vec![layer("Fotos", LayerKind::StopMotion, true, false)], 0, Some("No se encuentra la carpeta"));
        assert_eq!(gap_of(Need::StopMotionSequence, &f).unwrap().fixes[0].1, Fix::Relocate(0));
        let ok = facts(vec![layer("Fotos", LayerKind::StopMotion, true, false)], 0, None);
        assert!(gap_of(Need::StopMotionSequence, &ok).is_none());
        assert!(gap_of(Need::Nothing, &ok).is_none());
    }

    #[test]
    fn capa_de_dibujo_se_pide_solo_si_la_actual_no_lo_es() {
        let f = facts(vec![layer("Dibujo", LayerKind::Vector, false, false), layer("Luz", LayerKind::Light, false, false)], 1, None);
        let g = gap_of(Need::DrawingLayer, &f).unwrap();
        assert!(g.msg.contains("Luz") && g.msg.contains("Luz"));
        assert_eq!(g.fixes[0].1, Fix::GoToLayer(0));
        let only = facts(vec![layer("Luz", LayerKind::Light, false, false)], 0, None);
        assert_eq!(gap_of(Need::DrawingLayer, &only).unwrap().fixes[0].1, Fix::AddVectorLayer);
        let drawing = facts(vec![layer("Dibujo", LayerKind::Bitmap, false, false)], 0, None);
        assert!(gap_of(Need::DrawingLayer, &drawing).is_none());
    }

    #[test]
    fn las_curvas_piden_claves_y_ofrecen_la_animacion_automatica() {
        let f = facts(vec![layer("Pelota", LayerKind::Vector, false, false)], 0, None);
        assert_eq!(gap_of(Need::TransformKeys, &f).unwrap().fixes[0].1, Fix::AutoKey);
        let keyed = facts(vec![layer("Pelota", LayerKind::Vector, false, true)], 0, None);
        assert!(gap_of(Need::TransformKeys, &keyed).is_none());
    }

    #[test]
    fn cada_editor_declara_un_requisito_coherente() {
        for e in Editor::ALL {
            // Los que dependen de Stop Motion no pueden pedir otra cosa a la vez.
            if matches!(e, Editor::SmXSheet | Editor::Camera) {
                assert!(matches!(e.needs(), Need::StopMotionSequence | Need::StopMotionLayer));
            }
        }
        assert_eq!(Editor::Layers.needs(), Need::Nothing);
    }
}
