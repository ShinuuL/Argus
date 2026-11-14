//! Pasta raiz: escolher, varrer projetos e ligar o monitoramento.
use super::{blocking, err, monitor};
use crate::state::AppCtx;
use argus_core::workspace::{self, Project};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Clone, Serialize)]
pub struct RootInfo {
    pub root: PathBuf,
    pub projects: Vec<Project>,
    /// Preenchido quando o monitoramento de arquivos não pôde ser ligado.
    pub watch_error: Option<String>,
}

/// Varre `dir`, guarda como raiz atual, liga o watcher e dispara o cálculo do
/// status git de cada projeto (cada resultado chega no evento `project-changed`).
pub fn open_root(app: &AppHandle, dir: PathBuf) -> Result<RootInfo, String> {
    let ctx = app.state::<AppCtx>();
    let projects = workspace::scan(&dir).map_err(err)?;
    let paths: Vec<PathBuf> = projects.iter().map(|p| p.path.clone()).collect();
    *ctx.root.lock().map_err(err)? = Some(dir.clone());
    *ctx.projects.lock().map_err(err)? = projects.clone();
    // O acesso remoto (celular) lista estes projetos e só cria conversa neles.
    ctx.remote.set_projects(paths.clone());
    let watch_error = monitor::start_watch(app, paths.clone()).err().map(|e| format!("Monitoramento indisponível: {e}"));
    for p in paths {
        monitor::refresh_status(app.clone(), p);
    }
    Ok(RootInfo { root: dir, projects, watch_error })
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
