//! Huecos de editor (fase C de docs/arquitectura-editores.md, Issue #86).
//!
//! Una sala es un **preset**: una lista de huecos (`Slot`) con un editor por defecto cada uno (`RoomKind::default_editor`).
//! Cada hueco lleva en su cabecera el selector de tipo de editor, agrupado, con los que no encajan atenuados. Lo que la
//! persona elige se recuerda por sala en `editores.json` (carpeta de datos de LeonToon); elegir otra vez el editor por
//! defecto borra la elección.

use super::shell::{area_header, in_area};
use super::*;
use std::collections::BTreeMap;

/// La clave con la que se guarda la elección de un hueco: «sala/hueco».
pub(super) fn slot_key(room: RoomKind, slot: Slot) -> String {
    format!("{}/{}", room.def().id, slot.id())
}

/// El editor elegido para un hueco en lo guardado en el proyecto. Lo que no se entiende (sala, hueco o editor desconocidos, un
/// hueco que la sala no tiene, o el editor por defecto) se ignora: un archivo roto nunca impide abrirlo.
pub(super) fn slot_choice(saved: &BTreeMap<String, String>, room: RoomKind, slot: Slot) -> Option<Editor> {
    let ed = Editor::from_id(saved.get(&slot_key(room, slot))?)?;
    (room.slots().contains(&slot) && ed != room.default_editor(slot)).then_some(ed)
}

impl LeonApp {
    /// El editor que muestra un hueco en la sala actual: el que se eligió en este proyecto o el del preset.
    pub(super) fn slot_editor(&self, slot: Slot) -> Editor {
        let room = self.room_kind();
        slot_choice(&self.project.slots, room, slot).unwrap_or_else(|| room.default_editor(slot))
    }

    /// Elige el editor de un hueco. Se guarda en el proyecto (no en todos): elegir el editor por defecto borra la elección.
    pub(super) fn set_slot_editor(&mut self, slot: Slot, ed: Editor) {
        let room = self.room_kind();
        let key = slot_key(room, slot);
        if ed == room.default_editor(slot) {
            self.project.slots.remove(&key);
        } else {
            self.project.slots.insert(key, ed.id().to_string());
        }
        self.dirty = true;
    }

    /// Vuelve la sala actual a su preset de fábrica: se borran los editores que se eligieron en sus huecos.
    pub(super) fn reset_room_slots(&mut self) {
        let prefix = format!("{}/", self.room_kind().def().id);
        let before = self.project.slots.len();
        self.project.slots.retain(|k, _| !k.starts_with(&prefix));
        if self.project.slots.len() != before {
            self.dirty = true;
        }
        self.say(format!("Sala {} restablecida: sus huecos vuelven a los editores de fábrica", self.tab_name(self.room_idx)));
    }
    /// ¿La cabecera de herramienta del visor hace de cabecera del hueco principal? (Sala con visor.)
    pub(super) fn main_toolhead(&self) -> bool {
        let room = self.room_kind();
        if room.def().story {
            // La cuadrícula (se dibuja sobre las miniaturas) y el plano abierto traen la cabecera y la barra de herramientas.
            return self.slot_editor(Slot::Main) == Editor::Storyboard;
        }
        room == RoomKind::StopMotion || self.slot_editor(Slot::Main) == Editor::Viewer
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
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, UiScale::SELECTOR_H), Sense::click());
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
        let mut reset = false;
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
                ui.add_space(4.0);
                if ui.small_button("Restablecer los editores de esta sala").on_hover_text("Los huecos de la sala vuelven a los editores de fábrica").clicked() {
                    reset = true;
                }
            });
        });
        // Se cierra al elegir, con Esc o con un clic fuera del menú y del botón.
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().map_or(false, |p| !area.response.rect.contains(p) && !rect.contains(p)));
        if chosen != cur || reset || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.data_mut(|d| d.insert_temp(open_id, false));
        }
        if reset {
            self.reset_room_slots();
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
        let w = if matches!(self.slot_editor(slot), Editor::Timeline | Editor::SmStrip) { (hr.width() * 0.38).clamp(110.0, 170.0) } else { (hr.width() - 8.0).clamp(110.0, 220.0) };
        self.slot_combo(&mut child, slot, w);
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
                Editor::Storyboard if self.panel_canvas() => self.hover_area = 1,
                Editor::Timeline | Editor::Montage | Editor::SmStrip | Editor::SmXSheet => self.hover_area = 2,
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
        // Las salas de motion graphics traen la línea de tiempo de montaje; las demás, la de siempre.
        assert_eq!(RoomKind::Interp.default_editor(Slot::Time), Editor::Montage);
        assert_eq!(RoomKind::Comp.default_editor(Slot::Time), Editor::Montage);
        assert_eq!(RoomKind::Layout.default_editor(Slot::Time), Editor::Timeline);
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
    fn lo_elegido_viaja_con_el_proyecto_y_se_lee_igual() {
        let mut p = crate::model::Project::blank(1280, 720, 24, 24);
        p.slots.insert(slot_key(RoomKind::StopMotion, Slot::SmRight), Editor::Preview.id().into());
        p.slots.insert(slot_key(RoomKind::Layout, Slot::Time), Editor::Curves.id().into());
        let back: crate::model::Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(slot_choice(&back.slots, RoomKind::StopMotion, Slot::SmRight), Some(Editor::Preview));
        assert_eq!(slot_choice(&back.slots, RoomKind::Layout, Slot::Time), Some(Editor::Curves));
        // Un proyecto nuevo no trae ninguna elección: arranca con los presets de fábrica.
        assert!(crate::model::Project::blank(1280, 720, 24, 24).slots.is_empty());
    }

    #[test]
    fn una_eleccion_rota_o_con_cosas_desconocidas_se_ignora() {
        let mut m = BTreeMap::new();
        for (k, v) in [
            ("stopmotion/sm_right", "preview"),
            ("sala_inventada/main", "viewer"),
            ("layout/hueco_inventado", "viewer"),
            ("layout/main", "editor_inventado"),
            ("layout/sm_left", "preview"),
            ("layout/time", "timeline"),
            ("sin_barra", "viewer"),
        ] {
            m.insert(k.to_string(), v.to_string());
        }
        // Solo vale la primera: el hueco sm_left no existe en Layout y time con timeline es el valor por defecto.
        let valid: Vec<_> = RoomKind::ALL.into_iter().flat_map(|r| Slot::ALL.into_iter().map(move |s| (r, s))).filter(|(r, s)| slot_choice(&m, *r, *s).is_some()).collect();
        assert_eq!(valid, vec![(RoomKind::StopMotion, Slot::SmRight)]);
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