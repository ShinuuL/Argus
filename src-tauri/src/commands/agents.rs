//! Provedores (CLIs oficiais) e tarefas do orquestrador.
//!
//! A IDE só executa as CLIs; o login é feito pela própria CLI num terminal
//! embutido. Nenhum token é lido ou guardado aqui.
use super::{blocking, err};
use crate::state::AppCtx;
use argus_core::agents::orchestrator::{SubmitOptions, Task, TaskId, WorkMode};
use argus_core::agents::provider::{AgentEvent, CommandSpec, LoginStatus, ModelInfo, SubagentInfo};
use argus_core::isolation::IsolationReport;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
    /// Modelo inicial das conversas novas (None = padrão da CLI).
    pub default_model: Option<String>,
}

fn models_cache() -> &'static Mutex<HashMap<String, Vec<ModelInfo>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<ModelInfo>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Modelos que a CLI do provedor aceita, perguntando a ela (claude: resposta de
/// inicialização do stream-json; opencode: `opencode models`). Nenhum token é
/// gasto. A lista fica em memória; `refresh` força nova consulta.
#[tauri::command]
pub async fn agents_models(ctx: State<'_, AppCtx>, provider: String, refresh: Option<bool>) -> Result<Vec<ModelInfo>, String> {
    let p = ctx.provider(&provider)?;
    if refresh != Some(true) {
        if let Some(list) = models_cache().lock().map_err(err)?.get(&provider) {
            return Ok(list.clone());
        }
    }
    blocking(move || {
        // Uma consulta por provedor de cada vez: quem chega durante uma consulta
        // espera por ela e usa o resultado, em vez de subir outra CLI.
        let gate = {
            let mut gates = probe_gates().lock().map_err(err)?;
            gates.entry(provider.clone()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
        };
        let _one = gate.lock().unwrap_or_else(|e| e.into_inner());
        if refresh != Some(true) {
            if let Some(list) = models_cache().lock().map_err(err)?.get(&provider) {
                return Ok(list.clone());
            }
        }
        let list = p.list_models()?;
        // Só lista com modelos é guardada; vazia vira erro (a próxima abertura tenta de novo).
        if list.is_empty() {
            return Err("A CLI não informou nenhum modelo.".into());
        }
        models_cache().lock().map_err(err)?.insert(provider, list.clone());
        Ok(list)
    })
    .await
}

fn probe_gates() -> &'static Mutex<HashMap<String, Arc<Mutex<()>>>> {
    static GATES: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    GATES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Modo de trabalho padrão das conversas novas.
#[derive(Debug, Clone, Serialize)]
pub struct WorkModeInfo {
    /// Preferência geral (Isolated se nunca foi escolhida).
    pub global: WorkMode,
    /// Preferência do projeto (None = segue a geral).
    pub project: Option<WorkMode>,
    /// O que vale para uma conversa nova no projeto.
    pub effective: WorkMode,
    /// O projeto é um repositório git (sem isso só existe "Direto na pasta").
    pub is_git: bool,
}

/// Preferências de modo de trabalho; com `project`, inclui a do projeto.
#[tauri::command]
pub async fn workmode_get(ctx: State<'_, AppCtx>, project: Option<PathBuf>) -> Result<WorkModeInfo, String> {
    let orch = ctx.orch.clone();
    blocking(move || {
        let global = orch.work_mode_setting(None).unwrap_or_default();
        Ok(match &project {
            Some(p) => WorkModeInfo {
                global,
                project: orch.work_mode_setting(Some(p)),
                effective: orch.default_mode(p),
                is_git: argus_core::isolation::is_git_repo(p),
            },
            None => WorkModeInfo { global, project: None, effective: global, is_git: true },
        })
    })
    .await
}

/// Grava a preferência geral (`project` ausente) ou a de um projeto. `mode`
/// ausente apaga a preferência (o projeto volta a seguir a geral).
#[tauri::command]
pub async fn workmode_set(ctx: State<'_, AppCtx>, project: Option<PathBuf>, mode: Option<WorkMode>) -> Result<(), String> {
    let orch = ctx.orch.clone();
    blocking(move || orch.set_work_mode_setting(project.as_deref(), mode).map_err(err)).await
}

/// Permissões de comando do agente: o conjunto padrão e os níveis de git.
#[derive(Debug, Clone, Serialize)]
pub struct PermissionsInfo {
    /// O que está liberado sem configurar nada, e o que cada nível acrescenta.
    pub defaults: argus_core::permissions::DefaultSet,
    /// Preferência geral de cada nível (None = padrão do modo de trabalho).
    pub global_local: Option<bool>,
    pub global_remote: Option<bool>,
    /// Preferência do projeto (None = segue a geral).
    pub project_local: Option<bool>,
    pub project_remote: Option<bool>,
}

/// Conjunto padrão de comandos liberados e as preferências dos níveis de git.
#[tauri::command]
pub async fn permissions_get(ctx: State<'_, AppCtx>, project: Option<PathBuf>) -> Result<PermissionsInfo, String> {
    let orch = ctx.orch.clone();
    blocking(move || {
        let p = project.as_deref();
        Ok(PermissionsInfo {
            defaults: argus_core::permissions::default_set(),
            global_local: orch.git_setting(None, "local"),
            global_remote: orch.git_setting(None, "remote"),
            project_local: p.and_then(|p| orch.git_setting(Some(p), "local")),
            project_remote: p.and_then(|p| orch.git_setting(Some(p), "remote")),
        })
    })
    .await
}

/// Liga ou desliga um nível de git ("local" ou "remote"), no geral (`project`
/// ausente) ou num projeto. `on` ausente apaga a preferência.
#[tauri::command]
pub async fn permissions_set(ctx: State<'_, AppCtx>, project: Option<PathBuf>, level: String, on: Option<bool>) -> Result<(), String> {
    if level != "local" && level != "remote" {
        return Err("Nível de git desconhecido.".into());
    }
    let orch = ctx.orch.clone();
    blocking(move || orch.set_git_setting(project.as_deref(), &level, on).map_err(err)).await
}

/// "Liberar este comando neste projeto": acrescenta o comando negado à lista
/// "Comandos permitidos ao agente" do projeto. Devolve a lista resultante.
#[tauri::command]
pub async fn permissions_allow_denied(project: PathBuf, tool: String, command: String) -> Result<Vec<String>, String> {
    blocking(move || {
        let pattern = argus_core::permissions::pattern_for_denied(&tool, &command)
            .ok_or_else(|| "Este tipo de ferramenta não pode ser liberado por aqui.".to_string())?;
        let mut list = argus_core::procs::allowed_commands(&project);
        if !list.contains(&pattern) {
            list.push(pattern);
            argus_core::procs::set_allowed_commands(&project, list).map_err(err)?;
        }
        Ok(argus_core::procs::allowed_commands(&project))
    })
    .await
}

/// O que existe na pasta do projeto e não existiria numa cópia isolada (arquivos
/// não versionados, ignorados relevantes, alterações não commitadas) e os
/// caminhos citados em `prompt` que ficariam de fora. Só lê o git.
#[tauri::command]
pub async fn isolation_check(project: PathBuf, prompt: Option<String>) -> Result<IsolationReport, String> {
    blocking(move || {
        if !project.is_dir() {
            return Err("Pasta do projeto não encontrada.".into());
        }
        Ok(argus_core::isolation::check(&project, prompt.as_deref().unwrap_or_default()))
    })
    .await
}

/// Provedores disponíveis (claude, opencode, codex, gemini).
#[tauri::command]
pub async fn agents_providers(ctx: State<'_, AppCtx>) -> Result<Vec<ProviderInfo>, String> {
    Ok(ctx
        .providers
        .iter()
        .map(|p| ProviderInfo {
            id: p.id().to_string(),
            display_name: p.display_name().to_string(),
            default_model: p.default_model(),
        })
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

/// Cria uma tarefa e devolve o id. `model` ausente ou vazio = padrão da CLI;
/// `mode` ausente = o modo de trabalho padrão do projeto (isolado, num worktree,
/// ou direto na pasta).
#[tauri::command]
pub async fn task_create(
    ctx: State<'_, AppCtx>,
    project: PathBuf,
    provider: String,
    prompt: String,
    model: Option<String>,
    mode: Option<WorkMode>,
) -> Result<TaskId, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err("Escreva o pedido.".into());
    }
    ctx.orch.submit_with(project, &provider, prompt, SubmitOptions { model, mode }).map_err(err)
}

/// Troca o modelo da tarefa (ausente ou vazio = padrão da CLI); vale a partir do próximo turno.
#[tauri::command]
pub async fn task_set_model(ctx: State<'_, AppCtx>, id: TaskId, model: Option<String>) -> Result<(), String> {
    ctx.orch.set_model(id, model.as_deref()).map_err(err)
}

/// Troca o agente (provedor) da tarefa, com o turno parado. O próximo turno abre
/// (ou retoma) a sessão do provedor novo e leva o resumo da conversa.
#[tauri::command]
pub async fn task_set_provider(ctx: State<'_, AppCtx>, id: TaskId, provider: String) -> Result<(), String> {
    let orch = ctx.orch.clone();
    blocking(move || orch.set_provider(id, &provider).map_err(err)).await
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

/// Diff da tarefa: o do worktree (modo isolado) ou o da pasta do projeto em
/// relação ao último commit (modo direto).
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

/// Exclui a sessão: cancela o turno, remove worktree e branch (modo isolado),
/// apaga a tarefa e o histórico (o consumo fica nas métricas) e os anexos dela.
/// A interface recebe `Removed` no evento `orch`.
#[tauri::command]
pub async fn task_delete(app: tauri::AppHandle, ctx: State<'_, AppCtx>, id: TaskId) -> Result<(), String> {
    let project = ctx.orch.task(id).map(|t| t.project);
    ctx.orch.delete(id).await.map_err(err)?;
    after_delete(&app, &[id], project.as_deref());
    Ok(())
}

/// Exclui as sessões encerradas (aplicadas ou descartadas) do projeto. Devolve quantas.
#[tauri::command]
pub async fn task_delete_closed(app: tauri::AppHandle, ctx: State<'_, AppCtx>, project: PathBuf) -> Result<usize, String> {
    let ids = ctx.orch.delete_closed(&project).await.map_err(err)?;
    after_delete(&app, &ids, Some(&project));
    Ok(ids.len())
}

/// Limpeza fora do banco: as pastas de anexos das tarefas e, se a pasta do
/// projeto já não existe, a lista de projetos (o "pasta ausente" some sem sessões).
fn after_delete(app: &tauri::AppHandle, ids: &[TaskId], project: Option<&std::path::Path>) {
    let root = argus_core::attachments::root();
    for id in ids {
        if let Some(dir) = argus_core::attachments::dir_for(&root, Some(*id), None) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    if project.is_some_and(|p| !p.is_dir()) {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _ = super::workspace::refresh_root(&app, false);
        });
    }
}
