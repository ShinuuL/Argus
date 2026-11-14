//! Terminais (ConPTY). A saída chega nos eventos `pty-output` (base64) e `pty-exit`.
use super::err;
use crate::events::{PTY_EXIT, PTY_MISSING_COMMAND, PTY_OUTPUT};
use crate::state::AppCtx;
use argus_core::deps::{self, InstallPlan};
use argus_core::pty::{default_shell, CommandSpec, PtyEvent, SessionId};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

/// Comando para o terminal. Aceita também o `CommandSpec` dos provedores
/// (campos extras, como `stdin`, são ignorados).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CmdSpec {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: Vec<(String, String)>,
}

impl From<CmdSpec> for CommandSpec {
    fn from(c: CmdSpec) -> Self {
        CommandSpec { program: c.program, args: c.args, cwd: c.cwd, env: c.env }
    }
}

impl From<CommandSpec> for CmdSpec {
    fn from(c: CommandSpec) -> Self {
        CmdSpec { program: c.program, args: c.args, cwd: c.cwd, env: c.env }
    }
}

#[derive(Clone, Serialize)]
struct PtyOutput {
    id: SessionId,
    /// Bytes da saída em base64.
    data: String,
}

#[derive(Clone, Serialize)]
struct PtyExit {
    id: SessionId,
    code: Option<u32>,
}

#[derive(Clone, Serialize)]
struct MissingCommand {
    id: SessionId,
    command: String,
    plan: super::deps::InstallPlanDto,
}

#[derive(Debug, Clone, Serialize)]
pub struct PtySession {
    pub id: SessionId,
    pub program: String,
    pub cols: u16,
    pub rows: u16,
}

/// Se a saída diz "comando não encontrado" para uma ferramenta conhecida,
/// devolve o plano de instalação (uma única vez por comando).
fn missing_command(app: &AppHandle, text: &str) -> Option<(String, InstallPlan)> {
    let cmd = deps::detect_missing_command(text)?;
    let plan = deps::install_plan_for(&cmd)?;
    let ctx = app.state::<AppCtx>();
    let first = ctx.suggested.lock().ok()?.insert(cmd.to_ascii_lowercase());
    first.then_some((cmd, plan))
}

/// Abre um terminal. Sem `command`, usa o shell padrão. `cwd` (se dado) vale
/// sobre o `cwd` do comando. Devolve o id da sessão.
#[tauri::command]
pub async fn pty_spawn(
    app: AppHandle,
    ctx: State<'_, AppCtx>,
    cwd: Option<PathBuf>,
    command: Option<CmdSpec>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<SessionId, String> {
    let mut spec: CommandSpec = command.map(Into::into).unwrap_or_else(default_shell);
    if cwd.is_some() {
        spec.cwd = cwd;
    }
    let (id, mut events) = ctx
        .pty
        .spawn(spec, cols.unwrap_or(80).max(2), rows.unwrap_or(24).max(2))
        .map_err(|e| format!("Falha ao iniciar terminal: {e}"))?;
    tauri::async_runtime::spawn(async move {
        let b64 = base64::engine::general_purpose::STANDARD;
        while let Some(ev) = events.recv().await {
            match ev {
                PtyEvent::Output(bytes) => {
                    if let Some((command, plan)) = missing_command(&app, &String::from_utf8_lossy(&bytes)) {
                        let _ = app.emit(PTY_MISSING_COMMAND, MissingCommand { id, command, plan: plan.into() });
                    }
                    let _ = app.emit(PTY_OUTPUT, PtyOutput { id, data: b64.encode(&bytes) });
                }
                PtyEvent::Exit(code) => {
                    let _ = app.emit(PTY_EXIT, PtyExit { id, code });
                    break;
                }
            }
        }
    });
    Ok(id)
}

/// Envia texto digitado para o terminal.
#[tauri::command]
pub async fn pty_write(ctx: State<'_, AppCtx>, id: SessionId, data: String) -> Result<(), String> {
    ctx.pty.write(id, data.as_bytes()).map_err(err)
}

/// Redimensiona o terminal.
#[tauri::command]
pub async fn pty_resize(ctx: State<'_, AppCtx>, id: SessionId, cols: u16, rows: u16) -> Result<(), String> {
    ctx.pty.resize(id, cols.max(2), rows.max(2)).map_err(err)
}

/// Encerra o processo do terminal.
#[tauri::command]
pub async fn pty_kill(ctx: State<'_, AppCtx>, id: SessionId) -> Result<(), String> {
    ctx.pty.kill(id).map_err(err)
}

/// Terminais abertos.
#[tauri::command]
pub async fn pty_list(ctx: State<'_, AppCtx>) -> Result<Vec<PtySession>, String> {
    Ok(ctx
        .pty
        .list()
        .into_iter()
        .map(|s| PtySession { id: s.id, program: s.program, cols: s.cols, rows: s.rows })
        .collect())
}

/// Shell padrão do sistema (pwsh, powershell ou `$SHELL`).
#[tauri::command]
pub async fn pty_default_shell() -> Result<CmdSpec, String> {
    Ok(default_shell().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_spec_accepts_provider_command_spec() {
        // O CommandSpec dos provedores tem `stdin`; o terminal ignora.
        let spec = argus_core::agents::provider::CommandSpec::new("claude").arg("/login").stdin("x");
        let json = serde_json::to_string(&spec).unwrap();
        let c: CmdSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(c.program, "claude");
        assert_eq!(c.args, ["/login"]);
        let only_program: CmdSpec = serde_json::from_str(r#"{"program":"pwsh"}"#).unwrap();
        assert!(only_program.args.is_empty() && only_program.cwd.is_none());
    }
}
