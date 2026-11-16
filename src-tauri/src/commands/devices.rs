//! Celulares Android: o Screen-Mirror roda como processo auxiliar e o painel dele
//! é embutido na aba Dispositivos. Ver `argus_core::devices`.
use super::{blocking, err};
use crate::state::AppCtx;
use argus_core::devices::{DeviceList, MirrorStatus};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

fn root(ctx: &AppCtx) -> Option<PathBuf> {
    ctx.root.lock().unwrap().clone()
}

/// Estado do Screen-Mirror: pasta, Node, servidor no ar e último erro.
#[tauri::command]
pub async fn devices_status(app: AppHandle) -> Result<MirrorStatus, String> {
    blocking(move || {
        let ctx = app.state::<AppCtx>();
        Ok(ctx.mirror.status(root(&ctx).as_deref()))
    })
    .await
}

/// Inicia o Screen-Mirror (se ainda não estiver no ar) e devolve o estado com a URL.
#[tauri::command]
pub async fn devices_start(app: AppHandle) -> Result<MirrorStatus, String> {
    blocking(move || {
        let ctx = app.state::<AppCtx>();
        ctx.mirror.start(root(&ctx).as_deref()).map_err(err)
    })
    .await
}

/// Encerra o Screen-Mirror.
#[tauri::command]
pub async fn devices_stop(ctx: State<'_, AppCtx>) -> Result<(), String> {
    ctx.mirror.stop();
    Ok(())
}

/// Salva a pasta do projeto Screen-Mirror escolhida pelo usuário.
#[tauri::command]
pub async fn devices_set_dir(app: AppHandle, dir: PathBuf) -> Result<MirrorStatus, String> {
    blocking(move || {
        let ctx = app.state::<AppCtx>();
        ctx.mirror.set_dir(&dir).map_err(err)?;
        let root = root(&ctx);
        ctx.mirror.sync_agent_mcp(root.as_deref());
        Ok(ctx.mirror.status(root.as_deref()))
    })
    .await
}

/// Liga/desliga o MCP do Screen-Mirror nas conversas de agente (vale do próximo turno em diante).
#[tauri::command]
pub async fn devices_set_agents(app: AppHandle, on: bool) -> Result<MirrorStatus, String> {
    blocking(move || {
        let ctx = app.state::<AppCtx>();
        ctx.mirror.set_agents(on).map_err(err)?;
        let root = root(&ctx);
        ctx.mirror.sync_agent_mcp(root.as_deref());
        Ok(ctx.mirror.status(root.as_deref()))
    })
    .await
}

/// Aparelhos conectados por ADB.
#[tauri::command]
pub async fn devices_list(app: AppHandle) -> Result<DeviceList, String> {
    blocking(move || app.state::<AppCtx>().mirror.devices().map_err(err)).await
}
