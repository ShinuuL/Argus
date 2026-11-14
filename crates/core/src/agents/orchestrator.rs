//! Orquestrador de tarefas de agentes: fila, limite de concorrência,
//! um git worktree por tarefa, sessões conversacionais (vários turnos),
//! eventos via broadcast e persistência opcional em SQLite.

use super::provider::{AgentEvent, AgentProvider, CommandSpec};
use crate::db::Db;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, Notify, Semaphore};

pub type TaskId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskState {
    Queued,
    Running,
    Completed,
    Failed(String),
    Cancelled,
    Applied,
    Discarded,
}

impl TaskState {
    pub fn is_active(&self) -> bool {
        matches!(self, TaskState::Queued | TaskState::Running)
    }
    fn to_db(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    fn from_db(s: &str) -> TaskState {
        serde_json::from_str(s).unwrap_or_else(|_| match s {
            "Queued" => TaskState::Queued,
            "Running" => TaskState::Running,
            "Completed" => TaskState::Completed,
            "Cancelled" => TaskState::Cancelled,
            "Applied" => TaskState::Applied,
            "Discarded" => TaskState::Discarded,
            o => TaskState::Failed(o.to_string()),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub project: PathBuf,
    pub provider: String,
    pub prompt: String,
    pub state: TaskState,
    /// Diretório onde o agente roda (worktree, ou o próprio projeto se worktrees desligados).
    pub workdir: Option<PathBuf>,
    /// Sessão da CLI (claude: `session_id`; opencode: `sessionID`), para `reply`.
    pub session_id: Option<String>,
    /// Número de turnos já iniciados (1 = só o prompt inicial).
    pub turns: u32,
}

#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    StateChanged { task: TaskId, state: TaskState },
    Agent { task: TaskId, event: AgentEvent },
    Stderr { task: TaskId, line: String },
    /// Progresso do modo auto-work (ver `crate::autowork`).
    AutoWork(crate::autowork::AutoWorkEvent),
}

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("provedor desconhecido: {0}")]
    UnknownProvider(String),
    #[error("tarefa não encontrada: {0}")]
    UnknownTask(TaskId),
    #[error("tarefa sem worktree")]
    NoWorktree,
    #[error("tarefa sem sessão para continuar")]
    NoSession,
    #[error("tarefa ainda em execução")]
    Busy,
    #[error("tarefa encerrada (aplicada ou descartada)")]
    Closed,
    #[error("git falhou: {0}")]
    Git(String),
    #[error("banco de dados: {0}")]
    Db(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

struct Inner {
    providers: HashMap<String, Arc<dyn AgentProvider>>,
    tasks: Mutex<HashMap<TaskId, Task>>,
    cancels: Mutex<HashMap<TaskId, Arc<Notify>>>,
    sem: Arc<Semaphore>,
    tx: broadcast::Sender<OrchestratorEvent>,
    next_id: AtomicU64,
    use_worktrees: bool,
    db: Option<Arc<Db>>,
}

#[derive(Clone)]
pub struct Orchestrator {
    inner: Arc<Inner>,
}

impl Orchestrator {
    pub fn new(providers: Vec<Arc<dyn AgentProvider>>, max_concurrent: usize, use_worktrees: bool) -> Self {
        Self::with_db(providers, max_concurrent, use_worktrees, None)
    }

    /// Como `new`, gravando tarefas/eventos/consumo em `db` e recarregando o
    /// histórico salvo. Tarefas que estavam na fila/rodando ao fechar viram `Failed`.
    pub fn with_db(
        providers: Vec<Arc<dyn AgentProvider>>,
        max_concurrent: usize,
        use_worktrees: bool,
        db: Option<Db>,
    ) -> Self {
        let (tx, _) = broadcast::channel(1024);
        let db = db.map(Arc::new);
        let mut tasks = HashMap::new();
        let mut next = 1u64;
        if let Some(db) = &db {
            match db.list_tasks() {
                Ok(recs) => {
                    for r in recs {
                        let id = r.id.max(0) as u64;
                        let mut state = TaskState::from_db(&r.state);
                        if state.is_active() {
                            state = TaskState::Failed("interrompida (app fechado)".into());
                            let _ = db.update_task_state(r.id, &state.to_db());
                        }
                        next = next.max(id + 1);
                        tasks.insert(
                            id,
                            Task {
                                id,
                                project: r.project.into(),
                                provider: r.provider,
                                prompt: r.prompt,
                                state,
                                workdir: r.workdir.map(PathBuf::from),
                                session_id: r.session_id,
                                turns: 1,
                            },
                        );
                    }
                }
                Err(e) => tracing::warn!("falha ao carregar histórico: {e}"),
            }
        }
        Self {
            inner: Arc::new(Inner {
                providers: providers.into_iter().map(|p| (p.id().to_string(), p)).collect(),
                tasks: Mutex::new(tasks),
                cancels: Mutex::new(HashMap::new()),
                sem: Arc::new(Semaphore::new(max_concurrent.max(1))),
                tx,
                next_id: AtomicU64::new(next),
                use_worktrees,
                db,
            }),
        }
    }

    pub fn db(&self) -> Option<&Db> {
        self.inner.db.as_deref()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<OrchestratorEvent> {
        self.inner.tx.subscribe()
    }

    /// Publica um evento no broadcast (usado pelo auto-work).
    pub fn emit(&self, ev: OrchestratorEvent) {
        let _ = self.inner.tx.send(ev);
    }

    pub fn tasks(&self) -> Vec<Task> {
        let mut v: Vec<_> = self.inner.tasks.lock().unwrap().values().cloned().collect();
        v.sort_by_key(|t| t.id);
        v
    }

    pub fn task(&self, id: TaskId) -> Option<Task> {
        self.inner.tasks.lock().unwrap().get(&id).cloned()
    }

    /// Eventos gravados de uma tarefa (vazio sem banco). Inclui `User` dos turnos
    /// de resposta; linhas de stderr voltam como `Raw`.
    pub fn history(&self, id: TaskId) -> Vec<AgentEvent> {
        let Some(db) = &self.inner.db else { return vec![] };
        db.list_events(id as i64)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|e| serde_json::from_str(&e.payload).ok())
            .collect()
    }

    /// Enfileira uma tarefa. Precisa estar dentro de um runtime tokio.
    pub fn submit(&self, project: impl Into<PathBuf>, provider: &str, prompt: &str) -> Result<TaskId, OrchestratorError> {
        let prov = self.provider(provider)?;
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let task = Task {
            id,
            project: project.into(),
            provider: provider.into(),
            prompt: prompt.into(),
            state: TaskState::Queued,
            workdir: None,
            session_id: None,
            turns: 1,
        };
        if let Some(db) = &self.inner.db {
            db.insert_task(
                Some(id as i64),
                &task.project.to_string_lossy(),
                provider,
                prompt,
                &TaskState::Queued.to_db(),
            )?;
            let _ = db.insert_event(id as i64, "User", &json(&AgentEvent::User(prompt.into())));
        }
        self.inner.tasks.lock().unwrap().insert(id, task.clone());
        let cancel = self.new_cancel(id);
        self.emit_state(id, TaskState::Queued);

        let this = self.clone();
        tokio::spawn(async move {
            let Some(_permit) = this.acquire(id, &cancel).await else { return };
            let st = match this.first_turn(&task, prov, &cancel).await {
                Ok(s) => s,
                Err(e) => TaskState::Failed(e.to_string()),
            };
            this.finish(id, &cancel, st);
        });
        Ok(id)
    }

    /// Envia uma nova mensagem na mesma sessão da tarefa (novo turno no mesmo
    /// diretório de trabalho). Os eventos são anexados à mesma tarefa.
    pub fn reply(&self, id: TaskId, message: &str) -> Result<(), OrchestratorError> {
        let t = self.task(id).ok_or(OrchestratorError::UnknownTask(id))?;
        match t.state {
            ref s if s.is_active() => return Err(OrchestratorError::Busy),
            TaskState::Applied | TaskState::Discarded => return Err(OrchestratorError::Closed),
            _ => {}
        }
        let session = t.session_id.clone().ok_or(OrchestratorError::NoSession)?;
        let workdir = match &t.workdir {
            Some(w) => w.clone(),
            None if !self.inner.use_worktrees => t.project.clone(),
            None => return Err(OrchestratorError::NoWorktree),
        };
        let prov = self.provider(&t.provider)?;
        if let Some(x) = self.inner.tasks.lock().unwrap().get_mut(&id) {
            x.turns += 1;
        }
        self.record(id, &t, AgentEvent::User(message.into()));
        let cancel = self.new_cancel(id);
        self.set_state(id, TaskState::Queued);

        let this = self.clone();
        let message = message.to_string();
        tokio::spawn(async move {
            let Some(_permit) = this.acquire(id, &cancel).await else { return };
            this.set_state(id, TaskState::Running);
            let st = match this.run_turn(&t, prov, &message, Some(&session), &workdir, &cancel).await {
                Ok(s) => s,
                Err(e) => TaskState::Failed(e.to_string()),
            };
            this.finish(id, &cancel, st);
        });
        Ok(())
    }

    /// Cancela (mata o processo, ou retira da fila).
    pub fn cancel(&self, id: TaskId) -> Result<(), OrchestratorError> {
        let n = self.inner.cancels.lock().unwrap().get(&id).cloned().ok_or(OrchestratorError::UnknownTask(id))?;
        n.notify_one();
        Ok(())
    }

    fn provider(&self, id: &str) -> Result<Arc<dyn AgentProvider>, OrchestratorError> {
        self.inner.providers.get(id).cloned().ok_or_else(|| OrchestratorError::UnknownProvider(id.into()))
    }

    fn new_cancel(&self, id: TaskId) -> Arc<Notify> {
        let n = Arc::new(Notify::new());
        self.inner.cancels.lock().unwrap().insert(id, n.clone());
        n
    }

    /// Espera vaga no semáforo; None se cancelada enquanto na fila.
    async fn acquire(&self, id: TaskId, cancel: &Arc<Notify>) -> Option<tokio::sync::OwnedSemaphorePermit> {
        tokio::select! {
            p = self.inner.sem.clone().acquire_owned() => p.ok(),
            _ = cancel.notified() => { self.finish(id, cancel, TaskState::Cancelled); None }
        }
    }

    /// Estado final do turno + limpeza do mapa de cancelamento.
    fn finish(&self, id: TaskId, cancel: &Arc<Notify>, st: TaskState) {
        {
            let mut m = self.inner.cancels.lock().unwrap();
            if m.get(&id).is_some_and(|n| Arc::ptr_eq(n, cancel)) {
                m.remove(&id);
            }
        }
        self.set_state(id, st);
    }

    async fn first_turn(&self, task: &Task, prov: Arc<dyn AgentProvider>, cancel: &Notify) -> Result<TaskState, OrchestratorError> {
        let id = task.id;
        let workdir = if self.inner.use_worktrees {
            create_worktree(&task.project, id).await?
        } else {
            task.project.clone()
        };
        if let Some(t) = self.inner.tasks.lock().unwrap().get_mut(&id) {
            t.workdir = Some(workdir.clone());
        }
        if let Some(db) = &self.inner.db {
            let _ = db.set_task_workdir(id as i64, &workdir.to_string_lossy());
        }
        self.set_state(id, TaskState::Running);
        self.run_turn(task, prov, &task.prompt, None, &workdir, cancel).await
    }

    /// Grava (se houver banco) e transmite um evento do agente.
    fn record(&self, id: TaskId, task: &Task, ev: AgentEvent) {
        match &ev {
            AgentEvent::Session(s) => {
                let changed = {
                    let mut m = self.inner.tasks.lock().unwrap();
                    match m.get_mut(&id) {
                        Some(t) if t.session_id.as_deref() != Some(s) => {
                            t.session_id = Some(s.clone());
                            true
                        }
                        _ => false,
                    }
                };
                if !changed {
                    return; // não repete eventos de sessão
                }
                if let Some(db) = &self.inner.db {
                    let _ = db.set_task_session(id as i64, s);
                }
            }
            AgentEvent::Usage(u) => {
                if let Some(db) = &self.inner.db {
                    let _ = db.insert_usage(id as i64, &task.project.to_string_lossy(), &task.provider, u);
                }
            }
            _ => {}
        }
        if let Some(db) = &self.inner.db {
            let _ = db.insert_event(id as i64, ev.kind(), &json(&ev));
        }
        let _ = self.inner.tx.send(OrchestratorEvent::Agent { task: id, event: ev });
    }

    async fn run_turn(
        &self,
        task: &Task,
        prov: Arc<dyn AgentProvider>,
        prompt: &str,
        session: Option<&str>,
        workdir: &Path,
        cancel: &Notify,
    ) -> Result<TaskState, OrchestratorError> {
        let id = task.id;
        let spec = prov.task_command(prompt, workdir, session);
        let spec = crate::procs::apply_allowed_tools(spec, &task.project);
        let mut cmd = build_command(&spec, Some(workdir));
        #[cfg(unix)]
        cmd.process_group(0); // permite matar a árvore inteira no cancelamento
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if spec.stdin.is_some() { Stdio::piped() } else { Stdio::null() })
            .kill_on_drop(true)
            .spawn()?;
        if let Some(pid) = child.id() {
            crate::procs::tracker().track(pid, crate::procs::Owner::Session(id));
        }

        if let (Some(mut sin), Some(text)) = (child.stdin.take(), spec.stdin.clone()) {
            tokio::spawn(async move {
                let _ = sin.write_all(text.as_bytes()).await;
                let _ = sin.shutdown().await; // fecha o stdin (EOF)
            });
        }

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let this = self.clone();
        let t = task.clone();
        let out_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                for ev in prov.parse_line(&l) {
                    this.record(id, &t, ev);
                }
            }
        });
        let this = self.clone();
        let err_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                if let Some(db) = &this.inner.db {
                    let _ = db.insert_event(id as i64, "Stderr", &json(&AgentEvent::Raw(l.clone())));
                }
                let _ = this.inner.tx.send(OrchestratorEvent::Stderr { task: id, line: l });
            }
        });

        let result = tokio::select! {
            s = child.wait() => {
                let _ = out_task.await;
                let _ = err_task.await;
                let s = s?;
                if s.success() { TaskState::Completed } else { TaskState::Failed(format!("código de saída {:?}", s.code())) }
            }
            _ = cancel.notified() => {
                kill_tree(&mut child).await;
                out_task.abort();
                err_task.abort();
                TaskState::Cancelled
            }
        };
        Ok(result)
    }

    /// Diff das mudanças feitas no worktree da tarefa (inclui arquivos novos).
    pub async fn diff(&self, id: TaskId) -> Result<String, OrchestratorError> {
        let wt = self.worktree_of(id)?;
        git(&wt, &["add", "-A", "-N"]).await?;
        git(&wt, &["diff", "HEAD"]).await
    }

    /// Commita no worktree, faz merge da branch no projeto e remove o worktree.
    pub async fn apply(&self, id: TaskId) -> Result<(), OrchestratorError> {
        let wt = self.worktree_of(id)?;
        let t = self.task(id).ok_or(OrchestratorError::UnknownTask(id))?;
        git(&wt, &["add", "-A"]).await?;
        if !git(&wt, &["status", "--porcelain"]).await?.trim().is_empty() {
            let msg = format!("argus: tarefa {id}");
            git(&wt, &["commit", "-m", &msg]).await?;
        }
        let branch = branch_name(id);
        git(&t.project, &["merge", "--no-ff", "--no-edit", &branch]).await?;
        remove_worktree(&t.project, &wt, &branch).await?;
        self.set_state(id, TaskState::Applied);
        Ok(())
    }

    /// Remove o worktree e a branch sem aplicar.
    pub async fn discard(&self, id: TaskId) -> Result<(), OrchestratorError> {
        let wt = self.worktree_of(id)?;
        let t = self.task(id).ok_or(OrchestratorError::UnknownTask(id))?;
        remove_worktree(&t.project, &wt, &branch_name(id)).await?;
        self.set_state(id, TaskState::Discarded);
        Ok(())
    }

    fn worktree_of(&self, id: TaskId) -> Result<PathBuf, OrchestratorError> {
        if !self.inner.use_worktrees {
            return Err(OrchestratorError::NoWorktree);
        }
        self.task(id).ok_or(OrchestratorError::UnknownTask(id))?.workdir.ok_or(OrchestratorError::NoWorktree)
    }

    fn set_state(&self, id: TaskId, st: TaskState) {
        if let Some(t) = self.inner.tasks.lock().unwrap().get_mut(&id) {
            t.state = st.clone();
        }
        if let Some(db) = &self.inner.db {
            let _ = db.update_task_state(id as i64, &st.to_db());
        }
        self.emit_state(id, st);
    }

    fn emit_state(&self, id: TaskId, st: TaskState) {
        let _ = self.inner.tx.send(OrchestratorEvent::StateChanged { task: id, state: st });
    }
}

pub fn branch_name(id: TaskId) -> String {
    format!("argus/{id}")
}

/// `<proj>/../.argus-worktrees/<proj>-<id>`
pub fn worktree_path(project: &Path, id: TaskId) -> PathBuf {
    let name = project.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "proj".into());
    project
        .parent()
        .unwrap_or(project)
        .join(".argus-worktrees")
        .join(format!("{name}-{id}"))
}

async fn create_worktree(project: &Path, id: TaskId) -> Result<PathBuf, OrchestratorError> {
    let wt = worktree_path(project, id);
    if let Some(p) = wt.parent() {
        tokio::fs::create_dir_all(p).await?;
    }
    let wt_s = wt.to_string_lossy().into_owned();
    git(project, &["worktree", "add", "-b", &branch_name(id), &wt_s]).await?;
    Ok(wt)
}

async fn remove_worktree(project: &Path, wt: &Path, branch: &str) -> Result<(), OrchestratorError> {
    let wt_s = wt.to_string_lossy().into_owned();
    git(project, &["worktree", "remove", "--force", &wt_s]).await?;
    git(project, &["branch", "-D", branch]).await?;
    Ok(())
}

async fn git(dir: &Path, args: &[&str]) -> Result<String, OrchestratorError> {
    let out = build_command(&CommandSpec::new("git"), Some(dir)).args(args).stdin(Stdio::null()).output().await?;
    if !out.status.success() {
        return Err(OrchestratorError::Git(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn json(ev: &AgentEvent) -> String {
    serde_json::to_string(ev).unwrap_or_default()
}

/// Mata o processo e seus filhos (wrappers `.cmd`/shell deixam netos órfãos).
async fn kill_tree(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        #[cfg(windows)]
        let _ = build_command(&CommandSpec::new("taskkill"), None)
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        #[cfg(unix)]
        let _ = tokio::process::Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
    let _ = child.kill().await;
}

/// Monta um tokio Command a partir do CommandSpec (resolve o executável real,
/// preferindo `.exe` a wrappers `.cmd`; sem janela no Windows).
pub fn build_command(spec: &CommandSpec, default_cwd: Option<&Path>) -> tokio::process::Command {
    let program = resolve_program(&spec.program);
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(&spec.args);
    if let Some(c) = spec.cwd.as_deref().or(default_cwd) {
        cmd.current_dir(c);
    }
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

/// Caminho real do programa (`.exe` > `.cmd`), ou o nome como veio.
pub fn resolve_program(program: &str) -> PathBuf {
    let p = Path::new(program);
    if p.components().count() > 1 || p.is_absolute() {
        return p.to_path_buf();
    }
    super::provider::which_bin(program).unwrap_or_else(|| PathBuf::from(program))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::provider::LoginStatus;
    use std::time::Duration;

    /// Provider falso: roda um comando de shell e transforma cada linha em Text.
    /// "SESSION:x" vira `Session`; "USAGE" vira `Usage`. Com sessão (reply),
    /// ecoa o prompt recebido pelo stdin.
    struct Fake(Vec<String>);
    impl AgentProvider for Fake {
        fn id(&self) -> &str {
            "fake"
        }
        fn display_name(&self) -> &str {
            "Fake"
        }
        fn detect(&self) -> Option<PathBuf> {
            None
        }
        fn login_status(&self) -> LoginStatus {
            LoginStatus::Unknown
        }
        fn login_command(&self) -> CommandSpec {
            CommandSpec::new("cmd")
        }
        fn task_command(&self, p: &str, cwd: &Path, session: Option<&str>) -> CommandSpec {
            let mut c = CommandSpec::new(if cfg!(windows) { "cmd" } else { "sh" }).cwd(cwd).stdin(p);
            c.args = if session.is_some() { stdin_echo_args() } else { self.0.clone() };
            c
        }
        fn parse_line(&self, l: &str) -> Vec<AgentEvent> {
            let l = l.trim();
            if let Some(s) = l.strip_prefix("SESSION:") {
                return vec![AgentEvent::Session(s.to_string())];
            }
            if l == "USAGE" {
                return vec![AgentEvent::Usage(crate::agents::provider::TokenUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    model: Some("claude-sonnet-4-5".into()),
                    ..Default::default()
                })];
            }
            if l.is_empty() { vec![] } else { vec![AgentEvent::Text(l.to_string())] }
        }
    }

    fn stdin_echo_args() -> Vec<String> {
        if cfg!(windows) {
            vec!["/c".into(), "more".into()]
        } else {
            vec!["-c".into(), "cat".into()]
        }
    }

    fn script_args(lines: &[&str]) -> Vec<String> {
        if cfg!(windows) {
            vec!["/c".into(), lines.iter().map(|l| format!("echo {l}")).collect::<Vec<_>>().join("& ")]
        } else {
            vec!["-c".into(), lines.iter().map(|l| format!("echo {l}")).collect::<Vec<_>>().join("; ")]
        }
    }

    fn echo_args(text: &str) -> Vec<String> {
        if cfg!(windows) {
            vec!["/c".into(), format!("echo {text}")]
        } else {
            vec!["-c".into(), format!("echo {text}")]
        }
    }
    fn sleep_args() -> Vec<String> {
        if cfg!(windows) {
            vec!["/c".into(), "ping -n 30 127.0.0.1 >NUL".into()]
        } else {
            vec!["-c".into(), "sleep 30".into()]
        }
    }

    async fn wait_final(o: &Orchestrator, id: TaskId) -> TaskState {
        for _ in 0..300 {
            let s = o.task(id).unwrap().state;
            if !matches!(s, TaskState::Queued | TaskState::Running) {
                return s;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("timeout")
    }

    #[tokio::test]
    async fn runs_fake_and_broadcasts() {
        let dir = tempfile::tempdir().unwrap();
        let o = Orchestrator::new(vec![Arc::new(Fake(echo_args("ola")))], 2, false);
        let mut rx = o.subscribe();
        let id = o.submit(dir.path(), "fake", "x").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        let mut got = false;
        while let Ok(ev) = rx.try_recv() {
            if let OrchestratorEvent::Agent { event: AgentEvent::Text(t), .. } = ev {
                got |= t == "ola";
            }
        }
        assert!(got);
        assert!(o.submit(dir.path(), "nope", "x").is_err());
    }

    #[tokio::test]
    async fn cancel_kills() {
        let dir = tempfile::tempdir().unwrap();
        let o = Orchestrator::new(vec![Arc::new(Fake(sleep_args()))], 1, false);
        let id = o.submit(dir.path(), "fake", "x").unwrap();
        let id2 = o.submit(dir.path(), "fake", "x").unwrap(); // fica na fila
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(o.task(id2).unwrap().state, TaskState::Queued);
        o.cancel(id2).unwrap();
        o.cancel(id).unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Cancelled);
        assert_eq!(wait_final(&o, id2).await, TaskState::Cancelled);
    }

    #[tokio::test]
    async fn worktree_diff_and_discard() {
        if which::which("git").is_err() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let proj = root.path().join("proj");
        std::fs::create_dir(&proj).unwrap();
        std::fs::write(proj.join("a.txt"), "a\n").unwrap();
        for a in [
            vec!["init", "-q"],
            vec!["-c", "user.email=t@t", "-c", "user.name=t", "add", "."],
            vec!["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "init"],
        ] {
            git(&proj, &a).await.unwrap();
        }
        let write = if cfg!(windows) { "echo novo> b.txt" } else { "echo novo > b.txt" };
        let args = if cfg!(windows) { vec!["/c".into(), write.into()] } else { vec!["-c".into(), write.into()] };
        let o = Orchestrator::new(vec![Arc::new(Fake(args))], 1, true);
        let id = o.submit(&proj, "fake", "x").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        let wt = o.task(id).unwrap().workdir.unwrap();
        assert_eq!(wt, worktree_path(&proj, id));
        let d = o.diff(id).await.unwrap();
        assert!(d.contains("b.txt"), "{d}");
        o.discard(id).await.unwrap();
        assert!(!wt.exists());
        assert_eq!(o.task(id).unwrap().state, TaskState::Discarded);
    }

    #[tokio::test]
    async fn prompt_goes_through_stdin() {
        let dir = tempfile::tempdir().unwrap();
        let o = Orchestrator::new(vec![Arc::new(Fake(stdin_echo_args()))], 1, false);
        let mut rx = o.subscribe();
        let prompt = "aspas \"x\" 100% %PATH% & | > fim";
        let id = o.submit(dir.path(), "fake", prompt).unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        let mut got = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            if let OrchestratorEvent::Agent { event: AgentEvent::Text(t), .. } = ev {
                got.push(t);
            }
        }
        assert!(got.iter().any(|t| t == prompt), "{got:?}");
        assert!(o.inner.cancels.lock().unwrap().is_empty(), "mapa de cancelamento não foi limpo");
    }

    #[tokio::test]
    async fn reply_resumes_session_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let mk = |db| {
            Orchestrator::with_db(vec![Arc::new(Fake(script_args(&["SESSION:abc", "ola", "USAGE"])))], 1, false, db)
        };
        let dbp = dir.path().join("h.db");
        let o = mk(Some(Db::open(&dbp).unwrap()));
        assert!(matches!(o.reply(99, "x"), Err(OrchestratorError::UnknownTask(99))));
        let id = o.submit(dir.path(), "fake", "primeiro").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        assert_eq!(o.task(id).unwrap().session_id.as_deref(), Some("abc"));

        o.reply(id, "segunda mensagem").unwrap();
        assert!(matches!(o.reply(id, "x"), Err(OrchestratorError::Busy)));
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        assert_eq!(o.task(id).unwrap().turns, 2);

        let h = o.history(id);
        assert_eq!(h.first(), Some(&AgentEvent::User("primeiro".into())));
        assert!(h.contains(&AgentEvent::Session("abc".into())));
        assert!(h.contains(&AgentEvent::User("segunda mensagem".into())));
        // Segundo turno ecoa o prompt (stdin) no mesmo diretório.
        assert!(h.contains(&AgentEvent::Text("segunda mensagem".into())), "{h:?}");
        let usage = o.db().unwrap().usage_by_project(crate::db::TimeRange::all()).unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].input_tokens, 10);
        drop(o);

        // Recarrega do disco: histórico, sessão e próximo id.
        let o2 = mk(Some(Db::open(&dbp).unwrap()));
        let t = o2.task(id).unwrap();
        assert_eq!(t.state, TaskState::Completed);
        assert_eq!(t.session_id.as_deref(), Some("abc"));
        let id2 = o2.submit(dir.path(), "fake", "outra").unwrap();
        assert!(id2 > id);
        wait_final(&o2, id2).await;
    }

    #[tokio::test]
    async fn reply_without_session_fails() {
        let dir = tempfile::tempdir().unwrap();
        let o = Orchestrator::new(vec![Arc::new(Fake(echo_args("ola")))], 1, false);
        let id = o.submit(dir.path(), "fake", "x").unwrap();
        wait_final(&o, id).await;
        assert!(matches!(o.reply(id, "y"), Err(OrchestratorError::NoSession)));
    }

    #[test]
    fn state_db_roundtrip() {
        for s in [TaskState::Completed, TaskState::Failed("x y".into()), TaskState::Running] {
            assert_eq!(TaskState::from_db(&s.to_db()), s);
        }
        assert_eq!(TaskState::from_db("Completed"), TaskState::Completed);
    }
}
