//! Acesso remoto: servidor web para o celular, pareado por QR code.
//! O token de pareamento é do próprio Argus e pode ser revogado a qualquer momento.
//!
//! O padrão é publicar por Tailscale Funnel (HTTPS); a porta só é publicada enquanto o
//! servidor estiver ligado. Ver `argus_core::tailscale`.
use super::{blocking, err};
use crate::state::AppCtx;
use argus_core::remote::{self, LanAddr, RemoteInfo};
use argus_core::tailscale::{self, Exposure, TailscaleStatus};
use tauri::State;

/// Estado do servidor remoto (None se estiver desligado).
#[tauri::command]
pub async fn remote_info(ctx: State<'_, AppCtx>) -> Result<Option<RemoteInfo>, String> {
    Ok(ctx.remote.info().ok())
}

/// Liga o servidor na porta dada. Devolve a URL de pareamento e o QR code (SVG).
///
/// `exposure` ausente = Funnel (HTTPS na internet), o padrão do Argus.
#[tauri::command]
pub async fn remote_start(ctx: State<'_, AppCtx>, port: u16, exposure: Option<Exposure>) -> Result<RemoteInfo, String> {
    if port == 0 {
        return Err("Porta inválida.".into());
    }
    ctx.remote
        .start(ctx.orch.clone(), port, exposure.unwrap_or_default())
        .await
        .map_err(|e| format!("Falha ao iniciar: {e}"))
}

/// Estado do Tailscale nesta máquina (instalado, conectado, nome no tailnet).
#[tauri::command]
pub async fn remote_tailscale_status() -> Result<TailscaleStatus, String> {
    blocking(|| Ok(tailscale::status())).await
}

/// Desliga o servidor.
#[tauri::command]
pub async fn remote_stop(ctx: State<'_, AppCtx>) -> Result<(), String> {
    ctx.remote.stop();
    Ok(())
}

/// Gera um novo token (revoga o acesso de todos os aparelhos pareados).
#[tauri::command]
pub async fn remote_regenerate_token(ctx: State<'_, AppCtx>) -> Result<RemoteInfo, String> {
    ctx.remote.regenerate_token().map_err(err)
}

/// Escolhe o IP usado na URL de pareamento (None = automático).
#[tauri::command]
pub async fn remote_set_host(ctx: State<'_, AppCtx>, host: Option<String>) -> Result<RemoteInfo, String> {
    ctx.remote.set_host(host.filter(|h| !h.is_empty())).map_err(err)
}

/// IPs locais disponíveis (LAN primeiro, depois VPN/virtuais).
#[tauri::command]
pub async fn remote_hosts() -> Result<Vec<LanAddr>, String> {
    blocking(|| Ok(remote::lan_addrs())).await
}
