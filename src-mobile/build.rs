fn main() {
    // Android 15+ com páginas de 16 KB exige os segmentos LOAD das bibliotecas
    // nativas de 64 bits alinhados a 16 KB; sem isto o Android 16 avisa "Este app
    // não é compatível com 16 KB" ao abrir (visto num SM-M556B). Vai aqui, e não
    // em .cargo/config.toml, porque o `tauri android build` define
    // CARGO_TARGET_*_RUSTFLAGS e essa variável anula o rustflags do config.
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let width = std::env::var("CARGO_CFG_TARGET_POINTER_WIDTH").unwrap_or_default();
    if os == "android" && width == "64" {
        println!("cargo:rustc-link-arg=-Wl,-z,max-page-size=16384");
    }
    tauri_build::build()
}
