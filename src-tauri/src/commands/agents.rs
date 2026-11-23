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

/// Permissões de comando do agente: o conjunto padrão e os níveis liga/desliga.
#[derive(Debug, Clone, Serialize)]
pub struct PermissionsInfo {
    /// O que está liberado sem configurar nada, e o que cada nível acrescenta.
    pub defaults: argus_core::permissions::DefaultSet,
    /// Preferência geral de cada nível (None = padrão do modo de trabalho).
    pub global_local: Option<bool>,
    pub global_remote: Option<bool>,
    pub global_build: Option<bool>,
    /// Preferência do projeto (None = segue a geral).
    pub project_local: Option<bool>,
    pub project_remote: Option<bool>,
    pub project_build: Option<bool>,
    /// Modo de permissão ("auto" | "restricted"): geral e do projeto (None = segue).
    pub global_mode: Option<argus_core::permissions::PermissionMode>,
    pub project_mode: Option<argus_core::permissions::PermissionMode>,
}

/// Conjunto padrão de comandos liberados e as preferências dos níveis.
#[tauri::command]
pub async fn permissions_get(ctx: State<'_, AppCtx>, project: Option<PathBuf>) -> Result<PermissionsInfo, String> {
    let orch = ctx.orch.clone();
    blocking(move || {
        let p = project.as_deref();
        Ok(PermissionsInfo {
            defaults: argus_core::permissions::default_set(),
            global_local: orch.git_setting(None, "local"),
            global_remote: orch.git_setting(None, "remote"),
            global_build: orch.git_setting(None, "build"),
            project_local: p.and_then(|p| orch.git_setting(Some(p), "local")),
            project_remote: p.and_then(|p| orch.git_setting(Some(p), "remote")),
            project_build: p.and_then(|p| orch.git_setting(Some(p), "build")),
            global_mode: orch.permission_mode_setting(None),
            project_mode: p.and_then(|p| orch.permission_mode_setting(Some(p))),
        })
    })
    .await
}

/// Liga ou desliga um nível ("local", "remote" ou "build"), no geral (`project`
/// ausente) ou num projeto. `on` ausente apaga a preferência.
#[tauri::command]
pub async fn permissions_set(ctx: State<'_, AppCtx>, project: Option<PathBuf>, level: String, on: Option<bool>) -> Result<(), String> {
    if level != "local" && level != "remote" && level != "build" {
        return Err("Nível de permissão desconhecido.".into());
    }
    let orch = ctx.orch.clone();
    blocking(move || orch.set_git_setting(project.as_deref(), &level, on).map_err(err)).await
}

/// Limite (tokens de contexto) para compactar a conversa antes do próximo turno; 0 = desligado.
#[tauri::command]
pub async fn autocompact_get(ctx: State<'_, AppCtx>) -> Result<u64, String> {
    let orch = ctx.orch.clone();
    blocking(move || Ok(orch.autocompact_limit())).await
}

/// Segundos desde a última atividade gravada da conversa (None = sem histórico).
#[tauri::command]
pub async fn task_idle_secs(ctx: State<'_, AppCtx>, id: u64) -> Result<Option<i64>, String> {
    let orch = ctx.orch.clone();
    blocking(move || Ok(orch.idle_secs(id))).await
}

/// Grava o limite de compactação (`tokens` ausente volta ao padrão).
#[tauri::command]
pub async fn autocompact_set(ctx: State<'_, AppCtx>, tokens: Option<u64>) -> Result<(), String> {
    let orch = ctx.orch.clone();
    blocking(move || orch.set_autocompact_limit(tokens).map_err(err)).await
}

/// Grava o modo de permissão ("auto" ou "restricted") no geral (`project`
/// ausente) ou num projeto. `mode` ausente apaga a preferência.
#[tauri::command]
pub async fn permission_mode_set(
    ctx: State<'_, AppCtx>,
    project: Option<PathBuf>,
    mode: Option<argus_core::permissions::PermissionMode>,
) -> Result<(), String> {
    let orch = ctx.orch.clone();
    blocking(move || orch.set_permission_mode_setting(project.as_deref(), mode).map_err(err)).await
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
    access: Option<String>,
) -> Result<TaskId, String> {
    let prompt = prompt.trim();
    let access = access.as_deref().and_then(argus_core::permissions::AccessMode::parse).unwrap_or_default();
    if prompt.is_empty() {
        return Err("Escreva o pedido.".into());
    }
    ctx.orch.submit_with(project, &provider, prompt, SubmitOptions { model, mode, access }).map_err(err)
}

/// Troca o modelo da tarefa (ausente ou vazio = padrão da CLI); vale a partir do próximo turno.
#[tauri::command]
pub async fn task_set_model(ctx: State<'_, AppCtx>, id: TaskId, model: Option<String>) -> Result<(), String> {
    ctx.orch.set_model(id, model.as_deref()).map_err(err)
}

/// Mural de arquivos: quem está mexendo em quê agora (ver `argus_core::mural`).
///
/// A tela também recebe cada anotação ao vivo no evento `orch`; este comando é
/// o retrato inicial, para quem abre o painel no meio de um turno.
#[tauri::command]
pub async fn mural_list() -> Result<Vec<argus_core::mural::Claim>, String> {
    Ok(argus_core::mural::global().list(argus_core::mural::agora()))
}

/// Últimos limites de uso do plano informados pela CLI (um `RateLimit` por janela).
///
/// Lê todos os provedores: quem grava usa `ratelimit.<provedor>.<janela>`, e
/// fixar "claude" aqui deixava de fora qualquer outra CLI. Janela cujo horário
/// de reinício já passou não volta: o valor guardado é de um ciclo vencido e
/// mostrá-lo seria inventar um número.
#[tauri::command]
pub async fn rate_limits(ctx: State<'_, AppCtx>) -> Result<Vec<serde_json::Value>, String> {
    let Some(db) = ctx.orch.db() else { return Ok(Vec::new()) };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // A seleção mora no core, ao lado de quem grava: o formato já se separou
    // duas vezes quando leitura e escrita ficaram em arquivos diferentes.
    Ok(argus_core::agents::orchestrator::live_rate_limits(db.settings_by_prefix("ratelimit.").map_err(err)?, now))
}

/// Servidores MCP da CLI do Claude (confere a saúde de cada um; leva alguns segundos).
#[tauri::command]
pub async fn mcp_list() -> Result<Vec<argus_core::mcp::McpServer>, String> {
    tauri::async_runtime::spawn_blocking(argus_core::mcp::list).await.map_err(|e| e.to_string())?
}

/// Adiciona um servidor MCP (`claude mcp add`).
#[tauri::command]
pub async fn mcp_add(name: String, transport: String, target: String, args: Vec<String>, scope: String, env: Vec<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || argus_core::mcp::add(&name, &transport, &target, &args, &scope, &env))
        .await
        .map_err(|e| e.to_string())?
}

/// Remove um servidor MCP (`claude mcp remove`).
#[tauri::command]
pub async fn mcp_remove(name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || argus_core::mcp::remove(&name)).await.map_err(|e| e.to_string())?
}

/// Plugins instalados na CLI do Claude.
#[tauri::command]
pub async fn plugins_list() -> Result<Vec<argus_core::mcp::Plugin>, String> {
    tauri::async_runtime::spawn_blocking(argus_core::mcp::plugins).await.map_err(|e| e.to_string())?
}

/// Ativa ou desativa um plugin (`claude plugin enable/disable`).
#[tauri::command]
pub async fn plugin_set(id: String, enabled: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || argus_core::mcp::set_plugin(&id, enabled)).await.map_err(|e| e.to_string())?
}

/// Notificação do Windows (lembretes).
#[tauri::command]
pub async fn notify_user(title: String, body: String) -> Result<(), String> {
    let title: String = title.chars().take(120).collect();
    let body: String = body.chars().take(300).collect();
    std::thread::spawn(move || crate::commands::schedules::notify(&title, &body));
    Ok(())
}

/// Lembretes do usuário (JSON livre do front, guardado em `settings`).
#[tauri::command]
pub async fn reminders_get(ctx: State<'_, AppCtx>) -> Result<String, String> {
    let Some(db) = ctx.orch.db() else { return Ok("[]".into()) };
    Ok(db.get_setting("reminders").map_err(err)?.unwrap_or_else(|| "[]".into()))
}

/// Grava a lista de lembretes (JSON de um array; até 64 KB).
#[tauri::command]
pub async fn reminders_set(ctx: State<'_, AppCtx>, data: String) -> Result<(), String> {
    if data.len() > 64 * 1024 {
        return Err("Lembretes demais.".into());
    }
    serde_json::from_str::<Vec<serde_json::Value>>(&data).map_err(|e| format!("lembretes inválidos: {e}"))?;
    let Some(db) = ctx.orch.db() else { return Err("Sem banco de dados.".into()) };
    db.set_setting("reminders", Some(&data)).map_err(err)
}

/// Ramifica a conversa (tarefa nova com o mesmo histórico; ver `Orchestrator::fork`).
#[tauri::command]
pub async fn task_fork(ctx: State<'_, AppCtx>, id: TaskId) -> Result<TaskId, String> {
    ctx.orch.fork(id).map_err(err)
}

/// Teto de custo por turno e modelo reserva da conversa.
#[tauri::command]
pub async fn task_limits(ctx: State<'_, AppCtx>, id: TaskId) -> Result<argus_core::agents::todos::Limits, String> {
    Ok(ctx.orch.limits(id))
}

#[tauri::command]
pub async fn task_set_limits(ctx: State<'_, AppCtx>, id: TaskId, limits: argus_core::agents::todos::Limits) -> Result<(), String> {
    ctx.orch.set_limits(id, limits).map_err(err)
}

/// Nível de esforço da conversa (None = padrão da CLI).
#[tauri::command]
pub async fn task_effort(ctx: State<'_, AppCtx>, id: TaskId) -> Result<Option<String>, String> {
    Ok(ctx.orch.effort(id))
}

/// Troca o esforço ("low" a "max"; ausente = padrão da CLI); vale no próximo turno.
#[tauri::command]
pub async fn task_set_effort(ctx: State<'_, AppCtx>, id: TaskId, effort: Option<String>) -> Result<(), String> {
    ctx.orch.set_effort(id, effort.as_deref()).map_err(err)
}

/// Lista de tarefas do usuário na conversa.
#[tauri::command]
pub async fn task_todos(ctx: State<'_, AppCtx>, id: TaskId) -> Result<Vec<argus_core::agents::todos::Todo>, String> {
    Ok(ctx.orch.todos(id))
}

/// Grava a lista inteira e devolve como ficou (sem itens vazios).
#[tauri::command]
pub async fn task_set_todos(ctx: State<'_, AppCtx>, id: TaskId, todos: Vec<argus_core::agents::todos::Todo>) -> Result<Vec<argus_core::agents::todos::Todo>, String> {
    ctx.orch.set_todos(id, todos).map_err(err)
}

/// Troca o modo de acesso da conversa ("plan", "normal" ou "bypass"); vale a
/// partir do próximo turno. O Bypass só chega aqui depois da confirmação na tela.
/// Pedidos da tarefa com ponto de retorno (1 = o primeiro pedido).
#[tauri::command]
pub async fn task_checkpoints(ctx: State<'_, AppCtx>, id: TaskId) -> Result<Vec<u32>, String> {
    let orch = ctx.orch.clone();
    blocking(move || Ok(orch.checkpoints(id))).await
}

/// Volta a pasta de trabalho ao estado de antes do pedido `n`. Devolve quantos
/// arquivos criados depois foram apagados.
#[tauri::command]
pub async fn task_restore_checkpoint(ctx: State<'_, AppCtx>, id: TaskId, n: u32) -> Result<usize, String> {
    ctx.orch.restore_checkpoint(id, n).await.map_err(err)
}

#[tauri::command]
pub async fn task_set_access(ctx: State<'_, AppCtx>, id: TaskId, access: String) -> Result<(), String> {
    let a = argus_core::permissions::AccessMode::parse(&access).ok_or_else(|| format!("modo de acesso desconhecido: {access}"))?;
    ctx.orch.set_access(id, a).map_err(err)
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

/// Rejeita as alterações da tarefa num arquivo (volta ao último commit).
#[tauri::command]
pub async fn task_revert_file(ctx: State<'_, AppCtx>, id: TaskId, path: String) -> Result<(), String> {
    ctx.orch.revert_file(id, &path).await.map_err(err)
}

/// Rejeita um trecho do diff (`hunk` = a linha `@@ -a,b +c,d @@`).
#[tauri::command]
pub async fn task_revert_hunk(ctx: State<'_, AppCtx>, id: TaskId, path: String, hunk: String) -> Result<(), String> {
    ctx.orch.revert_hunk(id, &path, &hunk).await.map_err(err)
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
