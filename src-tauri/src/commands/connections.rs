//! Conexões (gh, gcloud, firebase, vercel) e cartão do repositório GitHub.
//! Só consulta as CLIs; o login roda no terminal embutido, pela interface.
use super::blocking;
use argus_core::connections::{self, Connection, RepoInfo};
use std::path::PathBuf;

/// Estado de login de cada CLI (instalada, logada, conta).
#[tauri::command]
pub async fn connections_status() -> Result<Vec<Connection>, String> {
    blocking(|| Ok(connections::check_all())).await
}

/// Repositório GitHub do projeto e até 5 PRs abertos (null se não for repo do GitHub).
#[tauri::command]
pub async fn connections_repo(project: PathBuf) -> Result<Option<RepoInfo>, String> {
    blocking(move || connections::repo_info(&project)).await
}
