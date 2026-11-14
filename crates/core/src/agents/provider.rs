//! Contrato comum dos provedores de agente (CLIs oficiais).
//! A IDE nunca lê nem guarda tokens: cada CLI cuida do próprio login.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Comando a ser executado (no PTY ou via tokio::process).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    /// Texto enviado pela entrada padrão (o prompt). Evita problemas de
    /// escape de aspas/%/quebras de linha nos wrappers `.cmd` do npm no Windows.
    #[serde(default)]
    pub stdin: Option<String>,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self { program: program.into(), args: Vec::new(), cwd: None, env: Vec::new(), stdin: None }
    }
    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.args.push(a.into());
        self
    }
    pub fn cwd(mut self, p: impl Into<PathBuf>) -> Self {
        self.cwd = Some(p.into());
        self
    }
    pub fn stdin(mut self, s: impl Into<String>) -> Self {
        self.stdin = Some(s.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoginStatus {
    NotInstalled,
    LoggedOut,
    LoggedIn,
    Unknown,
}

/// Consumo de tokens de um turno (ou de um passo) do agente.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub model: Option<String>,
    /// Custo informado pela própria CLI (quando houver). Com assinatura OAuth
    /// pode vir 0/ausente; o custo "equivalente em API" é calculado em `metrics`.
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentEvent {
    /// Id de sessão da CLI (para continuar a conversa).
    Session(String),
    /// Mensagem enviada pelo usuário (turnos de resposta).
    User(String),
    Text(String),
    ToolUse { name: String, input: serde_json::Value },
    Result { ok: bool, summary: String, cost: Option<f64> },
    Usage(TokenUsage),
    Raw(String),
    /// Um subagente nasceu (ferramenta Task/Agent). `id` é o id do tool_use que
    /// o criou; `parent` é o id do subagente que o criou (None = agente principal).
    SubagentStart {
        id: String,
        parent: Option<String>,
        description: String,
        subagent_type: String,
        prompt: String,
    },
    /// O subagente `id` terminou; `result` é o texto devolvido a quem o chamou.
    SubagentEnd { id: String, ok: bool, result: String },
    /// Evento interno do subagente `subagent` (texto, ferramenta...). Os eventos
    /// do agente principal continuam vindo sem este invólucro.
    Scoped { subagent: String, event: Box<AgentEvent> },
}

/// Resumo de um subagente, montado a partir dos eventos de uma tarefa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubagentInfo {
    pub id: String,
    pub parent: Option<String>,
    pub description: String,
    pub subagent_type: String,
    pub prompt: String,
    /// Já recebeu `SubagentEnd`.
    pub done: bool,
    /// Resultado do `SubagentEnd` (None enquanto roda).
    pub ok: Option<bool>,
    pub result: Option<String>,
}

/// Lista os subagentes de uma sequência de eventos, na ordem em que nasceram.
pub fn subagents_of(events: &[AgentEvent]) -> Vec<SubagentInfo> {
    let mut out: Vec<SubagentInfo> = Vec::new();
    for ev in events {
        match ev {
            AgentEvent::SubagentStart { id, parent, description, subagent_type, prompt } => {
                if !out.iter().any(|s| &s.id == id) {
                    out.push(SubagentInfo {
                        id: id.clone(),
                        parent: parent.clone(),
                        description: description.clone(),
                        subagent_type: subagent_type.clone(),
                        prompt: prompt.clone(),
                        done: false,
                        ok: None,
                        result: None,
                    });
                }
            }
            AgentEvent::SubagentEnd { id, ok, result } => {
                if let Some(s) = out.iter_mut().find(|s| &s.id == id) {
                    s.done = true;
                    s.ok = Some(*ok);
                    s.result = Some(result.clone());
                }
            }
            _ => {}
        }
    }
    out
}

impl AgentEvent {
    /// Envolve o evento em `Scoped` quando `subagent` é Some. `SubagentStart` e
    /// `SubagentEnd` nunca são envolvidos (já carregam `id`/`parent`).
    pub fn scoped(self, subagent: Option<&str>) -> AgentEvent {
        match (subagent, &self) {
            (None, _) | (_, AgentEvent::SubagentStart { .. } | AgentEvent::SubagentEnd { .. } | AgentEvent::Scoped { .. }) => self,
            (Some(s), _) => AgentEvent::Scoped { subagent: s.to_string(), event: Box::new(self) },
        }
    }

    /// Id do subagente a que o evento se refere: o de `Scoped`, ou o próprio
    /// `id` em `SubagentStart`/`SubagentEnd`. None = evento do agente principal.
    pub fn subagent_id(&self) -> Option<&str> {
        match self {
            AgentEvent::Scoped { subagent, .. } => Some(subagent),
            AgentEvent::SubagentStart { id, .. } | AgentEvent::SubagentEnd { id, .. } => Some(id),
            _ => None,
        }
    }

    /// O evento sem o invólucro `Scoped`.
    pub fn inner(&self) -> &AgentEvent {
        match self {
            AgentEvent::Scoped { event, .. } => event.inner(),
            o => o,
        }
    }

    /// Nome curto do tipo (usado na persistência). `Scoped` usa o do evento interno.
    pub fn kind(&self) -> &'static str {
        match self {
            AgentEvent::SubagentStart { .. } => "SubagentStart",
            AgentEvent::SubagentEnd { .. } => "SubagentEnd",
            AgentEvent::Scoped { event, .. } => event.kind(),
            AgentEvent::Session(_) => "Session",
            AgentEvent::User(_) => "User",
            AgentEvent::Text(_) => "Text",
            AgentEvent::ToolUse { .. } => "ToolUse",
            AgentEvent::Result { .. } => "Result",
            AgentEvent::Usage(_) => "Usage",
            AgentEvent::Raw(_) => "Raw",
        }
    }
}

pub trait AgentProvider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    /// Caminho do executável, se instalado.
    fn detect(&self) -> Option<PathBuf>;
    fn login_status(&self) -> LoginStatus;
    /// Comando interativo de login (rodar no PTY).
    fn login_command(&self) -> CommandSpec;
    /// Comando de um turno. O prompt vai em `CommandSpec::stdin`.
    /// Com `session` = Some, continua a conversa existente.
    fn task_command(&self, prompt: &str, cwd: &Path, session: Option<&str>) -> CommandSpec;
    /// Uma linha de saída pode gerar vários eventos.
    fn parse_line(&self, line: &str) -> Vec<AgentEvent>;
}

/// Localiza um executável no PATH, preferindo `.exe` real a wrappers
/// `.cmd`/`.bat`/`.ps1` quando houver mais de um candidato.
pub fn which_bin(name: &str) -> Option<PathBuf> {
    let all: Vec<PathBuf> = which::which_all(name).map(|i| i.collect()).unwrap_or_default();
    pick_executable(&all).or_else(|| which::which(name).ok())
}

/// Escolhe o melhor candidato: `.exe` > sem extensão em unix > `.cmd`/`.bat`.
pub(crate) fn pick_executable(cands: &[PathBuf]) -> Option<PathBuf> {
    let ext = |p: &PathBuf| p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    if cfg!(windows) {
        cands
            .iter()
            .find(|p| ext(p).as_deref() == Some("exe"))
            .or_else(|| cands.iter().find(|p| matches!(ext(p).as_deref(), Some("cmd") | Some("bat"))))
            .cloned()
    } else {
        cands.first().cloned()
    }
}

/// Executa um comando curto e devolve (sucesso, stdout+stderr). Sem janela de console no Windows.
pub(crate) fn run_capture(program: &Path, args: &[&str]) -> Option<(bool, String)> {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push('\n');
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), s))
}

/// Remove sequências ANSI (cores/cursor) de uma saída de terminal.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for c2 in it.by_ref() {
                    if ('@'..='~').contains(&c2) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_exe_over_cmd() {
        let c = vec![PathBuf::from(r"C:\npm\opencode.cmd"), PathBuf::from(r"C:\bun\opencode.exe")];
        let p = pick_executable(&c).unwrap();
        if cfg!(windows) {
            assert!(p.ends_with("opencode.exe"));
        } else {
            assert_eq!(p, c[0]);
        }
        assert_eq!(pick_executable(&[]), None);
    }

    #[test]
    fn subagent_events_json_is_stable() {
        let start = AgentEvent::SubagentStart {
            id: "toolu_1".into(),
            parent: None,
            description: "Listar".into(),
            subagent_type: "general-purpose".into(),
            prompt: "liste".into(),
        };
        assert_eq!(
            serde_json::to_string(&start).unwrap(),
            r#"{"SubagentStart":{"id":"toolu_1","parent":null,"description":"Listar","subagent_type":"general-purpose","prompt":"liste"}}"#
        );
        let end = AgentEvent::SubagentEnd { id: "toolu_1".into(), ok: true, result: "a".into() };
        assert_eq!(serde_json::to_string(&end).unwrap(), r#"{"SubagentEnd":{"id":"toolu_1","ok":true,"result":"a"}}"#);
        let sc = AgentEvent::Text("oi".into()).scoped(Some("toolu_1"));
        let js = r#"{"Scoped":{"subagent":"toolu_1","event":{"Text":"oi"}}}"#;
        assert_eq!(serde_json::to_string(&sc).unwrap(), js);
        assert_eq!(serde_json::from_str::<AgentEvent>(js).unwrap(), sc);
        assert_eq!(sc.kind(), "Text");
        assert_eq!(sc.subagent_id(), Some("toolu_1"));
        assert_eq!(sc.inner(), &AgentEvent::Text("oi".into()));
        // Variantes antigas não mudam; Start/End não são envolvidos.
        assert_eq!(serde_json::to_string(&AgentEvent::Text("oi".into())).unwrap(), r#"{"Text":"oi"}"#);
        assert_eq!(AgentEvent::Text("x".into()).scoped(None), AgentEvent::Text("x".into()));
        assert_eq!(end.clone().scoped(Some("p")), end);
        let info = subagents_of(&[start, sc, end]);
        assert_eq!(info.len(), 1);
        assert!(info[0].done && info[0].ok == Some(true) && info[0].result.as_deref() == Some("a"));
    }

    #[test]
    fn strips_ansi() {
        assert_eq!(strip_ansi("\u{1b}[1;32mok\u{1b}[0m x"), "ok x");
    }
}
