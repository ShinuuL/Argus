//! Servidores vinculados às sessões e terminais, e os "Comandos permitidos ao agente".
//! A lista atualizada chega no evento `servers-changed`.
use super::{blocking, err};
use argus_core::procs::{self, OwnedProc, Owner, ServerInfo};
use std::path::PathBuf;

/// Servidores em execução rastreados (de sessões e terminais).
#[tauri::command]
pub async fn servers_list() -> Result<Vec<ServerInfo>, String> {
    Ok(procs::tracker().servers())
}

/// Servidores de um dono (sessão ou terminal).
#[tauri::command]
pub async fn servers_of(owner: Owner) -> Result<Vec<ServerInfo>, String> {
    Ok(procs::tracker().servers_of(owner))
}

/// Processos vivos de um dono (tarefas em segundo plano da sessão), sem a raiz.
#[tauri::command]
pub async fn owner_processes(owner: Owner) -> Result<Vec<OwnedProc>, String> {
    blocking(move || Ok(procs::tracker().processes_of(owner))).await
}

/// Derruba um servidor (o processo e os filhos). Devolve a lista atualizada.
#[tauri::command]
pub async fn server_kill(pid: u32) -> Result<Vec<ServerInfo>, String> {
    blocking(move || {
        procs::tracker().kill(pid).map_err(err)?;
        Ok(procs::tracker().servers())
    })
    .await
}

/// Derruba todos os servidores de um dono. Devolve quantos foram derrubados.
#[tauri::command]
pub async fn servers_stop_all(owner: Owner) -> Result<usize, String> {
    blocking(move || Ok(procs::tracker().stop_all(owner))).await
}

/// Reinicia um servidor com a mesma linha de comando. Devolve o novo PID.
#[tauri::command]
pub async fn server_restart(server: ServerInfo) -> Result<u32, String> {
    blocking(move || procs::restart(&server).map_err(err)).await
}

/// Padrões de comando liberados ao agente no projeto (viram `--allowedTools`).
#[tauri::command]
pub async fn allowed_commands_get(project: PathBuf) -> Result<Vec<String>, String> {
    blocking(move || Ok(procs::allowed_commands(&project))).await
}

/// Salva os comandos permitidos do projeto (vale a partir do próximo turno).
#[tauri::command]
pub async fn allowed_commands_set(project: PathBuf, commands: Vec<String>) -> Result<Vec<String>, String> {
    blocking(move || {
        procs::set_allowed_commands(&project, commands).map_err(err)?;
        Ok(procs::allowed_commands(&project))
    })
    .await
}
