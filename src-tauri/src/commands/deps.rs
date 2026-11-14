//! Dependências do projeto. Nada é instalado aqui: os comandos só devolvem o
//! plano (com o comando exato); a interface pede confirmação e roda num terminal.
use super::blocking;
use argus_core::agents::provider::CommandSpec;
use argus_core::deps::{self, DepKind, InstallPlan};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallPlanDto {
    pub command: CommandSpec,
    pub needs_admin: bool,
    /// Linha de comando pronta para exibir.
    pub display: String,
}

impl From<InstallPlan> for InstallPlanDto {
    fn from(p: InstallPlan) -> Self {
        let display = p.display();
        Self { command: p.command, needs_admin: p.needs_admin, display }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DepIssueDto {
    pub kind: DepKind,
    pub tool: String,
    pub description: String,
    pub fix: InstallPlanDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissingCommandDto {
    pub command: String,
    /// None quando o comando não está na tabela de instalação conhecida.
    pub plan: Option<InstallPlanDto>,
}

/// Pendências de dependências do projeto (ferramentas ausentes, pacotes não instalados).
#[tauri::command]
pub async fn deps_analyze(project: PathBuf) -> Result<Vec<DepIssueDto>, String> {
    blocking(move || {
        Ok(deps::analyze(&project)
            .into_iter()
            .map(|i| DepIssueDto { kind: i.kind, tool: i.tool, description: i.description, fix: i.fix.into() })
            .collect())
    })
    .await
}

/// Reconhece "comando não encontrado" num texto de terminal.
#[tauri::command]
pub async fn deps_detect_missing_command(text: String) -> Result<Option<MissingCommandDto>, String> {
    Ok(deps::detect_missing_command(&text).map(|command| {
        let plan = deps::install_plan_for(&command).map(Into::into);
        MissingCommandDto { command, plan }
    }))
}

/// Plano de instalação de uma ferramenta conhecida (None se desconhecida).
#[tauri::command]
pub async fn deps_install_plan(command: String) -> Result<Option<InstallPlanDto>, String> {
    Ok(deps::install_plan_for(&command).map(Into::into))
}
