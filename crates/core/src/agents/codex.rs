//! Provedor Codex (CLI oficial da OpenAI, pacote npm `@openai/codex`, binário `codex`).
//!
//! Login pela própria CLI (`codex login`, conta ChatGPT); status por
//! `codex login status` (só a primeira linha da saída é interpretada; a IDE não
//! lê nem guarda tokens).
//!
//! Turno: `codex exec --json ... -` (prompt pela entrada padrão), retomada com
//! `codex exec ... resume <thread_id> -`. Fontes (repositório github.com/openai/codex):
//! - `codex-rs/exec/src/cli.rs`: `--json` (global), `resume <SESSION_ID> [PROMPT]`,
//!   PROMPT `-` = ler da entrada padrão, `--skip-git-repo-check`;
//! - `codex-rs/utils/cli` (`SharedCliOptions`): `-m/--model`, `-s/--sandbox`,
//!   `-C/--cd`; as opções antes de `resume` são herdadas por ele
//!   (`inherit_exec_root_options`);
//! - `codex-rs/exec/src/exec_events.rs`: os eventos JSONL (`thread.started`,
//!   `turn.started`, `turn.completed`, `turn.failed`, `item.started`,
//!   `item.updated`, `item.completed`, `error`) e os tipos de item;
//! - `codex-rs/cli/src/login.rs`: mensagens de `codex login status`.
//!
//! Permissões: `--sandbox workspace-write` (edição só dentro da pasta de trabalho,
//! sem rede) e `-c approval_policy=never` (o valor sem aspas é lido como texto pela CLI) (sem ninguém para aprovar, o que o sandbox
//! não permite falha em vez de pedir). Os comandos de shell que o codex roda ficam
//! DENTRO desse sandbox; não há `--add-dir` nem `danger-full-access`.

use super::provider::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default, Clone)]
pub struct CodexProvider {
    /// Último total acumulado de tokens por sessão (thread). O `turn.completed`
    /// do `codex exec` traz o total da thread, não o do turno.
    totals: Arc<Mutex<HashMap<String, TokenUsage>>>,
}

impl CodexProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Modelos conhecidos (a CLI não tem comando para listá-los). Fonte: página de
/// modelos do Codex (learn.chatgpt.com/docs/models, consultada em 2026-11-16).
const KNOWN_MODELS: &[(&str, &str)] = &[
    ("gpt-6.1-sol", "GPT-6.1 Sol"),
    ("gpt-6.1-terra", "GPT-6.1 Terra (CLI 0.159.0 ou mais nova)"),
    ("gpt-6-astra", "GPT-6 Astra"),
    ("gpt-6-luna", "GPT-6 Luna"),
];

impl AgentProvider for CodexProvider {
    fn id(&self) -> &str {
        "codex"
    }
    fn display_name(&self) -> &str {
        "Codex"
    }
    fn detect(&self) -> Option<PathBuf> {
        which_bin("codex")
    }

    fn login_status(&self) -> LoginStatus {
        let Some(bin) = self.detect() else { return LoginStatus::NotInstalled };
        match run_capture(&bin, &["login", "status"]) {
            Some((ok, out)) => parse_login_status(ok, &out),
            None => LoginStatus::Unknown,
        }
    }

    fn login_command(&self) -> CommandSpec {
        CommandSpec::new("codex").arg("login")
    }

    fn task_command(&self, prompt: &str, cwd: &Path, session: Option<&str>, opts: &TurnOptions) -> CommandSpec {
        let mut c = CommandSpec::new("codex")
            .arg("exec")
            .arg("--json")
            .arg("--sandbox")
            .arg("workspace-write")
            .arg("-c")
            .arg("approval_policy=never")
            .arg("--skip-git-repo-check")
            .arg("-C")
            .arg(cwd.to_string_lossy().to_string());
        if let Some(m) = opts.model.as_deref().filter(|m| valid_model_name(m)) {
            c = c.arg("-m").arg(m);
        }
        if let Some(s) = session.filter(|s| valid_model_name(s)) {
            c = c.arg("resume").arg(s);
        }
        // `codex exec` não tem opção de prompt de sistema: o aviso de ambiente vai
        // na frente do primeiro pedido da sessão (como no opencode).
        let text = match opts.system_note.as_deref().filter(|n| !n.is_empty() && session.is_none()) {
            Some(n) => format!("[Aviso do ambiente] {n}\n\n{prompt}"),
            None => prompt.to_string(),
        };
        c.arg("-").cwd(cwd).stdin(text)
    }

    fn parse_line(&self, line: &str) -> Vec<AgentEvent> {
        parse_json_event(line)
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        Ok(KNOWN_MODELS
            .iter()
            .map(|(v, l)| ModelInfo::new(*v, *l, "lista fixa do Argus"))
            .collect())
    }

    fn turn_tracker(&self) -> Box<dyn TurnTracker> {
        Box::new(CodexTracker { totals: self.totals.clone(), ..Default::default() })
    }
}

/// Saída de `codex login status` (stdout+stderr). Código 0 com "Logged in" =
/// logado; "Not logged in" = deslogado. Nenhuma chave é lida (a CLI já a mascara).
pub fn parse_login_status(ok: bool, out: &str) -> LoginStatus {
    let l = strip_ansi(out).to_ascii_lowercase();
    if l.contains("not logged in") {
        LoginStatus::LoggedOut
    } else if ok && (l.contains("logged in") || l.contains("api key configured")) {
        LoginStatus::LoggedIn
    } else {
        LoginStatus::Unknown
    }
}

fn u(v: Option<&Value>) -> u64 {
    v.and_then(Value::as_u64).unwrap_or(0)
}

/// `usage` do `turn.completed`. `input_tokens` inclui os de cache
/// (`cached_input_tokens`); a saída de raciocínio é tratada como parte de
/// `output_tokens` (SUPOSIÇÃO: a API da OpenAI conta assim).
fn usage_of(v: &Value) -> TokenUsage {
    let input = u(v.get("input_tokens"));
    let cached = u(v.get("cached_input_tokens"));
    TokenUsage {
        input_tokens: input.saturating_sub(cached),
        output_tokens: u(v.get("output_tokens")),
        cache_read: cached,
        cache_write: u(v.get("cache_write_input_tokens")),
        model: None,
        cost_usd: None,
    }
}

/// Parse de uma linha do `codex exec --json`.
pub fn parse_json_event(line: &str) -> Vec<AgentEvent> {
    let line = line.trim();
    if line.is_empty() {
        return vec![];
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![AgentEvent::Raw(line.to_string())];
    };
    let st = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    match v.get("type").and_then(Value::as_str) {
        Some("thread.started") => {
            let id = st(&v, "thread_id");
            if id.is_empty() { vec![] } else { vec![AgentEvent::Session(id)] }
        }
        Some("turn.completed") => {
            let usage = usage_of(v.get("usage").unwrap_or(&Value::Null));
            vec![AgentEvent::Usage(usage), AgentEvent::Result { ok: true, summary: String::new(), cost: None }]
        }
        Some("turn.failed") => {
            let msg = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("o turno falhou").to_string();
            vec![AgentEvent::Result { ok: false, summary: msg, cost: None }]
        }
        // Erro do stream: o `turn.failed` (ou o fim do processo) fecha o turno.
        Some("error") => vec![AgentEvent::Raw(st(&v, "message"))],
        Some(kind @ ("item.started" | "item.completed")) => {
            let item = v.get("item").unwrap_or(&Value::Null);
            parse_item(item, kind == "item.completed")
        }
        Some(_) => vec![],
        None => vec![AgentEvent::Raw(line.to_string())],
    }
}

fn parse_item(item: &Value, completed: bool) -> Vec<AgentEvent> {
    let st = |k: &str| item.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    match (item.get("type").and_then(Value::as_str), completed) {
        (Some("agent_message"), true) => {
            let t = st("text");
            if t.is_empty() { vec![] } else { vec![AgentEvent::Text(t)] }
        }
        // O comando aparece quando começa; no fim, só a recusa interessa.
        (Some("command_execution"), false) => {
            vec![AgentEvent::ToolUse { name: "shell".into(), input: serde_json::json!({ "command": st("command") }) }]
        }
        (Some("command_execution"), true) if st("status") == "declined" => {
            vec![AgentEvent::Denied { tool: "shell".into(), command: st("command") }]
        }
        // Alterações de arquivo só chegam concluídas.
        (Some("file_change"), true) => vec![AgentEvent::ToolUse {
            name: "file_change".into(),
            input: serde_json::json!({ "changes": item.get("changes").cloned().unwrap_or(Value::Null), "status": st("status") }),
        }],
        (Some("mcp_tool_call"), false) => vec![AgentEvent::ToolUse {
            name: format!("{}.{}", st("server"), st("tool")),
            input: item.get("arguments").cloned().unwrap_or(Value::Null),
        }],
        (Some("web_search"), false) => {
            vec![AgentEvent::ToolUse { name: "web_search".into(), input: serde_json::json!({ "query": st("query") }) }]
        }
        (Some("error"), true) => vec![AgentEvent::Raw(st("message"))],
        _ => vec![],
    }
}

/// Acompanhante de um processo do codex.
#[derive(Debug, Default)]
pub struct CodexTracker {
    totals: Arc<Mutex<HashMap<String, TokenUsage>>>,
    thread: Option<String>,
    /// Itens de ferramenta no turno: com eles houve mais de um pedido ao modelo.
    tools: u32,
    saw_result: bool,
    last_error: Option<String>,
}

fn sub(a: &TokenUsage, b: &TokenUsage) -> TokenUsage {
    TokenUsage {
        input_tokens: a.input_tokens.saturating_sub(b.input_tokens),
        output_tokens: a.output_tokens.saturating_sub(b.output_tokens),
        cache_read: a.cache_read.saturating_sub(b.cache_read),
        cache_write: a.cache_write.saturating_sub(b.cache_write),
        model: None,
        cost_usd: None,
    }
}

impl TurnTracker for CodexTracker {
    fn observe(&mut self, line: &str, events: &[AgentEvent]) {
        for e in events {
            match e {
                AgentEvent::Session(s) => self.thread = Some(s.clone()),
                AgentEvent::ToolUse { .. } | AgentEvent::Denied { .. } => self.tools += 1,
                AgentEvent::Result { .. } => self.saw_result = true,
                AgentEvent::Raw(m) if !m.is_empty() && line.trim_start().starts_with('{') => self.last_error = Some(m.clone()),
                _ => {}
            }
        }
    }

    /// O `usage` do `turn.completed` é o total da thread: vira a diferença em
    /// relação ao último total visto da mesma thread (SUPOSIÇÃO: numa retomada o
    /// total continua de onde parou; se vier menor, conta inteiro). Contexto só
    /// sai quando o turno teve um único pedido ao modelo (sem ferramentas): aí a
    /// entrada do turno é exatamente o que ocupou a janela.
    fn transform(&mut self, events: Vec<AgentEvent>) -> Vec<AgentEvent> {
        let mut out = Vec::with_capacity(events.len() + 1);
        for e in events {
            match e {
                AgentEvent::Usage(total) => {
                    let mut map = self.totals.lock().unwrap_or_else(|e| e.into_inner());
                    let prev = self.thread.as_ref().and_then(|t| map.get(t)).cloned();
                    let turn = match prev {
                        Some(p) if total.input_tokens + total.cache_read >= p.input_tokens + p.cache_read
                            && total.output_tokens >= p.output_tokens =>
                        {
                            sub(&total, &p)
                        }
                        _ => total.clone(),
                    };
                    if let Some(t) = &self.thread {
                        map.insert(t.clone(), total);
                    }
                    let ctx = (self.tools == 0 && turn.input_tokens + turn.cache_read > 0).then(|| {
                        crate::agents::context::ContextUsage {
                            input_tokens: turn.input_tokens,
                            cache_read: turn.cache_read,
                            cache_write: turn.cache_write,
                            output_tokens: turn.output_tokens,
                            model: None,
                            window: None,
                        }
                    });
                    out.push(AgentEvent::Usage(turn));
                    out.extend(ctx.map(AgentEvent::Context));
                }
                o => out.push(o),
            }
        }
        out
    }

    fn finish(&mut self, end: TurnEnd) -> Vec<AgentEvent> {
        match end {
            TurnEnd::Exited { ok } if !self.saw_result => vec![AgentEvent::Result {
                ok,
                summary: if ok { String::new() } else { self.last_error.take().unwrap_or_default() },
                cost: None,
            }],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Linhas REAIS, copiadas do exemplo de `codex exec --json` da documentação
    // oficial (learn.chatgpt.com/docs/non-interactive-mode, antes
    // developers.openai.com/codex/noninteractive; consultada em 2026-11-16).
    const STARTED: &str = r#"{"type":"thread.started","thread_id":"0199a213-81c0-7800-8aa1-bbab2a035a53"}"#;
    const TURN: &str = r#"{"type":"turn.started"}"#;
    const CMD: &str = r#"{"type":"item.started","item":{"id":"item_1","type":"command_execution","command":"bash -lc ls","status":"in_progress"}}"#;
    const MSG: &str = r#"{"type":"item.completed","item":{"id":"item_3","type":"agent_message","text":"Repo contains docs, sdk, and examples directories."}}"#;
    const DONE: &str = r#"{"type":"turn.completed","usage":{"input_tokens":24763,"cached_input_tokens":24448,"output_tokens":122,"reasoning_output_tokens":0}}"#;

    // Linhas SINTÉTICAS, montadas a partir dos tipos de
    // `codex-rs/exec/src/exec_events.rs` (não vistas numa execução real).
    const FILE: &str = r#"{"type":"item.completed","item":{"id":"item_4","type":"file_change","changes":[{"path":"src/a.rs","kind":"update"}],"status":"completed"}}"#;
    const DECLINED: &str = r#"{"type":"item.completed","item":{"id":"item_5","type":"command_execution","command":"curl x","aggregated_output":"","exit_code":null,"status":"declined"}}"#;
    const FAILED: &str = r#"{"type":"turn.failed","error":{"message":"stream disconnected"}}"#;

    #[test]
    fn parses_doc_stream() {
        assert_eq!(parse_json_event(STARTED), vec![AgentEvent::Session("0199a213-81c0-7800-8aa1-bbab2a035a53".into())]);
        assert!(parse_json_event(TURN).is_empty());
        assert!(matches!(parse_json_event(CMD).as_slice(), [AgentEvent::ToolUse { name, input }] if name == "shell" && input["command"] == "bash -lc ls"));
        assert_eq!(parse_json_event(MSG), vec![AgentEvent::Text("Repo contains docs, sdk, and examples directories.".into())]);
        let e = parse_json_event(DONE);
        assert!(matches!(&e[0], AgentEvent::Usage(u) if u.input_tokens == 315 && u.cache_read == 24448 && u.output_tokens == 122));
        assert!(matches!(&e[1], AgentEvent::Result { ok: true, .. }));
        assert!(matches!(parse_json_event(FILE).as_slice(), [AgentEvent::ToolUse { name, .. }] if name == "file_change"));
        assert_eq!(parse_json_event(DECLINED), vec![AgentEvent::Denied { tool: "shell".into(), command: "curl x".into() }]);
        assert!(matches!(parse_json_event(FAILED).as_slice(), [AgentEvent::Result { ok: false, summary, .. }] if summary == "stream disconnected"));
        assert_eq!(parse_json_event("aviso solto"), vec![AgentEvent::Raw("aviso solto".into())]);
    }

    fn run(p: &CodexProvider, lines: &[&str], end: TurnEnd) -> Vec<AgentEvent> {
        let mut t = p.turn_tracker();
        let mut all = Vec::new();
        for l in lines {
            let evs = parse_json_event(l);
            t.observe(l, &evs);
            all.extend(t.transform(evs));
        }
        all.extend(t.finish(end));
        all
    }

    #[test]
    fn usage_is_per_turn_and_context_only_without_tools() {
        let p = CodexProvider::new();
        let a = run(&p, &[STARTED, TURN, MSG, DONE], TurnEnd::Exited { ok: true });
        assert!(a.iter().any(|e| matches!(e, AgentEvent::Context(c) if c.total() == 24763)));
        assert_eq!(a.iter().filter(|e| matches!(e, AgentEvent::Result { .. })).count(), 1);
        // Retomada: o total da thread cresce; só a diferença conta. Com ferramenta, sem contexto.
        let more = DONE.replace("24763", "30000").replace("24448", "29000").replace("122", "200");
        let b = run(&p, &[STARTED, TURN, CMD, MSG, &more], TurnEnd::Exited { ok: true });
        assert!(b.iter().any(|e| matches!(e, AgentEvent::Usage(u) if u.input_tokens == 685 && u.cache_read == 4552 && u.output_tokens == 78)), "{b:?}");
        assert!(!b.iter().any(|e| matches!(e, AgentEvent::Context(_))));
    }

    #[test]
    fn missing_result_is_synthesized() {
        let p = CodexProvider::new();
        let err = r#"{"type":"error","message":"falhou"}"#;
        let e = run(&p, &[STARTED, err], TurnEnd::Exited { ok: false });
        assert_eq!(e.last(), Some(&AgentEvent::Result { ok: false, summary: "falhou".into(), cost: None }));
        assert!(!run(&p, &[STARTED], TurnEnd::Cancelled).iter().any(|e| matches!(e, AgentEvent::Result { .. })));
    }

    #[test]
    fn command_line() {
        let p = CodexProvider::new();
        let opts = TurnOptions { model: Some("gpt-6.1-sol".into()), system_note: Some("cópia".into()) };
        let c = p.task_command("oi", Path::new("w"), None, &opts);
        assert_eq!(
            c.args,
            ["exec", "--json", "--sandbox", "workspace-write", "-c", "approval_policy=never", "--skip-git-repo-check", "-C", "w", "-m", "gpt-6.1-sol", "-"]
        );
        assert_eq!(c.stdin.as_deref(), Some("[Aviso do ambiente] cópia\n\noi"));
        let c = p.task_command("oi", Path::new("w"), Some("0199a213"), &TurnOptions::default());
        assert_eq!(c.args[c.args.len() - 3..], ["resume", "0199a213", "-"]);
        assert_eq!(c.stdin.as_deref(), Some("oi"));
        assert!(!c.args.iter().any(|a| a.contains("danger") || a == "--add-dir"));
    }

    #[test]
    fn login_status_messages() {
        // Mensagens de `codex-rs/cli/src/login.rs` (`run_login_status`).
        assert_eq!(parse_login_status(true, "\nLogged in using ChatGPT\n"), LoginStatus::LoggedIn);
        assert_eq!(parse_login_status(false, "\nNot logged in\n"), LoginStatus::LoggedOut);
        assert_eq!(parse_login_status(false, "Error checking login status: x"), LoginStatus::Unknown);
    }
}
