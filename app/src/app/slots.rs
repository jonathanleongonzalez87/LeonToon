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

    /// El selector de tipo de editor de un hueco, como el de Blender: un botón con el icono y el nombre del editor actual
    /// que abre un menú con una columna por grupo (icono y nombre de cada editor). Los editores a los que les falta
    /// contexto salen atenuados y dicen por qué al pasar el cursor; el que ya está elegido sigue activo.
    pub(super) fn slot_combo(&mut self, ui: &mut egui::Ui, slot: Slot, width: f32) {
        let room = self.room_kind();
        let cur = self.slot_editor(slot);
        let title = if cur == Editor::RoomProps { room.def().right.title() } else { cur.name() };
        let ctx = ui.ctx().clone();
        let open_id = egui::Id::new(("slot_menu", slot.id()));
        // Para capturas de pantalla: LEONTOON_SELECTOR=<hueco> abre el menú de ese hueco al arrancar (una sola vez).
        static SHOT_OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if std::env::var("LEONTOON_SELECTOR").ok().as_deref() == Some(slot.id()) && !SHOT_OPENED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            ctx.data_mut(|d| d.insert_temp(open_id, true));
        }
        let open = ctx.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
        // El botón
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::click());
        let bg = if open { BG4 } else if resp.hovered() { Color32::from_rgb(0x44, 0x44, 0x44) } else { Color32::from_rgb(0x38, 0x38, 0x38) };
        {
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 4.0, bg);
            draw_editor_icon(cur, &p, Rect::from_center_size(Pos2::new(rect.min.x + 14.0, rect.center().y), Vec2::splat(15.0)), group_tint(cur.group()));
            let galley = p.layout_no_wrap(title.to_string(), FontId::proportional(11.5), TX);
            let clip = Rect::from_min_max(Pos2::new(rect.min.x + 26.0, rect.min.y), Pos2::new(rect.max.x - 16.0, rect.max.y));
            p.with_clip_rect(clip).galley(Pos2::new(clip.min.x, rect.center().y - galley.size().y / 2.0), galley, TX);
            draw_chevron(&p, Pos2::new(rect.max.x - 9.0, rect.center().y), INK3);
        }
        let resp = resp.on_hover_text("Tipo de editor de este hueco");
        if resp.clicked() {
            ctx.data_mut(|d| d.insert_temp(open_id, !open));
        }
        if !open {
            return;
        }
        // El menú: una columna por grupo.
        let mut chosen = cur;
        let area = egui::Area::new(open_id.with("area")).order(egui::Order::Foreground).fixed_pos(rect.left_bottom() + Vec2::new(0.0, 3.0)).constrain(true).show(&ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(6, 6)).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 2.0);
                ui.horizontal_top(|ui| {
                    for g in EditorGroup::ALL {
                        let list: Vec<Editor> = Editor::ALL.into_iter().filter(|e| e.group() == g).collect();
                        if list.is_empty() {
                            continue;
                        }
                        ui.vertical(|ui| {
                            ui.set_width(196.0);
                            ui.label(RichText::new(g.name()).size(11.0).color(MU));
                            let (hl, _) = ui.allocate_exact_size(Vec2::new(188.0, 1.0), Sense::hover());
                            ui.painter().rect_filled(hl, 0.0, LINE_STRONG);
                            for e in list {
                                let name = e.name().to_string();
                                // El que ya está elegido sigue activo aunque falte contexto: así siempre se puede salir de él.
                                let reason = self.context_gap(e.needs()).map(|g| g.msg);
                                let enabled = reason.is_none() || e == cur;
                                let (row, r) = ui.allocate_exact_size(Vec2::new(188.0, 22.0), if enabled { Sense::click() } else { Sense::hover() });
                                if e == chosen {
                                    ui.painter().rect_filled(row, 4.0, ACCENT);
                                } else if r.hovered() && enabled {
                                    ui.painter().rect_filled(row, 4.0, BG4);
                                }
                                let (tint, ink) = if e == chosen { (ACCENT_INK, ACCENT_INK) } else if enabled { (group_tint(g), TX) } else { (with_alpha(group_tint(g), 0.35), INK3) };
                                draw_editor_icon(e, ui.painter(), Rect::from_center_size(Pos2::new(row.min.x + 14.0, row.center().y), Vec2::splat(15.0)), tint);
                                ui.painter().with_clip_rect(row).text(Pos2::new(row.min.x + 28.0, row.center().y), Align2::LEFT_CENTER, &name, FontId::proportional(11.5), ink);
                                if let Some(m) = &reason {
                                    r.on_hover_text(m.clone());
                                } else if r.clicked() {
                                    chosen = e;
                                }
                            }
                        });
                    }
                });
            });
        });
        // Se cierra al elegir, con Esc o con un clic fuera del menú y del botón.
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().map_or(false, |p| !area.response.rect.contains(p) && !rect.contains(p)));
        if chosen != cur || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.data_mut(|d| d.insert_temp(open_id, false));
        }
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

    #[test]
    fn todos_los_iconos_de_editor_se_dibujan_sin_fallar() {
        let ctx = egui::Context::default();
        let p = ctx.layer_painter(egui::LayerId::background());
        for e in Editor::ALL {
            draw_editor_icon(e, &p, Rect::from_min_size(Pos2::ZERO, Vec2::splat(16.0)), group_tint(e.group()));
        }
        for g in EditorGroup::ALL {
            let _ = group_tint(g);
        }
    }
}