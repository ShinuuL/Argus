//! Pasta raiz: escolher, varrer projetos e ligar o monitoramento.
use super::{blocking, err, monitor};
use crate::state::AppCtx;
use argus_core::workspace::{self, Project};
use serde::Serialize;
use crate::events::WORKSPACE_CHANGED;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Clone, Serialize)]
pub struct RootInfo {
    pub root: PathBuf,
    pub projects: Vec<Project>,
    /// Preenchido quando o monitoramento de arquivos não pôde ser ligado.
    pub watch_error: Option<String>,
}

/// Projetos de `dir`: os varridos mais os que têm sessões e cuja pasta sumiu
/// (ficam como "pasta ausente" até as sessões serem excluídas).
fn list_projects(app: &AppHandle, dir: &Path) -> Result<Vec<Project>, String> {
    let ctx = app.state::<AppCtx>();
    let scanned = workspace::scan(dir).map_err(err)?;
    let with_sessions = ctx.orch.tasks().into_iter().map(|t| t.project);
    Ok(workspace::with_session_projects(dir, scanned, with_sessions))
}

/// Pastas existentes (as que o watcher e o remoto usam).
fn existing(projects: &[Project]) -> Vec<PathBuf> {
    projects.iter().filter(|p| !p.missing).map(|p| p.path.clone()).collect()
}

/// Varre `dir`, guarda como raiz atual, liga os watchers (o dos projetos e o do
/// primeiro nível da raiz) e dispara o cálculo do status git de cada projeto
/// (cada resultado chega no evento `project-changed`).
pub fn open_root(app: &AppHandle, dir: PathBuf) -> Result<RootInfo, String> {
    let ctx = app.state::<AppCtx>();
    let projects = list_projects(app, &dir)?;
    let paths = existing(&projects);
    *ctx.root.lock().map_err(err)? = Some(dir.clone());
    // Preferências e memória dos agentes ficam em `<raiz>/Argus`.
    argus_core::memory::set_root(Some(dir.clone()));
    *ctx.projects.lock().map_err(err)? = projects.clone();
    // O acesso remoto (celular) lista estes projetos e só cria conversa neles.
    ctx.remote.set_projects(paths.clone());
    // O Screen-Mirror pode estar dentro da pasta raiz: atualiza o MCP dos agentes.
    ctx.mirror.sync_agent_mcp(Some(&dir));
    let mut watch_error = monitor::start_watch(app, paths.clone()).err().map(|e| format!("Monitoramento indisponível: {e}"));
    if let Err(e) = start_root_watch(app, dir.clone()) {
        watch_error.get_or_insert(format!("Monitoramento da pasta raiz indisponível: {e}"));
    }
    for p in paths {
        monitor::refresh_status(app.clone(), p);
    }
    Ok(RootInfo { root: dir, projects, watch_error })
}

/// Observa o primeiro nível da raiz: subpasta criada, removida ou renomeada
/// dispara `refresh_root` (o próprio watcher faz o debounce de ~500 ms).
fn start_root_watch(app: &AppHandle, dir: PathBuf) -> Result<(), String> {
    let ctx = app.state::<AppCtx>();
    let mut slot = ctx.root_watch.lock().map_err(err)?;
    *slot = None;
    let (handle, mut rx) = argus_core::monitor::watch_root(dir).map_err(err)?;
    *slot = Some(handle);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // O canal fecha quando o `WatchHandle` é trocado (outra raiz).
        while rx.recv().await.is_some() {
            let a = app.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || refresh_root(&a, false)).await;
        }
    });
    Ok(())
}

/// Varre de novo a raiz atual. Se a lista mudou, troca o watcher dos projetos,
/// atualiza o remoto e emite `workspace-changed`. Com `statuses`, recalcula o
/// status git (branch e alterados) de todos; sem, só o dos projetos novos.
/// Devolve None se não há raiz aberta.
pub fn refresh_root(app: &AppHandle, statuses: bool) -> Result<Option<RootInfo>, String> {
    let ctx = app.state::<AppCtx>();
    let Some(dir) = ctx.root.lock().map_err(err)?.clone() else { return Ok(None) };
    if !dir.is_dir() {
        return Err(format!("a pasta raiz {} não existe mais", dir.display()));
    }
    let projects = list_projects(app, &dir)?;
    let (before, changed) = {
        let mut cur = ctx.projects.lock().map_err(err)?;
        let before = existing(&cur);
        let changed = cur
            .iter()
            .map(|p| (&p.path, p.missing, &p.kinds))
            .ne(projects.iter().map(|p| (&p.path, p.missing, &p.kinds)));
        *cur = projects.clone();
        (before, changed)
    };
    let after = existing(&projects);
    let mut watch_error = None;
    if before != after {
        ctx.remote.set_projects(after.clone());
        watch_error = monitor::start_watch(app, after.clone()).err().map(|e| format!("Monitoramento indisponível: {e}"));
    }
    for p in &after {
        if statuses || !before.contains(p) {
            monitor::refresh_status(app.clone(), p.clone());
        }
    }
    let info = RootInfo { root: dir, projects, watch_error };
    if changed {
        let _ = app.emit(WORKSPACE_CHANGED, &info);
    }
    Ok(Some(info))
}

/// Atualiza a lista de projetos e o status git de todos (botão "Atualizar" e
/// foco da janela). None se não há raiz aberta.
#[tauri::command]
pub async fn workspace_refresh(app: AppHandle) -> Result<Option<RootInfo>, String> {
    blocking(move || refresh_root(&app, true)).await
}

/// Abre o seletor de pasta do sistema. Devolve None se o usuário cancelar.
#[tauri::command]
pub async fn workspace_pick_root(app: AppHandle) -> Result<Option<PathBuf>, String> {
    blocking(move || {
        Ok(app.dialog().file().set_title("Pasta raiz").blocking_pick_folder().and_then(|f| f.into_path().ok()))
    })
    .await
}

/// Abre `path` como pasta raiz (varre projetos, liga o watcher, emite status).
#[tauri::command]
pub async fn workspace_open_root(app: AppHandle, path: PathBuf) -> Result<RootInfo, String> {
    blocking(move || open_root(&app, path)).await
}

/// Pasta raiz já aberta (ex.: via `ARGUS_ROOT`), ou None.
#[tauri::command]
pub async fn workspace_current(ctx: State<'_, AppCtx>) -> Result<Option<RootInfo>, String> {
    let root = ctx.root.lock().map_err(err)?.clone();
    let projects = ctx.projects.lock().map_err(err)?.clone();
    Ok(root.map(|root| RootInfo { root, projects, watch_error: None }))
}

/// Só varre uma pasta, sem mudar a raiz atual.
#[tauri::command]
pub async fn workspace_scan(path: PathBuf) -> Result<Vec<Project>, String> {
    blocking(move || workspace::scan(&path).map_err(err)).await
}
