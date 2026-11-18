#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod nucleo;

use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

fn main() -> eframe::Result {
    // Modo consola: armador-pdf --salida final.pdf nota.docx proyecto.pdf
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--salida") && args.len() >= 3 {
        let entradas: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
        match nucleo::armar(&entradas, Path::new(&args[1]), |m| println!("{m}")) {
            Ok(n) => println!("Listo: {} ({n} páginas)", args[1]),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    let opciones = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 640.0])
            .with_min_inner_size([620.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("Armador de PDF", opciones, Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::Theme::Light);
            cc.egui_ctx.all_styles_mut(|st| {
                for f in st.text_styles.values_mut() {
                    f.size *= 1.2;
                }
                st.spacing.button_padding = egui::vec2(10.0, 6.0);
                st.visuals.panel_fill = egui::Color32::from_rgb(244, 246, 248);
                st.visuals.selection.bg_fill = egui::Color32::from_rgb(214, 236, 243);
                st.visuals.selection.stroke.color = PRIMARIO;
                st.visuals.widgets.noninteractive.fg_stroke.color = TINTA;
                st.spacing.item_spacing = egui::vec2(8.0, 6.0);
            });
            Ok(Box::new(App::default()))
        }),)
}

enum Mensaje {
    Avance(String),
    Fin(Result<(PathBuf, usize), String>),
}

#[derive(Default)]
struct App {
    archivos: Vec<PathBuf>,
    trabajo: Option<Receiver<Mensaje>>,
    estado: String,
    hubo_error: bool,
    ultimo: Option<PathBuf>,
    en_ayuda: bool,
}

impl App {
    fn agregar(&mut self, rutas: impl IntoIterator<Item = PathBuf>) {
        let mut rechazados = Vec::new();
        for r in rutas {
            if nucleo::es_aceptado(&r) {
                self.archivos.push(r);
            } else {
                rechazados.push(r.file_name().unwrap_or_default().to_string_lossy().to_string());
            }
        }
        if !rechazados.is_empty() {
            self.hubo_error = true;
            self.estado = format!("No se pudo agregar: {}. Solo sirven archivos PDF y de Word (.doc, .docx).", rechazados.join(", "));
        }
    }

    fn exportar(&mut self, ctx: &egui::Context) {
        let sugerido = self
            .archivos
            .first()
            .and_then(|p| p.file_stem())
            .map(|s| format!("{} - completo.pdf", s.to_string_lossy()))
            .unwrap_or_else(|| "documento.pdf".into());
        let Some(mut salida) = rfd::FileDialog::new()
            .set_title("Guardar PDF final")
            .add_filter("PDF", &["pdf"])
            .set_file_name(sugerido)
            .save_file()
        else {
            return;
        };
        if !nucleo::es_pdf(&salida) {
            salida.set_extension("pdf");
        }
        if self.archivos.contains(&salida) {
            self.hubo_error = true;
            self.estado = "El PDF final no puede reemplazar a uno de los documentos de la lista.".into();
            return;
        }
        let (tx, rx) = channel();
        let entradas = self.archivos.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let ctx2 = ctx.clone();
            let tx2 = tx.clone();
            let r = nucleo::armar(&entradas, &salida, move |m| {
                let _ = tx2.send(Mensaje::Avance(m));
                ctx2.request_repaint();
            });
            let _ = tx.send(Mensaje::Fin(r.map(|n| (salida, n))));
            ctx.request_repaint();
        });
        self.trabajo = Some(rx);
        self.hubo_error = false;
        self.ultimo = None;
        self.estado = "Preparando…".into();
    }

    fn recibir(&mut self) {
        let Some(rx) = &self.trabajo else { return };
        let mut terminado = false;
        for m in rx.try_iter() {
            match m {
                Mensaje::Avance(t) => self.estado = t,
                Mensaje::Fin(Ok((ruta, n))) => {
                    self.estado = format!("Listo: {} ({n} páginas)", ruta.display());
                    self.ultimo = Some(ruta);
                    terminado = true;
                }
                Mensaje::Fin(Err(e)) => {
                    self.estado = e;
                    self.hubo_error = true;
                    terminado = true;
                }
            }
        }
        if terminado {
            self.trabajo = None;
        }
    }
}

fn abrir(ruta: &Path) {
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        c.arg(ruta);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(ruta);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(ruta);
        c
    };
    let _ = cmd.spawn();
}

const VERDE: egui::Color32 = egui::Color32::from_rgb(30, 122, 70);
const PRIMARIO: egui::Color32 = egui::Color32::from_rgb(20, 96, 122);
const CELESTE: egui::Color32 = egui::Color32::from_rgb(41, 157, 190);
const TINTA: egui::Color32 = egui::Color32::from_rgb(27, 39, 51);

fn boton_grande(texto: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(texto).size(17.0)).min_size(egui::vec2(0.0, 36.0))
}

fn paso(ui: &mut egui::Ui, n: u8, titulo: &str) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), 15.0, CELESTE);
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, n.to_string(), egui::FontId::proportional(18.0), TINTA);
        ui.label(egui::RichText::new(titulo).size(19.0).strong());
    });
    ui.add_space(4.0);
}

impl App {
    fn pantalla_ayuda(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Cómo se usa");
            ui.label("Este programa junta varios documentos (por ejemplo, una nota y un proyecto) en un solo PDF.");

            let seccion = |ui: &mut egui::Ui, titulo: &str, lineas: &[&str]| {
                ui.add_space(14.0);
                ui.label(egui::RichText::new(titulo).size(18.0).strong());
                for l in lineas {
                    ui.add_space(3.0);
                    ui.label(*l);
                }
            };
            seccion(ui, "Paso 1  ·  Agregá los documentos", &[
                "• Tocá el botón «Agregar documentos…» y elegí los archivos. Podés elegir varios a la vez.",
                "• También podés arrastrarlos desde una carpeta y soltarlos sobre la ventana.",
                "• Sirven archivos PDF y de Word (.doc y .docx). Los de Word se pasan a PDF solos.",
            ]);
            seccion(ui, "Paso 2  ·  Revisá el orden", &[
                "• Los documentos aparecen en una lista numerada.",
                "• El número 1 va al principio del PDF final, el 2 después, y así.",
                "• Con «Subir» y «Bajar» cambiás el lugar de cada documento.",
                "• Con «Quitar» lo sacás de la lista. El archivo original no se borra.",
            ]);
            seccion(ui, "Paso 3  ·  Guardá el PDF final", &[
                "• Tocá «Guardar PDF final…», elegí la carpeta y el nombre, y aceptá.",
                "• Esperá unos segundos. La primera vez puede tardar un poco más.",
                "• Cuando termina aparece el aviso «Listo» y podés abrir el PDF para revisarlo.",
            ]);
            seccion(ui, "Si el documento está en Google Docs", &[
                "• Abrilo en Google Docs y andá a Archivo → Descargar → Microsoft Word (.docx) o Documento PDF.",
                "• Después agregá ese archivo descargado en el Paso 1.",
            ]);
            seccion(ui, "Para tener en cuenta", &[
                "• Los documentos originales nunca se modifican ni se borran.",
                "• El PDF final no puede guardarse con el mismo nombre y carpeta que uno de los documentos de la lista.",
                "• Si un PDF tiene contraseña, no se puede unir: pedí una copia sin contraseña.",
                "• Para empezar otro trámite, tocá «Empezar de nuevo».",
            ]);
            ui.add_space(16.0);
            if ui.add(boton_grande("Volver a armar el documento")).clicked() {
                self.en_ayuda = false;
            }
            ui.add_space(12.0);
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.recibir();
        let ocupado = self.trabajo.is_some();
        let ctx = ui.ctx().clone();

        let soltados: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !soltados.is_empty() && !ocupado {
            self.en_ayuda = false;
            self.agregar(soltados);
        }
        let arrastrando = ctx.input(|i| !i.raw.hovered_files.is_empty());

        egui::Panel::top("pestanas").show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.en_ayuda, false, egui::RichText::new("  Armar documento  ").size(17.0));
                ui.selectable_value(&mut self.en_ayuda, true, egui::RichText::new("  Cómo se usa  ").size(17.0));
            });
            ui.add_space(8.0);
        });

        if self.en_ayuda {
            egui::CentralPanel::default().show(ui, |ui| self.pantalla_ayuda(ui));
            return;
        }

        egui::Panel::bottom("pie").show(ui, |ui| {
            paso(ui, 3, "Guardá el PDF final");
            ui.horizontal(|ui| {
                let puede = !ocupado && !self.archivos.is_empty();
                let b = boton_grande("  Guardar PDF final…  ");
                let b = if puede { b.fill(PRIMARIO) } else { b };
                let texto_blanco = |ui: &mut egui::Ui| ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
                let r = ui.scope(|ui| {
                    if puede {
                        texto_blanco(ui);
                    }
                    ui.add_enabled(puede, b)
                });
                if r.inner.clicked() {
                    self.exportar(&ctx);
                }
                if let Some(u) = self.ultimo.clone() {
                    if ui.add(boton_grande("Abrir el PDF")).clicked() {
                        abrir(&u);
                    }
                    if ui.add(boton_grande("Abrir la carpeta")).clicked() {
                        if let Some(dir) = u.parent() {
                            abrir(dir);
                        }
                    }
                }
                if ocupado {
                    ui.spinner();
                }
            });
            ui.add_space(4.0);
            if self.estado.is_empty() {
                if self.archivos.is_empty() {
                    ui.weak("Primero agregá al menos un documento.");
                }
            } else {
                let color = if self.hubo_error {
                    ui.visuals().error_fg_color
                } else if self.ultimo.is_some() {
                    VERDE
                } else {
                    ui.visuals().text_color()
                };
                ui.add(egui::Label::new(egui::RichText::new(&self.estado).color(color).strong()).wrap());
            }
            ui.add_space(10.0);
        });

        egui::CentralPanel::default().show(ui, |ui| {
            paso(ui, 1, "Agregá los documentos");
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!ocupado, |ui| {
                    if ui.add(boton_grande("  Agregar documentos…  ")).clicked() {
                        if let Some(rutas) = rfd::FileDialog::new()
                            .set_title("Agregar documentos")
                            .add_filter("Documentos (PDF y Word)", &["pdf", "doc", "docx", "odt", "rtf"])
                            .pick_files()
                        {
                            self.agregar(rutas);
                        }
                    }
                    if !self.archivos.is_empty() && ui.add(boton_grande("Empezar de nuevo")).clicked() {
                        self.archivos.clear();
                        self.estado.clear();
                        self.hubo_error = false;
                        self.ultimo = None;
                    }
                });
            });
            ui.weak("También podés arrastrar los archivos y soltarlos sobre esta ventana.");

            paso(ui, 2, "Revisá el orden");
            if self.archivos.is_empty() {
                ui.weak("Todavía no hay documentos en la lista.");
                return;
            }
            ui.label("El número 1 queda al principio del PDF final.");
            ui.add_space(4.0);
            let mut accion: Option<(usize, i8)> = None; // -1 subir, 1 bajar, 0 quitar
            let total = self.archivos.len();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, ruta) in self.archivos.iter().enumerate() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("{}", i + 1)).size(20.0).strong());
                            ui.add_enabled_ui(!ocupado, |ui| {
                                if ui.add_enabled(i > 0, egui::Button::new("▲ Subir")).clicked() {
                                    accion = Some((i, -1));
                                }
                                if ui.add_enabled(i + 1 < total, egui::Button::new("▼ Bajar")).clicked() {
                                    accion = Some((i, 1));
                                }
                                if ui.button("✖ Quitar").clicked() {
                                    accion = Some((i, 0));
                                }
                            });
                            let nombre = ruta.file_name().unwrap_or_default().to_string_lossy();
                            ui.vertical(|ui| {
                                ui.add(egui::Label::new(egui::RichText::new(nombre).strong()).truncate())
                                    .on_hover_text(ruta.display().to_string());
                                if !nucleo::es_pdf(ruta) {
                                    ui.weak("Documento de Word: se pasa a PDF al guardar");
                                }
                            });
                            ui.allocate_space(egui::vec2(ui.available_width(), 0.0));
                        });
                    });
                }
            });
            match accion {
                Some((i, -1)) => self.archivos.swap(i, i - 1),
                Some((i, 1)) => self.archivos.swap(i, i + 1),
                Some((i, _)) => {
                    self.archivos.remove(i);
                }
                None => {}
            }
        });

        if arrastrando && !ocupado {
            let area = ctx.content_rect();
            let pintor = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("soltar")));
            pintor.rect_filled(area, 0.0, egui::Color32::from_rgba_unmultiplied(20, 96, 122, 230));
            pintor.text(
                area.center(),
                egui::Align2::CENTER_CENTER,
                "Soltá los documentos acá",
                egui::FontId::proportional(30.0),
                egui::Color32::WHITE,
            );
        }
    }
}
