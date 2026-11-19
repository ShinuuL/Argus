//! Eventos enviados à interface.
//!
//! | Evento                | Payload                                                |
//! |-----------------------|--------------------------------------------------------|
//! | `orch`                | `OrchMsg` (enum do orquestrador, serde com tag externa) |
//! | `project-changed`     | `{ path, status }` (status git recalculado)            |
//! | `workspace-changed`   | `RootInfo` (a lista de projetos da raiz mudou)         |
//! | `pty-output`          | `{ id, data }` (bytes em base64)                       |
//! | `pty-exit`            | `{ id, code }`                                         |
//! | `pty-missing-command` | `{ id, command, plan }` (sugestão de instalação)       |
//! | `servers-changed`     | `ServerInfo[]`                                         |
//! | `runs-changed`        | `RunInfo[]` (builds e servidores de dev)                |
//! | `run-finished`        | `RunInfo` (uma execução terminou)                      |
//! | `media-local`         | `{ type: "Error", message }` ou `{ type: "Finished" }` |
use crate::state::AppCtx;
use argus_core::agents::orchestrator::{OrchestratorEvent, TaskId, TaskState};
use argus_core::agents::provider::AgentEvent;
use argus_core::autowork::AutoWorkEvent;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const ORCH: &str = "orch";
/// A janela perdeu eventos do orquestrador (fila cheia): relê o histórico.
pub const ORCH_LAGGED: &str = "orch-lagged";
pub const PROJECT_CHANGED: &str = "project-changed";
pub const WORKSPACE_CHANGED: &str = "workspace-changed";
pub const PTY_OUTPUT: &str = "pty-output";
pub const PTY_EXIT: &str = "pty-exit";
pub const PTY_MISSING_COMMAND: &str = "pty-missing-command";
pub const SERVERS_CHANGED: &str = "servers-changed";
pub const MEDIA_LOCAL: &str = "media-local";
pub const RUNS_CHANGED: &str = "runs-changed";
pub const RUN_FINISHED: &str = "run-finished";

/// Espelho serializável de `OrchestratorEvent` (que não deriva `Serialize`).
///
/// Formato serde padrão (tag externa), igual ao dos enums do core:
/// `{"Agent":{"task":1,"event":{"Text":"oi"}}}`. O `AgentEvent`, o `TaskState` e o
/// `AutoWorkEvent` passam direto pelo serde: variantes novas chegam à interface
/// sem mudar este arquivo.
#[derive(Clone, Serialize)]
pub enum OrchMsg<'a> {
    StateChanged { task: TaskId, state: &'a TaskState },
    Agent { task: TaskId, event: &'a AgentEvent },
    Stderr { task: TaskId, line: &'a str },
    AutoWork(&'a AutoWorkEvent),
    /// A tarefa foi excluída.
    Removed { task: TaskId },
    /// Variante do orquestrador ainda sem espelho aqui: vai no formato do remoto.
    #[allow(dead_code)]
    Other(serde_json::Value),
}

pub fn orch_msg(ev: &OrchestratorEvent) -> OrchMsg<'_> {
    #[allow(unreachable_patterns)]
    match ev {
        OrchestratorEvent::StateChanged { task, state } => OrchMsg::StateChanged { task: *task, state },
        OrchestratorEvent::Agent { task, event } => OrchMsg::Agent { task: *task, event },
        OrchestratorEvent::Stderr { task, line } => OrchMsg::Stderr { task: *task, line },
        OrchestratorEvent::AutoWork(e) => OrchMsg::AutoWork(e),
        OrchestratorEvent::Removed { task } => OrchMsg::Removed { task: *task },
        other => OrchMsg::Other(argus_core::remote::event_json(other)),
    }
}

/// Repassa todos os eventos do orquestrador para a interface no evento `orch`.
pub fn start_orchestrator_forwarder(app: &AppHandle) {
    let mut rx = app.state::<AppCtx>().orch.subscribe();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let _ = app.emit(ORCH, orch_msg(&ev));
                }
                Err(RecvError::Lagged(n)) => {
                    // Descartar em silêncio deixaria a tela sem Text/ToolUse/estado
                    // até o fim do turno: avisa para ela reler.
                    let _ = app.emit(ORCH_LAGGED, n);
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

/// Emite `servers-changed` sempre que a lista de servidores rastreados muda.
pub fn start_servers_forwarder(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("argus-servers".into()).spawn(move || {
        let tracker = argus_core::procs::tracker();
        let mut rev = tracker.rev();
        loop {
            std::thread::sleep(Duration::from_millis(1000));
            let r = tracker.rev();
            if r != rev {
                rev = r;
                let _ = app.emit(SERVERS_CHANGED, tracker.servers());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use argus_core::agents::provider::TokenUsage;
    use serde_json::json;

    #[test]
    fn orch_msg_uses_serde_external_tags() {
        let ev = OrchestratorEvent::Agent { task: 7, event: AgentEvent::Text("oi".into()) };
        assert_eq!(serde_json::to_value(orch_msg(&ev)).unwrap(), json!({"Agent": {"task": 7, "event": {"Text": "oi"}}}));

        let ev = OrchestratorEvent::StateChanged { task: 1, state: TaskState::Failed("x".into()) };
        assert_eq!(
            serde_json::to_value(orch_msg(&ev)).unwrap(),
            json!({"StateChanged": {"task": 1, "state": {"Failed": "x"}}})
        );

        let ev = OrchestratorEvent::StateChanged { task: 1, state: TaskState::Running };
        assert_eq!(serde_json::to_value(orch_msg(&ev)).unwrap(), json!({"StateChanged": {"task": 1, "state": "Running"}}));

        let ev = OrchestratorEvent::Removed { task: 9 };
        assert_eq!(serde_json::to_value(orch_msg(&ev)).unwrap(), json!({"Removed": {"task": 9}}));

        let ev = OrchestratorEvent::Stderr { task: 2, line: "aviso".into() };
        assert_eq!(serde_json::to_value(orch_msg(&ev)).unwrap(), json!({"Stderr": {"task": 2, "line": "aviso"}}));

        let ev = OrchestratorEvent::Agent { task: 3, event: AgentEvent::Usage(TokenUsage { input_tokens: 5, ..Default::default() }) };
        let v = serde_json::to_value(orch_msg(&ev)).unwrap();
        assert_eq!(v["Agent"]["event"]["Usage"]["input_tokens"], 5);
    }
}
