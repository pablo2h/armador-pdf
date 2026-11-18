// Incrusta el ícono y los datos de versión en el .exe de Windows.
fn main() {
    println!("cargo:rerun-if-changed=assets/armador.rc");
    println!("cargo:rerun-if-changed=assets/icono.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/armador.rc", embed_resource::NONE).manifest_optional().unwrap();
    }
}
