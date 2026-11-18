#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod marca;
mod nucleo;
mod vista;

use eframe::egui;
use egui::{Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Layout, Margin, RichText, Stroke};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, channel};

static MARCA: OnceLock<marca::Marca> = OnceLock::new();

/// Marca activa (colores, nombre y logo de la organización).
fn marca() -> &'static marca::Marca {
    MARCA.get_or_init(marca::cargar)
}

// --- Sistema de diseño «Armador de PDF» ---
const FONDO: Color32 = Color32::from_rgb(0xF4, 0xF6, 0xF8);
const SUPERFICIE: Color32 = Color32::WHITE;
const TINTA: Color32 = Color32::from_rgb(0x1B, 0x27, 0x33);
const TINTA_SUAVE: Color32 = Color32::from_rgb(0x4A, 0x58, 0x66);
const BORDE: Color32 = Color32::from_rgb(0x7A, 0x87, 0x94);
const BORDE_SUAVE: Color32 = Color32::from_rgb(0xDD, 0xE2, 0xE8);
const EXITO: Color32 = Color32::from_rgb(0x1E, 0x7A, 0x46);
const EXITO_FONDO: Color32 = Color32::from_rgb(0xE6, 0xF4, 0xEC);
const ERROR: Color32 = Color32::from_rgb(0xB3, 0x26, 0x1E);
const ERROR_FONDO: Color32 = Color32::from_rgb(0xFB, 0xE9, 0xE7);

const MARGEN: i8 = 24; // espacio-4
const RADIO_MD: u8 = 10;
const RADIO_SM: u8 = 6;

fn negrita(tam: f32) -> FontId {
    FontId::new(tam, FontFamily::Name("negrita".into()))
}

fn main() -> eframe::Result {
    // Modo consola: armador-pdf --salida final.pdf nota.docx proyecto.pdf
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--salida") && args.len() >= 3 {
        let entradas: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
        match nucleo::armar(&entradas, Path::new(&args[1]), |m| println!("{m}")) {
            Ok(n) => println!("Listo: {} ({n} páginas)", args[1]),
            Err(e) => {
                eprintln!("Error: {e}");
                nucleo::limpiar_temporal();
                std::process::exit(1);
            }
        }
        nucleo::limpiar_temporal();
        return Ok(());
    }

    let opciones = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 680.0])
            .with_min_inner_size([1000.0, 580.0])
            .with_icon(icono_de_ventana()),
        ..Default::default()
    };
    let titulo = if marca().nombre.is_empty() { "Armador de PDF".to_string() } else { format!("Armador de PDF · {}", marca().nombre) };
    let resultado = eframe::run_native(
        &titulo,
        opciones,
        Box::new(|cc| {
            aplicar_tema(&cc.egui_ctx);
            Ok(Box::new(App { logo: cargar_logo(&cc.egui_ctx), ..Default::default() }))
        }),
    );
    nucleo::limpiar_temporal();
    resultado
}

fn icono_de_ventana() -> egui::IconData {
    let img = image::load_from_memory(include_bytes!("../assets/icono.png")).expect("ícono incluido").to_rgba8();
    egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() }
}

/// Carga el logo de la marca como textura. Si falta o no se puede leer, el programa sigue sin logo.
fn cargar_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let ruta = marca().logo.as_ref()?;
    let img = image::open(ruta).ok()?.to_rgba8();
    let tam = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(tam, img.as_raw());
    Some(ctx.load_texture("logo", color, egui::TextureOptions::LINEAR))
}

fn aplicar_tema(ctx: &egui::Context) {
    let mut fuentes = egui::FontDefinitions::default();
    fuentes.font_data.insert(
        "atkinson".into(),
        egui::FontData::from_static(include_bytes!("../assets/AtkinsonHyperlegible-Regular.ttf")).into(),
    );
    fuentes.font_data.insert(
        "atkinson-negrita".into(),
        egui::FontData::from_static(include_bytes!("../assets/AtkinsonHyperlegible-Bold.ttf")).into(),
    );
    let mut resto = fuentes.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    fuentes.families.entry(FontFamily::Proportional).or_default().insert(0, "atkinson".into());
    resto.insert(0, "atkinson-negrita".into());
    fuentes.families.insert(FontFamily::Name("negrita".into()), resto);
    ctx.set_fonts(fuentes);

    ctx.set_theme(egui::Theme::Light);
    ctx.all_styles_mut(|st| {
        use egui::TextStyle::*;
        st.text_styles.insert(Body, FontId::proportional(17.0));
        st.text_styles.insert(Button, negrita(18.0));
        st.text_styles.insert(Heading, negrita(28.0));
        st.text_styles.insert(Small, FontId::proportional(15.0));
        st.spacing.button_padding = egui::vec2(18.0, 8.0);
        st.spacing.item_spacing = egui::vec2(8.0, 8.0);
        st.visuals.panel_fill = FONDO;
        st.visuals.window_fill = FONDO;
        st.visuals.widgets.noninteractive.fg_stroke.color = TINTA;
        st.visuals.widgets.inactive.fg_stroke.color = TINTA;
        st.visuals.widgets.hovered.fg_stroke.color = TINTA;
        st.visuals.widgets.active.fg_stroke.color = TINTA;
        st.visuals.selection.bg_fill = marca().tinte;
        st.visuals.selection.stroke.color = marca().primario;
    });
}

enum Mensaje {
    Avance(String),
    Fin(Result<(PathBuf, usize), String>),
}

#[derive(Default)]
enum Aviso {
    #[default]
    Nada,
    Avance(String),
    Listo(PathBuf, usize),
    /// (título en negrita, detalle)
    Error(String, String),
}

#[derive(Default)]
struct App {
    archivos: Vec<PathBuf>,
    trabajo: Option<Receiver<Mensaje>>,
    aviso: Aviso,
    en_ayuda: bool,
    cuadros: u32,
    logo: Option<egui::TextureHandle>,
    vista: vista::Vista,
    capturada: bool,
    /// Aviso junto al paso 1: (es un error, texto).
    nota_agregar: Option<(bool, String)>,
}

impl App {
    fn agregar(&mut self, rutas: impl IntoIterator<Item = PathBuf>) {
        let (mut rechazados, mut repetidos, mut agregados) = (Vec::new(), Vec::new(), 0);
        for r in rutas {
            let nombre = format!("«{}»", r.file_name().unwrap_or_default().to_string_lossy());
            if !nucleo::es_aceptado(&r) {
                rechazados.push(nombre);
                continue;
            }
            if self.archivos.contains(&r) {
                repetidos.push(nombre);
            }
            self.archivos.push(r);
            agregados += 1;
        }
        if agregados > 0 && !matches!(self.aviso, Aviso::Avance(_)) {
            // Un «Listo» o un error anterior ya no corresponden a la lista nueva.
            self.aviso = Aviso::Nada;
        }
        self.nota_agregar = if !rechazados.is_empty() {
            Some((true, format!("{}: solo sirven archivos PDF y de Word (.doc, .docx).", rechazados.join(", "))))
        } else if !repetidos.is_empty() {
            let verbo = if repetidos.len() == 1 { "ya estaba en la lista y quedó" } else { "ya estaban en la lista y quedaron" };
            Some((false, format!("{} {verbo} dos veces. Si fue sin querer, tocá «Quitar» en uno.", repetidos.join(", "))))
        } else {
            None
        };
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
            self.aviso = Aviso::Error(
                "No se pudo guardar.".into(),
                "El PDF final no puede reemplazar a uno de los documentos de la lista. Elegí otro nombre.".into(),
            );
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
        self.aviso = Aviso::Avance("Preparando…".into());
    }

    fn recibir(&mut self) {
        let Some(rx) = &self.trabajo else { return };
        let mut terminado = false;
        for m in rx.try_iter() {
            match m {
                Mensaje::Avance(t) => self.aviso = Aviso::Avance(t),
                Mensaje::Fin(Ok((ruta, n))) => {
                    self.aviso = Aviso::Listo(ruta, n);
                    terminado = true;
                }
                Mensaje::Fin(Err(e)) => {
                    self.aviso = Aviso::Error("No se pudo guardar.".into(), e);
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
    let programa = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(programa).arg(ruta).spawn();
}

// ---------------------------------------------------------------------------
// Componentes
// ---------------------------------------------------------------------------

/// Botón principal: uno por pantalla, marca lo que hay que tocar ahora.
fn boton_principal(texto: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(texto).font(negrita(18.0)).color(Color32::WHITE))
        .fill(marca().primario)
        .corner_radius(CornerRadius::same(RADIO_MD))
        .min_size(egui::vec2(0.0, 48.0))
}

fn boton_secundario(texto: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(texto).font(negrita(18.0)).color(TINTA))
        .fill(SUPERFICIE)
        .stroke(Stroke::new(1.0, BORDE))
        .corner_radius(CornerRadius::same(RADIO_MD))
        .min_size(egui::vec2(0.0, 48.0))
}

/// Botón de la lista (Subir, Bajar, Quitar).
fn boton_chico(texto: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(texto).font(negrita(15.0)).color(TINTA))
        .fill(SUPERFICIE)
        .stroke(Stroke::new(1.0, BORDE))
        .corner_radius(CornerRadius::same(RADIO_SM))
        .min_size(egui::vec2(0.0, 40.0))
}

fn ayuda(texto: &str) -> RichText {
    RichText::new(texto).size(15.0).color(TINTA_SUAVE)
}

fn numero_de_paso(ui: &mut egui::Ui, n: u8) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::hover());
    ui.painter().circle_filled(r.center(), 16.0, marca().acento);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, n.to_string(), negrita(17.0), marca::texto_sobre(marca().acento, TINTA));
}

fn paso(ui: &mut egui::Ui, n: u8, titulo: &str, aclaracion: &str) {
    ui.horizontal(|ui| {
        numero_de_paso(ui, n);
        ui.add_space(4.0);
        ui.label(RichText::new(titulo).font(negrita(20.0)).color(TINTA));
        if !aclaracion.is_empty() {
            ui.add_space(4.0);
            ui.label(ayuda(aclaracion));
        }
    });
    ui.add_space(4.0);
}

fn tarjeta() -> Frame {
    Frame::new()
        .fill(SUPERFICIE)
        .stroke(Stroke::new(1.0, BORDE_SUAVE))
        .corner_radius(CornerRadius::same(RADIO_MD))
        .inner_margin(Margin::symmetric(16, 12))
}

fn borde_punteado(ui: &egui::Ui, r: egui::Rect, trazo: Stroke) {
    let p = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    ui.painter().extend(egui::Shape::dashed_line(&p, trazo, 9.0, 6.0));
}

/// Aviso con ícono: la primera palabra nombra el estado, no depende solo del color.
fn aviso(ui: &mut egui::Ui, fondo: Color32, icono: impl FnOnce(&mut egui::Ui), titulo: &str, detalle: &str) {
    Frame::new()
        .fill(fondo)
        .corner_radius(CornerRadius::same(RADIO_MD))
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icono(ui);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        ui.label(RichText::new(titulo).font(negrita(17.0)).color(TINTA));
                        ui.label(RichText::new(detalle).color(TINTA));
                    });
                });
            });
        });
}

fn icono_listo(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
    let c = r.center();
    ui.painter().circle_filled(c, 11.0, EXITO);
    let t = Stroke::new(2.5, Color32::WHITE);
    ui.painter().line_segment([c + egui::vec2(-5.0, 0.5), c + egui::vec2(-1.5, 4.0)], t);
    ui.painter().line_segment([c + egui::vec2(-1.5, 4.0), c + egui::vec2(5.0, -3.0)], t);
}

fn icono_error(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
    ui.painter().circle_filled(r.center(), 11.0, ERROR);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, "!", negrita(17.0), Color32::WHITE);
}

fn icono_info(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
    ui.painter().circle_filled(r.center(), 11.0, marca().primario);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, "i", negrita(17.0), Color32::WHITE);
}

fn icono_avance(ui: &mut egui::Ui) {
    ui.add(egui::Spinner::new().size(22.0).color(marca().primario));
}

// ---------------------------------------------------------------------------
// Pantallas
// ---------------------------------------------------------------------------

impl App {
    fn pestanas(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("pestanas")
            .frame(Frame::new().fill(SUPERFICIE).inner_margin(Margin::symmetric(MARGEN, 0)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 24.0;
                    ui.spacing_mut().button_padding.x = 0.0;
                    for (es_ayuda, texto) in [(false, "Armar documento"), (true, "Cómo se usa")] {
                        let activa = self.en_ayuda == es_ayuda;
                        let color = if activa { marca().primario } else { TINTA_SUAVE };
                        let r = ui.add(
                            egui::Button::new(RichText::new(texto).font(negrita(18.0)).color(color))
                                .frame(false)
                                .min_size(egui::vec2(0.0, 54.0)),
                        );
                        if activa {
                            let linea = egui::Rect::from_min_max(
                                egui::pos2(r.rect.left(), r.rect.bottom() - 3.0),
                                r.rect.right_bottom(),
                            );
                            ui.painter().rect_filled(linea, 0.0, marca().primario);
                        }
                        if r.clicked() {
                            self.en_ayuda = es_ayuda;
                        }
                    }
                    if let Some(logo) = &self.logo {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let alto = 40.0;
                            let ancho = alto * logo.aspect_ratio();
                            ui.add(egui::Image::new(logo).fit_to_exact_size(egui::vec2(ancho, alto)).alt_text(marca().nombre.clone()));
                        });
                    }
                });
            });
    }

    fn pantalla_ayuda(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new("Cómo se usa").font(negrita(28.0)).color(TINTA));
            ui.label(
                RichText::new("Este programa junta varios documentos, por ejemplo una nota y un proyecto, en un solo PDF.")
                    .color(TINTA_SUAVE),
            );
            ui.add_space(4.0);

            let pasos: [(u8, &str, &str); 3] = [
                (1, "Agregá los documentos", "Tocá «Agregar documentos…» y elegí los archivos, o arrastralos desde una carpeta y soltalos sobre la ventana. Sirven archivos PDF y de Word."),
                (2, "Revisá el orden", "El número 1 va al principio del PDF final. Con «Subir» y «Bajar» cambiás el lugar de cada documento; con «Quitar» lo sacás de la lista y con «Ver» lo abrís. A la derecha, la vista previa muestra cómo va quedando el PDF."),
                (3, "Guardá el PDF final", "Tocá «Guardar PDF final…», elegí la carpeta y el nombre. Esperá unos segundos hasta que aparezca el aviso «Listo»."),
            ];
            for (n, titulo, texto) in pasos {
                tarjeta().inner_margin(Margin::same(16)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| numero_de_paso(ui, n));
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(titulo).font(negrita(20.0)).color(TINTA));
                            ui.label(texto);
                        });
                    });
                });
            }
            let notas: [(&str, &[&str]); 3] = [
                ("Si el documento está en Google Docs", &[
                    "Abrilo y andá a Archivo → Descargar → Documento PDF (.pdf). Después agregá ese archivo en el paso 1.",
                    "Si necesitás seguir editándolo en Word, también podés descargarlo como Microsoft Word (.docx): el programa lo pasa a PDF solo.",
                ]),
                ("Historial", &[
                    "Cada PDF que armás queda anotado en el archivo «historial.txt», en la carpeta del programa: fecha, dónde se guardó y con qué documentos.",
                ]),
                ("Para tener en cuenta", &[
                    "Los documentos originales nunca se modifican ni se borran.",
                    "Si un PDF tiene contraseña no se puede unir: pedí una copia sin contraseña.",
                    "Para empezar otro trámite, tocá «Empezar de nuevo».",
                ]),
            ];
            for (titulo, lineas) in notas {
                tarjeta().inner_margin(Margin::same(16)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(titulo).font(negrita(20.0)).color(TINTA));
                    for l in lineas {
                        ui.label(*l);
                    }
                });
            }
            ui.add_space(4.0);
            if ui.add(boton_principal("Volver a armar el documento")).clicked() {
                self.en_ayuda = false;
            }
        });
    }

    fn paso_guardar(&mut self, ui: &mut egui::Ui, ocupado: bool) {
        let ctx = ui.ctx().clone();
        paso(ui, 3, "Guardá el PDF final", "");
        let listo = match &self.aviso {
            Aviso::Listo(ruta, _) => Some(ruta.clone()),
            _ => None,
        };
        ui.horizontal(|ui| {
            if let Some(ruta) = &listo {
                if ui.add(boton_principal("Abrir el PDF")).clicked() {
                    abrir(ruta);
                }
                if ui.add(boton_secundario("Abrir la carpeta")).clicked() {
                    if let Some(dir) = ruta.parent() {
                        abrir(dir);
                    }
                }
                if ui.add(boton_secundario("Guardar de nuevo…")).clicked() {
                    self.exportar(&ctx);
                }
            } else if ocupado {
                ui.add_enabled(false, boton_secundario("Guardando…"));
            } else if self.archivos.is_empty() {
                ui.add_enabled(false, boton_secundario("Guardar PDF final…"));
                ui.add_space(8.0);
                ui.label(ayuda("Primero agregá al menos un documento."));
            } else {
                if ui.add(boton_principal("Guardar PDF final…")).clicked() {
                    self.exportar(&ctx);
                }
                ui.add_space(8.0);
                ui.label(ayuda("Los documentos originales no se modifican."));
            }
        });
        match &self.aviso {
            Aviso::Nada => {}
            Aviso::Avance(t) => {
                ui.add_space(4.0);
                aviso(ui, marca().tinte, icono_avance, t, "Puede tardar unos segundos.");
            }
            Aviso::Listo(ruta, n) => {
                ui.add_space(4.0);
                let nombre = ruta.file_name().unwrap_or_default().to_string_lossy();
                let paginas = if *n == 1 { "1 página".to_string() } else { format!("{n} páginas") };
                aviso(ui, EXITO_FONDO, icono_listo, "Listo.", &format!("Se guardó «{nombre}», de {paginas}."));
            }
            Aviso::Error(titulo, detalle) => {
                ui.add_space(4.0);
                aviso(ui, ERROR_FONDO, icono_error, titulo, detalle);
            }
        }
    }

    fn paso_agregar(&mut self, ui: &mut egui::Ui, ocupado: bool) {
        paso(ui, 1, "Agregá los documentos", "");
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!ocupado, |ui| {
                let vacio = self.archivos.is_empty();
                let boton = if vacio { boton_principal("+  Agregar documentos…") } else { boton_secundario("+  Agregar más documentos…") };
                if ui.add(boton).clicked() {
                    if let Some(rutas) = rfd::FileDialog::new()
                        .set_title("Agregar documentos")
                        .add_filter("Documentos (PDF y Word)", &["pdf", "doc", "docx", "odt", "rtf"])
                        .pick_files()
                    {
                        self.agregar(rutas);
                    }
                }
                if vacio {
                    ui.add_space(8.0);
                    ui.label(ayuda("Sirven archivos PDF y de Word."));
                } else if ui.add(boton_secundario("Empezar de nuevo")).clicked() {
                    self.archivos.clear();
                    self.aviso = Aviso::Nada;
                    self.nota_agregar = None;
                }
            });
        });
        if let Some((es_error, texto)) = &self.nota_agregar {
            ui.add_space(4.0);
            if *es_error {
                aviso(ui, ERROR_FONDO, icono_error, "No se pudo agregar.", texto);
            } else {
                aviso(ui, marca().tinte, icono_info, "Atención.", texto);
            }
        }
    }

    fn paso_ordenar(&mut self, ui: &mut egui::Ui, ocupado: bool) {
        let vacio = self.archivos.is_empty();
        paso(ui, 2, "Revisá el orden", if vacio { "" } else { "El número 1 queda al principio del PDF." });
        if vacio {
            let tam = egui::vec2(ui.available_width(), ui.available_height().max(110.0));
            let (r, _) = ui.allocate_exact_size(tam, egui::Sense::hover());
            borde_punteado(ui, r.shrink(1.0), Stroke::new(2.0, BORDE));
            let c = r.center();
            ui.painter().text(c - egui::vec2(0.0, 12.0), Align2::CENTER_CENTER, "Todavía no hay documentos", negrita(17.0), TINTA);
            ui.painter().text(
                c + egui::vec2(0.0, 13.0),
                Align2::CENTER_CENTER,
                "Arrastrá acá la nota y el proyecto, o usá el botón de arriba.",
                FontId::proportional(15.0),
                TINTA_SUAVE,
            );
            return;
        }
        let mut accion: Option<(usize, i8)> = None; // -1 subir, 1 bajar, 0 quitar
        let total = self.archivos.len();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (i, ruta) in self.archivos.iter().enumerate() {
                tarjeta().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.add_sized([28.0, 40.0], egui::Label::new(RichText::new((i + 1).to_string()).font(negrita(24.0)).color(TINTA)));
                        ui.add_space(4.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_enabled_ui(!ocupado, |ui| {
                                if ui.add(boton_chico("Quitar")).clicked() {
                                    accion = Some((i, 0));
                                }
                                if ui.add_enabled(i + 1 < total, boton_chico("Bajar")).clicked() {
                                    accion = Some((i, 1));
                                }
                                if ui.add_enabled(i > 0, boton_chico("Subir")).clicked() {
                                    accion = Some((i, -1));
                                }
                            });
                            if ui.add(boton_chico("Ver")).on_hover_text("Abrir este documento para revisarlo").clicked() {
                                abrir(ruta);
                            }
                            ui.add_space(4.0);
                            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                let nombre = ruta.file_name().unwrap_or_default().to_string_lossy();
                                ui.add(egui::Label::new(RichText::new(nombre).font(negrita(17.0)).color(TINTA)).truncate())
                                    .on_hover_text(ruta.display().to_string());
                                let tipo = if nucleo::es_pdf(ruta) { "PDF" } else { "Documento de Word · se pasa a PDF al guardar" };
                                ui.add(egui::Label::new(ayuda(tipo)).truncate());
                            });
                        });
                    });
                });
            }
        });
        if accion.is_some() {
            // El aviso «Listo» ya no corresponde a la lista que se ve.
            self.aviso = Aviso::Nada;
            self.nota_agregar = None;
        }
        match accion {
            Some((i, -1)) => self.archivos.swap(i, i - 1),
            Some((i, 1)) => self.archivos.swap(i, i + 1),
            Some((i, _)) => {
                self.archivos.remove(i);
            }
            None => {}
        }
    }
}

impl App {
    /// Solo para pruebas: con ARMADOR_CAPTURA=archivo.ppm guarda una imagen de la ventana y cierra.
    /// ARMADOR_DEMO elige el estado: lista, listo, error, avance o ayuda.
    fn captura_de_prueba(&mut self, ctx: &egui::Context) {
        let Ok(destino) = std::env::var("ARMADOR_CAPTURA") else { return };
        self.cuadros += 1;
        if self.cuadros == 1 {
            let demo = std::env::var("ARMADOR_DEMO").unwrap_or_default();
            if demo != "" && demo != "ayuda" {
                // Con los documentos de demostración del repositorio la vista previa muestra páginas reales.
                let demo_dir = Path::new("tests/datos/demo");
                self.archivos = ["Nota de elevación.docx", "Proyecto de ordenanza.pdf"]
                    .iter()
                    .map(|n| if demo_dir.join(n).is_file() { demo_dir.join(n) } else { PathBuf::from(n) })
                    .collect();
            }
            match demo.as_str() {
                "listo" => self.aviso = Aviso::Listo("Nota de elevación - completo.pdf".into(), 4),
                "error" => self.aviso = Aviso::Error("No se pudo guardar.".into(), "«Proyecto de ordenanza.pdf» tiene contraseña y no se puede unir. Pedí una copia sin contraseña y volvé a agregarla.".into()),
                "avance" => self.aviso = Aviso::Avance("Pasando a PDF «Nota de elevación.docx»…".into()),
                "ayuda" => self.en_ayuda = true,
                _ => {}
            }
        }
        // Espera a que la vista previa termine (o un máximo) antes de capturar.
        let lista = self.archivos.is_empty() || self.vista.total(&self.archivos).is_some();
        if !self.capturada && ((self.cuadros >= 8 && lista) || self.cuadros > 3000) {
            self.capturada = true;
            let ctx2 = ctx.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                let mut datos = format!("P6\n{} {}\n255\n", img.size[0], img.size[1]).into_bytes();
                for p in &img.pixels {
                    datos.extend_from_slice(&[p.r(), p.g(), p.b()]);
                }
                let _ = std::fs::write(destino, datos);
                ctx2.send_viewport_cmd(egui::ViewportCommand::Close);
                ctx2.request_repaint();
            })));
        }
        ctx.request_repaint();
    }
}

impl App {
    fn panel_vista(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Vista previa").font(negrita(20.0)).color(TINTA));
        let resumen = if self.archivos.is_empty() {
            "Acá vas a ver cómo queda el PDF final.".to_string()
        } else {
            match self.vista.total(&self.archivos) {
                Some(1) => "Así va a quedar el PDF final: 1 página.".to_string(),
                Some(n) => format!("Así va a quedar el PDF final: {n} páginas."),
                None if self.vista.hay_error(&self.archivos) => "Algunos documentos no se pueden mostrar acá, pero igual se unen.".to_string(),
                None => "Preparando la vista previa…".to_string(),
            }
        };
        ui.label(ayuda(&resumen));
        ui.add_space(4.0);
        egui::ScrollArea::vertical().id_salt("vista").auto_shrink([false, false]).show(ui, |ui| {
            // Número de página dentro del PDF final; se pierde si un documento anterior todavía no se conoce.
            let mut anteriores = Some(0usize);
            for (i, ruta) in self.archivos.iter().enumerate() {
                let nombre = ruta.file_name().unwrap_or_default().to_string_lossy();
                ui.add(egui::Label::new(RichText::new(format!("{} · {nombre}", i + 1)).font(negrita(15.0)).color(TINTA)).truncate());
                match self.vista.estado(ruta) {
                    None | Some(vista::Estado::Preparando) => {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().size(18.0).color(marca().primario));
                            ui.label(ayuda(if nucleo::es_pdf(ruta) { "Preparando…" } else { "Pasando a PDF para mostrarlo…" }));
                        });
                        anteriores = None;
                    }
                    Some(vista::Estado::Error) => {
                        ui.label(ayuda("No se puede mostrar la vista previa de este documento."));
                        anteriores = None;
                    }
                    Some(vista::Estado::Lista { paginas, total, completa }) => {
                        for (k, pagina) in paginas.iter().enumerate() {
                            let ancho = ui.available_width() - 14.0;
                            let alto = ancho / pagina.aspect_ratio();
                            Frame::new().stroke(Stroke::new(1.0, BORDE_SUAVE)).show(ui, |ui| {
                                ui.add(egui::Image::new(pagina).fit_to_exact_size(egui::vec2(ancho, alto)));
                            });
                            let etiqueta = match anteriores {
                                Some(a) => format!("Página {}", a + k + 1),
                                None => format!("Página {} de este documento", k + 1),
                            };
                            ui.label(ayuda(&etiqueta));
                        }
                        if *completa && *total > paginas.len() {
                            ui.label(ayuda(&format!("… y {} páginas más de este documento.", total - paginas.len())));
                        }
                        anteriores = if *completa { anteriores.map(|a| a + total) } else { None };
                    }
                }
                ui.add_space(10.0);
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.recibir();
        self.captura_de_prueba(ui.ctx());
        let ocupado = self.trabajo.is_some();
        let ctx = ui.ctx().clone();

        let soltados: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !soltados.is_empty() && !ocupado {
            self.en_ayuda = false;
            self.agregar(soltados);
        }
        let arrastrando = ctx.input(|i| !i.raw.hovered_files.is_empty());

        self.vista.actualizar(&ctx, &self.archivos);
        self.pestanas(ui);
        let cuerpo = Frame::new().fill(FONDO).inner_margin(Margin::same(MARGEN));

        if self.en_ayuda {
            egui::CentralPanel::default().frame(cuerpo).show(ui, |ui| self.pantalla_ayuda(ui));
        } else {
            egui::Panel::right("vista")
                .exact_size(340.0)
                .resizable(false)
                .frame(Frame::new().fill(SUPERFICIE).inner_margin(Margin { left: 16, right: 8, top: MARGEN, bottom: 8 }))
                .show(ui, |ui| self.panel_vista(ui));
            egui::Panel::bottom("pie")
                .show_separator_line(false)
                .frame(Frame::new().fill(FONDO).inner_margin(Margin { left: MARGEN, right: MARGEN, top: 8, bottom: MARGEN }))
                .show(ui, |ui| self.paso_guardar(ui, ocupado));
            egui::CentralPanel::default()
                .frame(Frame::new().fill(FONDO).inner_margin(Margin { left: MARGEN, right: MARGEN, top: MARGEN, bottom: 8 }))
                .show(ui, |ui| {
                    self.paso_agregar(ui, ocupado);
                    ui.add_space(16.0);
                    self.paso_ordenar(ui, ocupado);
                });
        }

        if arrastrando && !ocupado {
            let area = ctx.content_rect();
            let pintor = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("soltar")));
            pintor.rect_filled(area, 0.0, FONDO);
            let zona = area.shrink(MARGEN as f32);
            pintor.rect_filled(zona, RADIO_MD as f32, marca().tinte);
            let p = [zona.left_top(), zona.right_top(), zona.right_bottom(), zona.left_bottom(), zona.left_top()];
            pintor.extend(egui::Shape::dashed_line(&p, Stroke::new(3.0, marca().primario), 12.0, 8.0));
            pintor.text(zona.center() - egui::vec2(0.0, 14.0), Align2::CENTER_CENTER, "Soltá los documentos acá", negrita(28.0), marca().primario);
            pintor.text(
                zona.center() + egui::vec2(0.0, 22.0),
                Align2::CENTER_CENTER,
                "Se agregan al final de la lista.",
                FontId::proportional(17.0),
                TINTA,
            );
        }
    }
}
