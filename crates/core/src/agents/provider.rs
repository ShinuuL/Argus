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
}

impl AgentEvent {
    /// Nome curto do tipo (usado na persistência).
    pub fn kind(&self) -> &'static str {
        match self {
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
    fn strips_ansi() {
        assert_eq!(strip_ansi("\u{1b}[1;32mok\u{1b}[0m x"), "ok x");
    }
}
