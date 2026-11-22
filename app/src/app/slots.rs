//! Huecos de editor (fase C de docs/arquitectura-editores.md, Issue #86).
//!
//! Una sala es un **preset**: una lista de huecos (`Slot`) con un editor por defecto cada uno (`RoomKind::default_editor`).
//! Cada hueco lleva en su cabecera el selector de tipo de editor, agrupado, con los que no encajan atenuados. Lo que la
//! persona elige se recuerda por sala en `editores.json` (carpeta de datos de LeonToon); elegir otra vez el editor por
//! defecto borra la elección.

use super::shell::{area_header, in_area};
use super::*;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

const FILE: &str = "editores.json";

/// Convierte el texto guardado en elecciones válidas. Lo que no se entiende (sala, hueco o editor desconocidos, un hueco
/// que la sala no tiene, o el editor por defecto) se ignora: un archivo roto nunca impide arrancar.
pub(super) fn parse(text: &str) -> HashMap<(RoomKind, Slot), Editor> {
    let raw: BTreeMap<String, String> = serde_json::from_str(text).unwrap_or_default();
    let mut out = HashMap::new();
    for (k, v) in raw {
        let Some((room, slot)) = k.split_once('/') else { continue };
        let (Some(room), Some(slot), Some(ed)) = (RoomKind::from_id(room), Slot::from_id(slot), Editor::from_id(&v)) else { continue };
        if room.slots().contains(&slot) && ed != room.default_editor(slot) {
            out.insert((room, slot), ed);
        }
    }
    out
}

pub(super) fn serialize(map: &HashMap<(RoomKind, Slot), Editor>) -> String {
    let sorted: BTreeMap<String, &str> = map.iter().map(|((r, s), e)| (format!("{}/{}", r.def().id, s.id()), e.id())).collect();
    serde_json::to_string_pretty(&sorted).unwrap_or_else(|_| "{}".into())
}

pub(super) fn load(dir: &Path) -> HashMap<(RoomKind, Slot), Editor> {
    std::fs::read_to_string(dir.join(FILE)).map(|s| parse(&s)).unwrap_or_default()
}

fn save(dir: &Path, map: &HashMap<(RoomKind, Slot), Editor>) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, serialize(map)).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join(FILE)).map_err(|e| e.to_string())
}

impl LeonApp {
    /// El editor que muestra un hueco en la sala actual: el elegido o el del preset.
    pub(super) fn slot_editor(&self, slot: Slot) -> Editor {
        let room = self.room_kind();
        self.slot_editors.get(&(room, slot)).copied().unwrap_or_else(|| room.default_editor(slot))
    }

    pub(super) fn set_slot_editor(&mut self, slot: Slot, ed: Editor) {
        let room = self.room_kind();
        if ed == room.default_editor(slot) {
            self.slot_editors.remove(&(room, slot));
        } else {
            self.slot_editors.insert((room, slot), ed);
        }
        if self.shot.is_some() {
            return;
        }
        if let Some(dir) = safety::data_dir() {
            if let Err(e) = save(&dir, &self.slot_editors) {
                safety::log_line(&format!("No se pudieron guardar los editores elegidos: {e}"));
            }
        }
    }

    /// ¿La cabecera de herramienta del visor hace de cabecera del hueco principal? (Sala con visor.)
    pub(super) fn main_toolhead(&self) -> bool {
        let room = self.room_kind();
        !room.def().story && (room == RoomKind::StopMotion || self.slot_editor(Slot::Main) == Editor::Viewer)
    }

    /// El selector de tipo de editor de un hueco: menú agrupado; los editores a los que les falta contexto salen
    /// atenuados y dicen por qué al pasar el cursor.
    pub(super) fn slot_combo(&mut self, ui: &mut egui::Ui, slot: Slot, width: f32) {
        let room = self.room_kind();
        let cur = self.slot_editor(slot);
        let title = if cur == Editor::RoomProps { room.def().right.title() } else { cur.name() };
        let mut chosen = cur;
        egui::ComboBox::from_id_salt(("slot_editor", slot.id())).selected_text(RichText::new(title).color(TX)).width(width).show_ui(ui, |ui| {
            for g in EditorGroup::ALL {
                let list: Vec<Editor> = Editor::ALL.into_iter().filter(|e| e.group() == g).collect();
                if list.is_empty() {
                    continue;
                }
                ui.label(RichText::new(g.name().to_uppercase()).size(10.0).color(INK3));
                for e in list {
                    let n = if e == Editor::RoomProps { format!("Propiedades de la sala ({})", room.def().right.title()) } else { e.name().to_string() };
                    // El que ya está elegido sigue activo aunque falte contexto: así siempre se puede salir de él.
                    let reason = self.context_gap(e.needs()).map(|g| g.msg);
                    let r = ui.add_enabled(reason.is_none() || e == cur, egui::SelectableLabel::new(chosen == e, n));
                    let r = match &reason {
                        Some(m) => r.on_hover_text(m.clone()).on_disabled_hover_text(m.clone()),
                        None => r,
                    };
                    if r.clicked() {
                        chosen = e;
                    }
                }
            }
        });
        if chosen != cur {
            self.set_slot_editor(slot, chosen);
        }
    }

    /// Cabecera de un hueco: el fondo, el selector de editor a la izquierda y, a su derecha, el espacio que le queda al
    /// editor para sus propios controles (se devuelve ese rectángulo).
    pub(super) fn slot_head(&mut self, ui: &mut egui::Ui) -> Rect {
        let hr = area_header(ui, "");
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(hr.shrink2(Vec2::new(4.0, 2.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let slot = self.cur_slot;
        // En cabeceras estrechas (la hoja de exposición al lado del visor) el selector cede sitio a los controles del editor.
        self.slot_combo(&mut child, slot, (hr.width() * 0.38).clamp(110.0, 170.0));
        let x = (child.min_rect().max.x + 6.0).min(hr.max.x - 6.0);
        Rect::from_min_max(Pos2::new(x, hr.min.y + 2.0), Pos2::new(hr.max.x - 6.0, hr.max.y - 2.0))
    }

    /// Dibuja un hueco: su cabecera con el selector y el editor que toque. Si al contexto le falta algo que el editor
    /// necesita, se explica y se ofrece arreglarlo, y la cabecera siempre está para poder cambiar de editor.
    pub(super) fn draw_slot(&mut self, ui: &mut egui::Ui, rect: Rect, slot: Slot) {
        let ed = self.slot_editor(slot);
        let right = self.room_kind().def().right;
        if let Some(p) = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p)) {
            let _ = p;
            match ed {
                Editor::Viewer | Editor::SmStage => self.hover_area = 1,
                Editor::Timeline | Editor::SmStrip | Editor::SmXSheet => self.hover_area = 2,
                _ => {}
            }
        }
        in_area(ui, rect, |ui| {
            self.cur_slot = slot;
            if let Some(gap) = self.context_gap(ed.needs()) {
                self.slot_head(ui);
                self.gap_ui(ui, gap);
                return;
            }
            if ed.own_header() {
                self.draw_editor(ui, ed, right);
                return;
            }
            if !(slot == Slot::Main && ed == Editor::Viewer && self.main_toolhead()) {
                self.slot_head(ui);
            }
            if ed.scrolls_itself() {
                self.draw_editor(ui, ed, right);
            } else {
                egui::ScrollArea::vertical().id_salt(("slot", slot.id(), ed.id())).auto_shrink([false, false]).show(ui, |ui| {
                    egui::Frame::default().inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 6.0;
                        self.draw_editor(ui, ed, right);
                    });
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_sala_tiene_editor_por_defecto_en_todos_sus_huecos_y_el_principal_es_visor_o_storyboard() {
        for room in RoomKind::ALL {
            assert!(!room.slots().is_empty());
            // No hay huecos repetidos.
            let mut seen = std::collections::HashSet::new();
            assert!(room.slots().iter().all(|s| seen.insert(*s)), "hueco repetido en {}", room.def().name);
            // Las salas con visor o storyboard en el centro son las que tienen hueco principal.
            if room.slots().contains(&Slot::Main) {
                let d = room.default_editor(Slot::Main);
                assert!(d == Editor::Viewer || d == Editor::Storyboard);
                assert_eq!(d == Editor::Storyboard, room.def().story);
            }
            assert!(room.slots().contains(&Slot::Layers) && room.slots().contains(&Slot::Props));
        }
        assert_eq!(RoomKind::StopMotion.default_editor(Slot::SmStage), Editor::SmStage);
        assert!(!RoomKind::StopMotion.slots().contains(&Slot::Main));
    }

    #[test]
    fn los_identificadores_son_unicos_y_se_pueden_leer_de_vuelta() {
        for e in Editor::ALL {
            assert_eq!(Editor::from_id(e.id()), Some(e));
        }
        for s in Slot::ALL {
            assert_eq!(Slot::from_id(s.id()), Some(s));
        }
        let ids: std::collections::HashSet<&str> = Editor::ALL.iter().map(|e| e.id()).collect();
        assert_eq!(ids.len(), Editor::ALL.len());
    }

    #[test]
    fn lo_elegido_se_guarda_y_se_lee_igual() {
        let mut m = HashMap::new();
        m.insert((RoomKind::StopMotion, Slot::SmRight), Editor::Preview);
        m.insert((RoomKind::Layout, Slot::Time), Editor::Curves);
        let back = parse(&serialize(&m));
        assert_eq!(back, m);
    }

    #[test]
    fn un_archivo_roto_o_con_cosas_desconocidas_nunca_falla() {
        assert!(parse("esto no es json").is_empty());
        assert!(parse("[1,2,3]").is_empty());
        let t = r#"{
            "stopmotion/sm_right": "preview",
            "sala_inventada/main": "viewer",
            "layout/hueco_inventado": "viewer",
            "layout/main": "editor_inventado",
            "layout/sm_left": "preview",
            "layout/time": "timeline",
            "sin_barra": "viewer"
        }"#;
        let m = parse(t);
        // Solo vale la primera: el hueco sm_left no existe en Layout y time con timeline es el valor por defecto.
        assert_eq!(m.len(), 1);
        assert_eq!(m[&(RoomKind::StopMotion, Slot::SmRight)], Editor::Preview);
    }
}
