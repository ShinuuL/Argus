//! Comandos Tauri, um módulo por área. Todos devolvem `Result<T, String>`:
//! o erro chega à interface como a mensagem (em português) da exceção rejeitada.
pub mod agents;
pub mod autowork;
pub mod connections;
pub mod deps;
pub mod devices;
pub mod extra_agents;
pub mod extra_chat;
pub mod fs;
pub mod media;
pub mod metrics;
pub mod monitor;
pub mod pty;
pub mod remote;
pub mod runs;
pub mod servers;
pub mod system;
pub mod workspace;

/// Converte qualquer erro em mensagem para a interface.
pub fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Roda trabalho bloqueante (git, disco, processos) fora das threads do runtime.
pub async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?
}
