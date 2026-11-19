#!/bin/bash
# Arma dist/ArmadorPDF-macOS-<sufijo>.zip. Se corre en una Mac, desde la raíz del repositorio,
# con LibreOffice instalado en /Applications. Uso: bash empaquetar/mac.sh AppleSilicon
set -euo pipefail
sufijo="${1:?Falta el sufijo: AppleSilicon o Intel}"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

cargo build --release

carpeta="dist/ArmadorPDF-macOS"
app="$carpeta/Armador de PDF.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/armador-pdf "$app/Contents/MacOS/"
sed "s/@VERSION@/$version/g" assets/Info.plist > "$app/Contents/Info.plist"

# Ícono: el .icns se arma a partir del PNG.
iconos="dist/icono.iconset"
mkdir -p "$iconos"
sips -z 16 16 assets/icono.png --out "$iconos/icon_16x16.png" >/dev/null
sips -z 32 32 assets/icono.png --out "$iconos/icon_16x16@2x.png" >/dev/null
sips -z 32 32 assets/icono.png --out "$iconos/icon_32x32.png" >/dev/null
sips -z 64 64 assets/icono.png --out "$iconos/icon_32x32@2x.png" >/dev/null
sips -z 128 128 assets/icono.png --out "$iconos/icon_128x128.png" >/dev/null
sips -z 256 256 assets/icono.png --out "$iconos/icon_128x128@2x.png" >/dev/null
sips -z 256 256 assets/icono.png --out "$iconos/icon_256x256.png" >/dev/null
iconutil -c icns "$iconos" -o "$app/Contents/Resources/icono.icns"

# LibreOffice va entero y sin tocar: así conserva la firma de The Document Foundation.
ditto /Applications/LibreOffice.app "$app/Contents/Resources/LibreOffice.app"
cp empaquetar/LEEME-mac.txt "$carpeta/LEEME.txt"

xattr -cr "$carpeta"
codesign --force --sign - "$app"
codesign --verify --strict "$app" && echo "Firma local correcta"

ditto -c -k --keepParent "$carpeta" "dist/ArmadorPDF-macOS-$sufijo.zip"
ls -l "dist/ArmadorPDF-macOS-$sufijo.zip"
