# Armador de PDF

[![Pruebas](https://github.com/pablo2h/armador-pdf/actions/workflows/pruebas.yml/badge.svg)](https://github.com/pablo2h/armador-pdf/actions/workflows/pruebas.yml)

Programa de escritorio para oficina que **junta varios documentos en un solo PDF**: por ejemplo, una nota y el proyecto que la acompaña. Si algún documento está en Word, lo pasa a PDF solo.

Funciona en **Windows y Linux**, sin instalar nada. Desarrollado por **Horno de Barro**.

![Pantalla principal con dos documentos cargados](docs/2-lista.png)

## Descargar

| Sistema | Archivo | Cómo abrirlo |
|---|---|---|
| **Windows** | [ArmadorPDF-Windows.zip](https://github.com/pablo2h/armador-pdf/releases/latest/download/ArmadorPDF-Windows.zip) (≈ 250 MB) | Descomprimir la carpeta y abrir `Armador de PDF.exe` |
| **Linux** | [ArmadorPDF-Linux.tar.xz](https://github.com/pablo2h/armador-pdf/releases/latest/download/ArmadorPDF-Linux.tar.xz) (≈ 170 MB) | Descomprimir la carpeta y abrir `armador-pdf` |

Los paquetes pesan porque traen LibreOffice adentro, que es lo que convierte los documentos de Word. Por eso **el programa tiene que quedar siempre dentro de su carpeta**, junto a la carpeta `libreoffice`.

> **Aviso de Windows.** El programa todavía no está firmado digitalmente, así que al abrirlo Windows puede mostrar «Windows protegió su PC». Tocá **Más información** y después **Ejecutar de todas formas**.

Todas las versiones están en [Releases](https://github.com/pablo2h/armador-pdf/releases), y los cambios de cada una en el [historial de cambios](CHANGELOG.md).

## Cómo se usa

### 1. Agregá los documentos

Tocá **Agregar documentos…** y elegí los archivos, o arrastralos desde una carpeta y soltalos sobre la ventana. Sirven archivos PDF y de Word (`.doc`, `.docx`).

![Pantalla vacía, lista para agregar documentos](docs/1-vacio.png)

### 2. Revisá el orden

Cada documento aparece con un número. El número 1 queda al principio del PDF final. Con **Subir** y **Bajar** se cambia el lugar; con **Quitar** se saca de la lista (el archivo original no se borra).

### 3. Guardá el PDF final

Tocá **Guardar PDF final…**, elegí carpeta y nombre, y esperá el aviso **Listo**. Desde ahí se puede abrir el PDF o su carpeta.

![Aviso de que el PDF se guardó](docs/3-listo.png)

Si algo falla, el programa dice qué pasó y qué hacer:

![Aviso de error por un PDF con contraseña](docs/4-error.png)

### Si el documento está en Google Docs

Abrilo y andá a **Archivo → Descargar → Documento PDF (.pdf)**, y agregá ese archivo. Si necesitás seguir editándolo en Word, también podés descargarlo como `.docx`: el programa lo pasa a PDF solo.

### Para tener en cuenta

- Los documentos originales **nunca se modifican ni se borran**.
- Cada PDF armado queda anotado en `historial.txt`, dentro de la carpeta del programa: fecha, dónde se guardó y con qué documentos.
- Los PDF con contraseña no se pueden unir.
- La primera vez que se pasa un Word a PDF puede tardar hasta un minuto.
- El mismo instructivo está dentro del programa, en la pestaña **Cómo se usa**.

![Instructivo dentro del programa](docs/5-ayuda.png)

## Qué hace y qué no

| Hace | No hace |
|---|---|
| Une PDF en el orden que elijas | Intercalar páginas sueltas de un documento dentro de otro |
| Pasa Word (`.doc`, `.docx`), ODT y RTF a PDF | Renumerar folios ni agregar numeración |
| Funciona sin internet y sin instalación | Conservar marcadores ni formularios rellenables de los PDF originales |
| Lleva un historial de lo armado | Abrir PDF protegidos con contraseña |

## Estado de las pruebas

| | Linux | Windows |
|---|---|---|
| Unir y ordenar PDF | Probado | Probado |
| Pasar Word a PDF | Probado | Probado en las pruebas automáticas (ver abajo) |
| Paquete portable con su LibreOffice | Probado | **Falta la prueba manual en una PC con Windows** |

Si lo probás en Windows y algo no anda, por favor [abrí un aviso](https://github.com/pablo2h/armador-pdf/issues/new) contando qué hiciste y qué mensaje apareció.

## Para quien desarrolla

### Tecnología

- **[Rust](https://www.rust-lang.org/)**: un solo ejecutable por sistema, sin dependencias.
- **[egui / eframe](https://github.com/emilk/egui)**: la ventana.
- **[lopdf](https://github.com/J-F-Liu/lopdf)**: unión de los PDF.
- **[rfd](https://github.com/PolyMeilex/rfd)**: diálogos de abrir y guardar del sistema.
- **[LibreOffice](https://es.libreoffice.org/)** en modo sin ventana: pasa Word a PDF. El programa lo busca primero junto al ejecutable (`libreoffice/`) y después instalado en el sistema.
- Tipografía **[Atkinson Hyperlegible](https://www.brailleinstitute.org/freefont/)**, incluida en el ejecutable (licencia OFL, en `assets/`).

### Estructura

```
src/nucleo.rs    conversión, unión, historial y pruebas (sin interfaz)
src/main.rs      la ventana y el sistema de diseño
assets/          tipografía
tests/datos/     documentos de Word para las pruebas
docs/            capturas de este README
```

### Compilar

```bash
cargo build --release
```

Para generar el `.exe` de Windows desde Linux hacen falta MinGW y el target de Rust:

```bash
rustup target add x86_64-pc-windows-gnu
```

```bash
cargo build --release --target x86_64-pc-windows-gnu
```

### Pruebas

```bash
cargo test --release
```

Son siete pruebas: formatos aceptados, orden de las páginas, tamaño de página heredado, documento repetido, errores, que los originales no se toquen y una prueba de punta a punta que pasa un `.docx` y un `.doc` a PDF con LibreOffice real y los une con otro PDF. Esa última se omite si LibreOffice no está instalado.

En cada cambio, GitHub las corre automáticamente **en Linux y en Windows**, con LibreOffice instalado en ambos: es la pestaña [Actions](https://github.com/pablo2h/armador-pdf/actions).

### Uso sin ventana

```bash
armador-pdf --salida final.pdf nota.docx proyecto.pdf
```

### Capturas

Las imágenes de este README las genera el propio programa:

```bash
ARMADOR_DEMO=lista ARMADOR_CAPTURA=captura.ppm armador-pdf
```

`ARMADOR_DEMO` admite `lista`, `listo`, `error`, `avance` y `ayuda`.

## Créditos y licencias

- Desarrollado por **Horno de Barro**.
- Los paquetes incluyen LibreOffice, de The Document Foundation, bajo licencia MPL 2.0; su licencia va dentro de la carpeta `libreoffice`.
- Atkinson Hyperlegible, del Braille Institute, bajo licencia SIL OFL 1.1.
