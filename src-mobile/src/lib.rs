//! Argus Celular: a casca nativa (Tauri 2) do aplicativo do aparelho.
//!
//! A tela que vem embutida aqui é só o pareamento: o endereço e o token do
//! computador. Depois de parear, a webview vai para o próprio servidor do Argus
//! (`crates/core/assets/remote`), que é a interface de verdade — a mesma do
//! navegador, agora dentro do aplicativo, com acesso à galeria do aparelho pelo
//! seletor de arquivos do sistema.
//!
//! O aplicativo não guarda token nenhum do lado do Rust: o endereço pareado fica
//! no `localStorage` da tela de pareamento e o token, no cookie do servidor.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Argus Celular");
}
