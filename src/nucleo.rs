//! Núcleo sin interfaz: convertir documentos a PDF (vía LibreOffice) y unir PDF.

use lopdf::{Dictionary, Document, Object, ObjectId};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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
/// `n` distingue las salidas para que dos archivos con el mismo nombre no se pisen.
pub fn convertir_a_pdf(soffice: &Path, entrada: &Path, n: usize) -> Result<PathBuf, String> {
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
            return Err(format!("«{nombre}» está protegido con contraseña y no se puede unir."));
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

/// Proceso completo: convierte lo que haga falta y une todo en `salida`.
/// `avisar` recibe mensajes de avance para mostrar al usuario.
pub fn armar(entradas: &[PathBuf], salida: &Path, mut avisar: impl FnMut(String)) -> Result<usize, String> {
    let mut pdfs: Vec<PathBuf> = Vec::with_capacity(entradas.len());
    let mut soffice: Option<PathBuf> = None;
    for (n, e) in entradas.iter().enumerate() {
        let nombre = e.file_name().unwrap_or_default().to_string_lossy().to_string();
        if !e.is_file() {
            return Err(format!("No se encuentra el archivo «{nombre}»."));
        }
        if es_pdf(e) {
            pdfs.push(e.clone());
            continue;
        }
        if soffice.is_none() {
            soffice = Some(buscar_libreoffice().ok_or(
                "No se encontró LibreOffice. Tiene que estar en la carpeta «libreoffice» junto al programa, o instalado en el equipo.",
            )?);
        }
        avisar(format!("Convirtiendo «{nombre}» a PDF…"));
        pdfs.push(convertir_a_pdf(soffice.as_ref().unwrap(), e, n)?);
    }
    avisar("Uniendo documentos…".into());
    let r = unir_pdfs(&pdfs, salida);
    limpiar_temporal();
    r
}
