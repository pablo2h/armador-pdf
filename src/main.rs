#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod marca;
mod nucleo;
mod vista;

use chrono::TimeZone;
use eframe::egui;
use egui::{Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};
use nucleo::Registro;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

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
/// Segundos durante los que se puede deshacer un «Quitar».
const SEGUNDOS_DESHACER: u64 = 10;

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
            Ok(Box::new(App { logo: cargar_logo(&cc.egui_ctx), historial: nucleo::leer_historial(), ..Default::default() }))
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
        st.visuals.extreme_bg_color = SUPERFICIE;
        st.visuals.hyperlink_color = marca().primario;
        st.visuals.widgets.noninteractive.fg_stroke.color = TINTA;
        st.visuals.widgets.inactive.fg_stroke.color = TINTA;
        st.visuals.widgets.hovered.fg_stroke.color = TINTA;
        st.visuals.widgets.active.fg_stroke.color = TINTA;
        st.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDE);
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

#[derive(Default, Clone, Copy, PartialEq)]
enum Pantalla {
    #[default]
    Inicio,
    Armar,
    Historial,
    Ayuda,
}

enum AccionHistorial {
    Abrir,
    Carpeta,
    Rearmar,
}

#[derive(Default)]
struct App {
    pantalla: Pantalla,
    archivos: Vec<PathBuf>,
    trabajo: Option<Receiver<Mensaje>>,
    aviso: Aviso,
    /// Aviso junto al paso 1: (es un error, texto).
    nota_agregar: Option<(bool, String)>,
    /// Último documento quitado: (posición, ruta, momento), para poder deshacer.
    quitado: Option<(usize, PathBuf, Instant)>,
    /// Fila que se está arrastrando para reordenar.
    arrastre: Option<usize>,
    /// Carpeta elegida la última vez que se guardó, en esta sesión.
    ultima_carpeta: Option<PathBuf>,
    /// Nombre del PDF final, sin extensión. Se propone solo hasta que la persona lo cambia.
    nombre_pdf: String,
    nombre_editado: bool,
    historial: Vec<Registro>,
    buscar: String,
    confirmar_vaciar: bool,
    logo: Option<egui::TextureHandle>,
    vista: vista::Vista,
    // Solo para capturas de prueba.
    cuadros: u32,
    capturada: bool,
    demo: bool,
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
            Some((false, format!("{} {verbo} dos veces. Si fue sin querer, quitalo con el tacho.", repetidos.join(", "))))
        } else {
            None
        };
    }

    fn elegir_y_agregar(&mut self) {
        if let Some(rutas) = rfd::FileDialog::new()
            .set_title("Agregar documentos")
            .add_filter("Documentos (PDF y Word)", &["pdf", "doc", "docx", "odt", "rtf"])
            .pick_files()
        {
            self.agregar(rutas);
            self.pantalla = Pantalla::Armar;
        }
    }

    fn empezar_de_nuevo(&mut self) {
        self.archivos.clear();
        self.aviso = Aviso::Nada;
        self.nota_agregar = None;
        self.quitado = None;
        self.nombre_editado = false;
    }

    /// Carga en la lista los mismos documentos de un PDF ya armado.
    fn volver_a_armar(&mut self, registro: &Registro) {
        let (hay, faltan): (Vec<PathBuf>, Vec<PathBuf>) = registro.entradas.iter().cloned().partition(|e| e.is_file() || self.demo);
        self.empezar_de_nuevo();
        self.archivos = hay;
        if !faltan.is_empty() {
            let nombres: Vec<String> = faltan.iter().map(|f| format!("«{}»", f.file_name().unwrap_or_default().to_string_lossy())).collect();
            self.nota_agregar = Some((true, format!("{} ya no está donde estaba. Agregalo de nuevo desde su carpeta actual.", nombres.join(", "))));
        }
        self.pantalla = Pantalla::Armar;
    }

    /// Nombre que se propone para el PDF final: el del primer documento más «- completo».
    fn nombre_sugerido(&self) -> String {
        self.archivos
            .first()
            .and_then(|p| p.file_stem())
            .map(|s| format!("{} - completo", s.to_string_lossy()))
            .unwrap_or_else(|| "documento".into())
    }

    fn exportar(&mut self, ctx: &egui::Context) {
        let sugerido = format!("{}.pdf", nombre_de_archivo(&self.nombre_pdf).unwrap_or_else(|| self.nombre_sugerido()));
        let mut dialogo = rfd::FileDialog::new().set_title("Guardar PDF final").add_filter("PDF", &["pdf"]).set_file_name(sugerido);
        // Por defecto, la carpeta «exportaciones» del programa; después, la última que se eligió.
        if let Some(carpeta) = self.ultima_carpeta.clone().filter(|c| c.is_dir()).or_else(nucleo::carpeta_de_exportaciones) {
            dialogo = dialogo.set_directory(carpeta);
        }
        let Some(mut salida) = dialogo.save_file() else { return };
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
        self.ultima_carpeta = salida.parent().map(Path::to_path_buf);
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
                    self.historial = nucleo::leer_historial();
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

/// Limpia lo que la persona escribió como nombre del PDF: sin extensión y sin caracteres que no admite un archivo.
fn nombre_de_archivo(escrito: &str) -> Option<String> {
    let mut nombre = escrito.trim();
    if nombre.len() >= 4 && nombre[nombre.len() - 4..].eq_ignore_ascii_case(".pdf") {
        nombre = nombre[..nombre.len() - 4].trim_end();
    }
    let limpio: String = nombre.chars().map(|c| if "/\\:*?\"<>|".contains(c) || c.is_control() { '-' } else { c }).collect();
    let limpio = limpio.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
    (!limpio.is_empty()).then_some(limpio)
}

fn nombre_de(ruta: &Path) -> String {
    ruta.file_name().unwrap_or_default().to_string_lossy().to_string()
}

fn paginas_en_texto(n: usize) -> String {
    if n == 1 { "1 página".to_string() } else { format!("{n} páginas") }
}

/// Días transcurridos desde un registro del historial (0 = hoy).
fn dias_desde(cuando: i64) -> i64 {
    match chrono::Local.timestamp_opt(cuando, 0).single() {
        Some(f) => (chrono::Local::now().date_naive() - f.date_naive()).num_days(),
        None => i64::MAX,
    }
}

/// Fecha corta y cercana: «Hoy, 10:42», «Ayer, 16:05», «16/11».
fn fecha_en_texto(cuando: i64) -> String {
    let Some(f) = chrono::Local.timestamp_opt(cuando, 0).single() else { return String::new() };
    match dias_desde(cuando) {
        0 => format!("Hoy, {}", f.format("%H:%M")),
        1 => format!("Ayer, {}", f.format("%H:%M")),
        _ => f.format("%d/%m/%Y").to_string(),
    }
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

/// Botón chico con texto, para las filas del historial y acciones menores.
fn boton_chico(texto: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(texto).font(negrita(15.0)).color(TINTA))
        .fill(SUPERFICIE)
        .stroke(Stroke::new(1.0, BORDE))
        .corner_radius(CornerRadius::same(RADIO_SM))
        .min_size(egui::vec2(0.0, 40.0))
}

#[derive(Clone, Copy)]
enum Icono {
    Subir,
    Bajar,
    Tacho,
}

fn trazo(p: &egui::Painter, puntos: &[Pos2], s: Stroke) {
    p.add(egui::Shape::line(puntos.to_vec(), s));
}

/// Botón cuadrado de 44 px con un ícono dibujado. El texto de ayuda aparece al pasar el mouse.
fn boton_icono(ui: &mut egui::Ui, icono: Icono, habilitado: bool, texto: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), if habilitado { Sense::click() } else { Sense::hover() });
    let (fondo, borde, color) = if !habilitado {
        (FONDO, BORDE_SUAVE, BORDE)
    } else if resp.hovered() {
        (marca().tinte, marca().primario, TINTA)
    } else {
        (SUPERFICIE, BORDE, TINTA)
    };
    let p = ui.painter();
    p.rect_filled(r, RADIO_SM as f32, fondo);
    p.rect_stroke(r, RADIO_SM as f32, Stroke::new(1.0, borde), StrokeKind::Inside);
    let c = r.center();
    let v = |x: f32, y: f32| c + egui::vec2(x, y);
    let s = Stroke::new(2.2, color);
    match icono {
        Icono::Subir => {
            trazo(p, &[v(0.0, 8.0), v(0.0, -8.0)], s);
            trazo(p, &[v(-6.0, -2.0), v(0.0, -8.0), v(6.0, -2.0)], s);
        }
        Icono::Bajar => {
            trazo(p, &[v(0.0, -8.0), v(0.0, 8.0)], s);
            trazo(p, &[v(-6.0, 2.0), v(0.0, 8.0), v(6.0, 2.0)], s);
        }
        Icono::Tacho => {
            let s = Stroke::new(1.8, color);
            trazo(p, &[v(-9.0, -6.0), v(9.0, -6.0)], s);
            trazo(p, &[v(-3.0, -6.0), v(-3.0, -9.5), v(3.0, -9.5), v(3.0, -6.0)], s);
            trazo(p, &[v(-6.5, -6.0), v(-5.5, 9.0), v(5.5, 9.0), v(6.5, -6.0)], s);
            trazo(p, &[v(-2.0, -2.0), v(-2.0, 5.0)], s);
            trazo(p, &[v(2.0, -2.0), v(2.0, 5.0)], s);
        }
    }
    resp.on_hover_text(texto)
}

/// Botón principal «Guardar PDF», con un ícono de guardado adelante.
fn boton_guardar(ui: &mut egui::Ui, habilitado: bool) -> egui::Response {
    let texto = "Guardar PDF";
    let (fondo, color) = if habilitado { (marca().primario, Color32::WHITE) } else { (BORDE_SUAVE, TINTA_SUAVE) };
    let letras = ui.painter().layout_no_wrap(texto.to_string(), negrita(18.0), color);
    let ancho = 18.0 + 22.0 + 10.0 + letras.size().x + 18.0;
    let (r, resp) = ui.allocate_exact_size(egui::vec2(ancho, 48.0), if habilitado { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    p.rect_filled(r, RADIO_MD as f32, fondo);
    if habilitado && resp.hovered() {
        p.rect_stroke(r, RADIO_MD as f32, Stroke::new(2.0, marca().acento), StrokeKind::Inside);
    }
    // Ícono de guardado (disquete).
    let c = egui::pos2(r.left() + 18.0 + 11.0, r.center().y);
    let v = |x: f32, y: f32| c + egui::vec2(x, y);
    let s = Stroke::new(1.8, color);
    trazo(p, &[v(-9.0, -9.0), v(6.0, -9.0), v(9.0, -6.0), v(9.0, 9.0), v(-9.0, 9.0), v(-9.0, -9.0)], s);
    trazo(p, &[v(-5.0, -9.0), v(-5.0, -3.5), v(4.0, -3.5), v(4.0, -9.0)], s);
    trazo(p, &[v(-5.5, 9.0), v(-5.5, 1.5), v(5.5, 1.5), v(5.5, 9.0)], s);
    p.galley(egui::pos2(r.left() + 18.0 + 22.0 + 10.0, r.center().y - letras.size().y / 2.0), letras, color);
    if habilitado { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

fn ayuda(texto: &str) -> RichText {
    RichText::new(texto).size(15.0).color(TINTA_SUAVE)
}

fn numero_de_paso(ui: &mut egui::Ui, n: u8) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), Sense::hover());
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

fn borde_punteado(p: &egui::Painter, r: Rect, s: Stroke) {
    let puntos = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    p.extend(egui::Shape::dashed_line(&puntos, s, 9.0, 6.0));
}

/// Recuadro punteado con dos renglones centrados (estados vacíos y zonas para soltar).
fn recuadro_vacio(ui: &mut egui::Ui, alto: f32, titulo: &str, detalle: &str) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), alto), Sense::hover());
    borde_punteado(ui.painter(), r.shrink(1.0), Stroke::new(2.0, BORDE));
    let c = r.center();
    ui.painter().text(c - egui::vec2(0.0, 12.0), Align2::CENTER_CENTER, titulo, negrita(17.0), TINTA);
    ui.painter().text(c + egui::vec2(0.0, 13.0), Align2::CENTER_CENTER, detalle, FontId::proportional(15.0), TINTA_SUAVE);
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

fn icono_redondo(ui: &mut egui::Ui, color: Color32, letra: &str) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 11.0, color);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, letra, negrita(17.0), Color32::WHITE);
}

fn icono_listo(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::hover());
    let c = r.center();
    ui.painter().circle_filled(c, 11.0, EXITO);
    trazo(ui.painter(), &[c + egui::vec2(-5.0, 0.5), c + egui::vec2(-1.5, 4.0), c + egui::vec2(5.0, -3.0)], Stroke::new(2.5, Color32::WHITE));
}

fn icono_error(ui: &mut egui::Ui) {
    icono_redondo(ui, ERROR, "!");
}

fn icono_info(ui: &mut egui::Ui) {
    icono_redondo(ui, marca().primario, "i");
}

fn icono_avance(ui: &mut egui::Ui) {
    ui.add(egui::Spinner::new().size(22.0).color(marca().primario));
}

/// Hoja de papel dibujada, para las ilustraciones del instructivo.
fn hoja(p: &egui::Painter, r: Rect, renglones: usize) {
    p.rect_filled(r, 3.0, SUPERFICIE);
    p.rect_stroke(r, 3.0, Stroke::new(2.0, marca().primario), StrokeKind::Inside);
    for i in 0..renglones {
        let y = r.top() + 14.0 + i as f32 * 10.0;
        let ancho = if i + 1 == renglones { 0.55 } else { 1.0 };
        let color = if i == 0 { BORDE } else { BORDE_SUAVE };
        p.rect_filled(Rect::from_min_size(egui::pos2(r.left() + 9.0, y), egui::vec2((r.width() - 18.0) * ancho, 4.0)), 0.0, color);
    }
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
                    let pestanas = [
                        (Pantalla::Inicio, "Inicio"),
                        (Pantalla::Armar, "Armar documento"),
                        (Pantalla::Historial, "Historial"),
                        (Pantalla::Ayuda, "Cómo se usa"),
                    ];
                    for (pantalla, texto) in pestanas {
                        let activa = self.pantalla == pantalla;
                        let color = if activa { marca().primario } else { TINTA_SUAVE };
                        let r = ui.add(
                            egui::Button::new(RichText::new(texto).font(negrita(18.0)).color(color))
                                .frame(false)
                                .min_size(egui::vec2(0.0, 54.0)),
                        );
                        if activa {
                            let linea = Rect::from_min_max(egui::pos2(r.rect.left(), r.rect.bottom() - 3.0), r.rect.right_bottom());
                            ui.painter().rect_filled(linea, 0.0, marca().primario);
                        }
                        if r.clicked() {
                            self.pantalla = pantalla;
                            self.confirmar_vaciar = false;
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

    /// Indica si el PDF de un registro sigue en su carpeta.
    fn sigue_estando(&self, indice: usize, registro: &Registro) -> bool {
        if self.demo { indice != 3 } else { registro.salida.is_file() }
    }

    /// Una fila del historial. `con_fecha_aparte` la usa la pantalla Historial, que agrupa por día.
    fn fila_historial(&self, ui: &mut egui::Ui, indice: usize, registro: &Registro, completa: bool) -> Option<AccionHistorial> {
        let mut accion = None;
        let esta = self.sigue_estando(indice, registro);
        tarjeta().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(boton_chico("Volver a armar")).on_hover_text("Cargar los mismos documentos en el mismo orden").clicked() {
                        accion = Some(AccionHistorial::Rearmar);
                    }
                    if esta {
                        if completa && ui.add(boton_chico("Abrir la carpeta")).clicked() {
                            accion = Some(AccionHistorial::Carpeta);
                        }
                        if ui.add(boton_chico("Abrir")).clicked() {
                            accion = Some(AccionHistorial::Abrir);
                        }
                    }
                    ui.add_space(4.0);
                    ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        let color = if esta { TINTA } else { TINTA_SUAVE };
                        ui.add(egui::Label::new(RichText::new(nombre_de(&registro.salida)).font(negrita(17.0)).color(color)).truncate())
                            .on_hover_text(registro.salida.display().to_string());
                        let documentos: Vec<String> = registro.entradas.iter().map(|e| nombre_de(e)).collect();
                        let detalle = if esta { documentos.join(" + ") } else { "El archivo ya no está en esa carpeta".to_string() };
                        let texto = format!("{} · {} · {detalle}", fecha_en_texto(registro.cuando), paginas_en_texto(registro.paginas));
                        ui.add(egui::Label::new(ayuda(&texto)).truncate());
                    });
                });
            });
        });
        accion
    }

    fn aplicar_accion(&mut self, indice: usize, accion: AccionHistorial) {
        let Some(registro) = self.historial.get(indice).cloned() else { return };
        match accion {
            AccionHistorial::Abrir => abrir(&registro.salida),
            AccionHistorial::Carpeta => {
                if let Some(dir) = registro.salida.parent() {
                    abrir(dir);
                }
            }
            AccionHistorial::Rearmar => self.volver_a_armar(&registro),
        }
    }

    fn inicio_izquierda(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("¿Qué querés hacer?").font(negrita(28.0)).color(TINTA));
        ui.add_space(8.0);

        // Botón grande: la acción principal del programa.
        let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 100.0), Sense::click());
        let p = ui.painter();
        p.rect_filled(r, RADIO_MD as f32, marca().primario);
        if resp.hovered() {
            p.rect_stroke(r, RADIO_MD as f32, Stroke::new(3.0, marca().acento), StrokeKind::Inside);
        }
        let c = egui::pos2(r.left() + 52.0, r.center().y);
        let v = |x: f32, y: f32| c + egui::vec2(x, y);
        let s = Stroke::new(3.0, Color32::WHITE);
        trazo(p, &[v(-14.0, -20.0), v(4.0, -20.0), v(14.0, -10.0), v(14.0, 20.0), v(-14.0, 20.0), v(-14.0, -20.0)], s);
        trazo(p, &[v(4.0, -20.0), v(4.0, -10.0), v(14.0, -10.0)], s);
        trazo(p, &[v(0.0, -3.0), v(0.0, 13.0)], s);
        trazo(p, &[v(-8.0, 5.0), v(8.0, 5.0)], s);
        p.text(egui::pos2(r.left() + 96.0, r.center().y - 13.0), Align2::LEFT_CENTER, "Armar un documento nuevo", negrita(22.0), Color32::WHITE);
        p.text(
            egui::pos2(r.left() + 96.0, r.center().y + 15.0),
            Align2::LEFT_CENTER,
            "Juntá una nota y un proyecto en un solo PDF.",
            FontId::proportional(15.0),
            Color32::WHITE,
        );
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            self.empezar_de_nuevo();
            self.pantalla = Pantalla::Armar;
        }

        ui.add_space(8.0);
        recuadro_vacio(ui, 84.0, "O arrastrá los documentos acá", "Se abre «Armar documento» con esos archivos ya cargados.");

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            Frame::new().fill(marca().tinte).corner_radius(CornerRadius::same(RADIO_MD)).inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    icono_redondo(ui, marca().primario, "?");
                    ui.add_space(4.0);
                    ui.label(RichText::new("¿Primera vez? Son tres pasos.").size(15.0).color(TINTA));
                    if ui.link(RichText::new("Mirá cómo se usa").font(negrita(15.0))).clicked() {
                        self.pantalla = Pantalla::Ayuda;
                    }
                });
            });
        });
    }

    fn inicio_derecha(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Últimos documentos armados").font(negrita(20.0)).color(TINTA));
            if !self.historial.is_empty() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.link(RichText::new("Ver todo el historial").font(negrita(15.0))).clicked() {
                        self.pantalla = Pantalla::Historial;
                    }
                });
            }
        });
        ui.add_space(4.0);
        if self.historial.is_empty() {
            recuadro_vacio(ui, 120.0, "Todavía no armaste ningún documento", "Cuando guardes el primero, va a aparecer acá.");
            return;
        }
        let mut pedida = None;
        for (i, registro) in self.historial.iter().enumerate().take(5) {
            if let Some(a) = self.fila_historial(ui, i, registro, false) {
                pedida = Some((i, a));
            }
        }
        ui.label(ayuda("El historial se guarda solo en esta computadora."));
        if let Some((i, a)) = pedida {
            self.aplicar_accion(i, a);
        }
    }

    fn pantalla_historial(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Historial").font(negrita(28.0)).color(TINTA));
                ui.label(RichText::new("Todos los PDF que armaste en esta computadora.").color(TINTA_SUAVE));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.buscar)
                        .hint_text("Buscar por nombre")
                        .desired_width(280.0)
                        .margin(Margin::symmetric(12, 12)),
                );
            });
        });
        ui.add_space(8.0);

        if self.historial.is_empty() {
            recuadro_vacio(ui, 120.0, "El historial está vacío", "Cada PDF que guardes va a quedar anotado acá.");
            return;
        }

        let filtro = self.buscar.trim().to_lowercase();
        let coincide = |r: &Registro| {
            filtro.is_empty()
                || nombre_de(&r.salida).to_lowercase().contains(&filtro)
                || r.entradas.iter().any(|e| nombre_de(e).to_lowercase().contains(&filtro))
        };
        let visibles: Vec<usize> = (0..self.historial.len()).filter(|&i| coincide(&self.historial[i])).collect();

        let mut pedida = None;
        egui::Panel::bottom("pie-historial")
            .show_separator_line(false)
            .frame(Frame::new().fill(FONDO).inner_margin(Margin { left: 0, right: 0, top: 8, bottom: 0 }))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let cuenta = if self.historial.len() == 1 { "1 documento.".to_string() } else { format!("{} documentos.", self.historial.len()) };
                    ui.label(ayuda(&format!("{cuenta} El historial se guarda solo en esta computadora.")));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.confirmar_vaciar {
                            if ui.add(boton_chico("No, dejarlo")).clicked() {
                                self.confirmar_vaciar = false;
                            }
                            if ui.add(boton_chico("Sí, vaciar")).clicked() {
                                nucleo::vaciar_historial();
                                self.historial.clear();
                                self.confirmar_vaciar = false;
                            }
                            ui.label(RichText::new("¿Vaciar el historial? Los PDF no se borran.").font(negrita(15.0)).color(TINTA));
                        } else if ui.add(boton_chico("Vaciar el historial…")).clicked() {
                            self.confirmar_vaciar = true;
                        }
                    });
                });
            });

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if visibles.is_empty() {
                ui.label(ayuda("Ningún documento coincide con la búsqueda."));
            }
            let mut grupo_anterior = None;
            for i in visibles {
                let registro = &self.historial[i];
                let grupo = match dias_desde(registro.cuando) {
                    0 => "HOY",
                    1..=6 => "ESTA SEMANA",
                    _ => "ANTES",
                };
                if grupo_anterior != Some(grupo) {
                    ui.add_space(4.0);
                    ui.label(RichText::new(grupo).font(negrita(15.0)).color(TINTA_SUAVE));
                    grupo_anterior = Some(grupo);
                }
                if let Some(a) = self.fila_historial(ui, i, registro, true) {
                    pedida = Some((i, a));
                }
            }
        });
        if let Some((i, a)) = pedida {
            self.aplicar_accion(i, a);
        }
    }

    fn pantalla_ayuda(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Cómo se usa").font(negrita(28.0)).color(TINTA));
                    ui.label(RichText::new("Este programa junta varios documentos en un solo PDF. Son tres pasos.").color(TINTA_SUAVE));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(boton_principal("Armar un documento")).clicked() {
                        self.pantalla = Pantalla::Armar;
                    }
                });
            });
            ui.add_space(12.0);

            let pasos: [(u8, &str, &str); 3] = [
                (1, "Agregá los documentos", "Tocá «Agregar documento» o arrastralos a la ventana. Sirven archivos PDF y de Word."),
                (2, "Revisá el orden", "El número 1 va al principio. Movelos con las flechas o arrastrándolos, y mirá cómo queda en la vista previa."),
                (3, "Guardá el PDF final", "Tocá «Guardar PDF» y esperá el aviso «Listo». Queda anotado en el historial."),
            ];
            let separacion = 16.0;
            let ancho = ((ui.available_width() - 2.0 * separacion) / 3.0).floor();
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = separacion;
                for (n, titulo, texto) in pasos {
                    tarjeta().inner_margin(Margin::same(16)).show(ui, |ui| {
                        ui.set_width(ancho - 34.0);
                        ui.set_min_height(262.0);
                        ui.vertical(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 140.0), Sense::hover());
                            self.ilustracion(ui.painter(), r, n);
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                numero_de_paso(ui, n);
                                ui.add_space(4.0);
                                ui.label(RichText::new(titulo).font(negrita(20.0)).color(TINTA));
                            });
                            ui.label(texto);
                        });
                    });
                }
            });
            ui.add_space(8.0);

            let bloques: [(&str, [(&str, &str); 3]); 2] = [
                ("Preguntas frecuentes", [
                    ("Mi documento está en Google Docs", "Abrilo y andá a Archivo → Descargar → Documento PDF (.pdf). Después agregá ese archivo."),
                    ("¿Se modifican mis archivos?", "No. Los originales nunca se tocan ni se borran. Quitar un documento solo lo saca de la lista."),
                    ("¿Dónde veo lo que ya armé?", "En la pestaña «Historial», con fecha, páginas y documentos usados. Desde ahí también podés volver a armarlo."),
                ]),
                ("Si algo no anda", [
                    ("Un PDF tiene contraseña", "No se puede unir. Pedí una copia sin contraseña y volvé a agregarla."),
                    ("Tarda mucho", "La primera vez que se pasa un Word a PDF puede tardar hasta un minuto."),
                    ("¿Dónde quedan los PDF?", if cfg!(target_os = "macos") { "Se propone la carpeta «Armador de PDF» dentro de Documentos, pero al guardar podés elegir cualquier otra." } else { "Se propone la carpeta «exportaciones» del programa, pero al guardar podés elegir cualquier otra." }),
                ]),
            ];
            let ancho = ((ui.available_width() - separacion) / 2.0).floor();
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = separacion;
                for (titulo, items) in bloques {
                    tarjeta().inner_margin(Margin::same(20)).show(ui, |ui| {
                        ui.set_width(ancho - 42.0);
                        ui.set_min_height(236.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(titulo).font(negrita(20.0)).color(TINTA));
                            for (pregunta, respuesta) in items {
                                ui.add_space(6.0);
                                ui.label(RichText::new(pregunta).font(negrita(17.0)).color(TINTA));
                                ui.label(respuesta);
                            }
                        });
                    });
                }
            });
        });
    }

    /// Dibujo de cada paso del instructivo.
    fn ilustracion(&self, p: &egui::Painter, r: Rect, paso: u8) {
        p.rect_filled(r, RADIO_SM as f32, marca().tinte);
        let c = r.center();
        let primario = marca().primario;
        match paso {
            1 => {
                hoja(p, Rect::from_center_size(c - egui::vec2(62.0, 0.0), egui::vec2(62.0, 82.0)), 3);
                hoja(p, Rect::from_center_size(c + egui::vec2(62.0, 0.0), egui::vec2(62.0, 82.0)), 4);
                let s = Stroke::new(3.5, primario);
                trazo(p, &[c - egui::vec2(11.0, 0.0), c + egui::vec2(11.0, 0.0)], s);
                trazo(p, &[c - egui::vec2(0.0, 11.0), c + egui::vec2(0.0, 11.0)], s);
            }
            2 => {
                for (i, dy) in [-24.0_f32, 24.0].into_iter().enumerate() {
                    let fila = Rect::from_center_size(c + egui::vec2(0.0, dy), egui::vec2((r.width() - 48.0).min(240.0), 36.0));
                    p.rect_filled(fila, RADIO_SM as f32, SUPERFICIE);
                    let borde = if i == 0 { Stroke::new(2.0, primario) } else { Stroke::new(1.0, BORDE) };
                    p.rect_stroke(fila, RADIO_SM as f32, borde, StrokeKind::Inside);
                    p.text(egui::pos2(fila.left() + 16.0, fila.center().y), Align2::CENTER_CENTER, (i + 1).to_string(), negrita(17.0), TINTA);
                    let barra = Rect::from_min_max(egui::pos2(fila.left() + 34.0, fila.center().y - 2.5), egui::pos2(fila.right() - 40.0, fila.center().y + 2.5));
                    p.rect_filled(barra, 0.0, if i == 0 { BORDE } else { BORDE_SUAVE });
                    let f = egui::pos2(fila.right() - 20.0, fila.center().y);
                    let (sentido, color) = if i == 0 { (1.0, primario) } else { (-1.0, BORDE) };
                    let s = Stroke::new(2.5, color);
                    trazo(p, &[f - egui::vec2(0.0, 7.0 * sentido), f + egui::vec2(0.0, 7.0 * sentido)], s);
                    trazo(p, &[f + egui::vec2(-5.0, 2.0 * sentido), f + egui::vec2(0.0, 7.0 * sentido), f + egui::vec2(5.0, 2.0 * sentido)], s);
                }
            }
            _ => {
                let h = Rect::from_center_size(c - egui::vec2(34.0, 0.0), egui::vec2(70.0, 92.0));
                hoja(p, h, 3);
                let etiqueta = Rect::from_min_size(egui::pos2(h.left() + 9.0, h.bottom() - 30.0), egui::vec2(38.0, 20.0));
                p.rect_filled(etiqueta, 3.0, primario);
                p.text(etiqueta.center(), Align2::CENTER_CENTER, "PDF", negrita(13.0), Color32::WHITE);
                let k = c + egui::vec2(46.0, 0.0);
                p.circle_filled(k, 20.0, EXITO);
                trazo(p, &[k + egui::vec2(-9.0, 1.0), k + egui::vec2(-3.0, 7.0), k + egui::vec2(9.0, -6.0)], Stroke::new(4.0, Color32::WHITE));
            }
        }
    }

    fn paso_agregar(&mut self, ui: &mut egui::Ui, ocupado: bool) {
        paso(ui, 1, "Agregá los documentos", "");
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!ocupado, |ui| {
                let vacio = self.archivos.is_empty();
                let boton = if vacio { boton_principal("+  Agregar documento") } else { boton_secundario("+  Agregar documento") };
                if ui.add(boton).clicked() {
                    self.elegir_y_agregar();
                }
                if vacio {
                    ui.add_space(8.0);
                    ui.label(ayuda("Sirven archivos PDF y de Word."));
                } else if ui.add(boton_secundario("Empezar de nuevo")).clicked() {
                    self.empezar_de_nuevo();
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
        paso(ui, 2, "Revisá el orden", if vacio { "" } else { "Arrastrá un documento para moverlo, o usá las flechas." });

        // Aviso para deshacer el último «Quitar».
        let mut deshacer = false;
        if let Some((_, ruta, cuando)) = &self.quitado {
            if cuando.elapsed() < Duration::from_secs(SEGUNDOS_DESHACER) {
                Frame::new().fill(marca().tinte).corner_radius(CornerRadius::same(RADIO_MD)).inner_margin(Margin::symmetric(16, 6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            deshacer = ui.add(boton_chico("Deshacer")).clicked();
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                let texto = format!("Se quitó «{}» de la lista. El archivo no se borró.", nombre_de(ruta));
                                ui.add(egui::Label::new(RichText::new(texto).size(15.0).color(TINTA)).truncate());
                            });
                        });
                    });
                });
                ui.ctx().request_repaint_after(Duration::from_millis(500));
            } else {
                self.quitado = None;
            }
        }
        if deshacer {
            if let Some((i, ruta, _)) = self.quitado.take() {
                self.archivos.insert(i.min(self.archivos.len()), ruta);
            }
        }

        if self.archivos.is_empty() {
            let alto = ui.available_height().max(110.0);
            recuadro_vacio(ui, alto, "Todavía no hay documentos", "Arrastrá acá la nota y el proyecto, o usá el botón de arriba.");
            return;
        }

        let mut accion: Option<(usize, i8)> = None; // -1 subir, 1 bajar, 0 quitar
        let mut soltar = false;
        let mut filas: Vec<Rect> = Vec::with_capacity(self.archivos.len());
        let total = self.archivos.len();
        egui::ScrollArea::vertical().id_salt("lista").auto_shrink([false, false]).show(ui, |ui| {
            for (i, ruta) in self.archivos.iter().enumerate() {
                let arrastrada = self.arrastre == Some(i);
                let marco = if arrastrada { tarjeta().stroke(Stroke::new(2.0, marca().primario)) } else { tarjeta() };
                let fila = marco.inner_margin(Margin { left: 8, right: 16, top: 10, bottom: 10 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        // Asa para arrastrar.
                        let (a, asa) = ui.allocate_exact_size(egui::vec2(22.0, 44.0), if ocupado { Sense::hover() } else { Sense::drag() });
                        let color = if asa.hovered() || arrastrada { marca().primario } else { BORDE };
                        for (dx, dy) in [(-4.0, -8.0), (4.0, -8.0), (-4.0, 0.0), (4.0, 0.0), (-4.0, 8.0), (4.0, 8.0)] {
                            ui.painter().circle_filled(a.center() + egui::vec2(dx, dy), 2.2, color);
                        }
                        if asa.dragged() {
                            self.arrastre = Some(i);
                        }
                        if asa.drag_stopped() {
                            soltar = true;
                        }
                        asa.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text("Arrastrar para mover");

                        ui.add_sized([28.0, 44.0], egui::Label::new(RichText::new((i + 1).to_string()).font(negrita(24.0)).color(TINTA)));
                        ui.add_space(4.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_enabled_ui(!ocupado, |ui| {
                                if boton_icono(ui, Icono::Tacho, true, "Quitar de la lista (no borra el archivo)").clicked() {
                                    accion = Some((i, 0));
                                }
                                ui.add_space(8.0);
                                if boton_icono(ui, Icono::Bajar, i + 1 < total, "Bajar un lugar").clicked() {
                                    accion = Some((i, 1));
                                }
                                if boton_icono(ui, Icono::Subir, i > 0, "Subir un lugar").clicked() {
                                    accion = Some((i, -1));
                                }
                            });
                            ui.add_space(4.0);
                            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.add(egui::Label::new(RichText::new(nombre_de(ruta)).font(negrita(17.0)).color(TINTA)).truncate())
                                    .on_hover_text(ruta.display().to_string());
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    let tipo = if nucleo::es_pdf(ruta) { "PDF" } else { "Word" };
                                    let texto = match self.vista.estado(ruta) {
                                        Some(vista::Estado::Lista { total, completa: true, .. }) => format!("{tipo} · {} ·", paginas_en_texto(*total)),
                                        _ => format!("{tipo} ·"),
                                    };
                                    ui.label(ayuda(&texto));
                                    if ui.link(RichText::new("Ver").font(negrita(15.0))).on_hover_text("Abrir este documento para revisarlo").clicked() {
                                        abrir(ruta);
                                    }
                                });
                            });
                        });
                    });
                });
                filas.push(fila.response.rect);
            }

            // Mientras se arrastra, una línea marca dónde va a quedar el documento.
            if let (Some(origen), Some(puntero)) = (self.arrastre, ui.ctx().pointer_interact_pos()) {
                let destino = filas.iter().filter(|f| f.center().y < puntero.y).count();
                let y = if destino < filas.len() { filas[destino].top() - 4.0 } else { filas[filas.len() - 1].bottom() + 4.0 };
                let (x0, x1) = (filas[0].left(), filas[0].right());
                ui.painter().line_segment([egui::pos2(x0, y), egui::pos2(x1, y)], Stroke::new(3.0, marca().primario));
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                if soltar {
                    let hasta = if destino > origen { destino - 1 } else { destino };
                    if hasta != origen {
                        let doc = self.archivos.remove(origen);
                        self.archivos.insert(hasta, doc);
                        self.aviso = Aviso::Nada;
                    }
                }
            }
        });
        if soltar || !ui.input(|i| i.pointer.any_down()) {
            self.arrastre = None;
        }

        if accion.is_some() {
            // El aviso «Listo» ya no corresponde a la lista que se ve.
            self.aviso = Aviso::Nada;
            self.nota_agregar = None;
        }
        match accion {
            Some((i, -1)) => self.archivos.swap(i, i - 1),
            Some((i, 1)) => self.archivos.swap(i, i + 1),
            Some((i, _)) => {
                let ruta = self.archivos.remove(i);
                self.quitado = Some((i, ruta, Instant::now()));
            }
            None => {}
        }
    }

    fn paso_guardar(&mut self, ui: &mut egui::Ui, ocupado: bool) {
        let ctx = ui.ctx().clone();
        let total = self.vista.total(&self.archivos).filter(|_| !self.archivos.is_empty());
        let aclaracion = total.map(|n| format!("{} en total", paginas_en_texto(n))).unwrap_or_default();
        paso(ui, 3, "Guardá el PDF final", &aclaracion);
        let listo = match &self.aviso {
            Aviso::Listo(ruta, _) => Some(ruta.clone()),
            _ => None,
        };
        if !self.nombre_editado {
            self.nombre_pdf = self.nombre_sugerido();
        }
        let pide_nombre = listo.is_none() && !ocupado && !self.archivos.is_empty();
        if pide_nombre {
            ui.label(RichText::new("Nombre del PDF").font(negrita(15.0)).color(TINTA));
        }
        ui.horizontal(|ui| {
            if pide_nombre {
                let ancho = (ui.available_width() - 250.0).max(180.0);
                let campo = ui.add(
                    egui::TextEdit::singleline(&mut self.nombre_pdf)
                        .desired_width(ancho)
                        .margin(Margin::symmetric(12, 12))
                        .font(FontId::proportional(17.0)),
                );
                if campo.changed() {
                    self.nombre_editado = true;
                }
            }
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
                boton_guardar(ui, false);
                ui.add_space(8.0);
                ui.label(ayuda("Primero agregá al menos un documento."));
            } else {
                if boton_guardar(ui, true).clicked() {
                    self.exportar(&ctx);
                }
            }
        });
        if pide_nombre {
            ui.label(ayuda(if cfg!(target_os = "macos") {
                "Al guardar se abre la carpeta «Armador de PDF» de Documentos; ahí podés elegir otra."
            } else {
                "Al guardar se abre la carpeta «exportaciones» del programa; ahí podés elegir otra."
            }));
        }
        match &self.aviso {
            Aviso::Nada => {}
            Aviso::Avance(t) => {
                ui.add_space(4.0);
                aviso(ui, marca().tinte, icono_avance, t, "Puede tardar unos segundos.");
            }
            Aviso::Listo(ruta, n) => {
                ui.add_space(4.0);
                aviso(ui, EXITO_FONDO, icono_listo, "Listo.", &format!("Se guardó «{}», de {}.", nombre_de(ruta), paginas_en_texto(*n)));
            }
            Aviso::Error(titulo, detalle) => {
                ui.add_space(4.0);
                aviso(ui, ERROR_FONDO, icono_error, titulo, detalle);
            }
        }
    }

    fn panel_vista(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Vista previa").font(negrita(20.0)).color(TINTA));
            if let Some(n) = self.vista.total(&self.archivos).filter(|_| !self.archivos.is_empty()) {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(ayuda(&paginas_en_texto(n)));
                });
            }
        });
        if self.archivos.is_empty() {
            ui.label(ayuda("Acá vas a ver cómo queda el PDF final."));
            return;
        }
        if self.vista.hay_error(&self.archivos) {
            ui.label(ayuda("Algunos documentos no se pueden mostrar acá, pero igual se unen."));
        }
        ui.add_space(4.0);
        egui::ScrollArea::vertical().id_salt("vista").auto_shrink([false, false]).show(ui, |ui| {
            // Número de página dentro del PDF final; se pierde si un documento anterior todavía no se conoce.
            let mut anteriores = Some(0usize);
            let separacion = 10.0;
            for (i, ruta) in self.archivos.iter().enumerate() {
                ui.add(egui::Label::new(RichText::new(format!("{} · {}", i + 1, nombre_de(ruta))).font(negrita(15.0)).color(TINTA)).truncate());
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
                        let ancho = ((ui.available_width() - separacion - 14.0) / 2.0).floor();
                        for (fila, par) in paginas.chunks(2).enumerate() {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = separacion;
                                for (k, pagina) in par.iter().enumerate() {
                                    ui.vertical(|ui| {
                                        ui.set_width(ancho);
                                        ui.spacing_mut().item_spacing.y = 2.0;
                                        let alto = ancho / pagina.aspect_ratio();
                                        let (r, _) = ui.allocate_exact_size(egui::vec2(ancho, alto), Sense::hover());
                                        egui::Image::new(pagina).paint_at(ui, r);
                                        ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, BORDE), StrokeKind::Inside);
                                        let en_documento = fila * 2 + k + 1;
                                        let etiqueta = match anteriores {
                                            Some(a) => format!("Página {}", a + en_documento),
                                            None => format!("Página {en_documento} del documento"),
                                        };
                                        ui.label(ayuda(&etiqueta));
                                    });
                                }
                            });
                        }
                        if *completa && *total > paginas.len() {
                            ui.label(ayuda(&format!("… y {} más de este documento.", paginas_en_texto(total - paginas.len()))));
                        }
                        anteriores = if *completa { anteriores.map(|a| a + total) } else { None };
                    }
                }
                ui.add_space(12.0);
            }
        });
    }

    /// Solo para pruebas: con ARMADOR_CAPTURA=archivo.ppm guarda una imagen de la ventana y cierra.
    /// ARMADOR_DEMO elige qué mostrar: inicio, historial, vacio, lista, listo, error, avance o ayuda.
    fn captura_de_prueba(&mut self, ctx: &egui::Context) {
        let Ok(destino) = std::env::var("ARMADOR_CAPTURA") else { return };
        self.cuadros += 1;
        if self.cuadros == 1 {
            self.demo = true;
            let demo = std::env::var("ARMADOR_DEMO").unwrap_or_default();
            // Con los documentos de demostración del repositorio la vista previa muestra páginas reales.
            let carpeta = Path::new("tests/datos/demo");
            let documentos: Vec<PathBuf> = ["Nota de elevación.docx", "Proyecto de ordenanza.pdf"]
                .iter()
                .map(|n| if carpeta.join(n).is_file() { carpeta.join(n) } else { PathBuf::from(n) })
                .collect();
            let ahora = chrono::Local::now().timestamp();
            let registro = |hace: i64, paginas: usize, salida: &str, entradas: &[&str]| Registro {
                cuando: ahora - hace,
                paginas,
                salida: PathBuf::from(salida),
                entradas: entradas.iter().map(PathBuf::from).collect(),
            };
            self.historial = vec![
                registro(3_600, 14, "Nota 123 - completo.pdf", &["Nota 123.docx", "Proyecto de ordenanza.pdf"]),
                registro(90_000, 6, "Pedido de informes - completo.pdf", &["Pedido de informes.docx", "Anexo.pdf"]),
                registro(190_000, 22, "Nota 119 - completo.pdf", &["Nota 119.docx", "Proyecto de resolución.pdf", "Planilla.pdf"]),
                registro(900_000, 9, "Nota 117 - completo.pdf", &["Nota 117.docx", "Proyecto.pdf"]),
            ];
            self.pantalla = match demo.as_str() {
                "inicio" | "" => Pantalla::Inicio,
                "historial" => Pantalla::Historial,
                "ayuda" => Pantalla::Ayuda,
                _ => Pantalla::Armar,
            };
            if matches!(demo.as_str(), "lista" | "listo" | "error" | "avance") {
                self.archivos = documentos;
            }
            match demo.as_str() {
                "listo" => self.aviso = Aviso::Listo("Nota de elevación - completo.pdf".into(), 4),
                "error" => {
                    self.aviso = Aviso::Error(
                        "No se pudo guardar.".into(),
                        "«Proyecto de ordenanza.pdf» tiene contraseña y no se puede unir. Pedí una copia sin contraseña y volvé a agregarla.".into(),
                    )
                }
                "avance" => self.aviso = Aviso::Avance("Pasando a PDF «Nota de elevación.docx»…".into()),
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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.recibir();
        self.captura_de_prueba(ui.ctx());
        let ocupado = self.trabajo.is_some();
        let ctx = ui.ctx().clone();

        let soltados: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !soltados.is_empty() && !ocupado {
            if self.pantalla == Pantalla::Inicio {
                self.empezar_de_nuevo();
            }
            self.pantalla = Pantalla::Armar;
            self.agregar(soltados);
        }
        let arrastrando = ctx.input(|i| !i.raw.hovered_files.is_empty());

        self.vista.actualizar(&ctx, &self.archivos);
        self.pestanas(ui);
        let cuerpo = Frame::new().fill(FONDO).inner_margin(Margin { left: MARGEN, right: MARGEN, top: 28, bottom: MARGEN });

        match self.pantalla {
            Pantalla::Inicio => {
                egui::Panel::left("inicio-izquierda")
                    .exact_size(448.0)
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(Frame::new().fill(FONDO).inner_margin(Margin { left: MARGEN, right: 8, top: 28, bottom: MARGEN }))
                    .show(ui, |ui| self.inicio_izquierda(ui));
                egui::CentralPanel::default().frame(cuerpo).show(ui, |ui| self.inicio_derecha(ui));
            }
            Pantalla::Historial => {
                egui::CentralPanel::default().frame(cuerpo).show(ui, |ui| self.pantalla_historial(ui));
            }
            Pantalla::Ayuda => {
                egui::CentralPanel::default().frame(cuerpo).show(ui, |ui| self.pantalla_ayuda(ui));
            }
            Pantalla::Armar => {
                egui::Panel::right("vista")
                    .exact_size(372.0)
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
        }

        if arrastrando && !ocupado {
            let area = ctx.content_rect();
            let pintor = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("soltar")));
            pintor.rect_filled(area, 0.0, FONDO);
            let zona = area.shrink(MARGEN as f32);
            pintor.rect_filled(zona, RADIO_MD as f32, marca().tinte);
            borde_punteado(&pintor, zona, Stroke::new(3.0, marca().primario));
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

#[cfg(test)]
mod pruebas {
    use super::nombre_de_archivo;

    #[test]
    fn limpia_el_nombre_del_pdf() {
        assert_eq!(nombre_de_archivo("  Nota 123 - completo  ").as_deref(), Some("Nota 123 - completo"));
        assert_eq!(nombre_de_archivo("Nota 123.PDF").as_deref(), Some("Nota 123"));
        assert_eq!(nombre_de_archivo("exp 12/2026: nota").as_deref(), Some("exp 12-2026- nota"));
        assert_eq!(nombre_de_archivo("   "), None);
        assert_eq!(nombre_de_archivo(".pdf"), None);
    }
}
