// Impede a janela de console no Windows (o `cargo tauri android dev` roda daqui).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    argus_celular_lib::run()
}
