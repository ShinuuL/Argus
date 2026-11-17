//! Comando extra da tela de sessão: horários de início e fim dos subagentes.
use super::{blocking, err};
use crate::state::AppCtx;
use argus_core::agents::orchestrator::TaskId;
use argus_core::agents::provider::AgentEvent;
use serde::Serialize;
use tauri::State;

/// Quando um subagente nasceu e terminou (segundos Unix, do histórico gravado).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubagentTime {
    /// Id do subagente (o mesmo de `SubagentStart`/`SubagentEnd`).
    pub id: String,
    pub started_at: i64,
    /// None enquanto roda, ou se o turno foi interrompido antes do fim.
    pub ended_at: Option<i64>,
}

/// Horários dos subagentes da tarefa, na ordem em que nasceram. Vazio sem banco.
#[tauri::command]
pub async fn task_subagent_times(ctx: State<'_, AppCtx>, id: TaskId) -> Result<Vec<SubagentTime>, String> {
    let orch = ctx.orch.clone();
    blocking(move || {
        let Some(db) = orch.db() else { return Ok(vec![]) };
        let mut out: Vec<SubagentTime> = Vec::new();
        for rec in db.list_events(id as i64).map_err(err)? {
            match serde_json::from_str::<AgentEvent>(&rec.payload) {
                Ok(AgentEvent::SubagentStart { id, .. }) => {
                    out.push(SubagentTime { id, started_at: rec.created_at, ended_at: None });
                }
                Ok(AgentEvent::SubagentEnd { id, .. }) => {
                    if let Some(t) = out.iter_mut().rev().find(|t| t.id == id) {
                        t.ended_at = Some(rec.created_at);
                    }
                }
                _ => {}
            }
        }
        Ok(out)
    })
    .await
}
