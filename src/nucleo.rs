//! Núcleo sin interfaz: convertir documentos a PDF (vía LibreOffice) y unir PDF.

use lopdf::{Dictionary, Document, Object, ObjectId};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

/// LibreOffice no admite dos conversiones a la vez con el mismo perfil.
static LIBREOFFICE: Mutex<()> = Mutex::new(());
/// Documentos ya convertidos en esta sesión: original -> (fecha de modificación, PDF).
static CONVERTIDOS: Mutex<Option<HashMap<PathBuf, (SystemTime, PathBuf)>>> = Mutex::new(None);
static NUMERO: AtomicUsize = AtomicUsize::new(0);

/// Tiempo máximo de espera por cada documento que convierte LibreOffice.
const SEGUNDOS_LIMITE: u64 = 180;

/// Extensiones que se aceptan además de PDF (las convierte LibreOffice).
pub const EXT_CONVERTIBLES: &[&str] = &["doc", "docx", "odt", "rtf"];

pub fn extension(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn es_pdf(p: &Path) -> bool {
    extension(p) == "pdf"
}

pub fn es_aceptado(p: &Path) -> bool {
    es_pdf(p) || EXT_CONVERTIBLES.contains(&extension(p).as_str())
}

// ---------------------------------------------------------------------------
// LibreOffice
// ---------------------------------------------------------------------------

/// Busca LibreOffice: primero junto al ejecutable (modo portable), después en el sistema.
pub fn buscar_libreoffice() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "soffice.exe" } else { "soffice" };

    if let Some(base) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        let candidatos = [
            base.join("libreoffice").join("program").join(exe),
            base.join("LibreOfficePortable").join("App").join("libreoffice").join("program").join(exe),
            base.join("libreoffice").join("App").join("libreoffice").join("program").join(exe),
            base.join("libreoffice").join("opt").join("libreoffice").join("program").join(exe),
        ];
        if let Some(c) = candidatos.into_iter().find(|c| c.is_file()) {
            return Some(c);
        }
        // Linux: un AppImage dentro de la carpeta libreoffice/
        if let Ok(dir) = std::fs::read_dir(base.join("libreoffice")) {
            for e in dir.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("AppImage")) {
                    return Some(p);
                }
            }
        }
    }

    let mut sistema: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Ok(pf) = std::env::var(var) {
                sistema.push(Path::new(&pf).join("LibreOffice").join("program").join(exe));
            }
        }
    } else {
        sistema.push("/usr/bin/soffice".into());
        sistema.push("/usr/local/bin/soffice".into());
        sistema.push("/opt/libreoffice/program/soffice".into());
        sistema.push("/Applications/LibreOffice.app/Contents/MacOS/soffice".into());
    }
    if let Some(c) = sistema.into_iter().find(|c| c.is_file()) {
        return Some(c);
    }
    // Último recurso: el PATH.
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).map(|d| d.join(exe)).find(|c| c.is_file())
    })
}

fn url_de_archivo(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let mut url = String::from("file://");
    if !s.starts_with('/') {
        url.push('/');
    }
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' | b':' => url.push(b as char),
            _ => url.push_str(&format!("%{b:02X}")),
        }
    }
    url
}

/// Carpeta temporal propia de esta ejecución.
pub fn carpeta_temporal() -> PathBuf {
    std::env::temp_dir().join(format!("armador-pdf-{}", std::process::id()))
}

pub fn limpiar_temporal() {
    let _ = std::fs::remove_dir_all(carpeta_temporal());
}

/// Termina LibreOffice y sus procesos hijos (en Windows, soffice.exe lanza soffice.bin).
fn matar_arbol(hijo: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &hijo.id().to_string()])
            .creation_flags(0x0800_0000)
            .status();
    }
    let _ = hijo.kill();
    let _ = hijo.wait();
}

/// Convierte un documento a PDF y devuelve la ruta del PDF generado.
pub fn convertir_a_pdf(soffice: &Path, entrada: &Path) -> Result<PathBuf, String> {
    // Cada conversión va a su propia carpeta: dos archivos con el mismo nombre no se pisan.
    let n = NUMERO.fetch_add(1, Ordering::Relaxed);
    let tmp = carpeta_temporal();
    let salida_dir = tmp.join(format!("conv-{n}"));
    let perfil = tmp.join("perfil");
    std::fs::create_dir_all(&salida_dir).map_err(|e| format!("No se pudo crear la carpeta temporal: {e}"))?;
    let _ = std::fs::create_dir_all(&perfil);

    let mut cmd = Command::new(soffice);
    // Perfil propio: evita chocar con un LibreOffice que el usuario tenga abierto.
    cmd.arg(format!("-env:UserInstallation={}", url_de_archivo(&perfil)))
        .args(["--headless", "--norestore", "--nolockcheck", "--convert-to", "pdf", "--outdir"])
        .arg(&salida_dir)
        .arg(entrada);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let mut hijo = cmd.spawn().map_err(|e| format!("No se pudo ejecutar LibreOffice: {e}"))?;
    let segundos = std::env::var("ARMADOR_LIMITE_SEG").ok().and_then(|v| v.parse().ok()).unwrap_or(SEGUNDOS_LIMITE);
    let limite = Instant::now() + Duration::from_secs(segundos);
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= limite => {
                matar_arbol(&mut hijo);
                return Err(format!(
                    "La conversión de «{}» tardó demasiado y se canceló. Probá de nuevo; si vuelve a pasar, abrí el documento en Word y guardalo como PDF.",
                    entrada.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(150)),
            Err(e) => return Err(format!("Falló la espera de LibreOffice: {e}")),
        }
    }

    let nombre = entrada.file_stem().ok_or("Nombre de archivo inválido")?;
    let pdf = salida_dir.join(format!("{}.pdf", nombre.to_string_lossy()));
    if pdf.is_file() {
        return Ok(pdf);
    }
    // LibreOffice a veces ajusta el nombre: tomar cualquier PDF que haya dejado.
    if let Ok(dir) = std::fs::read_dir(&salida_dir) {
        if let Some(p) = dir.flatten().map(|e| e.path()).find(|p| es_pdf(p)) {
            return Ok(p);
        }
    }
    Err(format!(
        "No se pudo pasar a PDF «{}». Revisá que el archivo no esté dañado ni abierto en otro programa.",
        entrada.file_name().unwrap_or_default().to_string_lossy()
    ))
}

// ---------------------------------------------------------------------------
// Unión de PDF
// ---------------------------------------------------------------------------

/// Atributos que una página puede heredar de sus nodos padre.
const HEREDABLES: &[&[u8]] = &[b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

/// Copia a la página los atributos heredados, porque al unir pierde a sus padres.
fn bajar_heredados(doc: &mut Document, pagina: ObjectId) {
    let mut faltan: Vec<&[u8]> = match doc.get_dictionary(pagina) {
        Ok(d) => HEREDABLES.iter().copied().filter(|k| !d.has(k)).collect(),
        Err(_) => return,
    };
    let mut hallados: Vec<(&[u8], Object)> = Vec::new();
    let mut actual = pagina;
    for _ in 0..64 {
        if faltan.is_empty() {
            break;
        }
        let Ok(d) = doc.get_dictionary(actual) else { break };
        let Ok(padre) = d.get(b"Parent").and_then(Object::as_reference) else { break };
        let Ok(dp) = doc.get_dictionary(padre) else { break };
        faltan.retain(|k| match dp.get(k) {
            Ok(v) => {
                hallados.push((k, v.clone()));
                false
            }
            Err(_) => true,
        });
        actual = padre;
    }
    if let Ok(d) = doc.get_dictionary_mut(pagina) {
        for (k, v) in hallados {
            d.set(k, v);
        }
    }
}

/// Une los PDF en el orden dado. Devuelve la cantidad total de páginas.
pub fn unir_pdfs(entradas: &[PathBuf], salida: &Path) -> Result<usize, String> {
    if entradas.is_empty() {
        return Err("No hay documentos para unir.".into());
    }
    let mut final_doc = Document::with_version("1.7");
    let raiz_paginas = final_doc.new_object_id();
    let mut hijos: Vec<Object> = Vec::new();

    for ruta in entradas {
        let nombre = ruta.file_name().unwrap_or_default().to_string_lossy().to_string();
        let mut doc = Document::load(ruta).map_err(|e| format!("No se pudo leer «{nombre}»: {e}"))?;
        if doc.is_encrypted() && doc.decrypt("").is_err() {
            return Err(format!("«{nombre}» tiene contraseña y no se puede unir. Pedí una copia sin contraseña y volvé a agregarla."));
        }
        let ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
        if ids.is_empty() {
            return Err(format!("«{nombre}» no tiene páginas."));
        }
        for id in &ids {
            bajar_heredados(&mut doc, *id);
        }
        doc.renumber_objects_with(final_doc.max_id + 1);
        let ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
        final_doc.max_id = final_doc.max_id.max(doc.max_id);
        for (id, obj) in std::mem::take(&mut doc.objects) {
            final_doc.max_id = final_doc.max_id.max(id.0);
            final_doc.objects.insert(id, obj);
        }
        for id in ids {
            if let Ok(d) = final_doc.get_dictionary_mut(id) {
                d.set("Parent", raiz_paginas);
            }
            hijos.push(Object::Reference(id));
        }
    }

    let total = hijos.len();
    let mut paginas = Dictionary::new();
    paginas.set("Type", Object::Name(b"Pages".to_vec()));
    paginas.set("Count", total as i64);
    paginas.set("Kids", hijos);
    final_doc.objects.insert(raiz_paginas, Object::Dictionary(paginas));

    let mut catalogo = Dictionary::new();
    catalogo.set("Type", Object::Name(b"Catalog".to_vec()));
    catalogo.set("Pages", raiz_paginas);
    let id_catalogo = final_doc.add_object(catalogo);
    final_doc.trailer.set("Root", id_catalogo);

    // Quita catálogos y árboles de páginas viejos, que quedaron sin referencia.
    final_doc.prune_objects();
    final_doc.compress();
    final_doc.save(salida).map_err(|e| format!("No se pudo guardar el PDF: {e}"))?;
    Ok(total)
}

fn modificado(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn ya_convertido(entrada: &Path) -> Option<PathBuf> {
    let mapa = CONVERTIDOS.lock().ok()?;
    let (fecha, pdf) = mapa.as_ref()?.get(entrada)?;
    (Some(*fecha) == modificado(entrada) && pdf.is_file()).then(|| pdf.clone())
}

/// Devuelve el PDF que corresponde a un documento: él mismo si ya es PDF, o su conversión.
/// Las conversiones se guardan durante la sesión, así la vista previa y el guardado no repiten trabajo.
pub fn pdf_de(entrada: &Path) -> Result<PathBuf, String> {
    let nombre = entrada.file_name().unwrap_or_default().to_string_lossy().to_string();
    if !entrada.is_file() {
        return Err(format!("No se encuentra el archivo «{nombre}»."));
    }
    if es_pdf(entrada) {
        return Ok(entrada.to_path_buf());
    }
    let _turno = LIBREOFFICE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(pdf) = ya_convertido(entrada) {
        return Ok(pdf);
    }
    let soffice = buscar_libreoffice().ok_or(
        "No se encontró LibreOffice. Tiene que estar en la carpeta «libreoffice» junto al programa, o instalado en el equipo.",
    )?;
    let fecha = modificado(entrada);
    let pdf = convertir_a_pdf(&soffice, entrada)?;
    if let (Some(fecha), Ok(mut mapa)) = (fecha, CONVERTIDOS.lock()) {
        mapa.get_or_insert_with(HashMap::new).insert(entrada.to_path_buf(), (fecha, pdf.clone()));
    }
    Ok(pdf)
}

/// Proceso completo: convierte lo que haga falta y une todo en `salida`.
/// `avisar` recibe mensajes de avance para mostrar al usuario.
/// Los temporales quedan hasta que se llame a [`limpiar_temporal`], al cerrar el programa.
pub fn armar(entradas: &[PathBuf], salida: &Path, mut avisar: impl FnMut(String)) -> Result<usize, String> {
    let mut pdfs: Vec<PathBuf> = Vec::with_capacity(entradas.len());
    for e in entradas {
        if e.is_file() && !es_pdf(e) && ya_convertido(e).is_none() {
            let nombre = e.file_name().unwrap_or_default().to_string_lossy().to_string();
            avisar(format!("Pasando a PDF «{nombre}»…"));
        }
        pdfs.push(pdf_de(e)?);
    }
    avisar("Uniendo documentos…".into());
    let r = unir_pdfs(&pdfs, salida);
    if let Ok(paginas) = r {
        anotar_en_historial(entradas, salida, paginas);
    }
    r
}

/// Archivo de texto, junto al programa, con un renglón por cada PDF armado.
pub fn ruta_del_historial() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ARMADOR_HISTORIAL") {
        return Some(PathBuf::from(p));
    }
    Some(std::env::current_exe().ok()?.parent()?.join("historial.txt"))
}

/// Deja constancia de qué se armó y con qué documentos. Si no se puede escribir, no interrumpe.
fn anotar_en_historial(entradas: &[PathBuf], salida: &Path, paginas: usize) {
    use std::io::Write;
    let Some(ruta) = ruta_del_historial() else { return };
    let nombres: Vec<String> = entradas
        .iter()
        .map(|e| e.file_name().unwrap_or_default().to_string_lossy().to_string())
        .collect();
    let renglon = format!(
        "{} | {} | {} páginas | {}\r\n",
        chrono::Local::now().format("%d/%m/%Y %H:%M"),
        salida.display(),
        paginas,
        nombres.join(" + ")
    );
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(ruta) {
        let _ = f.write_all(renglon.as_bytes());
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use lopdf::{Stream, dictionary};

    fn carpeta(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("armador-pdf-prueba-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// PDF mínimo cuyo tamaño de página está definido en el nodo padre (heredado).
    fn pdf_de_prueba(ruta: &Path, paginas: usize, ancho: i64) {
        let mut d = Document::with_version("1.5");
        let raiz = d.new_object_id();
        let mut hijos: Vec<Object> = Vec::new();
        for _ in 0..paginas {
            let contenido = d.add_object(Stream::new(dictionary! {}, Vec::new()));
            let pagina = d.add_object(dictionary! {
                "Type" => Object::Name(b"Page".to_vec()),
                "Parent" => raiz,
                "Contents" => contenido,
            });
            hijos.push(pagina.into());
        }
        d.objects.insert(
            raiz,
            Object::Dictionary(dictionary! {
                "Type" => Object::Name(b"Pages".to_vec()),
                "Kids" => hijos,
                "Count" => paginas as i64,
                "MediaBox" => vec![0.into(), 0.into(), ancho.into(), 842.into()],
            }),
        );
        let catalogo = d.add_object(dictionary! { "Type" => Object::Name(b"Catalog".to_vec()), "Pages" => raiz });
        d.trailer.set("Root", catalogo);
        d.save(ruta).unwrap();
    }

    fn anchos(pdf: &Path) -> Vec<i64> {
        let d = Document::load(pdf).unwrap();
        d.get_pages()
            .into_values()
            .map(|id| {
                let caja = d.get_dictionary(id).unwrap().get(b"MediaBox").expect("la página perdió su tamaño");
                caja.as_array().unwrap()[2].as_i64().unwrap()
            })
            .collect()
    }

    #[test]
    fn acepta_pdf_y_word_y_rechaza_el_resto() {
        for ok in ["a.pdf", "A.PDF", "nota.doc", "nota.DOCX", "x.odt", "x.rtf"] {
            assert!(es_aceptado(Path::new(ok)), "{ok}");
        }
        for mal in ["foto.jpg", "planilla.xlsx", "sin_extension", "a.pdf.exe"] {
            assert!(!es_aceptado(Path::new(mal)), "{mal}");
        }
    }

    #[test]
    fn la_url_de_archivo_escapa_espacios_y_acentos() {
        assert_eq!(url_de_archivo(Path::new("/tmp/mi carpeta/ñ")), "file:///tmp/mi%20carpeta/%C3%B1");
        assert_eq!(url_de_archivo(Path::new("C:\\Users\\Ana Paz\\x")), "file:///C:/Users/Ana%20Paz/x");
    }

    #[test]
    fn une_respetando_el_orden_y_el_tamano_de_pagina() {
        let d = carpeta("orden");
        let (a, b, salida) = (d.join("a.pdf"), d.join("b.pdf"), d.join("final.pdf"));
        pdf_de_prueba(&a, 2, 500);
        pdf_de_prueba(&b, 3, 600);

        assert_eq!(unir_pdfs(&[a.clone(), b.clone()], &salida).unwrap(), 5);
        assert_eq!(anchos(&salida), [500, 500, 600, 600, 600]);

        assert_eq!(unir_pdfs(&[b, a], &salida).unwrap(), 5);
        assert_eq!(anchos(&salida), [600, 600, 600, 500, 500]);
    }

    #[test]
    fn el_mismo_documento_puede_ir_dos_veces() {
        let d = carpeta("repetido");
        let (a, salida) = (d.join("a.pdf"), d.join("final.pdf"));
        pdf_de_prueba(&a, 2, 500);
        assert_eq!(unir_pdfs(&[a.clone(), a], &salida).unwrap(), 4);
        assert_eq!(anchos(&salida).len(), 4);
    }

    #[test]
    fn avisa_si_no_hay_documentos_o_si_un_archivo_no_es_pdf() {
        let d = carpeta("errores");
        assert!(unir_pdfs(&[], &d.join("final.pdf")).is_err());
        let falso = d.join("falso.pdf");
        std::fs::write(&falso, b"esto no es un PDF").unwrap();
        let e = unir_pdfs(&[falso], &d.join("final.pdf")).unwrap_err();
        assert!(e.contains("falso.pdf"), "{e}");
    }

    #[test]
    fn no_modifica_los_originales() {
        let d = carpeta("originales");
        let a = d.join("a.pdf");
        pdf_de_prueba(&a, 1, 500);
        let antes = std::fs::read(&a).unwrap();
        unir_pdfs(&[a.clone()], &d.join("final.pdf")).unwrap();
        assert_eq!(antes, std::fs::read(&a).unwrap());
    }

    /// Prueba de punta a punta con LibreOffice real: Word (.docx y .doc) + PDF.
    /// Si LibreOffice no está instalado se omite, salvo que ARMADOR_EXIGIR_LIBREOFFICE=1.
    #[test]
    fn convierte_word_y_une_con_un_pdf() {
        if buscar_libreoffice().is_none() {
            assert!(std::env::var("ARMADOR_EXIGIR_LIBREOFFICE").is_err(), "No se encontró LibreOffice");
            eprintln!("LibreOffice no está instalado: se omite la prueba de conversión.");
            return;
        }
        let d = carpeta("word");
        let datos = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("datos");
        let proyecto = d.join("proyecto.pdf");
        pdf_de_prueba(&proyecto, 3, 595);
        let salida = d.join("final.pdf");
        let historial = d.join("historial.txt");
        // SAFETY: es la única prueba que usa esta variable.
        unsafe { std::env::set_var("ARMADOR_HISTORIAL", &historial) };

        let entradas = [datos.join("nota de prueba.docx"), proyecto, datos.join("nota-vieja.doc")];
        let mut avisos = Vec::new();
        let paginas = armar(&entradas, &salida, |m| avisos.push(m)).unwrap();

        assert_eq!(paginas, 5, "1 de la nota + 3 del proyecto + 1 de la nota vieja");
        assert_eq!(Document::load(&salida).unwrap().get_pages().len(), 5);
        assert_eq!(avisos.iter().filter(|m| m.starts_with("Pasando a PDF")).count(), 2);
        let h = std::fs::read_to_string(&historial).unwrap();
        assert!(h.contains("5 páginas") && h.contains("nota de prueba.docx + proyecto.pdf + nota-vieja.doc"), "{h}");
    }
}
