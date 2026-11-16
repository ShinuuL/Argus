//! Modo AUTO-WORK: ao fim de cada turno de uma tarefa num projeto com o modo
//! ligado, varre pendências (PENDENCIAS.md, TODO/FIXME no código, falhas de
//! build/testes), grava os achados como `- [ ]` no PENDENCIAS.md e dispara um
//! novo turno na mesma sessão para resolver o próximo item. Para quando tudo
//! está `- [x]` e a verificação final passa, ou ao atingir uma trava
//! (iterações, tokens, custo, pausa/parada do usuário).

use crate::agents::orchestrator::{build_command, Orchestrator, OrchestratorEvent, TaskId, TaskState};
use crate::agents::provider::CommandSpec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const FILE_NAME: &str = "PENDENCIAS.md";
/// Tentativas sem concluir antes de um item virar `- [!] bloqueado`.
pub const MAX_ITEM_FAILURES: u32 = 3;
pub const STOP_BY_USER: &str = "parado pelo usuário";

const HEADER: &str = "# Pendências\n\n\
Checklist mantido pelo modo auto-work. `- [ ]` aberto, `- [x]` concluído, `- [!]` bloqueado.\n\
Itens podem ser editados à vontade; o texto fora da lista é preservado.\n\n";

// ---------------------------------------------------------------- PENDENCIAS.md

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemStatus {
    Open,
    Done,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Índice da linha no documento.
    pub line: usize,
    pub status: ItemStatus,
    pub text: String,
}

/// Documento de pendências. Guarda as linhas originais: só a marcação dos
/// itens alterados é reescrita, o resto do texto do usuário fica intacto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDoc {
    lines: Vec<String>,
    crlf: bool,
    trailing_newline: bool,
}

/// Analisa uma linha de checklist: (offset do caractere de status, status, texto).
fn parse_item_line(l: &str) -> Option<(usize, ItemStatus, &str)> {
    let indent = l.len() - l.trim_start().len();
    let rest = &l[indent..];
    let mut ch = rest.chars();
    if !matches!(ch.next(), Some('-' | '*' | '+')) || ch.next() != Some(' ') {
        return None;
    }
    let after = rest[2..].trim_start();
    let off = l.len() - after.len();
    let b = after.as_bytes();
    if b.len() < 3 || b[0] != b'[' || b[2] != b']' {
        return None;
    }
    let status = match b[1] {
        b' ' => ItemStatus::Open,
        b'x' | b'X' => ItemStatus::Done,
        b'!' => ItemStatus::Blocked,
        _ => return None,
    };
    let tail = &after[3..];
    if !(tail.is_empty() || tail.starts_with(' ') || tail.starts_with('\t')) {
        return None;
    }
    Some((off + 1, status, tail.trim()))
}

impl PendingDoc {
    pub fn parse(s: &str) -> Self {
        let crlf = s.contains("\r\n");
        let trailing_newline = s.is_empty() || s.ends_with('\n');
        let body = s.strip_suffix('\n').unwrap_or(s);
        let lines = if s.is_empty() {
            Vec::new()
        } else {
            body.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect()
        };
        Self { lines, crlf, trailing_newline }
    }

    pub fn render(&self) -> String {
        let nl = if self.crlf { "\r\n" } else { "\n" };
        let mut s = self.lines.join(nl);
        if self.trailing_newline && !self.lines.is_empty() {
            s.push_str(nl);
        }
        s
    }

    /// Itens de checklist, ignorando blocos de código cercados por ```.
    pub fn items(&self) -> Vec<Item> {
        let mut out = Vec::new();
        let mut fenced = false;
        for (i, l) in self.lines.iter().enumerate() {
            let t = l.trim_start();
            if t.starts_with("```") || t.starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            if let Some((_, status, text)) = parse_item_line(l) {
                if !text.is_empty() {
                    out.push(Item { line: i, status, text: text.to_string() });
                }
            }
        }
        out
    }

    pub fn next_open(&self) -> Option<Item> {
        self.items().into_iter().find(|i| i.status == ItemStatus::Open)
    }

    /// (concluídos, bloqueados, total).
    pub fn counts(&self) -> (usize, usize, usize) {
        let it = self.items();
        let c = |s| it.iter().filter(|i| i.status == s).count();
        (c(ItemStatus::Done), c(ItemStatus::Blocked), it.len())
    }

    pub fn find(&self, text: &str) -> Option<Item> {
        self.items().into_iter().find(|i| i.text == text)
    }

    /// Troca a marcação do item na linha `line`. Ao bloquear, prefixa o texto
    /// com `bloqueado: <motivo> —`.
    pub fn set_status(&mut self, line: usize, status: ItemStatus, reason: Option<&str>) -> bool {
        let Some(l) = self.lines.get(line) else { return false };
        let Some((off, _, text)) = parse_item_line(l) else { return false };
        let c = match status {
            ItemStatus::Open => ' ',
            ItemStatus::Done => 'x',
            ItemStatus::Blocked => '!',
        };
        let mut new = format!("{}{c}]", &l[..off]);
        let text = text.to_string();
        new.push(' ');
        if status == ItemStatus::Blocked && !text.starts_with("bloqueado:") {
            new.push_str(&format!("bloqueado: {} — ", reason.unwrap_or("sem motivo informado")));
        }
        new.push_str(&text);
        self.lines[line] = new;
        true
    }

    /// Acrescenta `- [ ] text` logo após o último item (ou no fim do arquivo).
    pub fn append(&mut self, text: &str) {
        let text = text.replace(['\r', '\n'], " ");
        let line = format!("- [ ] {}", text.trim());
        match self.items().last() {
            Some(last) => self.lines.insert(last.line + 1, line),
            None => {
                while self.lines.last().is_some_and(|l| l.trim().is_empty()) {
                    self.lines.pop();
                }
                if !self.lines.is_empty() {
                    self.lines.push(String::new());
                }
                self.lines.push(line);
            }
        }
        self.trailing_newline = true;
    }

    /// Algum item (de qualquer estado em `states`) satisfaz `pred`.
    pub fn any(&self, states: &[ItemStatus], pred: impl Fn(&str) -> bool) -> bool {
        self.items().iter().any(|i| states.contains(&i.status) && pred(&i.text))
    }

    /// Lê `<dir>/PENDENCIAS.md`, criando-o (com cabeçalho) se não existir.
    pub fn load_or_create(dir: &Path) -> std::io::Result<Self> {
        let p = dir.join(FILE_NAME);
        match std::fs::read_to_string(&p) {
            Ok(s) => Ok(Self::parse(&s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::write(&p, HEADER)?;
                Ok(Self::parse(HEADER))
            }
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::write(dir.join(FILE_NAME), self.render())
    }
}

// ---------------------------------------------------------------- TODO/FIXME

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Caminho relativo à raiz, com `/`.
    pub file: String,
    pub line: usize,
    pub kind: &'static str,
    pub text: String,
}

impl Finding {
    pub fn item_text(&self) -> String {
        if self.text.is_empty() {
            format!("{} em {}:{}", self.kind, self.file, self.line)
        } else {
            format!("{} em {}:{} — {}", self.kind, self.file, self.line, self.text)
        }
    }
    /// Já listado (em qualquer estado) no documento?
    fn listed_in(&self, doc: &PendingDoc) -> bool {
        let file = format!("{}:", self.file);
        let line = format!("{}:{}", self.file, self.line);
        doc.any(&[ItemStatus::Open, ItemStatus::Done, ItemStatus::Blocked], |t| {
            t.contains(&file) && if self.text.is_empty() { t.contains(&line) } else { t.contains(&self.text) }
        })
    }
}

const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git", ".argus-worktrees"];
const COMMENT_MARKS: &[&str] = &["//", "#", "/*", "<!--", "--", ";", "*", "'", "REM "];

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Procura `TODO`/`FIXME` (palavra inteira, depois de um marcador de comentário).
pub fn find_marker(line: &str) -> Option<(&'static str, String)> {
    for kind in ["TODO", "FIXME"] {
        let mut from = 0;
        while let Some(rel) = line[from..].find(kind) {
            let at = from + rel;
            from = at + kind.len();
            if is_word(line[..at].chars().next_back()) || is_word(line[from..].chars().next()) {
                continue;
            }
            let before = &line[..at];
            if !COMMENT_MARKS.iter().any(|m| before.contains(m)) {
                continue;
            }
            let mut rest = line[from..].trim_start();
            if rest.starts_with('(') {
                rest = rest.find(')').map(|i| &rest[i + 1..]).unwrap_or(rest);
            }
            let rest = rest.trim_start_matches([':', '-', ' ', '\t']);
            let rest = rest.trim_end_matches("*/").trim_end_matches("-->").trim();
            let text: String = rest.chars().take(120).collect();
            return Some((kind, text));
        }
    }
    None
}

/// Varre TODO/FIXME em arquivos de texto, ignorando target/node_modules/.git.
pub fn scan_todos(root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(ft) = e.file_type() else { continue };
            let path = e.path();
            if ft.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push(path);
                }
                continue;
            }
            if !ft.is_file() || name == FILE_NAME || e.metadata().map(|m| m.len() > 1_000_000).unwrap_or(true) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            if bytes.contains(&0) {
                continue; // binário
            }
            let Ok(s) = String::from_utf8(bytes) else { continue };
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            for (i, l) in s.lines().enumerate() {
                if let Some((kind, text)) = find_marker(l) {
                    out.push(Finding { file: rel.clone(), line: i + 1, kind, text });
                }
            }
        }
    }
    out.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    out
}

// ---------------------------------------------------------------- build/testes

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckCmd {
    pub label: String,
    pub spec: CommandSpec,
}

fn check(label: &str, program: &str, args: &[&str]) -> CheckCmd {
    let mut spec = CommandSpec::new(program);
    spec.args = args.iter().map(|a| a.to_string()).collect();
    CheckCmd { label: label.into(), spec }
}

/// Comandos de verificação conforme o tipo de projeto.
pub fn detect_checks(dir: &Path) -> Vec<CheckCmd> {
    let mut v = Vec::new();
    if dir.join("Cargo.toml").is_file() {
        v.push(check("cargo check", "cargo", &["check", "--all-targets", "--quiet"]));
        v.push(check("cargo test", "cargo", &["test", "--quiet"]));
    }
    if let Ok(s) = std::fs::read_to_string(dir.join("package.json")) {
        let scripts = serde_json::from_str::<serde_json::Value>(&s).ok().and_then(|j| j.get("scripts").cloned());
        let has = |k: &str| {
            scripts.as_ref().and_then(|s| s.get(k)).and_then(|v| v.as_str()).is_some_and(|c| !c.contains("no test specified"))
        };
        if has("build") {
            v.push(check("npm run build", "npm", &["run", "build"]));
        }
        if has("test") {
            v.push(check("npm test", "npm", &["test"]));
        }
    }
    if dir.join("go.mod").is_file() {
        v.push(check("go build", "go", &["build", "./..."]));
        v.push(check("go test", "go", &["test", "./..."]));
    }
    v
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckFailure {
    pub label: String,
    pub detail: String,
}

impl CheckFailure {
    pub fn item_text(&self) -> String {
        format!("Corrigir falha em `{}`: {}", self.label, self.detail)
    }
}

fn excerpt(out: &str) -> String {
    let out = crate::agents::provider::strip_ansi(out);
    let line = out
        .lines()
        .map(str::trim)
        .find(|l| l.to_ascii_lowercase().starts_with("error") || l.contains("FAILED") || l.contains("failed"))
        .or_else(|| out.lines().map(str::trim).rfind(|l| !l.is_empty()))
        .unwrap_or("sem saída");
    line.chars().take(160).collect()
}

/// Roda as verificações em sequência; para na primeira falha.
pub async fn run_checks(dir: &Path, checks: &[CheckCmd], timeout: Duration) -> Vec<CheckFailure> {
    for c in checks {
        let mut cmd = build_command(&c.spec, Some(dir));
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        let fail = |detail: String| vec![CheckFailure { label: c.label.clone(), detail }];
        let child = match cmd.spawn() {
            Ok(ch) => ch,
            Err(e) => {
                tracing::debug!("verificação {} indisponível: {e}", c.label);
                continue; // ferramenta ausente: não vira pendência
            }
        };
        match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Err(_) => return fail(format!("tempo esgotado ({}s)", timeout.as_secs())),
            Ok(Err(e)) => return fail(e.to_string()),
            Ok(Ok(o)) if !o.status.success() => {
                let mut s = String::from_utf8_lossy(&o.stderr).into_owned();
                s.push('\n');
                s.push_str(&String::from_utf8_lossy(&o.stdout));
                return fail(format!("código {:?} — {}", o.status.code(), excerpt(&s)));
            }
            Ok(Ok(_)) => {}
        }
    }
    Vec::new()
}

// ---------------------------------------------------------------- configuração

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoWorkSettings {
    pub enabled: bool,
    pub max_iterations: u32,
    /// Teto de tokens (entrada+saída+cache) da execução; None = sem teto.
    pub max_tokens: Option<u64>,
    /// Teto de custo-equivalente em US$ (tabela de `metrics`); None = sem teto.
    pub max_cost_usd: Option<f64>,
    /// Roda build/testes na varredura.
    pub run_checks: bool,
    pub check_timeout_secs: u64,
}

impl Default for AutoWorkSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_iterations: 20,
            max_tokens: Some(5_000_000),
            max_cost_usd: Some(20.0),
            run_checks: true,
            check_timeout_secs: 600,
        }
    }
}

// ---------------------------------------------------------------- eventos

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AutoWorkStatus {
    /// Loop ativo (turno rodando ou varredura em andamento).
    Running,
    Paused,
    Completed,
    Stopped(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoWorkEvent {
    pub task: TaskId,
    pub project: PathBuf,
    pub status: AutoWorkStatus,
    pub iteration: u32,
    pub max_iterations: u32,
    pub current_item: Option<String>,
    pub done: usize,
    pub blocked: usize,
    pub total: usize,
    pub tokens: u64,
    pub cost_usd: f64,
}

// ---------------------------------------------------------------- motor

struct Run {
    project: PathBuf,
    status: AutoWorkStatus,
    iteration: u32,
    /// Item enviado no último turno disparado pelo auto-work.
    awaiting: Option<String>,
    failures: HashMap<String, u32>,
    tokens: u64,
    cost: f64,
    /// Varredura/disparo em andamento (evita dois turnos simultâneos).
    busy: bool,
    counts: (usize, usize, usize),
}

impl Run {
    fn new(project: PathBuf) -> Self {
        Self {
            project,
            status: AutoWorkStatus::Running,
            iteration: 0,
            awaiting: None,
            failures: HashMap::new(),
            tokens: 0,
            cost: 0.0,
            busy: false,
            counts: (0, 0, 0),
        }
    }
    fn restartable(&self) -> bool {
        match &self.status {
            AutoWorkStatus::Completed => true,
            AutoWorkStatus::Stopped(r) => r != STOP_BY_USER,
            _ => false,
        }
    }
}

struct Inner {
    orch: Orchestrator,
    settings: Mutex<HashMap<PathBuf, AutoWorkSettings>>,
    runs: Mutex<HashMap<TaskId, Run>>,
}

/// Controlador do auto-work. Clonável; escuta o broadcast do orquestrador.
#[derive(Clone)]
pub struct AutoWork {
    inner: Arc<Inner>,
}

enum Decision {
    Dispatch(String),
    Finish(AutoWorkStatus),
}

pub fn prompt_for(item: &str) -> String {
    format!(
        "[auto-work] Resolva o PRÓXIMO item pendente do arquivo {FILE_NAME} (na raiz do projeto):\n\n    {item}\n\n\
         Ao concluir de verdade (com build/testes passando), marque esse item como `- [x]` no {FILE_NAME}. \
         Não marque como concluído sem ter concluído. Se não conseguir resolver, explique o motivo e deixe o item como `- [ ]`. \
         Trabalhe somente nesse item."
    )
}

impl AutoWork {
    /// Cria o controlador e começa a escutar eventos (precisa de runtime tokio).
    pub fn start(orch: Orchestrator) -> Self {
        let this = Self { inner: Arc::new(Inner { orch, settings: Mutex::new(HashMap::new()), runs: Mutex::new(HashMap::new()) }) };
        let mut rx = this.inner.orch.subscribe();
        let me = this.clone();
        tokio::spawn(async move {
            use tokio::sync::broadcast::error::RecvError;
            loop {
                match rx.recv().await {
                    Ok(ev) => me.on_event(ev),
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        });
        this
    }

    fn on_event(&self, ev: OrchestratorEvent) {
        match ev {
            OrchestratorEvent::StateChanged { task, state } => match state {
                TaskState::Completed | TaskState::Failed(_) => {
                    let me = self.clone();
                    tokio::spawn(async move { me.turn_ended(task).await });
                }
                TaskState::Cancelled => self.finish_if_running(task, AutoWorkStatus::Stopped("turno cancelado".into())),
                TaskState::Applied | TaskState::Discarded => {
                    self.finish_if_running(task, AutoWorkStatus::Stopped("tarefa encerrada".into()))
                }
                _ => {}
            },
            OrchestratorEvent::Removed { task } => {
                // Sem evento: a interface já tirou a tarefa (e o progresso dela) das listas.
                self.inner.runs.lock().unwrap().remove(&task);
            }
            OrchestratorEvent::Agent { task, event: crate::agents::provider::AgentEvent::Usage(u) } => {
                let mut runs = self.inner.runs.lock().unwrap();
                if let Some(r) = runs.get_mut(&task) {
                    if matches!(r.status, AutoWorkStatus::Running | AutoWorkStatus::Paused) {
                        r.tokens += cap_tokens(&u);
                        r.cost += crate::metrics::cost(&u);
                    }
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ configuração

    pub fn settings(&self, project: &Path) -> AutoWorkSettings {
        if let Some(s) = self.inner.settings.lock().unwrap().get(project) {
            return s.clone();
        }
        let s: AutoWorkSettings = self
            .inner
            .orch
            .db()
            .and_then(|db| db.get_autowork(&project.to_string_lossy()).ok().flatten())
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
        self.inner.settings.lock().unwrap().insert(project.to_path_buf(), s.clone());
        s
    }

    /// Projetos com configuração salva.
    pub fn all_settings(&self) -> Vec<(PathBuf, AutoWorkSettings)> {
        let mut m: HashMap<PathBuf, AutoWorkSettings> = HashMap::new();
        if let Some(db) = self.inner.orch.db() {
            for (p, j) in db.list_autowork().unwrap_or_default() {
                if let Ok(s) = serde_json::from_str(&j) {
                    m.insert(PathBuf::from(p), s);
                }
            }
        }
        for (p, s) in self.inner.settings.lock().unwrap().iter() {
            m.insert(p.clone(), s.clone());
        }
        let mut v: Vec<_> = m.into_iter().collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// Salva a configuração (no banco, se houver). Desligar para as execuções do projeto.
    pub fn set_settings(&self, project: &Path, s: AutoWorkSettings) {
        if let Some(db) = self.inner.orch.db() {
            let _ = db.set_autowork(&project.to_string_lossy(), &serde_json::to_string(&s).unwrap_or_default());
        }
        let enabled = s.enabled;
        self.inner.settings.lock().unwrap().insert(project.to_path_buf(), s);
        if !enabled {
            let ids: Vec<TaskId> = self
                .inner
                .runs
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, r)| r.project == project && matches!(r.status, AutoWorkStatus::Running | AutoWorkStatus::Paused))
                .map(|(id, _)| *id)
                .collect();
            for id in ids {
                self.finish_if_running(id, AutoWorkStatus::Stopped("auto-work desligado".into()));
            }
        }
    }

    pub fn set_enabled(&self, project: &Path, enabled: bool) {
        let mut s = self.settings(project);
        s.enabled = enabled;
        self.set_settings(project, s);
    }

    // ------------------------------------------------------------ controle

    pub fn status(&self, task: TaskId) -> Option<AutoWorkEvent> {
        self.inner.runs.lock().unwrap().get(&task).map(|r| snapshot(task, r, &self.settings(&r.project)))
    }

    pub fn all_status(&self) -> Vec<AutoWorkEvent> {
        let runs: Vec<(TaskId, PathBuf)> = self.inner.runs.lock().unwrap().iter().map(|(k, r)| (*k, r.project.clone())).collect();
        let mut v: Vec<_> = runs.into_iter().filter_map(|(id, _)| self.status(id)).collect();
        v.sort_by_key(|e| e.task);
        v
    }

    /// Inicia (ou reinicia) o loop numa tarefa existente, mesmo sem esperar o fim de um turno.
    pub fn start_task(&self, task: TaskId) -> bool {
        let Some(t) = self.inner.orch.task(task) else { return false };
        {
            let mut runs = self.inner.runs.lock().unwrap();
            let r = runs.entry(task).or_insert_with(|| Run::new(t.project.clone()));
            if r.status != AutoWorkStatus::Running {
                *r = Run::new(t.project.clone());
            }
        }
        self.kick(task, &t.state);
        true
    }

    pub fn pause(&self, task: TaskId) -> bool {
        let ev = {
            let mut runs = self.inner.runs.lock().unwrap();
            let Some(r) = runs.get_mut(&task) else { return false };
            if r.status != AutoWorkStatus::Running {
                return false;
            }
            r.status = AutoWorkStatus::Paused;
            snapshot(task, r, &AutoWorkSettings { max_iterations: 0, ..Default::default() })
        };
        self.emit_fixed(ev);
        true
    }

    pub fn resume(&self, task: TaskId) -> bool {
        {
            let mut runs = self.inner.runs.lock().unwrap();
            let Some(r) = runs.get_mut(&task) else { return false };
            if r.status != AutoWorkStatus::Paused {
                return false;
            }
            r.status = AutoWorkStatus::Running;
        }
        let state = self.inner.orch.task(task).map(|t| t.state).unwrap_or(TaskState::Completed);
        self.kick(task, &state);
        true
    }

    /// Para o loop e cancela o turno em andamento.
    pub fn stop(&self, task: TaskId) -> bool {
        let existed = self.inner.runs.lock().unwrap().contains_key(&task);
        if !existed {
            return false;
        }
        self.finish_if_running(task, AutoWorkStatus::Stopped(STOP_BY_USER.into()));
        if self.inner.orch.task(task).is_some_and(|t| t.state.is_active()) {
            let _ = self.inner.orch.cancel(task);
        }
        true
    }

    /// Se a tarefa está parada, avalia já; senão o fim do turno dispara.
    fn kick(&self, task: TaskId, state: &TaskState) {
        if !state.is_active() {
            let me = self.clone();
            tokio::spawn(async move { me.turn_ended(task).await });
        } else {
            self.emit(task);
        }
    }

    fn finish_if_running(&self, task: TaskId, st: AutoWorkStatus) {
        let changed = {
            let mut runs = self.inner.runs.lock().unwrap();
            match runs.get_mut(&task) {
                Some(r) if matches!(r.status, AutoWorkStatus::Running | AutoWorkStatus::Paused) => {
                    r.status = st;
                    r.awaiting = None;
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.emit(task);
        }
    }

    fn emit(&self, task: TaskId) {
        if let Some(ev) = self.status(task) {
            self.emit_fixed(ev);
        }
    }

    fn emit_fixed(&self, mut ev: AutoWorkEvent) {
        if ev.max_iterations == 0 {
            ev.max_iterations = self.settings(&ev.project).max_iterations;
        }
        self.inner.orch.emit(OrchestratorEvent::AutoWork(ev));
    }

    // ------------------------------------------------------------ ciclo

    /// Chamado ao fim de cada turno da tarefa.
    async fn turn_ended(&self, task: TaskId) {
        let Some(t) = self.inner.orch.task(task) else { return };
        let settings = self.settings(&t.project);
        let turn_failed = matches!(t.state, TaskState::Failed(_));
        // Reserva a execução (uma varredura/disparo por vez).
        {
            let mut runs = self.inner.runs.lock().unwrap();
            let exists = runs.contains_key(&task);
            if !exists && !settings.enabled {
                return;
            }
            let r = runs.entry(task).or_insert_with(|| Run::new(t.project.clone()));
            if exists && r.restartable() {
                if !settings.enabled {
                    return;
                }
                *r = Run::new(t.project.clone());
            }
            if r.status != AutoWorkStatus::Running || r.busy {
                return;
            }
            r.busy = true;
        }
        self.emit(task);
        let workdir = t.workdir.clone().unwrap_or_else(|| t.project.clone());
        let decision = self.evaluate(task, &workdir, &settings, turn_failed).await;

        let mut dispatch_err = None;
        if let Decision::Dispatch(item) = &decision {
            match self.inner.orch.reply(task, &prompt_for(item)) {
                Ok(()) => {}
                Err(crate::agents::orchestrator::OrchestratorError::Busy) => {} // outro turno já começou
                Err(e) => dispatch_err = Some(e.to_string()),
            }
        }
        {
            let mut runs = self.inner.runs.lock().unwrap();
            if let Some(r) = runs.get_mut(&task) {
                r.busy = false;
                if r.status == AutoWorkStatus::Running {
                    match (&decision, dispatch_err) {
                        (_, Some(e)) => {
                            r.status = AutoWorkStatus::Stopped(format!("falha ao continuar: {e}"));
                            r.awaiting = None;
                        }
                        (Decision::Finish(st), None) => {
                            r.status = st.clone();
                            r.awaiting = None;
                        }
                        (Decision::Dispatch(_), None) => {}
                    }
                } else if let Decision::Dispatch(_) = decision {
                    // pausado/parado durante a varredura: o turno já foi disparado; fica registrado.
                }
            }
        }
        self.emit(task);
    }

    /// Varre, atualiza o PENDENCIAS.md e decide o próximo passo.
    async fn evaluate(&self, task: TaskId, workdir: &Path, s: &AutoWorkSettings, turn_failed: bool) -> Decision {
        let dir = workdir.to_path_buf();
        let scan = tokio::task::spawn_blocking(move || -> std::io::Result<(PendingDoc, usize)> {
            let mut doc = PendingDoc::load_or_create(&dir)?;
            let mut added = 0;
            for f in scan_todos(&dir) {
                if !f.listed_in(&doc) {
                    doc.append(&f.item_text());
                    added += 1;
                }
            }
            if added > 0 {
                doc.save(&dir)?;
            }
            Ok((doc, added))
        })
        .await;
        let (mut doc, mut added) = match scan {
            Ok(Ok(x)) => x,
            Ok(Err(e)) => return Decision::Finish(AutoWorkStatus::Stopped(format!("erro ao ler {FILE_NAME}: {e}"))),
            Err(e) => return Decision::Finish(AutoWorkStatus::Stopped(e.to_string())),
        };

        // Contabiliza o item do último turno disparado.
        let mut dirty = false;
        {
            let mut runs = self.inner.runs.lock().unwrap();
            let Some(r) = runs.get_mut(&task) else { return Decision::Finish(AutoWorkStatus::Stopped("execução removida".into())) };
            if let Some(item) = r.awaiting.take() {
                if let Some(it) = doc.find(&item).filter(|i| i.status == ItemStatus::Open) {
                    let n = r.failures.entry(item.clone()).or_insert(0);
                    *n += 1;
                    if *n >= MAX_ITEM_FAILURES {
                        let why = if turn_failed { "turno falhou 3x" } else { "não concluído após 3 tentativas" };
                        doc.set_status(it.line, ItemStatus::Blocked, Some(why));
                        dirty = true;
                    }
                }
            }
        }

        // Verificação (build/testes): só quando não há item aberto ou na varredura normal.
        let mut checks_ok = true;
        if s.run_checks {
            let checks = detect_checks(workdir);
            if !checks.is_empty() {
                let fails = run_checks(workdir, &checks, Duration::from_secs(s.check_timeout_secs.max(1))).await;
                for f in &fails {
                    checks_ok = false;
                    let text = f.item_text();
                    let key = format!("Corrigir falha em `{}`", f.label);
                    if !doc.any(&[ItemStatus::Open, ItemStatus::Blocked], |t| t.contains(&key)) {
                        doc.append(&text);
                        added += 1;
                        dirty = true;
                    }
                }
            }
        }
        if dirty {
            let d = workdir.to_path_buf();
            let doc2 = doc.clone();
            let _ = tokio::task::spawn_blocking(move || doc2.save(&d)).await;
        }

        let counts = doc.counts();
        let next = doc.next_open();
        let mut runs = self.inner.runs.lock().unwrap();
        let Some(r) = runs.get_mut(&task) else { return Decision::Finish(AutoWorkStatus::Stopped("execução removida".into())) };
        r.counts = counts;
        if r.status != AutoWorkStatus::Running {
            return Decision::Finish(r.status.clone());
        }
        let Some(item) = next else {
            if added == 0 && checks_ok {
                return Decision::Finish(if counts.1 > 0 {
                    AutoWorkStatus::Stopped(format!("{} item(ns) bloqueado(s)", counts.1))
                } else {
                    AutoWorkStatus::Completed
                });
            }
            // Falha de verificação já listada como bloqueada: nada mais a fazer.
            return Decision::Finish(AutoWorkStatus::Stopped("verificação falhando (item bloqueado)".into()));
        };
        if r.iteration >= s.max_iterations {
            return Decision::Finish(AutoWorkStatus::Stopped(format!("limite de {} iterações", s.max_iterations)));
        }
        if s.max_tokens.is_some_and(|m| r.tokens >= m) {
            return Decision::Finish(AutoWorkStatus::Stopped("teto de tokens atingido".into()));
        }
        if s.max_cost_usd.is_some_and(|m| r.cost >= m) {
            return Decision::Finish(AutoWorkStatus::Stopped("teto de custo atingido".into()));
        }
        r.iteration += 1;
        r.awaiting = Some(item.text.clone());
        Decision::Dispatch(item.text)
    }
}

fn snapshot(task: TaskId, r: &Run, s: &AutoWorkSettings) -> AutoWorkEvent {
    AutoWorkEvent {
        task,
        project: r.project.clone(),
        status: r.status.clone(),
        iteration: r.iteration,
        max_iterations: s.max_iterations,
        current_item: r.awaiting.clone(),
        done: r.counts.0,
        blocked: r.counts.1,
        total: r.counts.2,
        tokens: r.tokens,
        cost_usd: r.cost,
    }
}

/// Tokens que contam para o teto: entrada + saída + escrita de cache.
/// Leitura de cache fica de fora (é barata e se repete a cada turno);
/// o teto de custo continua usando o custo real.
pub fn cap_tokens(u: &crate::agents::provider::TokenUsage) -> u64 {
    u.input_tokens + u.output_tokens + u.cache_write
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::provider::{AgentEvent, AgentProvider, LoginStatus};

    #[test]
    fn parser_preserves_text() {
        let src = "# Título\r\n\r\nTexto livre.\r\n- [ ] um\r\n  * [x] dois\r\n- [!] bloqueado: x — tres\r\n- [] não é item\r\n```\r\n- [ ] no código\r\n```\r\n";
        let mut d = PendingDoc::parse(src);
        assert_eq!(d.render(), src);
        let it = d.items();
        assert_eq!(it.len(), 3);
        assert_eq!(it[0], Item { line: 3, status: ItemStatus::Open, text: "um".into() });
        assert_eq!(it[1].status, ItemStatus::Done);
        assert_eq!(it[2].status, ItemStatus::Blocked);
        assert_eq!(d.counts(), (1, 1, 3));
        assert!(d.set_status(3, ItemStatus::Done, None));
        assert!(d.render().contains("- [x] um\r\n"));
        assert!(d.set_status(4, ItemStatus::Blocked, Some("motivo")));
        assert!(d.render().contains("  * [!] bloqueado: motivo — dois\r\n"), "{}", d.render());
        d.append("novo");
        let r = d.render();
        assert!(r.contains("tres\r\n- [ ] novo\r\n- [] não"), "{r}");
        assert!(r.starts_with("# Título\r\n\r\nTexto livre.\r\n"));
        assert_eq!(d.next_open().unwrap().text, "novo");

        let mut e = PendingDoc::parse("# P\n\nnotas\n");
        e.append("a");
        assert_eq!(e.render(), "# P\n\nnotas\n\n- [ ] a\n");
    }

    #[test]
    fn creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let d = PendingDoc::load_or_create(dir.path()).unwrap();
        assert!(dir.path().join(FILE_NAME).exists());
        assert!(d.items().is_empty());
    }

    #[test]
    fn scans_todos() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        std::fs::create_dir_all(p.join("src")).unwrap();
        std::fs::create_dir_all(p.join("target")).unwrap();
        std::fs::create_dir_all(p.join("node_modules/x")).unwrap();
        std::fs::write(p.join("src/a.rs"), "fn a() {}\n// TODO: tratar erro\nlet todos = 1; // FIXME(ana) - lento\nlet s = \"TODO\";\n").unwrap();
        std::fs::write(p.join("target/b.rs"), "// TODO: ignorar\n").unwrap();
        std::fs::write(p.join("node_modules/x/c.js"), "// TODO: ignorar\n").unwrap();
        std::fs::write(p.join(FILE_NAME), "- [ ] TODO: não conta\n").unwrap();
        let f = scan_todos(p);
        assert_eq!(f.len(), 2, "{f:?}");
        assert_eq!(f[0], Finding { file: "src/a.rs".into(), line: 2, kind: "TODO", text: "tratar erro".into() });
        assert_eq!(f[1].kind, "FIXME");
        assert_eq!(f[1].text, "lento");
        assert_eq!(f[0].item_text(), "TODO em src/a.rs:2 — tratar erro");
        let doc = PendingDoc::parse("- [x] TODO em src/a.rs:7 — tratar erro\n");
        assert!(f[0].listed_in(&doc));
        assert!(!f[1].listed_in(&doc));
        assert_eq!(find_marker("x = TODOS"), None);
    }

    #[test]
    fn detects_checks() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_checks(dir.path()).is_empty());
        std::fs::write(dir.path().join("package.json"), r#"{"scripts":{"test":"jest"}}"#).unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let l: Vec<_> = detect_checks(dir.path()).into_iter().map(|c| c.label).collect();
        assert_eq!(l, ["cargo check", "cargo test", "npm test"]);
    }

    // ---------------------------------------------------- loop com provider falso

    /// Provider falso: cada turno emite uma sessão e, se `marks`, troca o
    /// primeiro `- [ ]` do PENDENCIAS.md por `- [x]`.
    struct Fake {
        marks: bool,
    }
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
        fn task_command(&self, p: &str, cwd: &Path, _s: Option<&str>, _o: &crate::agents::provider::TurnOptions) -> CommandSpec {
            let mut c;
            if cfg!(windows) {
                c = CommandSpec::new("powershell").cwd(cwd).stdin(p);
                let mark = "$p='PENDENCIAS.md'; $c=[IO.File]::ReadAllText($p); $i=$c.IndexOf('- [ ]'); if($i -ge 0){ $c=$c.Substring(0,$i)+'- [x]'+$c.Substring($i+5); [IO.File]::WriteAllText($p,$c) }; ";
                let script = format!("{}Write-Output 'SESSION:s1'; Write-Output 'USAGE'", if self.marks { mark } else { "" });
                c.args = vec!["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), script];
            } else {
                c = CommandSpec::new("sh").cwd(cwd).stdin(p);
                let mark = "sed -i '0,/- \\[ \\]/s//- [x]/' PENDENCIAS.md; ";
                c.args = vec!["-c".into(), format!("{}echo SESSION:s1; echo USAGE", if self.marks { mark } else { "" })];
            }
            c
        }
        fn parse_line(&self, l: &str) -> Vec<AgentEvent> {
            let l = l.trim();
            if let Some(s) = l.strip_prefix("SESSION:") {
                return vec![AgentEvent::Session(s.into())];
            }
            if l == "USAGE" {
                return vec![AgentEvent::Usage(crate::agents::provider::TokenUsage {
                    input_tokens: 100,
                    output_tokens: 10,
                    model: Some("claude-sonnet-4-5".into()),
                    ..Default::default()
                })];
            }
            vec![]
        }
    }

    /// O turno inicial do usuário também roda o provider falso (marca o 1º item).
    async fn run_loop(marks: bool, doc: &str, s: AutoWorkSettings) -> (AutoWorkEvent, String, Vec<AutoWorkEvent>) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), doc).unwrap();
        let orch = Orchestrator::new(vec![Arc::new(Fake { marks })], 1);
        let mut rx = orch.subscribe();
        let aw = AutoWork::start(orch.clone());
        aw.set_settings(dir.path(), AutoWorkSettings { run_checks: false, enabled: true, ..s });
        let id = orch.submit(dir.path(), "fake", "início").unwrap();
        let mut evs = Vec::new();
        let fin = tokio::time::timeout(Duration::from_secs(120), async {
            loop {
                if let Ok(OrchestratorEvent::AutoWork(e)) = rx.recv().await {
                    evs.push(e.clone());
                    if matches!(e.status, AutoWorkStatus::Completed | AutoWorkStatus::Stopped(_)) && e.task == id {
                        return e;
                    }
                }
            }
        })
        .await
        .expect("auto-work não terminou");
        // Nunca dois turnos simultâneos: a tarefa termina parada.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!orch.task(id).unwrap().state.is_active());
        let text = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        (fin, text, evs)
    }

    #[tokio::test]
    async fn loop_completes_when_items_marked() {
        let (fin, text, evs) = run_loop(true, "# P\n\nnota\n- [ ] a\n- [ ] b\n- [ ] c\n", AutoWorkSettings::default()).await;
        assert_eq!(fin.status, AutoWorkStatus::Completed, "{fin:?}");
        assert_eq!(fin.iteration, 2);
        assert_eq!((fin.done, fin.total), (3, 3));
        assert!(fin.tokens > 0 && fin.cost_usd > 0.0);
        assert_eq!(text, "# P\n\nnota\n- [x] a\n- [x] b\n- [x] c\n");
        assert!(evs.iter().any(|e| e.current_item.as_deref() == Some("b")));
    }

    #[tokio::test]
    async fn loop_respects_max_iterations() {
        let s = AutoWorkSettings { max_iterations: 2, ..Default::default() };
        let (fin, text, _) = run_loop(false, "- [ ] a\n", s).await;
        assert_eq!(fin.status, AutoWorkStatus::Stopped("limite de 2 iterações".into()));
        assert_eq!(fin.iteration, 2);
        assert_eq!(text, "- [ ] a\n");
    }

    #[tokio::test]
    async fn item_blocked_after_three_failures() {
        let (fin, text, _) = run_loop(false, "- [ ] a\n", AutoWorkSettings::default()).await;
        assert_eq!(fin.iteration, 3);
        assert_eq!(fin.status, AutoWorkStatus::Stopped("1 item(ns) bloqueado(s)".into()));
        assert_eq!(text, "- [!] bloqueado: não concluído após 3 tentativas — a\n");
    }

    #[tokio::test]
    async fn token_cap_stops() {
        let s = AutoWorkSettings { max_tokens: Some(1), ..Default::default() };
        let (fin, _, _) = run_loop(true, "- [ ] a\n- [ ] b\n- [ ] c\n", s).await;
        // O turno do usuário (marca "a") não conta; o 1º turno do auto-work (marca "b") estoura.
        assert_eq!(fin.status, AutoWorkStatus::Stopped("teto de tokens atingido".into()));
        assert_eq!(fin.iteration, 1);
    }

    #[test]
    fn cap_tokens_ignores_cache_read() {
        let u = crate::agents::provider::TokenUsage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read: 158_000,
            cache_write: 100,
            ..Default::default()
        };
        assert_eq!(super::cap_tokens(&u), 115);
    }
}
