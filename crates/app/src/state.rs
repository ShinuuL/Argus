//! Estado global da IDE, compartilhado via `use_context`.
use dioxus::prelude::*;
use argus_core::agents::orchestrator::{Orchestrator, TaskId};
use argus_core::monitor::ProjectStatus;
use argus_core::pty::CommandSpec;
use argus_core::workspace::Project;
use std::collections::HashMap;
use std::path::PathBuf;

/// Uma aba do painel de terminal. `command = None` abre o shell padrão.
#[derive(Clone, PartialEq)]
pub struct TermTab {
    pub id: u64,
    pub title: String,
    pub cwd: PathBuf,
    pub command: Option<CommandSpec>,
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub root: Signal<Option<PathBuf>>,
    pub projects: Signal<Vec<Project>>,
    pub statuses: Signal<HashMap<PathBuf, Option<ProjectStatus>>>,
    pub selected: Signal<Option<PathBuf>>,
    pub open_file: Signal<Option<PathBuf>>,
    pub terms: Signal<Vec<TermTab>>,
    pub active_term: Signal<Option<u64>>,
    pub next_term: Signal<u64>,
    /// Log de eventos por tarefa (texto pronto para exibir).
    pub task_log: Signal<HashMap<TaskId, Vec<String>>>,
    /// Incrementado a cada evento do orquestrador (força releitura de `tasks()`).
    pub task_rev: Signal<u64>,
}

impl AppState {
    pub fn open_terminal(mut self, title: String, cwd: PathBuf, command: Option<CommandSpec>) {
        let id = *self.next_term.peek();
        self.next_term.set(id + 1);
        self.terms.write().push(TermTab { id, title, cwd, command });
        self.active_term.set(Some(id));
    }
}

pub fn use_app() -> AppState {
    use_context::<AppState>()
}

pub fn use_orchestrator() -> Orchestrator {
    use_context::<Orchestrator>()
}
