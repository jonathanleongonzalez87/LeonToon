//! Salas sin visor: Esquema (columnas → salida), Explorador (resumen y escena) y Render (exportar).

use super::shell::{muted, pill};
use super::*;

fn heading(ui: &mut egui::Ui, t: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(t).strong().size(15.0).color(TX));
}

impl LeonApp {
    fn export_mp4(&mut self, a: u32, b: u32, scale: f32) {
        if self.ffmpeg.as_ref().map_or(true, |p| !p.is_file()) {
            self.ffmpeg = render::find_ffmpeg();
        }
        if self.ffmpeg.is_none() {
            self.ffmpeg = rfd::FileDialog::new()
                .set_title("No encuentro ffmpeg: indica dónde está ffmpeg.exe")
                .add_filter("ffmpeg", &["exe"])
                .pick_file();
        }
        let Some(ff) = self.ffmpeg.clone() else {
            self.render_opts.message = "Para exportar vídeo hace falta ffmpeg (gratuito, ffmpeg.org).".into();
            return;
        };
        if let Some(p) = rfd::FileDialog::new().add_filter("Vídeo MP4", &["mp4"]).set_file_name("mi_animacion.mp4").save_file() {
            // Se exporta la composición principal (aunque se esté dentro de otra), con la mezcla de sus audios si los tiene.
            let root = self.project.root_clone();
            let b = b.min(root.length.saturating_sub(1));
            let wav = std::env::temp_dir().join(format!("leontoon_audio_{}.wav", std::process::id()));
            let audio = match self.audio_mixdown_wav(&root, a, b, &wav) {
                Some(Ok(())) => Some(wav.clone()),
                Some(Err(e)) => {
                    self.render_opts.message = format!("Error de audio: {e}");
                    return;
                }
                None => None,
            };
            self.render_opts.message = match render::export_video(&root, &ff, &p, a, b, scale, audio.as_deref()) {
                Ok(n) => format!("Terminado: vídeo de {n} fotogramas{} en {}", if audio.is_some() { " con sonido" } else { "" }, p.display()),
                Err(e) => format!("Error: {e}"),
            };
            let _ = std::fs::remove_file(&wav);
            let m = self.render_opts.message.clone();
            self.say(m);
        }
    }

    pub(super) fn schematic_room(&mut self, ui: &mut egui::Ui) {
        egui::Frame::default().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            muted(ui, "Esquema de columnas (solo lectura). Cada columna se combina, de abajo arriba, hasta la SALIDA. El orden se cambia arrastrando las cabeceras del Xsheet.");
            let n = self.project.layers.len();
            let hgt = 60.0 + n as f32 * 54.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(720.0), hgt), Sense::hover());
            let p = ui.painter_at(rect);
            let out = Rect::from_min_size(Pos2::new(rect.min.x + 560.0, rect.center().y - 22.0), Vec2::new(130.0, 44.0));
            let mut pick = None;
            for (i, l) in self.project.layers.iter().enumerate().rev() {
                let row = n - 1 - i;
                let r = Rect::from_min_size(Pos2::new(rect.min.x + 40.0, rect.min.y + 20.0 + row as f32 * 54.0), Vec2::new(230.0, 40.0));
                let a = r.right_center();
                let b = out.left_center();
                let pts: Vec<Pos2> = (0..=24)
                    .map(|k| {
                        let t = k as f32 / 24.0;
                        let c1 = Pos2::new(a.x + 150.0, a.y);
                        let c2 = Pos2::new(b.x - 140.0, b.y);
                        let u = 1.0 - t;
                        Pos2::new(
                            u * u * u * a.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * b.x,
                            u * u * u * a.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * b.y,
                        )
                    })
                    .collect();
                p.add(Shape::line(pts, stk(1.5, Color32::from_gray(0x88))));
                let on = i == self.cl;
                p.rect_filled(r, 6.0, BG3);
                p.rect_stroke(r, 6.0, if on { stk(2.0, ACCENT) } else { stk(1.0, Color32::BLACK) }, EStrokeKind::Inside);
                let extra = if l.tracks.is_empty() { String::new() } else { format!(" · {} claves", l.tracks.count()) };
                let col = if l.visible { TX } else { MU };
                p.text(r.left_center() + Vec2::new(12.0, 0.0), Align2::LEFT_CENTER, format!("{}  ·  {} dibujos{}", l.name, l.keys.len(), extra), FontId::proportional(12.0), col);
                let resp = ui.interact(r, ui.id().with(("node", i)), Sense::click());
                if resp.on_hover_text("Clic: elegir esta columna").clicked() {
                    pick = Some(i);
                }
            }
            p.rect_filled(out, 6.0, SEL_BG);
            p.rect_stroke(out, 6.0, stk(1.0, Color32::BLACK), EStrokeKind::Inside);
            p.text(out.center(), Align2::CENTER_CENTER, "SALIDA", FontId::proportional(12.5), TX);
            if let Some(i) = pick {
                self.cl = i;
            }
        });
    }

    pub(super) fn browser_room(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::default().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                ui.set_max_width(760.0);
                heading(ui, "Proyecto");
                let pr = &self.project;
                ui.label(format!(
                    "{} · {}×{} · {} fps · {} fotogramas ({:.1} s) · {} columnas · archivo: {}",
                    self.file_name(),
                    pr.width,
                    pr.height,
                    pr.fps,
                    pr.length,
                    pr.length as f32 / pr.fps.max(1) as f32,
                    pr.layers.len(),
                    self.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "sin guardar".into())
                ));
                ui.horizontal(|ui| {
                    if pill(ui, "Nuevo proyecto…", false).clicked() {
                        let ctx = ui.ctx().clone();
                        self.request(&ctx, Pending::New);
                    }
                    if pill(ui, "Abrir…", false).clicked() {
                        let ctx = ui.ctx().clone();
                        self.request(&ctx, Pending::Open);
                    }
                    if pill(ui, "Guardar", false).clicked() {
                        self.save();
                    }
                });
                muted(ui, "También puedes arrastrar un archivo .ltoon sobre la ventana para abrirlo.");

                heading(ui, "Escena");
                egui::Grid::new("scene_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label(RichText::new("Tamaño").color(MU));
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.project.width).range(16..=4096).suffix(" px"));
                        ui.label("×");
                        ui.add(egui::DragValue::new(&mut self.project.height).range(16..=4096).suffix(" px"));
                    });
                    ui.end_row();
                    ui.label(RichText::new("Velocidad").color(MU));
                    ui.add(egui::DragValue::new(&mut self.project.fps).range(1..=120).suffix(" fps"));
                    ui.end_row();
                    ui.label(RichText::new("Duración").color(MU));
                    ui.add(egui::DragValue::new(&mut self.project.length).range(1..=100_000).suffix(" fotogramas"));
                    ui.end_row();
                    ui.label(RichText::new("Fondo").color(MU));
                    let mut bg = c32(self.project.background);
                    if egui::color_picker::color_edit_button_srgba(ui, &mut bg, egui::color_picker::Alpha::Opaque).changed() {
                        self.project.background = rgba(bg);
                    }
                    ui.end_row();
                });

                heading(ui, "Columnas (capas)");
                for l in &self.project.layers {
                    ui.label(format!(
                        "• {} — {} dibujos{}{}{}",
                        l.name,
                        l.keys.len(),
                        if l.tracks.is_empty() { String::new() } else { format!(" · {} claves de animación", l.tracks.count()) },
                        if l.visible { "" } else { " · oculta" },
                        if l.locked { " · bloqueada" } else { "" }
                    ));
                }
                heading(ui, "StoryBoard");
                let total: f32 = self.project.storyboard.iter().map(|p| p.dur).sum();
                ui.label(format!("{} planos · {:.1} s", self.project.storyboard.len(), total));
                heading(ui, "Seguridad de tus datos");
                muted(ui, "Al guardar, el archivo se comprueba volviéndolo a leer antes de dar el guardado por bueno, y la versión anterior se conserva (Archivo > Volver a la versión anterior). Cada 30 segundos se hace una copia automática que se ofrece recuperar tras un cierre inesperado.");
            });
        });
    }

    pub(super) fn render_room(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::default().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                ui.set_max_width(760.0);
                let n = self.project.length;
                let o = &mut self.render_opts;
                o.b = o.b.clamp(1, n);
                o.a = o.a.clamp(1, o.b);
                heading(ui, "Rango y calidad");
                egui::Grid::new("render_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label(RichText::new("Desde").color(MU));
                    ui.add(egui::DragValue::new(&mut o.a).range(1..=n));
                    ui.end_row();
                    ui.label(RichText::new("Hasta").color(MU));
                    ui.add(egui::DragValue::new(&mut o.b).range(1..=n));
                    ui.end_row();
                    ui.label(RichText::new("Escala").color(MU));
                    ui.add(egui::DragValue::new(&mut o.scale).range(0.1..=4.0).speed(0.01).max_decimals(2));
                    ui.end_row();
                    ui.label(RichText::new("Columnas (sprites)").color(MU));
                    ui.add(egui::DragValue::new(&mut o.columns).range(1..=64));
                    ui.end_row();
                });
                ui.checkbox(&mut o.transparent, "Fondo transparente (PNG)");
                ui.horizontal(|ui| {
                    if pill(ui, "Usar el rango de reproducción", false).clicked() {
                        if let Some((a, b)) = self.project.range {
                            self.render_opts.a = a + 1;
                            self.render_opts.b = b + 1;
                        }
                    }
                    if pill(ui, "Toda la escena", false).clicked() {
                        self.render_opts.a = 1;
                        self.render_opts.b = n;
                    }
                });
                muted(ui, "El rango de reproducción se marca en la barra de tiempo (doble clic abajo, o Alt+I / Alt+O).");

                heading(ui, "Exportar");
                let (a, b, scale, tr, cols) = {
                    let o = &self.render_opts;
                    (o.a - 1, o.b - 1, o.scale, o.transparent, o.columns)
                };
                ui.horizontal_wrapped(|ui| {
                    if pill(ui, "PNG del fotograma actual", false).clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("Imagen PNG", &["png"])
                            .set_file_name(format!("fotograma_{:04}.png", self.frame + 1))
                            .save_file()
                        {
                            self.render_opts.message = match render::export_png_opts(&self.project, self.frame, scale, tr, &p) {
                                Ok(()) => format!("Terminado: {}", p.display()),
                                Err(e) => format!("Error: {e}"),
                            };
                        }
                    }
                    if pill(ui, "Secuencia PNG (carpeta)", false).clicked() {
                        if let Some(dir) = rfd::FileDialog::new().set_title("Elige una carpeta para los fotogramas").pick_folder() {
                            self.render_opts.message = match render::export_sequence_range(&self.project, &dir, a, b, scale, tr) {
                                Ok(k) => format!("Terminado: {k} imágenes en {}", dir.display()),
                                Err(e) => format!("Error: {e}"),
                            };
                        }
                    }
                    if pill(ui, "Hoja de sprites (.png + .json)", false).clicked() {
                        if let Some(p) = rfd::FileDialog::new().add_filter("Imagen PNG", &["png"]).set_file_name("sprites.png").save_file() {
                            self.render_opts.message = match render::export_sprite_sheet(&self.project, &p, a, b, scale, cols, tr) {
                                Ok(k) => format!("Terminado: {k} fotogramas en {}", p.display()),
                                Err(e) => format!("Error: {e}"),
                            };
                        }
                    }
                    if pill(ui, "Storyboard a PDF", false).on_hover_text("Hojas A4 con la miniatura de cada plano y sus textos").clicked() {
                        self.export_storyboard_pdf();
                    }
                    if pill(ui, "GIF animado", false).clicked() {
                        if let Some(p) = rfd::FileDialog::new().add_filter("GIF animado", &["gif"]).set_file_name("mi_animacion.gif").save_file() {
                            let maxw = (self.project.width as f32 * scale).round() as u32;
                            self.render_opts.message = match render::export_gif_range(&self.project, &p, a, b, maxw.min(1920)) {
                                Ok(()) => format!("Terminado: {}", p.display()),
                                Err(e) => format!("Error: {e}"),
                            };
                        }
                    }
                    if pill(ui, "Vídeo MP4", false).on_hover_text("Necesita ffmpeg (gratuito); se busca solo en tu equipo").clicked() {
                        self.export_mp4(a, b, scale);
                    }
                    ui.add_enabled(false, egui::Button::new("StoryBoard PDF (pendiente)"));
                });
                ui.add_space(6.0);
                let msg = self.render_opts.message.clone();
                ui.label(RichText::new(msg).color(if self.render_opts.message.starts_with("Error") { ONION_BEFORE } else { MU }));
            });
        });
    }
}
