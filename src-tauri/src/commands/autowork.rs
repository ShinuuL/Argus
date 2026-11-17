//! Auto-work: configuração por projeto e controle da execução por tarefa.
//! O progresso chega no evento `orch` (variante `AutoWork`).
use crate::state::AppCtx;
use argus_core::agents::orchestrator::TaskId;
use argus_core::autowork::{AutoWorkEvent, AutoWorkSettings};
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct ProjectAutoWork {
    pub project: PathBuf,
    pub settings: AutoWorkSettings,
}

/// Configuração do auto-work de um projeto (padrões se nunca foi salva).
#[tauri::command]
pub async fn autowork_get_settings(ctx: State<'_, AppCtx>, project: PathBuf) -> Result<AutoWorkSettings, String> {
    Ok(ctx.autowork.settings(&project))
}

/// Salva a configuração do auto-work de um projeto.
#[tauri::command]
pub async fn autowork_set_settings(ctx: State<'_, AppCtx>, project: PathBuf, mut settings: AutoWorkSettings) -> Result<AutoWorkSettings, String> {
    settings.max_iterations = settings.max_iterations.max(1);
    ctx.autowork.set_settings(&project, settings.clone());
    Ok(settings)
}

/// Configurações salvas de todos os projetos.
#[tauri::command]
pub async fn autowork_all_settings(ctx: State<'_, AppCtx>) -> Result<Vec<ProjectAutoWork>, String> {
    Ok(ctx
        .autowork
        .all_settings()
        .into_iter()
        .map(|(project, settings)| ProjectAutoWork { project, settings })
        .collect())
}

/// Progresso do auto-work de uma tarefa (None se nunca rodou).
#[tauri::command]
pub async fn autowork_status(ctx: State<'_, AppCtx>, task: TaskId) -> Result<Option<AutoWorkEvent>, String> {
    Ok(ctx.autowork.status(task))
}

/// Progresso de todas as execuções de auto-work.
#[tauri::command]
pub async fn autowork_all_status(ctx: State<'_, AppCtx>) -> Result<Vec<AutoWorkEvent>, String> {
    Ok(ctx.autowork.all_status())
}

/// Inicia (ou reinicia) o auto-work numa tarefa. false se não foi possível.
#[tauri::command]
pub async fn autowork_start(ctx: State<'_, AppCtx>, task: TaskId) -> Result<bool, String> {
    Ok(ctx.autowork.start_task(task))
}

/// Pausa o auto-work (o turno atual termina).
#[tauri::command]
pub async fn autowork_pause(ctx: State<'_, AppCtx>, task: TaskId) -> Result<bool, String> {
    Ok(ctx.autowork.pause(task))
}

/// Retoma o auto-work pausado.
#[tauri::command]
pub async fn autowork_resume(ctx: State<'_, AppCtx>, task: TaskId) -> Result<bool, String> {
    Ok(ctx.autowork.resume(task))
}

/// Para o auto-work da tarefa.
#[tauri::command]
pub async fn autowork_stop(ctx: State<'_, AppCtx>, task: TaskId) -> Result<bool, String> {
    Ok(ctx.autowork.stop(task))
}
