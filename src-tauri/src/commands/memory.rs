//! Preferências globais e memória dos agentes (`<raiz>/Argus`, ver `argus_core::memory`).
use super::{blocking, err, system::open_with_system};
use argus_core::memory::{self, MemoryNote};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct MemoryInfo {
    pub root: PathBuf,
    /// Pasta `<raiz>/Argus` (existe = memória ativa nos agentes).
    pub dir: PathBuf,
    pub active: bool,
    pub prefs_path: PathBuf,
    pub index_path: PathBuf,
    /// Texto de `Preferencias.md` (None = ainda não existe).
    pub prefs: Option<String>,
    pub notes: Vec<MemoryNote>,
}

fn current_root() -> Result<PathBuf, String> {
    memory::root().ok_or_else(|| "Abra uma pasta raiz primeiro.".to_string())
}

/// Preferências e notas da memória da raiz atual.
#[tauri::command]
pub async fn memory_get() -> Result<MemoryInfo, String> {
    blocking(|| {
        let root = current_root()?;
        Ok(MemoryInfo {
            dir: memory::dir(&root),
            active: memory::active(&root),
            prefs_path: memory::prefs_path(&root),
            index_path: memory::index_path(&root),
            prefs: memory::read_prefs(&root).map_err(err)?,
            notes: memory::list_notes(&root).map_err(err)?,
            root,
        })
    })
    .await
}

/// Grava `Preferencias.md` (cria a pasta `Argus`, o índice e a pasta de notas se faltarem).
#[tauri::command]
pub async fn memory_set_prefs(text: String) -> Result<(), String> {
    blocking(move || {
        let root = current_root()?;
        memory::write_prefs(&root, &text).map_err(|e| format!("Erro ao salvar as preferências: {e}"))
    })
    .await
}

/// Abre um arquivo da memória no Obsidian (`obsidian://open?path=...`).
#[tauri::command]
pub async fn memory_open_obsidian(path: PathBuf) -> Result<(), String> {
    let root = current_root()?;
    if !memory::inside(&root, &path) {
        return Err("Só é possível abrir arquivos da pasta Argus da raiz.".into());
    }
    open_with_system(memory::obsidian_url(&path)).await
}
