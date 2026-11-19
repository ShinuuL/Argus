//! Agentes e skills em arquivo (`.claude/agents`, `.claude/skills`, `AGENTS.md`).
//!
//! O projeto vem da interface, e não do estado, porque a tela permite editar o
//! escopo global sem nenhum projeto aberto. Ver `argus_core::agentdefs`.
use super::{blocking, err};
use argus_core::agentdefs::{self, AgentDef, Scope, SkillDef};
use std::path::PathBuf;

fn as_path(project: Option<String>) -> Option<PathBuf> {
    project.filter(|p| !p.trim().is_empty()).map(PathBuf::from)
}

/// Agentes dos dois escopos; o global com o mesmo nome do projeto vem marcado
/// como `shadowed` (a CLI usa o do projeto).
#[tauri::command]
pub async fn agentdefs_list(project: Option<String>) -> Result<Vec<AgentDef>, String> {
    blocking(move || Ok(agentdefs::list_agents(as_path(project).as_deref()))).await
}

#[tauri::command]
pub async fn agentdefs_save(def: AgentDef, project: Option<String>) -> Result<String, String> {
    blocking(move || {
        agentdefs::save_agent(&def, as_path(project).as_deref())
            .map(|p| p.to_string_lossy().to_string())
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn agentdefs_delete(name: String, scope: Scope, project: Option<String>) -> Result<(), String> {
    blocking(move || agentdefs::delete_agent(&name, scope, as_path(project).as_deref()).map_err(err)).await
}

#[tauri::command]
pub async fn agentdefs_skills(project: Option<String>) -> Result<Vec<SkillDef>, String> {
    blocking(move || Ok(agentdefs::list_skills(as_path(project).as_deref()))).await
}

#[tauri::command]
pub async fn agentdefs_skill_save(def: SkillDef, project: Option<String>) -> Result<String, String> {
    blocking(move || {
        agentdefs::save_skill(&def, as_path(project).as_deref())
            .map(|p| p.to_string_lossy().to_string())
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn agentdefs_skill_delete(name: String, scope: Scope, project: Option<String>) -> Result<(), String> {
    blocking(move || agentdefs::delete_skill(&name, scope, as_path(project).as_deref()).map_err(err)).await
}

/// Grava o trecho do Argus no `AGENTS.md` do projeto (o resto do arquivo fica intacto).
#[tauri::command]
pub async fn agentdefs_sync_md(project: String, body: String) -> Result<String, String> {
    blocking(move || {
        agentdefs::sync_agents_md(std::path::Path::new(&project), &body)
            .map(|p| p.to_string_lossy().to_string())
            .map_err(err)
    })
    .await
}

/// Slug sugerido a partir de um título digitado (acentos viram ASCII).
#[tauri::command]
pub fn agentdefs_slug(text: String) -> String {
    agentdefs::slugify(&text)
}
