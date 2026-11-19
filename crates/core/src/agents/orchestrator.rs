//! Orquestrador de tarefas de agentes: fila, limite de concorrência,
//! modo de trabalho por tarefa (git worktree isolado ou direto na pasta do
//! projeto), sessões conversacionais (vários turnos), modelo e provedor
//! trocáveis por tarefa, eventos via broadcast e persistência opcional em SQLite.

use super::provider::{subagents_of, valid_model_name, AgentEvent, AgentProvider, CommandSpec, SubagentInfo, TurnEnd, TurnOptions};
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

/// Onde o agente de uma tarefa trabalha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WorkMode {
    /// Numa cópia do projeto (git worktree próprio): as alterações são revisadas
    /// e aplicadas depois. Só enxerga o que está versionado.
    #[default]
    Isolated,
    /// Na própria pasta do projeto: enxerga tudo e as alterações valem na hora.
    Direct,
}

impl WorkMode {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkMode::Isolated => "Isolated",
            WorkMode::Direct => "Direct",
        }
    }
    pub fn parse(s: &str) -> Option<WorkMode> {
        match s {
            "Isolated" => Some(WorkMode::Isolated),
            "Direct" => Some(WorkMode::Direct),
            _ => None,
        }
    }
}

/// O que fica guardado de um provedor que já atendeu a tarefa, para voltar a ele.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderState {
    /// Sessão da CLI desse provedor (retomada com `--resume`/`--session`).
    pub session: Option<String>,
    pub model: Option<String>,
}

/// Escolhas de uma tarefa nova.
#[derive(Debug, Clone, Default)]
pub struct SubmitOptions {
    /// Modelo (None = padrão da CLI).
    pub model: Option<String>,
    /// Modo de trabalho (None = o padrão configurado para o projeto).
    pub mode: Option<WorkMode>,
    /// Acesso desde o primeiro turno (Planejar / normal / Bypass).
    pub access: crate::permissions::AccessMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub project: PathBuf,
    pub provider: String,
    pub prompt: String,
    pub state: TaskState,
    /// Diretório onde o agente roda (o worktree, ou o próprio projeto no modo direto).
    pub workdir: Option<PathBuf>,
    /// Sessão da CLI (claude: `session_id`; opencode: `sessionID`), para `reply`.
    pub session_id: Option<String>,
    /// Número de turnos da conversa (1 = só o prompt inicial). Um comando de barra
    /// que a própria CLI resolve, sem chamar modelo, não conta.
    pub turns: u32,
    /// Modelo dos próximos turnos (None = padrão da CLI).
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub mode: WorkMode,
    /// Houve troca de provedor: o próximo turno leva o resumo da conversa.
    #[serde(default)]
    pub handoff: bool,
    /// Ocupação da janela de contexto no último pedido do agente principal.
    #[serde(default)]
    pub context: Option<crate::agents::context::ContextUsage>,
    /// Modo de acesso da conversa (Planejar / normal / Bypass), persistido.
    #[serde(default)]
    pub access: crate::permissions::AccessMode,
    /// Sessão e modelo dos provedores que já atenderam a tarefa (por id).
    #[serde(skip)]
    pub others: HashMap<String, ProviderState>,
}

/// Aviso de ambiente para o agente que roda numa cópia isolada. Uma linha só, sem
/// aspas nem caracteres especiais de shell: vai como argumento de linha de comando.
/// Como o agente faz uma pergunta com opções: no `-p` não há ferramenta de
/// pergunta, então o bloco `argus-ask` vira o cartão clicável no chat.
/// O claude nega `cd <pasta>` seguido de `git` no mesmo comando (proteção da
/// própria CLI, não da política do Argus): orienta o `git -C`.
/// Planejar: a escrita está negada e no `-p` não existe a ferramenta de sair do
/// modo plano; o agente termina com o plano em texto (a interface oferece executar).
pub const PLAN_NOTE: &str = "Modo Planejar: não altere nenhum arquivo e não tente escrever, editar nem chamar ferramenta de sair do modo plano. Leia o necessário e termine a resposta com o plano em passos curtos; o usuário decide executar.";
pub const SHELL_NOTE: &str = "Para rodar git em outra pasta use git -C pasta comando, sem encadear cd com git: a CLI nega essa combinação.";
pub const ASK_NOTE: &str = "Quando precisar que o usuário escolha entre opções antes de continuar, termine a resposta com um bloco de código de linguagem argus-ask contendo um objeto JSON com a chave questions: uma lista de perguntas, cada uma com header (rótulo curto), question (a pergunta completa), multiSelect (true ou false) e options (de 2 a 4, cada uma com label e description). Pare logo depois do bloco: a interface mostra as opções e a escolha volta como a próxima mensagem. Pergunta aberta vai em texto normal.";

/// Aviso de sistema completo: isolamento (se for o caso), preferências e memória
/// (`crate::memory::note`) e o aviso de perguntas com opções. Só depende do modo e
/// de caminhos, então não muda entre turnos.
pub fn compose_note(mode: WorkMode, project: &Path, memory: Option<&str>) -> String {
    let mut parts = vec![];
    if mode == WorkMode::Isolated {
        parts.push(isolation_note(project));
    }
    if let Some(m) = memory.filter(|m| !m.is_empty()) {
        parts.push(m.to_string());
    }
    parts.push(ASK_NOTE.to_string());
    parts.push(SHELL_NOTE.to_string());
    parts.join(" ")
}

pub fn isolation_note(project: &Path) -> String {
    let name = project.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    format!(
        "Você está trabalhando numa cópia isolada (git worktree) do projeto {name}, cuja pasta original é {}. \
         Esta cópia só tem os arquivos versionados no git: pastas e arquivos não versionados ou ignorados da pasta original \
         (por exemplo .env, dados locais e pastas fora do git) e alterações ainda não commitadas não existem aqui. \
         Se o pedido depender de algo assim, diga isso em uma frase logo no começo da resposta, sem gastar passos procurando, \
         e não leia nem edite a pasta original.",
        project.display()
    )
}

#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    StateChanged { task: TaskId, state: TaskState },
    Agent { task: TaskId, event: AgentEvent },
    Stderr { task: TaskId, line: String },
    /// Progresso do modo auto-work (ver `crate::autowork`).
    AutoWork(crate::autowork::AutoWorkEvent),
    /// A tarefa foi excluída (ver `Orchestrator::delete`): some das listas.
    Removed { task: TaskId },
}

/// Chave e padrão do limite de compactação automática (ver `autocompact_limit`).
const AUTOCOMPACT_KEY: &str = "autocompact_tokens";
/// 200 mil tokens: acima disso cada chamada de ferramenta relê um histórico
/// que um resumo substitui bem (a sessão #9 chegou a ~600 mil no modelo de 1M).
pub const AUTOCOMPACT_DEFAULT: u64 = 200_000;

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
    #[error("modelo inválido: {0}")]
    InvalidModel(String),
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
    db: Option<Arc<Db>>,
}

/// Chave da preferência de um nível de git ("local" ou "remote") no banco.
fn perm_key(project: Option<&Path>) -> String {
    mode_key(project).replacen("work_mode", "perm_mode", 1)
}

fn git_key(project: Option<&Path>, level: &str) -> String {
    mode_key(project).replacen("work_mode", &format!("git_{level}"), 1)
}

/// Chave das preferências de modo de trabalho no banco (geral, ou de um projeto).
fn mode_key(project: Option<&Path>) -> String {
    match project {
        Some(p) => format!("work_mode:{}", p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase()),
        None => "work_mode".to_string(),
    }
}

#[derive(Clone)]
pub struct Orchestrator {
    inner: Arc<Inner>,
}

impl Orchestrator {
    pub fn new(providers: Vec<Arc<dyn AgentProvider>>, max_concurrent: usize) -> Self {
        Self::with_db(providers, max_concurrent, None)
    }

    /// Como `new`, gravando tarefas/eventos/consumo em `db` e recarregando o
    /// histórico salvo. Tarefas que estavam na fila/rodando ao fechar viram `Failed`.
    pub fn with_db(
        providers: Vec<Arc<dyn AgentProvider>>,
        max_concurrent: usize,
        db: Option<Db>,
    ) -> Self {
        // Folga para o texto parcial (TextDelta) de várias conversas ao mesmo tempo.
        let (tx, _) = broadcast::channel(8192);
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
                                model: r.model,
                                // Tarefas antigas (sem a coluna) rodaram todas em worktree.
                                mode: r.mode.as_deref().and_then(WorkMode::parse).unwrap_or_default(),
                                handoff: r.handoff,
                                context: r.context.as_deref().and_then(|s| serde_json::from_str(s).ok()),
                                access: r.access.as_deref().and_then(crate::permissions::AccessMode::parse).unwrap_or_default(),
                                others: r
                                    .provider_state
                                    .as_deref()
                                    .and_then(|s| serde_json::from_str(s).ok())
                                    .unwrap_or_default(),
                            },
                        );
                    }
                }
                Err(e) => tracing::warn!("falha ao carregar histórico: {e}"),
            }
            // Ids de tarefas excluídas (e o consumo delas) nunca são reaproveitados.
            if let Ok(m) = db.max_task_id() {
                next = next.max(m.max(0) as u64 + 1);
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
                db,
            }),
        }
    }

    pub fn db(&self) -> Option<&Db> {
        self.inner.db.as_deref()
    }

    /// Provedores registrados: (id, nome de exibição), por id.
    pub fn providers(&self) -> Vec<(String, String)> {
        let mut v: Vec<_> = self.inner.providers.values().map(|p| (p.id().to_string(), p.display_name().to_string())).collect();
        v.sort();
        v
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
    /// de resposta; linhas de stderr voltam como `Raw`. Os eventos internos de
    /// subagentes vêm como `Scoped` (ver `subagents` e `subagent_history`).
    pub fn history(&self, id: TaskId) -> Vec<AgentEvent> {
        let Some(db) = &self.inner.db else { return vec![] };
        db.list_events(id as i64)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|e| serde_json::from_str(&e.payload).ok())
            .collect()
    }

    /// Subagentes da tarefa (lista plana: use `parent` para montar a árvore),
    /// na ordem em que nasceram. Lido do banco; vazio sem banco.
    pub fn subagents(&self, id: TaskId) -> Vec<SubagentInfo> {
        subagents_of(&self.history(id))
    }

    /// Conversa de um subagente (somente leitura): o `SubagentStart` dele, os
    /// eventos internos já sem o invólucro `Scoped`, e o `SubagentEnd`.
    pub fn subagent_history(&self, id: TaskId, subagent: &str) -> Vec<AgentEvent> {
        let Some(db) = &self.inner.db else { return vec![] };
        db.list_subagent_events(id as i64, subagent)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|e| serde_json::from_str::<AgentEvent>(&e.payload).ok())
            .map(|e| match e {
                AgentEvent::Scoped { event, .. } => *event,
                o => o,
            })
            .collect()
    }

    /// Preferência de modo de trabalho gravada: a geral (`project` = None) ou a de
    /// um projeto. None = nunca escolhida.
    pub fn work_mode_setting(&self, project: Option<&Path>) -> Option<WorkMode> {
        let db = self.inner.db.as_ref()?;
        db.get_setting(&mode_key(project)).ok().flatten().as_deref().and_then(WorkMode::parse)
    }

    /// Grava a preferência de modo de trabalho (None apaga: volta a valer a geral
    /// ou, na geral, o padrão "Isolado"). Sem banco, não faz nada.
    pub fn set_work_mode_setting(&self, project: Option<&Path>, mode: Option<WorkMode>) -> Result<(), OrchestratorError> {
        if let Some(db) = &self.inner.db {
            db.set_setting(&mode_key(project), mode.map(WorkMode::as_str))?;
        }
        Ok(())
    }

    /// Preferência gravada de um nível ("local", "remote" ou "build"): a geral
    /// (`project` = None) ou a de um projeto. None = segue o padrão.
    pub fn git_setting(&self, project: Option<&Path>, level: &str) -> Option<bool> {
        let db = self.inner.db.as_ref()?;
        match db.get_setting(&git_key(project, level)).ok().flatten().as_deref() {
            Some("on") => Some(true),
            Some("off") => Some(false),
            _ => None,
        }
    }

    /// Grava a preferência de um nível de git (None apaga). Sem banco, não faz nada.
    pub fn set_git_setting(&self, project: Option<&Path>, level: &str, on: Option<bool>) -> Result<(), OrchestratorError> {
        if let Some(db) = &self.inner.db {
            db.set_setting(&git_key(project, level), on.map(|b| if b { "on" } else { "off" }))?;
        }
        Ok(())
    }

    /// Níveis que valem para um turno em `project` no modo `mode`: a preferência
    /// do projeto, depois a geral, depois o padrão do modo ("Git local" ligado só
    /// na cópia isolada; "Git remoto" desligado; "Compilar e testar" ligado).
    pub fn git_levels(&self, project: &Path, mode: WorkMode) -> crate::permissions::GitLevels {
        let def = crate::permissions::GitLevels::defaults(mode);
        let pick = |level: &str, d: bool| self.git_setting(Some(project), level).or_else(|| self.git_setting(None, level)).unwrap_or(d);
        crate::permissions::GitLevels {
            local: pick("local", def.local),
            remote: pick("remote", def.remote),
            build: pick("build", def.build),
        }
    }

    /// Preferência gravada de modo de permissão: a geral (`project` = None) ou a de
    /// um projeto. None = segue a geral / o padrão ("Auto").
    pub fn permission_mode_setting(&self, project: Option<&Path>) -> Option<crate::permissions::PermissionMode> {
        let db = self.inner.db.as_ref()?;
        db.get_setting(&perm_key(project)).ok().flatten().as_deref().and_then(crate::permissions::PermissionMode::parse)
    }

    /// Grava a preferência de modo de permissão (None apaga). Sem banco, não faz nada.
    pub fn set_permission_mode_setting(&self, project: Option<&Path>, mode: Option<crate::permissions::PermissionMode>) -> Result<(), OrchestratorError> {
        if let Some(db) = &self.inner.db {
            db.set_setting(&perm_key(project), mode.map(|m| m.as_str()))?;
        }
        Ok(())
    }

    /// Modo de permissão de um turno em `project`: o do projeto, o geral, "Auto".
    pub fn permission_mode(&self, project: &Path) -> crate::permissions::PermissionMode {
        self.permission_mode_setting(Some(project)).or_else(|| self.permission_mode_setting(None)).unwrap_or_default()
    }

    /// Modo de trabalho de uma conversa nova em `project` quando ninguém escolhe:
    /// a preferência do projeto, depois a geral, depois "Isolado". Um projeto que
    /// não é repositório git só aceita "Direto na pasta".
    pub fn default_mode(&self, project: &Path) -> WorkMode {
        if !crate::isolation::is_git_repo(project) {
            return WorkMode::Direct;
        }
        self.work_mode_setting(Some(project)).or_else(|| self.work_mode_setting(None)).unwrap_or_default()
    }

    /// Enfileira uma tarefa com o modelo inicial do provedor e o modo de trabalho
    /// padrão do projeto. Precisa estar dentro de um runtime tokio.
    pub fn submit(&self, project: impl Into<PathBuf>, provider: &str, prompt: &str) -> Result<TaskId, OrchestratorError> {
        let model = self.provider(provider)?.default_model();
        self.submit_with(project, provider, prompt, SubmitOptions { model, mode: None, ..Default::default() })
    }

    /// Como `submit`, com o modelo e o modo de trabalho escolhidos.
    pub fn submit_with(
        &self,
        project: impl Into<PathBuf>,
        provider: &str,
        prompt: &str,
        opts: SubmitOptions,
    ) -> Result<TaskId, OrchestratorError> {
        let prov = self.provider(provider)?;
        let project: PathBuf = project.into();
        let model = opts.model.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
        if let Some(m) = model.as_deref().filter(|m| !valid_model_name(m)) {
            return Err(OrchestratorError::InvalidModel(m.to_string()));
        }
        // Sem repositório git não existe cópia isolada.
        let mode = match opts.mode {
            Some(WorkMode::Isolated) if crate::isolation::is_git_repo(&project) => WorkMode::Isolated,
            Some(_) => WorkMode::Direct,
            None => self.default_mode(&project),
        };
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let task = Task {
            id,
            project,
            provider: provider.into(),
            prompt: prompt.into(),
            state: TaskState::Queued,
            workdir: None,
            session_id: None,
            turns: 1,
            model,
            mode,
            handoff: false,
            context: None,
            access: opts.access,
            others: HashMap::new(),
        };
        if let Some(db) = &self.inner.db {
            db.insert_task(
                Some(id as i64),
                &task.project.to_string_lossy(),
                provider,
                prompt,
                &TaskState::Queued.to_db(),
            )?;
            db.set_task_mode(id as i64, mode.as_str())?;
            if task.access != Default::default() {
                db.set_task_access(id as i64, task.access.as_str())?;
            }
            if task.model.is_some() {
                db.set_task_model(id as i64, task.model.as_deref())?;
            }
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
        // Depois de uma troca de provedor a sessão pode não existir ainda: o turno
        // abre uma nova e leva o resumo da conversa.
        let session = t.session_id.clone();
        if session.is_none() && !t.handoff {
            return Err(OrchestratorError::NoSession);
        }
        let workdir = match &t.workdir {
            Some(w) => w.clone(),
            None if t.mode == WorkMode::Direct => t.project.clone(),
            None => return Err(OrchestratorError::NoWorktree),
        };
        let prov = self.provider(&t.provider)?;
        // O histórico até aqui (sem a mensagem nova) é a matéria do resumo.
        let before = if t.handoff { self.history(id) } else { Vec::new() };
        self.record(id, &t, 0, AgentEvent::User(message.into()));
        let cancel = self.new_cancel(id);
        self.set_state(id, TaskState::Queued);

        let this = self.clone();
        let message = message.to_string();
        tokio::spawn(async move {
            let Some(_permit) = this.acquire(id, &cancel).await else { return };
            this.set_state(id, TaskState::Running);
            let mut prompt = message;
            if t.handoff {
                let ctx = this.handoff_context(&t, &before, &workdir).await;
                if !ctx.is_empty() {
                    // O usuário não vê este bloco: no histórico fica só a mensagem dele.
                    prompt = format!("{ctx}\n\n{prompt}");
                }
            }
            let opts = this.turn_options(&t);
            // Conversa grande demais: compacta antes, para o turno (e cada ferramenta
            // dele) não reler o histórico inteiro. Só o claude tem `/compact` no `-p`.
            if let (Some(sid), false) = (session.as_deref(), t.handoff) {
                if prov.id() == "claude" && this.needs_compact(id) {
                    if let Ok((TaskState::Cancelled, _)) = this.run_turn(&t, prov.clone(), "/compact", Some(sid), &workdir, &opts, &cancel).await {
                        this.finish(id, &cancel, TaskState::Cancelled);
                        return;
                    }
                }
            }
            let st = match this.run_turn(&t, prov, &prompt, session.as_deref(), &workdir, &opts, &cancel).await {
                Ok((s, used_model)) => {
                    if t.handoff {
                        // O resumo foi entregue: o turno terminou, ou a sessão nova já existe.
                        let opened = session.is_none() && this.task(id).is_some_and(|x| x.session_id.is_some());
                        if s == TaskState::Completed || opened {
                            this.set_handoff(id, false);
                        }
                    }
                    // Conta o turno só se um modelo trabalhou (ou se foi interrompido no meio).
                    if used_model || s == TaskState::Cancelled {
                        if let Some(x) = this.inner.tasks.lock().unwrap().get_mut(&id) {
                            x.turns += 1;
                        }
                    }
                    s
                }
                Err(e) => TaskState::Failed(e.to_string()),
            };
            this.finish(id, &cancel, st);
        });
        Ok(())
    }

    /// Troca o modelo da tarefa; vale a partir do próximo turno (o que está em
    /// andamento segue com o modelo com que começou). `None` = padrão da CLI.
    pub fn set_model(&self, id: TaskId, model: Option<&str>) -> Result<(), OrchestratorError> {
        let model = model.map(str::trim).filter(|m| !m.is_empty());
        if let Some(m) = model.filter(|m| !valid_model_name(m)) {
            return Err(OrchestratorError::InvalidModel(m.to_string()));
        }
        {
            let mut tasks = self.inner.tasks.lock().unwrap();
            let t = tasks.get_mut(&id).ok_or(OrchestratorError::UnknownTask(id))?;
            t.model = model.map(str::to_string);
        }
        if let Some(db) = &self.inner.db {
            db.set_task_model(id as i64, model)?;
        }
        Ok(())
    }

    /// Troca o modo de acesso da conversa; vale a partir do próximo turno.
    pub fn set_access(&self, id: TaskId, access: crate::permissions::AccessMode) -> Result<(), OrchestratorError> {
        {
            let mut tasks = self.inner.tasks.lock().unwrap();
            let t = tasks.get_mut(&id).ok_or(OrchestratorError::UnknownTask(id))?;
            t.access = access;
        }
        if let Some(db) = &self.inner.db {
            db.set_task_access(id as i64, access.as_str())?;
        }
        Ok(())
    }

    /// Troca o agente (provedor) que atende a tarefa, com o turno parado. A tarefa,
    /// a pasta de trabalho e o histórico continuam os mesmos. A sessão e o modelo do
    /// provedor que sai ficam guardados; se o que entra já atendeu a tarefa, a sessão
    /// dele é retomada. O próximo turno leva um resumo da conversa (`handoff`).
    pub fn set_provider(&self, id: TaskId, provider: &str) -> Result<(), OrchestratorError> {
        let prov = self.provider(provider)?;
        let (from, task) = {
            let mut tasks = self.inner.tasks.lock().unwrap();
            let t = tasks.get_mut(&id).ok_or(OrchestratorError::UnknownTask(id))?;
            match t.state {
                ref s if s.is_active() => return Err(OrchestratorError::Busy),
                TaskState::Applied | TaskState::Discarded => return Err(OrchestratorError::Closed),
                _ => {}
            }
            if t.provider == provider {
                return Ok(());
            }
            let from = std::mem::replace(&mut t.provider, provider.to_string());
            let leaving = ProviderState { session: t.session_id.take(), model: t.model.take() };
            t.others.insert(from.clone(), leaving);
            match t.others.remove(provider) {
                Some(saved) => {
                    t.session_id = saved.session;
                    t.model = saved.model;
                }
                None => t.model = prov.default_model(),
            }
            t.handoff = true;
            (from, t.clone())
        };
        if let Some(db) = &self.inner.db {
            db.set_task_provider(
                id as i64,
                provider,
                task.session_id.as_deref(),
                task.model.as_deref(),
                &serde_json::to_string(&task.others).unwrap_or_default(),
                true,
            )?;
        }
        self.record(id, &task, 0, AgentEvent::ProviderSwitch { from, to: provider.to_string() });
        Ok(())
    }

    fn set_handoff(&self, id: TaskId, on: bool) {
        if let Some(t) = self.inner.tasks.lock().unwrap().get_mut(&id) {
            t.handoff = on;
        }
        if let Some(db) = &self.inner.db {
            let _ = db.set_task_handoff(id as i64, on);
        }
    }

    /// Limite de contexto (tokens) acima do qual a conversa é compactada antes do
    /// próximo turno. Configuração `autocompact_tokens`; 0 desliga.
    pub fn autocompact_limit(&self) -> u64 {
        self.inner
            .db
            .as_ref()
            .and_then(|db| db.get_setting(AUTOCOMPACT_KEY).ok().flatten())
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(AUTOCOMPACT_DEFAULT)
    }

    /// Grava o limite de compactação (None volta ao padrão).
    pub fn set_autocompact_limit(&self, tokens: Option<u64>) -> Result<(), OrchestratorError> {
        if let Some(db) = &self.inner.db {
            db.set_setting(AUTOCOMPACT_KEY, tokens.map(|t| t.to_string()).as_deref())?;
        }
        Ok(())
    }

    /// O contexto do último pedido da tarefa passou do limite?
    fn needs_compact(&self, id: TaskId) -> bool {
        let limit = self.autocompact_limit();
        let used = self.inner.tasks.lock().unwrap().get(&id).and_then(|t| t.context.as_ref().map(|c| c.input_tokens + c.cache_read + c.cache_write));
        limit > 0 && used.is_some_and(|u| u >= limit)
    }

    /// Modelo e aviso de ambiente do próximo turno de `t`.
    fn turn_options(&self, t: &Task) -> TurnOptions {
        TurnOptions {
            model: t.model.clone(),
            // Texto fixo (não muda entre turnos, para não regravar o cache).
            system_note: Some({
                let note = compose_note(t.mode, &t.project, crate::memory::for_project(&t.project).map(|m| m.0).as_deref());
                if t.access == crate::permissions::AccessMode::Plan { format!("{PLAN_NOTE} {note}") } else { note }
            }),
            access: t.access,
        }
    }

    /// Resumo da conversa para o provedor que assume a tarefa (ver `handoff`).
    async fn handoff_context(&self, t: &Task, before: &[AgentEvent], workdir: &Path) -> String {
        // Quem atendia: o `from` da última troca.
        let from = before.iter().rev().find_map(|e| match e {
            AgentEvent::ProviderSwitch { from, .. } => Some(from.clone()),
            _ => None,
        });
        let from_name = from
            .and_then(|f| self.inner.providers.get(&f).map(|p| p.display_name().to_string()).or(Some(f)))
            .unwrap_or_else(|| "outro agente".to_string());
        // Arquivos alterados na pasta de trabalho (no worktree, é o que a tarefa mudou).
        let files = match git(workdir, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]).await {
            Ok(raw) => {
                let (untracked, _, modified) = crate::isolation::parse_status(raw.as_bytes());
                let mut v: Vec<String> = modified.into_iter().chain(untracked).filter(|p| !crate::isolation::is_noise(p)).collect();
                v.sort();
                v
            }
            Err(_) => Vec::new(),
        };
        super::handoff::build_context(before, &t.provider, &from_name, t.session_id.is_some(), &files, super::handoff::MAX_CHARS)
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
        let workdir = match task.mode {
            WorkMode::Isolated => create_worktree(&task.project, id).await?,
            WorkMode::Direct => task.project.clone(),
        };
        if let Some(t) = self.inner.tasks.lock().unwrap().get_mut(&id) {
            t.workdir = Some(workdir.clone());
        }
        if let Some(db) = &self.inner.db {
            let _ = db.set_task_workdir(id as i64, &workdir.to_string_lossy());
        }
        self.set_state(id, TaskState::Running);
        let opts = self.turn_options(task);
        Ok(self.run_turn(task, prov, &task.prompt, None, &workdir, &opts, cancel).await?.0)
    }

    /// Grava (se houver banco) e transmite um evento do agente. `turn` identifica
    /// o turno nas linhas de consumo (um turno com dois modelos grava duas linhas).
    fn record(&self, id: TaskId, task: &Task, turn: i64, ev: AgentEvent) {
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
            AgentEvent::TextDelta(_) => {
                // Texto parcial: só ao vivo, o `Text` completo é que fica no histórico.
                let _ = self.inner.tx.send(OrchestratorEvent::Agent { task: id, event: ev });
                return;
            }
            AgentEvent::Context(c) => {
                // Só o último valor importa: fica na tarefa (e no banco), não no histórico.
                let mut c = c.clone();
                let model = self.inner.tasks.lock().unwrap().get(&id).and_then(|t| t.model.clone()).or_else(|| task.model.clone());
                crate::agents::context::resolve_window(&mut c, model.as_deref());
                if let Some(t) = self.inner.tasks.lock().unwrap().get_mut(&id) {
                    t.context = Some(c.clone());
                }
                if let Some(db) = &self.inner.db {
                    let _ = db.set_task_context(id as i64, &serde_json::to_string(&c).unwrap_or_default());
                }
                let _ = self.inner.tx.send(OrchestratorEvent::Agent { task: id, event: AgentEvent::Context(c) });
                return;
            }
            AgentEvent::Usage(u) => {
                if let Some(db) = &self.inner.db {
                    let _ = db.insert_usage_turn(id as i64, &task.project.to_string_lossy(), &task.provider, u, turn);
                }
            }
            _ => {}
        }
        if let Some(db) = &self.inner.db {
            let _ = db.insert_event_sub(id as i64, ev.kind(), &json(&ev), ev.subagent_id());
        }
        let _ = self.inner.tx.send(OrchestratorEvent::Agent { task: id, event: ev });
    }

    /// Grava um evento do processo do turno: preenche o modelo do uso que veio
    /// sem ele (opencode: vale o da tarefa) e marca se algum modelo foi usado.
    fn record_turn_event(&self, id: TaskId, task: &Task, turn: i64, mut ev: AgentEvent, model: Option<&str>, used: &std::sync::atomic::AtomicBool) {
        if let AgentEvent::Usage(u) = &mut ev {
            if u.model.is_none() {
                u.model = model.map(str::to_string);
            }
        }
        if matches!(ev.inner(), AgentEvent::Usage(u) if !u.is_empty()) {
            used.store(true, Ordering::SeqCst);
        }
        self.record(id, task, turn, ev);
    }

    /// Roda um turno. Devolve o estado final e se algum modelo foi usado (houve
    /// consumo): um comando de barra resolvido pela própria CLI devolve `false`.
    async fn run_turn(
        &self,
        task: &Task,
        prov: Arc<dyn AgentProvider>,
        prompt: &str,
        session: Option<&str>,
        workdir: &Path,
        opts: &TurnOptions,
        cancel: &Notify,
    ) -> Result<(TaskState, bool), OrchestratorError> {
        let id = task.id;
        // Identifica o turno nas linhas de consumo: uma tarefa roda um turno por vez.
        let turn = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        prov.begin_turn(session);
        let spec = prov.task_command(prompt, workdir, session, opts);
        // Comandos de shell liberados: o padrão (git de leitura, inspeção), os níveis
        // de git do projeto neste modo e a lista "Comandos permitidos" do projeto.
        let spec = crate::permissions::apply_for(spec, &task.project, self.git_levels(&task.project, task.mode), self.permission_mode(&task.project), opts.access);
        // Os anexos do chat ficam fora da pasta de trabalho: libera a leitura deles.
        let mut dirs = crate::attachments::dirs_for(&crate::attachments::root(), id, &[&task.prompt, prompt]);
        // Preferências, memória e notas de projeto: o aviso de sistema aponta
        // `<raiz>/_Argus` e `<raiz>/<Projeto>.md`. A raiz inteira fica legível
        // (decisão do usuário), para a nota do projeto e as outras do vault.
        // Codex e gemini liberam com escrita: para eles, só a pasta `_Argus`.
        if crate::memory::for_project(&task.project).is_some() {
            if let Some(root) = crate::memory::root() {
                let claude = Path::new(&spec.program).file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("claude"));
                dirs.push(if claude { root } else { crate::memory::dir(&root) });
            }
        }
        let spec = crate::attachments::apply_add_dirs(spec, &dirs);
        // Celulares Android: MCP do Screen-Mirror, se configurado e ligado.
        let spec = crate::devices::apply_agent_mcp(spec, workdir);
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
        // Agente que executa comandos por um daemon próprio (opencode) deixa o
        // servidor fora da árvore do processo lançado aqui: a pasta de trabalho
        // é o que permite reconhecê-lo mesmo assim.
        crate::procs::tracker().track_dir(crate::procs::Owner::Session(id), workdir);

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
        let used_model = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let used = used_model.clone();
        let turn_model = opts.model.clone();
        // Acompanha o processo para completar o que a CLI não imprimiu (uso de
        // turno cancelado, resultado que faltou). Compartilhado com a leitura.
        let tracker = Arc::new(std::sync::Mutex::new(prov.turn_tracker()));
        let tracker_out = tracker.clone();
        let model_out = turn_model.clone();
        // Loop (mesma ferramenta 3x seguidas): para o turno pelo core.
        let looped = Arc::new(Notify::new());
        let looped_out = looped.clone();
        let out_task = tokio::spawn(async move {
            let mut guard = super::loop_guard::RepeatGuard::new();
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let evs = prov.parse_line(&l);
                let evs = {
                    let mut tr = tracker_out.lock().unwrap_or_else(|e| e.into_inner());
                    tr.observe(&l, &evs);
                    tr.transform(evs)
                };
                for ev in evs {
                    let stop = guard.observe(&ev);
                    this.record_turn_event(id, &t, turn, ev, model_out.as_deref(), &used);
                    if let Some(msg) = stop {
                        this.record_turn_event(id, &t, turn, AgentEvent::Result { ok: false, summary: msg, cost: None }, model_out.as_deref(), &used);
                        looped_out.notify_one();
                        return;
                    }
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
                (if s.success() { TaskState::Completed } else { TaskState::Failed(format!("código de saída {:?}", s.code())) },
                 TurnEnd::Exited { ok: s.success() })
            }
            _ = async { tokio::select! { _ = cancel.notified() => {}, _ = looped.notified() => {} } } => {
                kill_tree(&mut child).await;
                out_task.abort();
                err_task.abort();
                (TaskState::Cancelled, TurnEnd::Cancelled)
            }
        };
        let (result, end) = result;
        let tail = tracker.lock().unwrap_or_else(|e| e.into_inner()).finish(end);
        for ev in tail {
            self.record_turn_event(id, task, turn, ev, turn_model.as_deref(), &used_model);
        }
        Ok((result, used_model.load(Ordering::SeqCst)))
    }

    /// Diff das mudanças da tarefa (inclui arquivos novos). No modo isolado, o que
    /// mudou no worktree; no modo direto, a pasta do projeto em relação ao último
    /// commit (sem tocar no índice do git do usuário).
    pub async fn diff(&self, id: TaskId) -> Result<String, OrchestratorError> {
        let t = self.task(id).ok_or(OrchestratorError::UnknownTask(id))?;
        if t.mode == WorkMode::Direct {
            return direct_diff(&t.project).await;
        }
        let wt = self.worktree_of(id)?;
        git(&wt, &["add", "-A", "-N"]).await?;
        git(&wt, &["diff", "HEAD"]).await
    }

    /// Pasta onde a tarefa altera arquivos: o worktree (isolado) ou o projeto (direto).
    fn change_dir(&self, id: TaskId) -> Result<(Task, PathBuf), OrchestratorError> {
        let t = self.task(id).ok_or(OrchestratorError::UnknownTask(id))?;
        let dir = if t.mode == WorkMode::Direct { t.project.clone() } else { self.worktree_of(id)? };
        Ok((t, dir))
    }

    /// Rejeita as alterações da tarefa num arquivo: volta ao último commit (ou
    /// apaga o arquivo novo). `path` é relativo à pasta de trabalho, como no diff.
    pub async fn revert_file(&self, id: TaskId, path: &str) -> Result<(), OrchestratorError> {
        let (t, dir) = self.change_dir(id)?;
        if t.state.is_active() {
            return Err(OrchestratorError::Busy);
        }
        revert_file_in(&dir, path, t.mode == WorkMode::Isolated).await
    }

    /// Rejeita um trecho do diff de um arquivo (`hunk` = a linha `@@ -a,b +c,d @@`).
    pub async fn revert_hunk(&self, id: TaskId, path: &str, hunk: &str) -> Result<(), OrchestratorError> {
        let (t, dir) = self.change_dir(id)?;
        if t.state.is_active() {
            return Err(OrchestratorError::Busy);
        }
        revert_hunk_in(&dir, path, hunk, t.mode == WorkMode::Isolated).await
    }

    /// Commita no worktree, faz merge da branch no projeto e remove o worktree.
    pub async fn apply(&self, id: TaskId) -> Result<(), OrchestratorError> {
        let wt = self.worktree_of(id)?;
        let t = self.task(id).ok_or(OrchestratorError::UnknownTask(id))?;
        // Pasta já removida numa tentativa anterior: só faltam o merge e a branch.
        if wt.exists() {
            git(&wt, &["add", "-A"]).await?;
        }
        if wt.exists() && !git(&wt, &["status", "--porcelain"]).await?.trim().is_empty() {
            let msg = format!("argus: tarefa {id}");
            git(&wt, &["commit", "-m", &msg]).await?;
        }
        let branch = branch_name(id);
        // O merge já feito se repete sem efeito ("Already up to date"): se a
        // remoção abaixo falhar, Aplicar pode ser chamado de novo; a tarefa só
        // vira Aplicada quando o worktree e a branch saíram.
        git(&t.project, &["merge", "--no-ff", "--no-edit", &branch]).await?;
        remove_worktree(&t.project, &wt, &branch, id).await?;
        self.set_state(id, TaskState::Applied);
        Ok(())
    }

    /// Remove o worktree e a branch sem aplicar.
    pub async fn discard(&self, id: TaskId) -> Result<(), OrchestratorError> {
        let wt = self.worktree_of(id)?;
        let t = self.task(id).ok_or(OrchestratorError::UnknownTask(id))?;
        remove_worktree(&t.project, &wt, &branch_name(id), id).await?;
        self.set_state(id, TaskState::Discarded);
        Ok(())
    }

    /// Exclui a tarefa: cancela o turno em andamento (e espera ele parar), no
    /// modo isolado remove o worktree e a branch que ainda existirem (as
    /// alterações não aplicadas se perdem) e apaga a tarefa e seus eventos do
    /// banco. No modo direto a pasta do projeto nunca é tocada. O consumo fica
    /// nas métricas (ver `Db::delete_task`). Emite `Removed`.
    pub async fn delete(&self, id: TaskId) -> Result<(), OrchestratorError> {
        let t = self.task(id).ok_or(OrchestratorError::UnknownTask(id))?;
        if t.state.is_active() {
            let _ = self.cancel(id);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            while self.task(id).is_some_and(|t| t.state.is_active()) {
                if std::time::Instant::now() >= deadline {
                    return Err(OrchestratorError::Busy);
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
        let t = self.task(id).ok_or(OrchestratorError::UnknownTask(id))?;
        let closed = matches!(t.state, TaskState::Applied | TaskState::Discarded);
        if t.mode == WorkMode::Isolated && !closed {
            if let Some(wt) = t.workdir.as_deref() {
                if wt.exists() {
                    remove_worktree(&t.project, wt, &branch_name(id), id).await?;
                } else if t.project.is_dir() {
                    // Pasta já removida: só sobra (talvez) a branch.
                    let _ = git(&t.project, &["worktree", "prune"]).await;
                    let _ = git(&t.project, &["branch", "-D", &branch_name(id)]).await;
                }
            }
        }
        if let Some(db) = &self.inner.db {
            db.delete_task(id as i64)?;
        }
        self.inner.tasks.lock().unwrap().remove(&id);
        self.inner.cancels.lock().unwrap().remove(&id);
        let _ = self.inner.tx.send(OrchestratorEvent::Removed { task: id });
        Ok(())
    }

    /// Exclui as tarefas encerradas (aplicadas ou descartadas) de um projeto.
    /// Devolve os ids excluídos.
    pub async fn delete_closed(&self, project: &Path) -> Result<Vec<TaskId>, OrchestratorError> {
        let key = mode_key(Some(project));
        let ids: Vec<TaskId> = self
            .tasks()
            .into_iter()
            .filter(|t| matches!(t.state, TaskState::Applied | TaskState::Discarded) && mode_key(Some(&t.project)) == key)
            .map(|t| t.id)
            .collect();
        for id in &ids {
            self.delete(*id).await?;
        }
        Ok(ids)
    }

    /// Worktree da tarefa; no modo direto não há (nada a aplicar nem a descartar).
    fn worktree_of(&self, id: TaskId) -> Result<PathBuf, OrchestratorError> {
        let t = self.task(id).ok_or(OrchestratorError::UnknownTask(id))?;
        if t.mode != WorkMode::Isolated {
            return Err(OrchestratorError::NoWorktree);
        }
        t.workdir.ok_or(OrchestratorError::NoWorktree)
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

/// Encerra os processos que prendem o worktree (a árvore da sessão e os que
/// rodam dentro dele; ver `procs::worktree_pids`). Devolve quantos.
async fn release_worktree(id: TaskId, wt: &Path) -> usize {
    let wt = wt.to_path_buf();
    tokio::task::spawn_blocking(move || crate::procs::tracker().kill_in_dir(crate::procs::Owner::Session(id), &wt).len())
        .await
        .unwrap_or(0)
}

/// Remove o worktree e, só se ele saiu, a branch. Um processo da sessão (o
/// serviço em segundo plano do opencode, um servidor) pode manter a pasta
/// aberta: encerra esses processos e tenta de novo algumas vezes; por fim,
/// `--force --force` + `prune`. Se ainda assim a pasta ficar, devolve erro e
/// mantém a branch, para que a operação possa ser repetida.
async fn remove_worktree(project: &Path, wt: &Path, branch: &str, id: TaskId) -> Result<(), OrchestratorError> {
    const TRIES: u32 = 4;
    let wt_s = wt.to_string_lossy().into_owned();
    let mut last = String::new();
    if !wt.exists() {
        let _ = git(project, &["worktree", "prune"]).await;
        git(project, &["branch", "-D", branch]).await?;
        return Ok(());
    }
    for i in 0..TRIES {
        if i > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(400 * i as u64)).await;
        }
        release_worktree(id, wt).await;
        match git(project, &["worktree", "remove", "--force", &wt_s]).await {
            Ok(_) => {
                last.clear();
                break;
            }
            Err(e) => last = e.to_string(),
        }
    }
    if !last.is_empty() {
        // Último recurso: força dupla (também para worktree travado) e limpeza do registro.
        if let Err(e) = git(project, &["worktree", "remove", "--force", "--force", &wt_s]).await {
            last = e.to_string();
            if wt.exists() {
                let _ = tokio::fs::remove_dir_all(wt).await;
            }
        }
        let _ = git(project, &["worktree", "prune"]).await;
        if wt.exists() {
            return Err(OrchestratorError::Git(format!(
                "não foi possível remover o worktree {wt_s}: a pasta continua em uso por outro processo. \
                 Feche programas abertos nessa pasta e tente de novo. A branch {branch} foi mantida. ({})",
                last.trim()
            )));
        }
    }
    git(project, &["branch", "-D", branch]).await?;
    Ok(())
}

/// Diff da pasta do projeto em relação ao último commit, com os arquivos novos
/// (não versionados e não ignorados) no fim. Só lê: não usa `git add -N`.
async fn direct_diff(project: &Path) -> Result<String, OrchestratorError> {
    if !crate::isolation::is_git_repo(project) {
        return Err(OrchestratorError::Git("a pasta do projeto não é um repositório git: não há commit para comparar".into()));
    }
    // Repositório sem nenhum commit: não há HEAD; ficam só os arquivos novos.
    let mut out = git(project, &["diff", "HEAD"]).await.unwrap_or_default();
    let new_files = git(project, &["ls-files", "--others", "--exclude-standard", "-z"]).await.unwrap_or_default();
    const MAX_NEW: usize = 60;
    for file in new_files.split('\0').filter(|f| !f.is_empty() && !crate::isolation::is_noise(f)).take(MAX_NEW) {
        // `--no-index` sai com código 1 quando há diferença: a saída vale mesmo assim.
        let raw = build_command(&CommandSpec::new("git"), Some(project))
            .args(["diff", "--no-index", "--", "/dev/null", file])
            .stdin(Stdio::null())
            .output()
            .await?;
        out.push_str(&String::from_utf8_lossy(&raw.stdout));
    }
    Ok(out)
}

/// Caminho relativo seguro vindo da tela (sem `..`, absoluto ou opção do git).
fn safe_rel(path: &str) -> Result<String, OrchestratorError> {
    let p = path.replace('\\', "/");
    let bad = p.is_empty()
        || p.starts_with('/')
        || p.starts_with('-')
        || p.contains(':')
        || p.split('/').any(|c| c == ".." || c.is_empty());
    if bad {
        return Err(OrchestratorError::Git(format!("caminho inválido: {path}")));
    }
    Ok(p)
}

/// Volta um arquivo ao último commit; arquivo novo é apagado (e, no isolado, sai
/// do índice, onde o `add -N` o pôs). No direto o índice do usuário não é tocado.
async fn revert_file_in(dir: &Path, path: &str, isolated: bool) -> Result<(), OrchestratorError> {
    let rel = safe_rel(path)?;
    let tracked = git(dir, &["cat-file", "-e", &format!("HEAD:{rel}")]).await.is_ok();
    if tracked {
        git(dir, &["restore", "--source=HEAD", "--worktree", "--", &rel]).await?;
    } else {
        if isolated {
            git(dir, &["rm", "--cached", "-q", "--ignore-unmatch", "--", &rel]).await?;
        }
        let f = dir.join(&rel);
        if f.is_file() {
            std::fs::remove_file(f)?;
        }
    }
    Ok(())
}

/// Patch com só um trecho de um arquivo: o cabeçalho do arquivo e o hunk cujo
/// cabeçalho começa com `hunk` (`@@ -a,b +c,d @@`). Mantém as quebras de linha
/// originais. None se o trecho não está no diff (o diff mudou).
pub fn hunk_patch(diff: &str, hunk: &str) -> Option<String> {
    let want = hunk.trim_end_matches(['\r', '\n']);
    let want = match want.find(" @@") {
        Some(i) => &want[..i + 3],
        None => want,
    };
    let lines: Vec<&str> = diff.split_inclusive('\n').collect();
    let first = lines.iter().position(|l| l.starts_with("@@"))?;
    let mut out: String = lines[..first].concat();
    let start = lines.iter().position(|l| l.starts_with(want))?;
    let end = lines[start + 1..].iter().position(|l| l.starts_with("@@") || l.starts_with("diff --git")).map(|i| start + 1 + i).unwrap_or(lines.len());
    out.extend(lines[start..end].iter().copied());
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

/// Desfaz um trecho com `git apply -R` (só nos arquivos, sem índice). Arquivo novo
/// ou apagado tem um trecho só: volta o arquivo inteiro.
async fn revert_hunk_in(dir: &Path, path: &str, hunk: &str, isolated: bool) -> Result<(), OrchestratorError> {
    let rel = safe_rel(path)?;
    if isolated {
        git(dir, &["add", "-A", "-N"]).await?;
    }
    let diff = git(dir, &["diff", "HEAD", "--", &rel]).await?;
    if diff.trim().is_empty() || diff.lines().any(|l| l.starts_with("new file mode") || l.starts_with("deleted file mode")) {
        return revert_file_in(dir, &rel, isolated).await;
    }
    let patch = hunk_patch(&diff, hunk).ok_or_else(|| OrchestratorError::Git("o trecho não está mais no diff: recarregue as alterações".into()))?;
    let mut child = build_command(&CommandSpec::new("git"), Some(dir))
        .args(["apply", "-R", "--whitespace=nowarn", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut sin) = child.stdin.take() {
        sin.write_all(patch.as_bytes()).await?;
        sin.shutdown().await?;
    }
    let out = child.wait_with_output().await?;
    if !out.status.success() {
        return Err(OrchestratorError::Git(format!("git apply -R: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
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
        // Quem abre o Argus por um shell (Git Bash, `npm run tauri dev`) deixa a
        // variável `PWD` apontando para a pasta de onde ele foi aberto. Há CLI que
        // confia nela em vez do diretório real: o `opencode run`, visto numa execução
        // real, trabalhou nessa pasta e não no worktree da tarefa.
        cmd.env("PWD", c);
    }
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

/// Caminho real do programa (ordem do PATH/PATHEXT, ver `which_bin`), ou o nome como veio.
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
        fn task_command(&self, p: &str, cwd: &Path, session: Option<&str>, _o: &TurnOptions) -> CommandSpec {
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

    /// Provider que imprime a fixture real do claude (via `type`/`cat`) e usa o parser real.
    struct Replay(PathBuf, crate::agents::claude::ClaudeProvider);
    impl AgentProvider for Replay {
        fn id(&self) -> &str {
            "replay"
        }
        fn display_name(&self) -> &str {
            "Replay"
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
        fn task_command(&self, _p: &str, cwd: &Path, _s: Option<&str>, _o: &TurnOptions) -> CommandSpec {
            let f = self.0.to_string_lossy().into_owned();
            let c = if cfg!(windows) {
                CommandSpec::new("cmd").arg("/c").arg("type").arg(f.replace('/', "\\"))
            } else {
                CommandSpec::new("cat").arg(f)
            };
            c.cwd(cwd)
        }
        fn parse_line(&self, l: &str) -> Vec<AgentEvent> {
            self.1.parse_line(l)
        }
    }

    #[tokio::test]
    async fn subagent_events_are_persisted_and_read_back() {
        const SUB: &str = "toolu_01CsNFNQqgKLL88nwZezfZRE";
        let dir = tempfile::tempdir().unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("claude_subagent.jsonl");
        let dbp = dir.path().join("h.db");
        let mk = || {
            let prov: Arc<dyn AgentProvider> = Arc::new(Replay(fixture.clone(), Default::default()));
            Orchestrator::with_db(vec![prov], 1, Some(Db::open(&dbp).unwrap()))
        };
        let o = mk();
        let mut rx = o.subscribe();
        let id = o.submit(dir.path(), "replay", "liste com um subagente").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);

        // Ao vivo: início, evento interno marcado e fim chegam pelo broadcast.
        let mut live = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            if let OrchestratorEvent::Agent { event, .. } = ev {
                live.push(event);
            }
        }
        assert!(live.iter().any(|e| matches!(e, AgentEvent::SubagentStart { id, parent: None, .. } if id == SUB)), "{live:?}");
        assert!(live.iter().any(|e| matches!(e, AgentEvent::Scoped { subagent, .. } if subagent == SUB)));
        assert!(live.iter().any(|e| matches!(e, AgentEvent::SubagentEnd { id, ok: true, .. } if id == SUB)));

        // A ocupação de contexto foi ao vivo, mas não para o histórico.
        assert!(live.iter().any(|e| matches!(e, AgentEvent::Context(_))));
        drop(o);

        // Do disco, noutra instância.
        let o = mk();
        // Último pedido do agente principal (o do subagente não conta), com a
        // janela informada pelo `result`; sobrevive ao reinício.
        let c = o.task(id).unwrap().context.expect("contexto salvo");
        assert_eq!((c.total(), c.window), (40538, Some(1_000_000)));
        let kinds: Vec<&str> = o.history(id).iter().map(|e| e.kind()).collect();
        assert_eq!(
            kinds,
            ["User", "Session", "ToolUse", "SubagentStart", "ToolUse", "ToolResult", "SubagentStats", "SubagentEnd", "Text", "Usage", "Result"]
        );
        let subs = o.subagents(id);
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].id, SUB);
        assert_eq!(subs[0].description, "Listar arquivos da pasta");
        assert_eq!(subs[0].result.as_deref(), Some("- alfa.txt\n- beta.md"));
        let h = o.subagent_history(id, SUB);
        assert_eq!(h.len(), 5, "{h:?}");
        assert!(matches!(&h[0], AgentEvent::SubagentStart { .. }));
        assert!(matches!(&h[1], AgentEvent::ToolUse { name, .. } if name == "Glob"));
        assert!(matches!(&h[2], AgentEvent::ToolResult { ok: true, .. }));
        assert!(matches!(&h[3], AgentEvent::SubagentStats { tokens: Some(29039), .. }));
        assert!(matches!(&h[4], AgentEvent::SubagentEnd { ok: true, .. }));
        assert!(o.subagent_history(id, "nao-existe").is_empty());
        // A coluna do banco separa principal e subagente.
        let recs = o.db().unwrap().list_events(id as i64).unwrap();
        assert_eq!(recs.iter().filter(|r| r.subagent.as_deref() == Some(SUB)).count(), 5);
        assert_eq!(recs.iter().filter(|r| r.subagent.is_none()).count(), 6);
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
        let o = Orchestrator::new(vec![Arc::new(Fake(echo_args("ola")))], 2);
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
        let o = Orchestrator::new(vec![Arc::new(Fake(sleep_args()))], 1);
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
        let o = Orchestrator::new(vec![Arc::new(Fake(args))], 1);
        let id = o.submit(&proj, "fake", "x").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        // Repositório git sem preferência gravada: o padrão é a cópia isolada.
        assert_eq!(o.task(id).unwrap().mode, WorkMode::Isolated);
        let wt = o.task(id).unwrap().workdir.unwrap();
        assert_eq!(wt, worktree_path(&proj, id));
        let d = o.diff(id).await.unwrap();
        assert!(d.contains("b.txt"), "{d}");
        o.discard(id).await.unwrap();
        assert!(!wt.exists());
        assert_eq!(o.task(id).unwrap().state, TaskState::Discarded);
    }

    /// Excluir: cancela o turno, remove worktree e branch (isolado), não toca na
    /// pasta (direto), apaga do banco mantendo o consumo e não reaproveita o id.
    #[tokio::test]
    async fn delete_removes_task_worktree_and_keeps_usage() {
        if which::which("git").is_err() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let proj = root.path().join("proj");
        std::fs::create_dir(&proj).unwrap();
        git_repo(&proj).await;
        let dbp = root.path().join("h.db");
        let mk = |args: Vec<String>| Orchestrator::with_db(vec![Arc::new(Fake(args))], 2, Some(Db::open(&dbp).unwrap()));

        let o = mk(script_args(&["SESSION:s1", "USAGE"]));
        let mut rx = o.subscribe();
        // Isolado, com worktree e branch.
        let iso = o.submit(&proj, "fake", "isolado").unwrap();
        assert_eq!(wait_final(&o, iso).await, TaskState::Completed);
        let wt = o.task(iso).unwrap().workdir.unwrap();
        assert!(wt.exists());
        std::fs::write(wt.join("nao-aplicado.txt"), "x").unwrap();
        // Direto: a pasta do projeto fica como está.
        let direct = o
            .submit_with(&proj, "fake", "direto", SubmitOptions { model: None, mode: Some(WorkMode::Direct), ..Default::default() })
            .unwrap();
        assert_eq!(wait_final(&o, direct).await, TaskState::Completed);
        std::fs::write(proj.join("local.txt"), "meu").unwrap();

        o.delete(iso).await.unwrap();
        assert!(o.task(iso).is_none());
        assert!(!wt.exists(), "o worktree sai");
        let branches = git(&proj, &["branch", "--list", &branch_name(iso)]).await.unwrap();
        assert!(branches.trim().is_empty(), "a branch sai: {branches}");
        o.delete(direct).await.unwrap();
        assert!(proj.join("local.txt").exists() && proj.join("a.txt").exists(), "modo direto não toca na pasta");
        assert!(matches!(o.delete(direct).await, Err(OrchestratorError::UnknownTask(_))));
        let mut removed = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            if let OrchestratorEvent::Removed { task } = ev {
                removed.push(task);
            }
        }
        assert_eq!(removed, vec![iso, direct]);
        let db = o.db().unwrap();
        assert!(db.list_tasks().unwrap().is_empty());
        assert!(db.list_events(iso as i64).unwrap().is_empty());
        assert_eq!(db.usage_rows(crate::db::TimeRange::all()).unwrap().len(), 2, "o consumo continua nas métricas");
        drop(o);

        // Recarregada do disco: nada volta e o id não é reaproveitado.
        let o = mk(sleep_args());
        assert!(o.tasks().is_empty());
        // Turno rodando: é cancelado antes da exclusão.
        let busy = o
            .submit_with(&proj, "fake", "longo", SubmitOptions { model: None, mode: Some(WorkMode::Direct), ..Default::default() })
            .unwrap();
        assert!(busy > direct);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(o.task(busy).unwrap().state.is_active());
        o.delete(busy).await.unwrap();
        assert!(o.task(busy).is_none());
        assert!(o.db().unwrap().get_task(busy as i64).unwrap().is_none());
    }

    /// "Excluir encerradas" só leva as aplicadas/descartadas do projeto.
    #[tokio::test]
    async fn delete_closed_only_takes_closed_of_the_project() {
        if which::which("git").is_err() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let proj = root.path().join("proj");
        std::fs::create_dir(&proj).unwrap();
        git_repo(&proj).await;
        let o = Orchestrator::new(vec![Arc::new(Fake(echo_args("ok")))], 2);
        let a = o.submit(&proj, "fake", "a").unwrap();
        let b = o.submit(&proj, "fake", "b").unwrap();
        let other = o.submit(root.path(), "fake", "c").unwrap();
        for id in [a, b, other] {
            wait_final(&o, id).await;
        }
        o.discard(a).await.unwrap();
        let gone = o.delete_closed(&proj).await.unwrap();
        assert_eq!(gone, vec![a]);
        assert!(o.task(a).is_none() && o.task(b).is_some() && o.task(other).is_some());
        assert!(o.delete_closed(&proj).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn prompt_goes_through_stdin() {
        let dir = tempfile::tempdir().unwrap();
        let o = Orchestrator::new(vec![Arc::new(Fake(stdin_echo_args()))], 1);
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
            Orchestrator::with_db(vec![Arc::new(Fake(script_args(&["SESSION:abc", "ola", "USAGE"])))], 1, db)
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
        // A resposta veio sem consumo (como um comando de barra que a própria CLI
        // resolve): não conta como turno nem grava linha de uso.
        assert_eq!(o.task(id).unwrap().turns, 1);
        assert_eq!(o.db().unwrap().usage_rows(crate::db::TimeRange::all()).unwrap().len(), 1);
        // Com consumo, conta: o provedor falso ecoa "USAGE", que vira um evento de uso.
        o.reply(id, "USAGE").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        assert_eq!(o.task(id).unwrap().turns, 2);
        let rows = o.db().unwrap().usage_rows(crate::db::TimeRange::all()).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.turn.is_some()));
        assert_ne!(rows[0].turn, rows[1].turn);
        assert_eq!(crate::metrics::by_project(&rows, &crate::metrics::Pricing::default())[0].turns, 2);

        let h = o.history(id);
        assert_eq!(h.first(), Some(&AgentEvent::User("primeiro".into())));
        assert!(h.contains(&AgentEvent::Session("abc".into())));
        assert!(h.contains(&AgentEvent::User("segunda mensagem".into())));
        // Segundo turno ecoa o prompt (stdin) no mesmo diretório.
        assert!(h.contains(&AgentEvent::Text("segunda mensagem".into())), "{h:?}");
        let usage = o.db().unwrap().usage_by_project(crate::db::TimeRange::all()).unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].input_tokens, 20);
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
        let o = Orchestrator::new(vec![Arc::new(Fake(echo_args("ola")))], 1);
        let id = o.submit(dir.path(), "fake", "x").unwrap();
        wait_final(&o, id).await;
        assert!(matches!(o.reply(id, "y"), Err(OrchestratorError::NoSession)));
    }

    /// Provedor que registra o comando de cada turno (modelo, aviso de ambiente,
    /// sessão, pasta e o texto recebido) e responde com uma sessão própria.
    struct Spy {
        id: &'static str,
        default_model: Option<&'static str>,
        seen: Arc<Mutex<Vec<Seen>>>,
    }
    #[derive(Debug, Clone)]
    struct Seen {
        provider: &'static str,
        prompt: String,
        cwd: PathBuf,
        session: Option<String>,
        opts: TurnOptions,
    }
    impl AgentProvider for Spy {
        fn id(&self) -> &str {
            self.id
        }
        fn display_name(&self) -> &str {
            if self.id == "um" { "Agente Um" } else { "Agente Dois" }
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
        fn default_model(&self) -> Option<String> {
            self.default_model.map(str::to_string)
        }
        fn task_command(&self, p: &str, cwd: &Path, session: Option<&str>, o: &TurnOptions) -> CommandSpec {
            self.seen.lock().unwrap().push(Seen {
                provider: self.id,
                prompt: p.to_string(),
                cwd: cwd.to_path_buf(),
                session: session.map(str::to_string),
                opts: o.clone(),
            });
            let mut c = CommandSpec::new(if cfg!(windows) { "cmd" } else { "sh" }).cwd(cwd);
            c.args = script_args(&[&format!("SESSION:ses-{}", self.id), &format!("resposta do {}", self.id), "USAGE"]);
            c
        }
        fn parse_line(&self, l: &str) -> Vec<AgentEvent> {
            let l = l.trim();
            if let Some(s) = l.strip_prefix("SESSION:") {
                return vec![AgentEvent::Session(s.to_string())];
            }
            if l == "USAGE" {
                // Sem modelo, como o opencode: o orquestrador preenche com o da tarefa.
                return vec![AgentEvent::Usage(crate::agents::provider::TokenUsage { input_tokens: 10, ..Default::default() })];
            }
            if l.is_empty() { vec![] } else { vec![AgentEvent::Text(l.to_string())] }
        }
    }

    fn spies() -> (Vec<Arc<dyn AgentProvider>>, Arc<Mutex<Vec<Seen>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let a: Arc<dyn AgentProvider> = Arc::new(Spy { id: "um", default_model: None, seen: seen.clone() });
        let b: Arc<dyn AgentProvider> = Arc::new(Spy { id: "dois", default_model: Some("x/padrao"), seen: seen.clone() });
        (vec![a, b], seen)
    }

    async fn git_repo(dir: &Path) {
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        for a in [
            vec!["init", "-q"],
            vec!["-c", "user.email=t@t", "-c", "user.name=t", "add", "."],
            vec!["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "init"],
        ] {
            git(dir, &a).await.unwrap();
        }
    }

    /// Rejeitar por arquivo e por trecho, nos dois modos (isolado usa `add -N`).
    #[tokio::test]
    async fn revert_file_and_hunk() {
        for isolated in [false, true] {
            let d = tempfile::tempdir().unwrap();
            let dir = d.path();
            git_repo(dir).await;
            let base: String = (1..=30).map(|i| format!("l{i}\n")).collect();
            std::fs::write(dir.join("b.txt"), &base).unwrap();
            git(dir, &["-c", "user.email=t@t", "-c", "user.name=t", "add", "."]).await.unwrap();
            git(dir, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "b"]).await.unwrap();
            // Dois trechos longe um do outro em b.txt, a.txt alterado e um arquivo novo.
            let changed = base.replace("l2\n", "l2 mudou\n").replace("l28\n", "l28 mudou\n");
            std::fs::write(dir.join("b.txt"), &changed).unwrap();
            std::fs::write(dir.join("a.txt"), "outro\n").unwrap();
            std::fs::write(dir.join("novo.txt"), "n\n").unwrap();
            if isolated {
                git(dir, &["add", "-A", "-N"]).await.unwrap();
            }
            let diff = git(dir, &["diff", "HEAD", "--", "b.txt"]).await.unwrap();
            let hunks: Vec<&str> = diff.lines().filter(|l| l.starts_with("@@")).collect();
            assert_eq!(hunks.len(), 2, "{diff}");
            // Só o segundo trecho volta; o primeiro continua.
            revert_hunk_in(dir, "b.txt", hunks[1], isolated).await.unwrap();
            let now = std::fs::read_to_string(dir.join("b.txt")).unwrap();
            assert!(now.contains("l2 mudou") && !now.contains("l28 mudou"), "{now}");
            assert!(matches!(revert_hunk_in(dir, "b.txt", "@@ -90,1 +90,1 @@", isolated).await, Err(OrchestratorError::Git(_))));
            revert_file_in(dir, "a.txt", isolated).await.unwrap();
            assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap().replace('\r', ""), "a\n");
            revert_file_in(dir, "novo.txt", isolated).await.unwrap();
            assert!(!dir.join("novo.txt").exists());
            // `diff` compara o conteúdo (o `status` pode acusar só a data do arquivo).
            let st = git(dir, &["diff", "HEAD", "--name-only"]).await.unwrap();
            assert_eq!(st.trim(), "b.txt", "isolado={isolated}");
            assert!(git(dir, &["ls-files", "--others", "--exclude-standard"]).await.unwrap().trim().is_empty());
            for bad in ["../x", "/etc/passwd", "C:/x", "-x", "a//b", ""] {
                assert!(revert_file_in(dir, bad, isolated).await.is_err(), "{bad}");
            }
        }
    }

    #[test]
    fn hunk_patch_keeps_header_and_one_hunk() {
        let d = "diff --git a/f b/f\r\nindex 1..2 100644\n--- a/f\n+++ b/f\n@@ -1,2 +1,2 @@ fn x\n-a\n+b\n c\n@@ -9,1 +9,1 @@\n-y\n+z\n";
        let p = hunk_patch(d, "@@ -9,1 +9,1 @@").unwrap();
        assert_eq!(p, "diff --git a/f b/f\r\nindex 1..2 100644\n--- a/f\n+++ b/f\n@@ -9,1 +9,1 @@\n-y\n+z\n");
        let p = hunk_patch(d, "@@ -1,2 +1,2 @@ fn x").unwrap();
        assert!(p.ends_with("@@ -1,2 +1,2 @@ fn x\n-a\n+b\n c\n"));
        assert_eq!(hunk_patch(d, "@@ -5,1 +5,1 @@"), None);
    }

    fn plain_other(root: &tempfile::TempDir) -> PathBuf {
        root.path().join("outro-projeto")
    }

    /// O modelo é da tarefa: vai em todo turno, pode ser trocado entre turnos, fica
    /// no banco e marca as linhas de uso de quem não informa o modelo.
    #[tokio::test]
    async fn model_is_per_task_and_changes_between_turns() {
        let dir = tempfile::tempdir().unwrap();
        let dbp = dir.path().join("h.db");
        let (provs, seen) = spies();
        let o = Orchestrator::with_db(provs, 1, Some(Db::open(&dbp).unwrap()));
        assert!(matches!(
            o.submit_with(dir.path(), "um", "x", SubmitOptions { model: Some("--perigo".into()), mode: None, ..Default::default() }),
            Err(OrchestratorError::InvalidModel(_))
        ));
        let id = o.submit_with(dir.path(), "um", "primeiro", SubmitOptions { model: Some("rapido".into()), mode: None, ..Default::default() }).unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        assert!(matches!(o.set_model(id, Some("a b")), Err(OrchestratorError::InvalidModel(_))));
        o.set_model(id, Some("forte[1m]")).unwrap();
        o.reply(id, "segundo").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        o.set_model(id, None).unwrap();
        o.reply(id, "terceiro").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        let models: Vec<Option<String>> = seen.lock().unwrap().iter().map(|s| s.opts.model.clone()).collect();
        assert_eq!(models, [Some("rapido".to_string()), Some("forte[1m]".to_string()), None]);
        assert_eq!(seen.lock().unwrap()[1].session.as_deref(), Some("ses-um"), "o modelo novo acompanha a retomada da sessão");
        let rows = o.db().unwrap().usage_rows(crate::db::TimeRange::all()).unwrap();
        let used: Vec<Option<&str>> = rows.iter().map(|r| r.usage.model.as_deref()).collect();
        assert_eq!(used, [Some("rapido"), Some("forte[1m]"), None]);
        // `submit` simples usa o modelo inicial do provedor.
        let id2 = o.submit(dir.path(), "dois", "outro").unwrap();
        assert_eq!(o.task(id2).unwrap().model.as_deref(), Some("x/padrao"));
        wait_final(&o, id2).await;
        o.set_model(id, Some("gravado")).unwrap();
        drop(o);
        let (provs, _) = spies();
        let o = Orchestrator::with_db(provs, 1, Some(Db::open(&dbp).unwrap()));
        assert_eq!(o.task(id).unwrap().model.as_deref(), Some("gravado"));
        assert_eq!(o.task(id2).unwrap().model.as_deref(), Some("x/padrao"));
    }

    /// Modo de acesso por conversa: começa normal, vai no turno seguinte, fica no
    /// banco e não vaza para outras tarefas.
    #[tokio::test]
    async fn access_mode_is_per_task_and_persisted() {
        use crate::permissions::AccessMode;
        let dir = tempfile::tempdir().unwrap();
        let dbp = dir.path().join("h.db");
        let (provs, seen) = spies();
        let o = Orchestrator::with_db(provs, 1, Some(Db::open(&dbp).unwrap()));
        let id = o.submit(dir.path(), "um", "primeiro").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        o.set_access(id, AccessMode::Bypass).unwrap();
        o.reply(id, "segundo").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        let id2 = o.submit(dir.path(), "um", "outra").unwrap();
        assert_eq!(wait_final(&o, id2).await, TaskState::Completed);
        let acc: Vec<AccessMode> = seen.lock().unwrap().iter().map(|s| s.opts.access).collect();
        assert_eq!(acc, [AccessMode::Normal, AccessMode::Bypass, AccessMode::Normal]);
        drop(o);
        let (provs, _) = spies();
        let o = Orchestrator::with_db(provs, 1, Some(Db::open(&dbp).unwrap()));
        assert_eq!(o.task(id).unwrap().access, AccessMode::Bypass);
        assert_eq!(o.task(id2).unwrap().access, AccessMode::Normal);
    }

    /// Troca de provedor: mesma tarefa e mesma pasta, sessão antiga guardada, sessão
    /// nova no provedor novo recebendo o resumo, divisória no histórico, uso por provedor.
    #[tokio::test]
    async fn provider_switch_hands_over_the_conversation() {
        let dir = tempfile::tempdir().unwrap();
        let dbp = dir.path().join("h.db");
        let (provs, seen) = spies();
        let o = Orchestrator::with_db(provs, 1, Some(Db::open(&dbp).unwrap()));
        let id = o.submit_with(dir.path(), "um", "crie o módulo", SubmitOptions { model: Some("m-um".into()), mode: None, ..Default::default() }).unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        let workdir = o.task(id).unwrap().workdir.unwrap();
        assert!(matches!(o.set_provider(id, "nenhum"), Err(OrchestratorError::UnknownProvider(_))));

        o.set_provider(id, "dois").unwrap();
        let t = o.task(id).unwrap();
        assert_eq!((t.provider.as_str(), t.session_id.clone(), t.handoff), ("dois", None, true));
        assert_eq!(t.model.as_deref(), Some("x/padrao"), "modelo inicial do provedor novo");
        assert_eq!(t.others["um"], ProviderState { session: Some("ses-um".into()), model: Some("m-um".into()) });
        assert_eq!(t.workdir.as_deref(), Some(workdir.as_path()));

        // Sem sessão no provedor novo, a resposta abre uma (não é mais "sem sessão").
        o.reply(id, "continue daqui").unwrap();
        assert!(matches!(o.set_provider(id, "um"), Err(OrchestratorError::Busy)), "troca só com o turno parado");
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        let turn = seen.lock().unwrap().last().unwrap().clone();
        assert_eq!((turn.provider, turn.session.clone(), turn.cwd.as_path()), ("dois", None, workdir.as_path()));
        assert!(turn.prompt.contains("Usuário: crie o módulo"), "{}", turn.prompt);
        assert!(turn.prompt.contains("Agente: resposta do um"));
        assert!(turn.prompt.contains("outro agente (Agente Um)"));
        assert!(turn.prompt.ends_with("[Fim do contexto. Pedido atual do usuário:]\n\ncontinue daqui"), "{}", turn.prompt);
        let t = o.task(id).unwrap();
        assert_eq!((t.session_id.as_deref(), t.handoff), (Some("ses-dois"), false));

        // O usuário não vê o bloco de contexto: o histórico tem a mensagem dele e a divisória.
        let h = o.history(id);
        assert!(h.contains(&AgentEvent::ProviderSwitch { from: "um".into(), to: "dois".into() }));
        assert!(h.contains(&AgentEvent::User("continue daqui".into())));
        assert!(!h.iter().any(|e| matches!(e, AgentEvent::User(u) if u.contains("Contexto montado"))));
        // Turno seguinte no mesmo provedor: sessão retomada, sem resumo.
        o.reply(id, "mais uma").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        let turn = seen.lock().unwrap().last().unwrap().clone();
        assert_eq!((turn.session.as_deref(), turn.prompt.as_str()), (Some("ses-dois"), "mais uma"));

        // Uso somado na mesma tarefa, separado por provedor e modelo.
        let rows = o.db().unwrap().usage_rows(crate::db::TimeRange::all()).unwrap();
        let by: Vec<(i64, &str, Option<&str>)> = rows.iter().map(|r| (r.task_id, r.provider.as_str(), r.usage.model.as_deref())).collect();
        let idi = id as i64;
        assert_eq!(by, [(idi, "um", Some("m-um")), (idi, "dois", Some("x/padrao")), (idi, "dois", Some("x/padrao"))]);
        drop(o);

        // Depois de reabrir, volta ao primeiro provedor: retoma a sessão guardada dele
        // e só recebe o que perdeu.
        let (provs, seen) = spies();
        let o = Orchestrator::with_db(provs, 1, Some(Db::open(&dbp).unwrap()));
        assert_eq!(o.task(id).unwrap().others["um"].session.as_deref(), Some("ses-um"));
        o.set_provider(id, "um").unwrap();
        let t = o.task(id).unwrap();
        assert_eq!((t.session_id.as_deref(), t.model.as_deref(), t.handoff), (Some("ses-um"), Some("m-um"), true));
        assert_eq!(t.others["dois"].session.as_deref(), Some("ses-dois"));
        o.reply(id, "voltei").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        let turn = seen.lock().unwrap().last().unwrap().clone();
        assert_eq!((turn.provider, turn.session.as_deref()), ("um", Some("ses-um")));
        assert!(turn.prompt.contains("Enquanto você esteve fora") && turn.prompt.contains("Usuário: continue daqui"));
        assert!(!turn.prompt.contains("crie o módulo"), "o que ele já viu não volta: {}", turn.prompt);
        assert!(!o.task(id).unwrap().handoff);
        // Tarefa encerrada não troca de provedor.
        let o2 = Orchestrator::new(spies().0, 1);
        assert!(matches!(o2.set_provider(99, "um"), Err(OrchestratorError::UnknownTask(99))));
    }

    /// A memória entra entre o aviso de isolamento e o de perguntas, sem aspas nem
    /// quebras, e o texto é o mesmo a cada turno.
    #[test]
    fn note_includes_memory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let proj = root.join("proj");
        crate::memory::ensure(root).unwrap();
        let mem = crate::memory::note(root, &proj).unwrap();
        let iso = compose_note(WorkMode::Isolated, &proj, Some(&mem));
        assert!(iso.starts_with(&isolation_note(&proj)) && iso.ends_with(SHELL_NOTE) && iso.contains(&mem), "{iso}");
        let direct = compose_note(WorkMode::Direct, &proj, Some(&mem));
        assert_eq!(direct, format!("{mem} {ASK_NOTE} {SHELL_NOTE}"));
        assert!(direct.contains("Preferencias.md") && direct.contains("Memoria.md"));
        assert!(!iso.contains('\n') && !iso.contains('"'));
        assert_eq!(compose_note(WorkMode::Isolated, &proj, Some(&mem)), iso, "estável entre turnos");
        assert_eq!(compose_note(WorkMode::Direct, &proj, None), format!("{ASK_NOTE} {SHELL_NOTE}"));
    }

    /// O modo de trabalho é da tarefa: no mesmo orquestrador, uma roda no worktree
    /// (e é avisada disso) e outra direto na pasta (sem aplicar nem descartar).
    #[tokio::test]
    async fn work_mode_is_per_task() {
        if which::which("git").is_err() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let proj = root.path().join("proj");
        std::fs::create_dir(&proj).unwrap();
        git_repo(&proj).await;
        std::fs::create_dir(proj.join("comite")).unwrap();
        std::fs::write(proj.join("comite").join("ata.md"), "fora do git\n").unwrap();
        let dbp = root.path().join("h.db");
        let (provs, seen) = spies();
        let o = Orchestrator::with_db(provs, 2, Some(Db::open(&dbp).unwrap()));

        let iso = o.submit_with(&proj, "um", "isolada", SubmitOptions { model: None, mode: Some(WorkMode::Isolated), ..Default::default() }).unwrap();
        assert_eq!(wait_final(&o, iso).await, TaskState::Completed);
        let direct = o.submit_with(&proj, "um", "direta", SubmitOptions { model: None, mode: Some(WorkMode::Direct), ..Default::default() }).unwrap();
        assert_eq!(wait_final(&o, direct).await, TaskState::Completed);

        let t = o.task(iso).unwrap();
        assert_eq!((t.mode, t.workdir.clone()), (WorkMode::Isolated, Some(worktree_path(&proj, iso))));
        assert!(!worktree_path(&proj, iso).join("comite").exists(), "o worktree só tem o que está versionado");
        let t = o.task(direct).unwrap();
        assert_eq!((t.mode, t.workdir.as_deref()), (WorkMode::Direct, Some(proj.as_path())));
        assert!(!worktree_path(&proj, direct).exists(), "modo direto não cria worktree");

        // O agente isolado é avisado de onde está; o direto, não. Os dois recebem
        // o aviso de perguntas com opções (bloco argus-ask).
        let seen = seen.lock().unwrap().clone();
        let note = seen[0].opts.system_note.clone().unwrap();
        assert!(note.contains("git worktree") && note.contains("proj") && note.contains(&proj.display().to_string()), "{note}");
        assert!(note.contains("argus-ask"));
        // Vai como argumento (no Windows, pelo cmd): sem quebra de linha nem aspas.
        assert!(!note.contains('\n') && !note.contains('"'));
        let direct_note = seen[1].opts.system_note.clone().unwrap();
        assert!(!direct_note.contains("git worktree") && direct_note.contains("argus-ask"));
        assert!(!direct_note.contains('\n') && !direct_note.contains('"'));
        assert_eq!(seen[1].cwd, proj);

        // Direto: o diff é o da pasta em relação ao último commit, com os arquivos novos,
        // e não há o que aplicar nem descartar.
        std::fs::write(proj.join("a.txt"), "a\nmudou\n").unwrap();
        let d = o.diff(direct).await.unwrap();
        assert!(d.contains("a.txt") && d.contains("+mudou"), "{d}");
        assert!(d.contains("comite/ata.md") && d.contains("+fora do git"), "{d}");
        assert_eq!(git(&proj, &["diff", "--cached", "--name-only"]).await.unwrap().trim(), "", "o índice do usuário não é tocado");
        assert!(matches!(o.apply(direct).await, Err(OrchestratorError::NoWorktree)));
        assert!(matches!(o.discard(direct).await, Err(OrchestratorError::NoWorktree)));
        assert_eq!(o.task(direct).unwrap().state, TaskState::Completed);
        // Resposta no modo direto continua na pasta do projeto.
        o.reply(direct, "de novo").unwrap();
        assert_eq!(wait_final(&o, direct).await, TaskState::Completed);

        // Níveis de git: o padrão segue o modo; a preferência do projeto vale sobre a geral.
        use crate::permissions::GitLevels;
        assert_eq!(o.git_levels(&proj, WorkMode::Isolated), GitLevels { local: true, remote: false, build: true });
        assert_eq!(o.git_levels(&proj, WorkMode::Direct), GitLevels { local: false, remote: false, build: true });
        {
            use crate::permissions::PermissionMode as P;
            assert_eq!(o.permission_mode(&proj), P::Auto);
            o.set_permission_mode_setting(None, Some(P::Restricted)).unwrap();
            assert_eq!(o.permission_mode(&proj), P::Restricted);
            o.set_permission_mode_setting(Some(&proj), Some(P::Auto)).unwrap();
            assert_eq!(o.permission_mode(&proj), P::Auto);
            assert_eq!(o.permission_mode(&plain_other(&root)), P::Restricted);
            o.set_permission_mode_setting(None, None).unwrap();
            o.set_permission_mode_setting(Some(&proj), None).unwrap();
        }
        o.set_git_setting(None, "remote", Some(true)).unwrap();
        o.set_git_setting(Some(&proj), "local", Some(false)).unwrap();
        assert_eq!(o.git_levels(&proj, WorkMode::Isolated), GitLevels { local: false, remote: true, build: true });
        o.set_git_setting(Some(&proj), "remote", Some(false)).unwrap();
        assert_eq!(o.git_levels(&proj, WorkMode::Direct), GitLevels { local: false, remote: false, build: true });
        assert_eq!(o.git_levels(&plain_other(&root), WorkMode::Direct), GitLevels { local: false, remote: true, build: true });
        for level in ["local", "remote"] {
            o.set_git_setting(None, level, None).unwrap();
            o.set_git_setting(Some(&proj), level, None).unwrap();
        }
        assert_eq!(o.git_levels(&proj, WorkMode::Isolated), GitLevels { local: true, remote: false, build: true });

        // Preferências: geral e por projeto; pasta sem git só aceita o modo direto.
        assert_eq!(o.default_mode(&proj), WorkMode::Isolated);
        o.set_work_mode_setting(None, Some(WorkMode::Direct)).unwrap();
        assert_eq!(o.default_mode(&proj), WorkMode::Direct);
        o.set_work_mode_setting(Some(&proj), Some(WorkMode::Isolated)).unwrap();
        assert_eq!(o.default_mode(&proj), WorkMode::Isolated);
        o.set_work_mode_setting(Some(&proj), None).unwrap();
        assert_eq!(o.default_mode(&proj), WorkMode::Direct);
        let id3 = o.submit(&proj, "um", "pelo padrão").unwrap();
        assert_eq!(o.task(id3).unwrap().mode, WorkMode::Direct);
        wait_final(&o, id3).await;
        let plain = root.path().join("sem-git");
        std::fs::create_dir(&plain).unwrap();
        let id4 = o.submit_with(&plain, "um", "x", SubmitOptions { model: None, mode: Some(WorkMode::Isolated), ..Default::default() }).unwrap();
        assert_eq!(o.task(id4).unwrap().mode, WorkMode::Direct);
        wait_final(&o, id4).await;
        assert!(o.diff(id4).await.is_err(), "sem git não há commit para comparar");

        o.discard(iso).await.unwrap();
        drop(o);
        // O modo fica gravado na tarefa.
        let o = Orchestrator::with_db(spies().0, 1, Some(Db::open(&dbp).unwrap()));
        assert_eq!(o.task(iso).unwrap().mode, WorkMode::Isolated);
        assert_eq!(o.task(direct).unwrap().mode, WorkMode::Direct);
    }

    /// O processo filho recebe `PWD` igual ao diretório de trabalho (nunca o herdado).
    #[test]
    fn child_gets_pwd_of_its_workdir() {
        let dir = tempfile::tempdir().unwrap();
        let pwd = |cmd: &tokio::process::Command| {
            cmd.as_std().get_envs().find(|(k, _)| *k == "PWD").and_then(|(_, v)| v.map(PathBuf::from))
        };
        let cmd = build_command(&CommandSpec::new("git"), Some(dir.path()));
        assert_eq!(pwd(&cmd).as_deref(), Some(dir.path()));
        assert_eq!(cmd.as_std().get_current_dir(), Some(dir.path()));
        // O diretório do próprio comando vale sobre o padrão.
        let own = dir.path().join("sub");
        let cmd = build_command(&CommandSpec::new("git").cwd(&own), Some(dir.path()));
        assert_eq!(pwd(&cmd), Some(own));
        // Sem diretório, nada é imposto.
        assert_eq!(pwd(&build_command(&CommandSpec::new("git"), None)), None);
    }

    #[test]
    fn state_db_roundtrip() {
        for s in [TaskState::Completed, TaskState::Failed("x y".into()), TaskState::Running] {
            assert_eq!(TaskState::from_db(&s.to_db()), s);
        }
        assert_eq!(TaskState::from_db("Completed"), TaskState::Completed);
    }

    /// Imprime um arquivo de linhas reais e, se `hang`, fica esperando (para o
    /// cancelamento). O parser e o acompanhante são os do provedor real.
    struct Lines(PathBuf, bool, Arc<dyn AgentProvider>);
    impl AgentProvider for Lines {
        fn id(&self) -> &str {
            "lines"
        }
        fn display_name(&self) -> &str {
            "Lines"
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
        fn task_command(&self, _p: &str, cwd: &Path, _s: Option<&str>, _o: &TurnOptions) -> CommandSpec {
            let f = self.0.to_string_lossy().into_owned();
            let c = if cfg!(windows) {
                let tail = if self.1 { " & ping -n 30 127.0.0.1 >NUL" } else { "" };
                CommandSpec::new("cmd").arg("/c").arg(format!("type {}{tail}", f.replace('/', "\\")))
            } else {
                let tail = if self.1 { "; sleep 30" } else { "" };
                CommandSpec::new("sh").arg("-c").arg(format!("cat '{f}'{tail}"))
            };
            c.cwd(cwd)
        }
        fn parse_line(&self, l: &str) -> Vec<AgentEvent> {
            self.2.parse_line(l)
        }
        fn turn_tracker(&self) -> Box<dyn crate::agents::provider::TurnTracker> {
            self.2.turn_tracker()
        }
    }

    #[tokio::test]
    async fn cancelled_claude_turn_records_partial_usage() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("parcial.jsonl");
        std::fs::write(
            &f,
            [
                r#"{"type":"system","subtype":"init","cwd":"x","session_id":"s-cancel","model":"claude-sonnet-4-5"}"#,
                r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-sonnet-4-5","content":[{"type":"text","text":"Vou ler"}],"usage":{"input_tokens":3,"cache_creation_input_tokens":2000,"cache_read_input_tokens":15000,"output_tokens":8}},"parent_tool_use_id":null,"session_id":"s-cancel"}"#,
                r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-sonnet-4-5","content":[{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"a.txt"}}],"usage":{"input_tokens":3,"cache_creation_input_tokens":2000,"cache_read_input_tokens":15000,"output_tokens":61}},"parent_tool_use_id":null,"session_id":"s-cancel"}"#,
            ]
            .join("\n")
                + "\n",
        )
        .unwrap();
        let dbp = dir.path().join("h.db");
        let prov: Arc<dyn AgentProvider> = Arc::new(Lines(f, true, Arc::new(crate::agents::claude::ClaudeProvider::new())));
        let o = Orchestrator::with_db(vec![prov], 1, Some(Db::open(&dbp).unwrap()));
        let id = o.submit(dir.path(), "lines", "leia a.txt").unwrap();
        // Espera as linhas chegarem antes de cancelar.
        for _ in 0..200 {
            if o.history(id).iter().any(|e| matches!(e, AgentEvent::ToolUse { .. })) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        o.cancel(id).unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Cancelled);
        let rows = o.db().unwrap().usage_rows(crate::db::TimeRange::all()).unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        let u = &rows[0].usage;
        assert_eq!((u.input_tokens, u.output_tokens, u.cache_read, u.cache_write), (3, 61, 15000, 2000));
        assert_eq!(u.model.as_deref(), Some("claude-sonnet-4-5"));
        assert!(o.history(id).iter().any(|e| matches!(e, AgentEvent::Usage(_))));
        assert!(!o.history(id).iter().any(|e| matches!(e, AgentEvent::Result { .. })));
    }

    #[tokio::test]
    async fn opencode_without_last_step_finish_gets_result() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("oc.jsonl");
        std::fs::write(
            &f,
            [
                r#"{"type":"step_start","timestamp":1,"sessionID":"ses_abc","part":{"id":"prt_1","sessionID":"ses_abc","messageID":"msg_1","type":"step-start"}}"#,
                r#"{"type":"tool_use","timestamp":2,"sessionID":"ses_abc","part":{"type":"tool","id":"call_3","tool":"read","state":{"status":"completed","input":{"filePath":"a.txt"},"output":"a"}}}"#,
                r#"{"type":"step_finish","timestamp":3,"sessionID":"ses_abc","part":{"type":"step-finish","reason":"tool-calls","cost":0,"tokens":{"input":10959,"output":36,"reasoning":114,"cache":{"read":0,"write":0}}}}"#,
                r#"{"type":"step_start","timestamp":4,"sessionID":"ses_abc","part":{"id":"prt_2","sessionID":"ses_abc","messageID":"msg_2","type":"step-start"}}"#,
                r#"{"type":"text","timestamp":5,"sessionID":"ses_abc","part":{"type":"text","text":"pronto","time":{"start":1,"end":2}}}"#,
            ]
            .join("\n")
                + "\n",
        )
        .unwrap();
        let dbp = dir.path().join("h.db");
        let prov: Arc<dyn AgentProvider> = Arc::new(Lines(f, false, Arc::new(crate::agents::opencode::OpencodeProvider::new())));
        let o = Orchestrator::with_db(vec![prov], 1, Some(Db::open(&dbp).unwrap()));
        let id = o.submit(dir.path(), "lines", "leia").unwrap();
        assert_eq!(wait_final(&o, id).await, TaskState::Completed);
        let h = o.history(id);
        assert!(matches!(h.last(), Some(AgentEvent::Result { ok: true, cost: None, .. })), "{h:?}");
        let rows = o.db().unwrap().usage_rows(crate::db::TimeRange::all()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].usage.input_tokens, 10959);
    }
}
