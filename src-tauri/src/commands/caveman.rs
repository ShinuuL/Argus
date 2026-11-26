//! Comandos da integração com o CLI `caveman`.

use super::blocking;
use argus_core::caveman::{CavemanResult, CavemanStatus};
use tauri::State;

use crate::state::AppCtx;

#[tauri::command]
pub async fn caveman_status(_ctx: State<'_, AppCtx>) -> Result<CavemanStatus, String> {
    blocking(|| Ok(argus_core::caveman::status())).await
}

/// Comprime um trecho escolhido na conversa. O texto volta junto com o
/// original para a interface oferecer as duas versões.
#[tauri::command]
pub async fn caveman_compress(_ctx: State<'_, AppCtx>, text: String) -> Result<CavemanResult, String> {
    blocking(move || argus_core::caveman::compress(&text)).await
}
