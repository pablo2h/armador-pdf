//! Paquete de marca: nombre, colores y logo de la organización que usa el programa.
//!
//! Se lee de una carpeta `marca/` junto al ejecutable (o de la que indique ARMADOR_MARCA).
//! Si no existe, el programa usa su aspecto neutro. Formato de `marca.txt`:
//!
//! ```text
//! nombre   = Nombre de la organización
//! primario = #6B2D5C      botón principal, pestaña activa, zona para soltar
//! acento   = #F2A541      número de cada paso
//! logo     = logo.png     PNG con fondo transparente, se muestra arriba a la derecha
//! ```

use eframe::egui::Color32;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Marca {
    pub nombre: String,
    pub primario: Color32,
    pub acento: Color32,
    /// Fondo suave derivado del primario (avisos de avance, zona para soltar).
    pub tinte: Color32,
    pub logo: Option<PathBuf>,
}

impl Default for Marca {
    fn default() -> Self {
        let primario = Color32::from_rgb(0x14, 0x60, 0x7A);
        Self { nombre: String::new(), primario, acento: Color32::from_rgb(0x29, 0x9D, 0xBE), tinte: tinte_de(primario), logo: None }
    }
}

/// Lee `#RRGGBB` (con o sin numeral).
pub fn color_hex(texto: &str) -> Option<Color32> {
    let h = texto.trim().trim_start_matches('#');
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let n = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some(Color32::from_rgb(n(0)?, n(2)?, n(4)?))
}

fn tinte_de(c: Color32) -> Color32 {
    let m = |v: u8| (255.0 - (255.0 - v as f32) * 0.10).round() as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

fn luminancia(c: Color32) -> f32 {
    let l = |v: u8| {
        let x = v as f32 / 255.0;
        if x <= 0.03928 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * l(c.r()) + 0.7152 * l(c.g()) + 0.0722 * l(c.b())
}

/// Contraste WCAG entre dos colores (1 a 21).
pub fn contraste(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminancia(a), luminancia(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Color de texto que mejor se lee sobre `fondo`: blanco o la tinta oscura.
pub fn texto_sobre(fondo: Color32, tinta: Color32) -> Color32 {
    if contraste(fondo, Color32::WHITE) >= contraste(fondo, tinta) { Color32::WHITE } else { tinta }
}

/// Interpreta el contenido de `marca.txt`. Las claves desconocidas y los colores inválidos se ignoran.
pub fn interpretar(texto: &str, carpeta: &Path) -> Marca {
    let mut m = Marca::default();
    for linea in texto.lines() {
        let linea = linea.trim();
        if linea.is_empty() || linea.starts_with('#') {
            continue;
        }
        let Some((clave, valor)) = linea.split_once('=') else { continue };
        let valor = valor.trim();
        match clave.trim().to_lowercase().as_str() {
            "nombre" => m.nombre = valor.to_string(),
            "primario" => {
                if let Some(c) = color_hex(valor) {
                    m.primario = c;
                    m.tinte = tinte_de(c);
                }
            }
            "acento" => {
                if let Some(c) = color_hex(valor) {
                    m.acento = c;
                }
            }
            "logo" => {
                let ruta = carpeta.join(valor);
                if ruta.is_file() {
                    m.logo = Some(ruta);
                }
            }
            _ => {}
        }
    }
    m
}

/// Carga la marca desde la carpeta `marca/` junto al ejecutable; si no hay, la neutra.
pub fn cargar() -> Marca {
    let carpeta = std::env::var_os("ARMADOR_MARCA")
        .map(PathBuf::from)
        .or_else(|| Some(crate::nucleo::carpeta_de_recursos()?.join("marca")));
    let Some(carpeta) = carpeta else { return Marca::default() };
    match std::fs::read_to_string(carpeta.join("marca.txt")) {
        Ok(texto) => interpretar(&texto, &carpeta),
        Err(_) => Marca::default(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_colores_hex() {
        assert_eq!(color_hex("#203961"), Some(Color32::from_rgb(0x20, 0x39, 0x61)));
        assert_eq!(color_hex(" 299dbe "), Some(Color32::from_rgb(0x29, 0x9D, 0xBE)));
        assert_eq!(color_hex("#abc"), None);
        assert_eq!(color_hex("celeste"), None);
    }

    #[test]
    fn interpreta_un_paquete_de_marca() {
        let m = interpretar("# comentario\nnombre = Ejemplo\nprimario = #203961\nacento=#299DBE\nlogo = no-existe.png\notra = cosa\n", Path::new("."));
        assert_eq!(m.nombre, "Ejemplo");
        assert_eq!(m.primario, Color32::from_rgb(0x20, 0x39, 0x61));
        assert_eq!(m.acento, Color32::from_rgb(0x29, 0x9D, 0xBE));
        assert_eq!(m.logo, None, "un logo que no existe se ignora");
        assert_ne!(m.tinte, Marca::default().tinte, "el tinte se deriva del primario");
    }

    #[test]
    fn un_color_invalido_no_rompe_la_marca() {
        let m = interpretar("primario = azulado\n", Path::new("."));
        assert_eq!(m.primario, Marca::default().primario);
    }

    #[test]
    fn la_marca_incluida_cumple_contraste() {
        let carpeta = Path::new(env!("CARGO_MANIFEST_DIR")).join("marcas").join("ejemplo");
        let m = interpretar(&std::fs::read_to_string(carpeta.join("marca.txt")).unwrap(), &carpeta);
        assert!(m.logo.is_some(), "falta el logo");
        let tinta = Color32::from_rgb(0x1B, 0x27, 0x33);
        assert!(contraste(m.primario, Color32::WHITE) >= 4.5, "texto blanco sobre el botón principal");
        assert!(contraste(m.acento, texto_sobre(m.acento, tinta)) >= 4.5, "número de paso");
        assert!(contraste(m.tinte, tinta) >= 4.5, "texto sobre el tinte");
    }

    #[test]
    fn elige_texto_legible_sobre_cada_fondo() {
        let tinta = Color32::from_rgb(0x1B, 0x27, 0x33);
        assert_eq!(texto_sobre(Color32::from_rgb(0x20, 0x39, 0x61), tinta), Color32::WHITE);
        assert_eq!(texto_sobre(Color32::from_rgb(0xFF, 0xE0, 0x80), tinta), tinta);
    }
}
