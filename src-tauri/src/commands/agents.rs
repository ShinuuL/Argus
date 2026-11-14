//! Provedores (CLIs oficiais) e tarefas do orquestrador.
//!
//! A IDE só executa as CLIs; o login é feito pela própria CLI num terminal
//! embutido. Nenhum token é lido ou guardado aqui.
use super::{blocking, err};
use crate::state::AppCtx;
use argus_core::agents::orchestrator::{Task, TaskId};
use argus_core::agents::provider::{AgentEvent, CommandSpec, LoginStatus, SubagentInfo};
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
}

/// Provedores disponíveis (claude, opencode).
#[tauri::command]
pub async fn agents_providers(ctx: State<'_, AppCtx>) -> Result<Vec<ProviderInfo>, String> {
    Ok(ctx
        .providers
        .iter()
        .map(|p| ProviderInfo { id: p.id().to_string(), display_name: p.display_name().to_string() })
        .collect())
}

/// Status de login de um provedor, perguntando à própria CLI.
#[tauri::command]
pub async fn agents_login_status(ctx: State<'_, AppCtx>, provider: String) -> Result<LoginStatus, String> {
    let p = ctx.provider(&provider)?;
    blocking(move || Ok(if p.detect().is_none() { LoginStatus::NotInstalled } else { p.login_status() })).await
}

/// Comando de login da CLI, para rodar num terminal embutido (`pty_spawn`).
#[tauri::command]
pub async fn agents_login_command(ctx: State<'_, AppCtx>, provider: String) -> Result<CommandSpec, String> {
    Ok(ctx.provider(&provider)?.login_command())
}

/// Todas as tarefas (inclui o histórico do banco).
#[tauri::command]
pub async fn tasks_list(ctx: State<'_, AppCtx>) -> Result<Vec<Task>, String> {
    Ok(ctx.orch.tasks())
}

/// Uma tarefa pelo id.
#[tauri::command]
pub async fn task_get(ctx: State<'_, AppCtx>, id: TaskId) -> Result<Option<Task>, String> {
    Ok(ctx.orch.task(id))
}

/// Cria uma tarefa (roda num worktree isolado). Devolve o id.
#[tauri::command]
pub async fn task_create(ctx: State<'_, AppCtx>, project: PathBuf, provider: String, prompt: String) -> Result<TaskId, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err("Escreva o pedido.".into());
    }
    ctx.orch.submit(project, &provider, prompt).map_err(err)
}

/// Cancela o turno em andamento.
#[tauri::command]
pub async fn task_cancel(ctx: State<'_, AppCtx>, id: TaskId) -> Result<(), String> {
    ctx.orch.cancel(id).map_err(err)
}

/// Responde ao agente (novo turno na mesma sessão da CLI).
#[tauri::command]
pub async fn task_reply(ctx: State<'_, AppCtx>, id: TaskId, message: String) -> Result<(), String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("Escreva a mensagem.".into());
    }
    ctx.orch.reply(id, message).map_err(err)
}

/// Eventos gravados da tarefa (vazio se o histórico estiver desligado).
#[tauri::command]
pub async fn task_history(ctx: State<'_, AppCtx>, id: TaskId) -> Result<Vec<AgentEvent>, String> {
    let orch = ctx.orch.clone();
    blocking(move || Ok(orch.history(id))).await
}

/// Subagentes criados pela tarefa (visão somente leitura), na ordem em que nasceram.
#[tauri::command]
pub async fn task_subagents(ctx: State<'_, AppCtx>, id: TaskId) -> Result<Vec<SubagentInfo>, String> {
    let orch = ctx.orch.clone();
    blocking(move || Ok(orch.subagents(id))).await
}

/// Eventos internos de um subagente da tarefa.
#[tauri::command]
pub async fn task_subagent_history(ctx: State<'_, AppCtx>, id: TaskId, subagent: String) -> Result<Vec<AgentEvent>, String> {
    let orch = ctx.orch.clone();
    blocking(move || Ok(orch.subagent_history(id, &subagent))).await
}

/// Diff do worktree da tarefa em relação ao projeto.
#[tauri::command]
pub async fn task_diff(ctx: State<'_, AppCtx>, id: TaskId) -> Result<String, String> {
    ctx.orch.diff(id).await.map_err(err)
}

/// Aplica as alterações da tarefa no projeto.
#[tauri::command]
pub async fn task_apply(ctx: State<'_, AppCtx>, id: TaskId) -> Result<(), String> {
    ctx.orch.apply(id).await.map_err(err)
}

/// Descarta as alterações da tarefa (remove o worktree).
#[tauri::command]
pub async fn task_discard(ctx: State<'_, AppCtx>, id: TaskId) -> Result<(), String> {
    ctx.orch.discard(id).await.map_err(err)
}
