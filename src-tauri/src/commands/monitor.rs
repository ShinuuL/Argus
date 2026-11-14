//! Status git e watcher de arquivos. Mudanças chegam no evento `project-changed`.
use super::{blocking, err};
use crate::events::PROJECT_CHANGED;
use crate::state::AppCtx;
use argus_core::monitor::{self, ProjectStatus};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectChanged {
    pub path: PathBuf,
    /// None quando a pasta não é um repositório git.
    pub status: Option<ProjectStatus>,
}

/// Recalcula o status git de um projeto fora do runtime e emite `project-changed`.
pub fn refresh_status(app: AppHandle, path: PathBuf) {
    tauri::async_runtime::spawn(async move {
        let p = path.clone();
        let status = tauri::async_runtime::spawn_blocking(move || monitor::status(&p).ok().flatten())
            .await
            .ok()
            .flatten();
        let _ = app.emit(PROJECT_CHANGED, ProjectChanged { path, status });
    });
}

/// Substitui o watcher atual por um que observa `paths`.
pub fn start_watch(app: &AppHandle, paths: Vec<PathBuf>) -> Result<(), String> {
    let ctx = app.state::<AppCtx>();
    let mut slot = ctx.watch.lock().map_err(err)?;
    *slot = None;
    let (handle, mut rx) = monitor::watch(paths).map_err(err)?;
    *slot = Some(handle);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // O canal fecha quando o `WatchHandle` é descartado (nova raiz ou `monitor_unwatch`).
        while let Some(p) = rx.recv().await {
            refresh_status(app.clone(), p);
        }
    });
    Ok(())
}

/// Status git de um projeto (None se não for repositório).
#[tauri::command]
pub async fn monitor_status(path: PathBuf) -> Result<Option<ProjectStatus>, String> {
    blocking(move || monitor::status(&path).map_err(err)).await
}

/// Status git de vários projetos de uma vez (erros viram `status: null`).
#[tauri::command]
pub async fn monitor_statuses(paths: Vec<PathBuf>) -> Result<Vec<ProjectChanged>, String> {
    blocking(move || {
        Ok(paths
            .into_iter()
            .map(|path| {
                let status = monitor::status(&path).ok().flatten();
                ProjectChanged { path, status }
            })
            .collect())
    })
    .await
}

/// Observa `paths`; cada mudança emite `project-changed` com o status novo.
#[tauri::command]
pub async fn monitor_watch(app: AppHandle, paths: Vec<PathBuf>) -> Result<(), String> {
    start_watch(&app, paths)
}

/// Desliga o watcher.
#[tauri::command]
pub async fn monitor_unwatch(ctx: State<'_, AppCtx>) -> Result<(), String> {
    *ctx.watch.lock().map_err(err)? = None;
    Ok(())
}
