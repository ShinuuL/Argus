//! Comandos do fallback de imagem (Google AI Studio).
//! A chave vem do ambiente ou da tela e fica só na memória do processo;
//! nada aqui a devolve à interface.

use super::blocking;
use argus_core::imagem::{ImagemArquivo, ImagemDados, ImagemGerada, ImagemStatus};
use std::path::PathBuf;
use tauri::State;

use crate::state::AppCtx;

/// Chave configurada? A `source` é a origem (nome da variável ou "tela"), nunca o valor.
#[tauri::command]
pub async fn imagem_status(_ctx: State<'_, AppCtx>) -> Result<ImagemStatus, String> {
    blocking(|| Ok(argus_core::imagem::status())).await
}

/// Guarda a chave colada na tela apenas na memória, até o Argus reiniciar.
#[tauri::command]
pub async fn imagem_chave_definir(_ctx: State<'_, AppCtx>, key: String) -> Result<(), String> {
    blocking(move || argus_core::imagem::set_key_from_ui(key)).await
}

/// Esquece a chave da tela; a variável de ambiente continua valendo, se houver.
#[tauri::command]
pub async fn imagem_chave_limpar(_ctx: State<'_, AppCtx>) -> Result<(), String> {
    blocking(|| {
        argus_core::imagem::clear_key_from_ui();
        Ok(())
    })
    .await
}

/// Gera e salva uma imagem. Só é chamado por ação explícita do usuário.
#[tauri::command]
pub async fn imagem_gerar(_ctx: State<'_, AppCtx>, prompt: String) -> Result<ImagemGerada, String> {
    blocking(move || argus_core::imagem::generate(&prompt)).await
}

/// Galeria local, mais recente primeiro.
#[tauri::command]
pub async fn imagem_listar(_ctx: State<'_, AppCtx>) -> Result<Vec<ImagemArquivo>, String> {
    blocking(|| argus_core::imagem::list()).await
}

/// Lê uma imagem da galeria como base64 (para miniatura e lightbox).
#[tauri::command]
pub async fn imagem_ler(_ctx: State<'_, AppCtx>, path: PathBuf) -> Result<ImagemDados, String> {
    blocking(move || argus_core::imagem::read(&path)).await
}
