//! Vista previa: dibuja en segundo plano las páginas de cada documento de la lista.

use crate::nucleo;
use eframe::egui;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings, render};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

/// Ancho en píxeles al que se dibuja cada página.
const ANCHO_PAGINA: f32 = 420.0;
/// Páginas que se dibujan como máximo por documento; del resto solo se informa la cantidad.
pub const MAX_PAGINAS: usize = 30;

pub enum Estado {
    Preparando,
    Lista { paginas: Vec<egui::TextureHandle>, total: usize, completa: bool },
    Error,
}

enum Parte {
    Pagina(egui::ColorImage, usize),
    Fin,
    Error,
}

#[derive(Default)]
pub struct Vista {
    docs: HashMap<PathBuf, Estado>,
    canal: Option<(Sender<PathBuf>, Receiver<(PathBuf, Parte)>)>,
}

impl Vista {
    pub fn estado(&self, ruta: &Path) -> Option<&Estado> {
        self.docs.get(ruta)
    }

    /// Cantidad total de páginas del PDF final, si ya se conocen todas.
    pub fn total(&self, archivos: &[PathBuf]) -> Option<usize> {
        archivos
            .iter()
            .map(|a| match self.docs.get(a) {
                Some(Estado::Lista { total, completa: true, .. }) => Some(*total),
                _ => None,
            })
            .sum()
    }

    /// Indica si algún documento de la lista no se pudo mostrar.
    pub fn hay_error(&self, archivos: &[PathBuf]) -> bool {
        archivos.iter().any(|a| matches!(self.docs.get(a), Some(Estado::Error)))
    }

    /// Recibe páginas ya dibujadas y pide las de los documentos nuevos. Se llama en cada cuadro.
    pub fn actualizar(&mut self, ctx: &egui::Context, archivos: &[PathBuf]) {
        let (pedidos, resultados) = self.canal.get_or_insert_with(|| {
            let (tx_pedido, rx_pedido) = channel::<PathBuf>();
            let (tx_parte, rx_parte) = channel();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                for ruta in rx_pedido {
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        dibujar(&ruta, |img, total| {
                            let _ = tx_parte.send((ruta.clone(), Parte::Pagina(img, total)));
                            ctx.request_repaint();
                        })
                    }));
                    let fin = if matches!(r, Ok(Ok(()))) { Parte::Fin } else { Parte::Error };
                    let _ = tx_parte.send((ruta, fin));
                    ctx.request_repaint();
                }
            });
            (tx_pedido, rx_parte)
        });

        for (ruta, parte) in resultados.try_iter() {
            let Some(estado) = self.docs.get_mut(&ruta) else { continue };
            match parte {
                Parte::Pagina(img, total) => {
                    let textura = ctx.load_texture("pagina", img, egui::TextureOptions::LINEAR);
                    match estado {
                        Estado::Lista { paginas, .. } => paginas.push(textura),
                        _ => *estado = Estado::Lista { paginas: vec![textura], total, completa: false },
                    }
                }
                Parte::Fin => {
                    if let Estado::Lista { completa, .. } = estado {
                        *completa = true;
                    } else {
                        *estado = Estado::Error;
                    }
                }
                Parte::Error => *estado = Estado::Error,
            }
        }

        self.docs.retain(|ruta, _| archivos.contains(ruta));
        for a in archivos {
            if !self.docs.contains_key(a) {
                self.docs.insert(a.clone(), Estado::Preparando);
                let _ = pedidos.send(a.clone());
            }
        }
    }
}

/// Dibuja las primeras páginas de un documento (convirtiéndolo antes si es de Word).
fn dibujar(ruta: &Path, mut entregar: impl FnMut(egui::ColorImage, usize)) -> Result<(), String> {
    let pdf_ruta = nucleo::pdf_de(ruta)?;
    let datos = std::fs::read(&pdf_ruta).map_err(|e| e.to_string())?;
    let pdf = Pdf::new(datos).map_err(|_| "No se pudo leer el PDF".to_string())?;
    let paginas = pdf.pages();
    let total = paginas.len();
    if total == 0 {
        return Err("Sin páginas".into());
    }
    let cache = RenderCache::new();
    let interprete = InterpreterSettings::default();
    for pagina in paginas.iter().take(MAX_PAGINAS) {
        let (ancho, _) = pagina.render_dimensions();
        let escala = (ANCHO_PAGINA / ancho.max(1.0)).clamp(0.05, 4.0);
        let ajustes = PixmapSettings { x_scale: escala, y_scale: escala, bg_color: WHITE };
        let mapa = render(pagina, &cache, &interprete, &RenderSettings::default(), &ajustes);
        let tam = [mapa.width() as usize, mapa.height() as usize];
        entregar(egui::ColorImage::from_rgba_premultiplied(tam, mapa.data_as_u8_slice()), total);
    }
    Ok(())
}
