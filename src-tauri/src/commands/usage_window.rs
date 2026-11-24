//! Janelas de 5 horas do plugin `usage-window` do OpenCode.
//!
//! O plugin já calcula o consumo; aqui só se lê o estado dele e se aplicam os
//! ajustes manuais (`set_limit` e `reset_samples`) na interface. Leitura e
//! escrita de disco vão para `spawn_blocking`.

use super::blocking;
use argus_core::usage_window::{self, UsageWindowInfo};

/// Janelas ativas, com percentual calculado pelo mesmo critério do plugin.
#[tauri::command]
pub async fn usage_windows() -> Result<Vec<UsageWindowInfo>, String> {
    blocking(usage_window::list_windows).await
}

/// Define limite manual de tokens e/ou requisições para uma janela.
#[tauri::command]
pub async fn usage_window_set_limit(key: String, tokens: Option<u64>, requests: Option<u64>) -> Result<(), String> {
    blocking(move || usage_window::set_limit(&key, tokens, requests)).await
}

/// Apaga amostras e limite manual de uma janela.
#[tauri::command]
pub async fn usage_window_reset_samples(key: String) -> Result<(), String> {
    blocking(move || usage_window::reset_samples(&key)).await
}
