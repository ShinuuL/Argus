//! Provedor OpenCode (CLI oficial `opencode`).
//!
//! Formato conferido com uma execução real de `opencode run --format json`
//! (prompt pela entrada padrão): um objeto por linha com `type` em
//! `step_start` | `text` | `tool_use` | `step_finish` | `error`, `sessionID`
//! no topo e os dados em `part`. `step_finish` traz `reason` ("tool-calls" nos
//! passos intermediários, "stop" no fim), `cost` e `tokens`.

use super::provider::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct OpencodeProvider {
    /// Modelo inicial das conversas novas (`provedor/modelo`). O modelo de cada
    /// turno vem da tarefa (`TurnOptions::model`).
    pub model: Option<String>,
}

impl OpencodeProvider {
    pub fn new() -> Self {
        Self { model: None }
    }
    pub fn with_model(model: impl Into<String>) -> Self {
        Self { model: Some(model.into()) }
    }
}

impl AgentProvider for OpencodeProvider {
    fn id(&self) -> &str {
        "opencode"
    }
    fn display_name(&self) -> &str {
        "OpenCode"
    }
    fn detect(&self) -> Option<PathBuf> {
        which_bin("opencode")
    }

    fn login_status(&self) -> LoginStatus {
        let Some(bin) = self.detect() else { return LoginStatus::NotInstalled };
        match run_capture(&bin, &["auth", "list"]) {
            Some((true, out)) => parse_auth_list(&out),
            _ => LoginStatus::Unknown,
        }
    }

    fn login_command(&self) -> CommandSpec {
        CommandSpec::new("opencode").arg("auth").arg("login")
    }

    fn task_command(&self, prompt: &str, cwd: &Path, session: Option<&str>, opts: &TurnOptions) -> CommandSpec {
        // Sem argumento de mensagem, `opencode run` lê o prompt da entrada padrão.
        // A pasta da sessão é a do processo; a CLI confia na variável `PWD` quando ela
        // existe, por isso `build_command` a acerta junto com o diretório.
        let mut c = CommandSpec::new("opencode").arg("run").arg("--format").arg("json");
        if let Some(m) = opts.model.as_deref().filter(|m| valid_model_name(m)) {
            c = c.arg("-m").arg(m);
        }
        if let Some(s) = session {
            c = c.arg("--session").arg(s);
        }
        // O `opencode run` não tem opção de prompt de sistema (conferido no `--help`
        // da 2.3.0): o aviso de ambiente vai na frente do primeiro pedido da sessão.
        let text = match opts.system_note.as_deref().filter(|n| !n.is_empty() && session.is_none()) {
            Some(n) => format!("[Aviso do ambiente] {n}\n\n{prompt}"),
            None => prompt.to_string(),
        };
        c.cwd(cwd).stdin(text)
    }

    /// O modelo das linhas de uso é preenchido pelo orquestrador (o da tarefa).
    fn parse_line(&self, line: &str) -> Vec<AgentEvent> {
        parse_json_event(line, None)
    }

    fn default_model(&self) -> Option<String> {
        self.model.clone().filter(|m| valid_model_name(m))
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        let bin = self.detect().ok_or_else(|| "A CLI opencode não está instalada.".to_string())?;
        match run_capture_stdout(&bin, &["models"]) {
            Some((true, out)) => Ok(parse_models(&out)),
            Some((false, _)) => Err("O comando opencode models falhou.".into()),
            None => Err("Não foi possível executar o opencode.".into()),
        }
    }
}

/// Saída de `opencode models`: um `provedor/modelo` por linha.
pub fn parse_models(out: &str) -> Vec<ModelInfo> {
    let mut seen = std::collections::HashSet::new();
    strip_ansi(out)
        .lines()
        .map(str::trim)
        .filter(|l| l.contains('/') && valid_model_name(l))
        .filter(|l| seen.insert(l.to_string()))
        .map(|l| {
            let (prov, name) = l.split_once('/').unwrap_or(("", l));
            ModelInfo { value: l.to_string(), label: name.to_string(), hint: prov.to_string() }
        })
        .collect()
}

/// Interpreta `opencode auth list`: só conta as linhas de credenciais
/// ("<provedor>  <método>  stored"); nenhum segredo é lido.
pub fn parse_auth_list(out: &str) -> LoginStatus {
    let clean = strip_ansi(out);
    let n = clean
        .lines()
        .filter(|l| {
            let l = l.to_ascii_lowercase();
            l.split_whitespace().any(|w| w == "stored") || l.contains("oauth") || l.contains("api key")
        })
        .count();
    if n > 0 {
        LoginStatus::LoggedIn
    } else {
        LoginStatus::LoggedOut
    }
}

fn u(v: Option<&Value>) -> u64 {
    v.and_then(Value::as_u64).unwrap_or(0)
}

/// Parse do `opencode run --format json` (um evento JSON por linha).
pub fn parse_json_event(line: &str, model: Option<&str>) -> Vec<AgentEvent> {
    let line = line.trim();
    if line.is_empty() {
        return vec![];
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![AgentEvent::Raw(line.to_string())];
    };
    let part = v.get("part").unwrap_or(&v);
    match v.get("type").and_then(Value::as_str) {
        Some("step_start") => v
            .get("sessionID")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|s| vec![AgentEvent::Session(s.to_string())])
            .unwrap_or_default(),
        Some("text") => part
            .get("text")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(|t| vec![AgentEvent::Text(t.to_string())])
            .unwrap_or_default(),
        Some("tool_use") | Some("tool") => {
            let name = part.get("tool").or_else(|| part.get("name")).and_then(Value::as_str).unwrap_or("?").to_string();
            let input = part.pointer("/state/input").or_else(|| part.get("input")).cloned().unwrap_or(Value::Null);
            let mut evs = Vec::new();
            // Ferramenta "task" = subagente (sessão filha). O `run --format json` só
            // imprime a ferramenta já concluída e não repassa os eventos internos da
            // sessão filha, então início e fim saem juntos.
            let sub = (name == "task")
                .then(|| part.get("callID").or_else(|| part.get("id")).and_then(Value::as_str))
                .flatten()
                .map(str::to_string);
            let st = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
            let start = sub.as_ref().map(|id| AgentEvent::SubagentStart {
                id: id.clone(),
                parent: None,
                description: st("description"),
                subagent_type: st("subagent_type"),
                prompt: st("prompt"),
            });
            evs.push(AgentEvent::ToolUse { name, input });
            evs.extend(start);
            if let Some(id) = sub {
                let state = part.get("state");
                let status = state.and_then(|s| s.get("status")).and_then(Value::as_str).unwrap_or("completed");
                if status == "completed" || status == "error" {
                    let result = state
                        .and_then(|s| s.get(if status == "error" { "error" } else { "output" }))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    evs.push(AgentEvent::SubagentEnd { id, ok: status == "completed", result });
                }
            }
            evs
        }
        Some("step_finish") => {
            let t = part.get("tokens");
            let mut evs = vec![AgentEvent::Usage(TokenUsage {
                input_tokens: u(t.and_then(|t| t.get("input"))),
                // Tokens de raciocínio são cobrados como saída.
                output_tokens: u(t.and_then(|t| t.get("output"))) + u(t.and_then(|t| t.get("reasoning"))),
                cache_read: u(t.and_then(|t| t.pointer("/cache/read"))),
                cache_write: u(t.and_then(|t| t.pointer("/cache/write"))),
                model: model.map(str::to_string),
                cost_usd: part.get("cost").and_then(Value::as_f64),
            })];
            let reason = part.get("reason").and_then(Value::as_str).unwrap_or_default();
            if reason != "tool-calls" {
                evs.push(AgentEvent::Result {
                    ok: part.get("error").is_none(),
                    summary: reason.to_string(),
                    cost: part.get("cost").and_then(Value::as_f64),
                });
            }
            evs
        }
        Some("error") => {
            let e = v.get("error");
            let msg = e
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| e.map(|e| e.to_string()))
                .unwrap_or_default();
            vec![AgentEvent::Result { ok: false, summary: msg, cost: None }]
        }
        Some(_) => vec![],
        None => vec![AgentEvent::Raw(line.to_string())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Linhas reais (encurtadas) de `opencode run --format json`.
    const START: &str = r#"{"type":"step_start","timestamp":1,"sessionID":"ses_e288325f7ffe","part":{"id":"prt_1","sessionID":"ses_e288325f7ffe","messageID":"msg_1","type":"step-start"}}"#;
    const TOOL: &str = r#"{"type":"tool_use","timestamp":2,"sessionID":"ses_e288325f7ffe","part":{"type":"tool","id":"call_3","tool":"patch","state":{"status":"completed","input":{"patchText":"*** Begin Patch"},"output":"Success."}}}"#;
    const STEP_TOOLS: &str = r#"{"type":"step_finish","timestamp":3,"sessionID":"ses_e288325f7ffe","part":{"type":"step-finish","reason":"tool-calls","cost":0,"tokens":{"input":11182,"output":36,"reasoning":114,"cache":{"read":0,"write":0}}}}"#;
    const TEXT: &str = r#"{"type":"text","timestamp":4,"sessionID":"ses_e288325f7ffe","part":{"type":"text","text":"pronto","time":{"start":1,"end":2}}}"#;
    const STEP_STOP: &str = r#"{"type":"step_finish","timestamp":5,"sessionID":"ses_e288325f7ffe","part":{"type":"step-finish","reason":"stop","cost":0,"tokens":{"input":344,"output":6,"reasoning":0,"cache":{"read":11008,"write":0}}}}"#;
    const ERR: &str = r#"{"type":"error","timestamp":6,"sessionID":"ses_x","error":{"type":"provider.free-limit","message":"Free usage limit reached.","status":429}}"#;

    #[test]
    fn parses_real_stream() {
        assert_eq!(parse_json_event(START, None), vec![AgentEvent::Session("ses_e288325f7ffe".into())]);
        assert!(matches!(
            parse_json_event(TOOL, None).as_slice(),
            [AgentEvent::ToolUse { name, input }] if name == "patch" && input["patchText"] == "*** Begin Patch"
        ));
        let e = parse_json_event(STEP_TOOLS, Some("openai/x"));
        assert_eq!(e.len(), 1);
        assert!(matches!(&e[0], AgentEvent::Usage(u) if u.input_tokens == 11182 && u.output_tokens == 150 && u.model.as_deref() == Some("openai/x")));
        assert_eq!(parse_json_event(TEXT, None), vec![AgentEvent::Text("pronto".into())]);
        let e = parse_json_event(STEP_STOP, None);
        assert_eq!(e.len(), 2);
        assert!(matches!(&e[0], AgentEvent::Usage(u) if u.cache_read == 11008 && u.cost_usd == Some(0.0)));
        assert!(matches!(&e[1], AgentEvent::Result { ok: true, summary, .. } if summary == "stop"));
        assert!(matches!(
            parse_json_event(ERR, None).as_slice(),
            [AgentEvent::Result { ok: false, summary, .. }] if summary == "Free usage limit reached."
        ));
        assert_eq!(parse_json_event("log solto", None), vec![AgentEvent::Raw("log solto".into())]);
    }

    // Linha real (opencode 2.0.22, `run --format json`, encurtada): o id da
    // chamada vem em `part.id` ("call_..."), e existe também `part.partID`.
    const READ: &str = r#"{"type":"tool_use","timestamp":1794677373580,"sessionID":"ses_e24e4b9b4ffe","part":{"partID":"prt_1db1b7974001","sessionID":"ses_e24e4b9b4ffe","messageID":"msg_1db1b738c001","type":"tool","id":"call_ff5bd8c5844a","tool":"read","state":{"status":"completed","input":{"path":"README.md","limit":20},"output":"Read file README.md, lines 1-3","title":"read","time":{"start":1,"end":2}}}}"#;

    #[test]
    fn parses_real_tool_line_of_v2() {
        assert!(matches!(
            parse_json_event(READ, Some("openai/x")).as_slice(),
            [AgentEvent::ToolUse { name, input }] if name == "read" && input["path"] == "README.md" && input["limit"] == 20
        ));
        // A mesma forma com a ferramenta "task": o subagente recebe o id da chamada (`part.id`).
        let task = READ.replace(r#""tool":"read""#, r#""tool":"task""#);
        let e = parse_json_event(&task, None);
        assert!(matches!(&e[1], AgentEvent::SubagentStart { id, .. } if id == "call_ff5bd8c5844a"), "{e:?}");
        assert!(matches!(&e[2], AgentEvent::SubagentEnd { id, ok: true, .. } if id == "call_ff5bd8c5844a"));
    }

    // Linha SINTÉTICA (formato da ferramenta "task" segundo o código-fonte da CLI;
    // não conferida numa execução real).
    const TASK: &str = r#"{"type":"tool_use","timestamp":2,"sessionID":"ses_1","part":{"type":"tool","id":"prt_9","callID":"call_7","tool":"task","state":{"status":"completed","input":{"description":"Listar","prompt":"liste","subagent_type":"general"},"output":"a.txt","metadata":{"sessionId":"ses_filha"}}}}"#;

    #[test]
    fn task_tool_becomes_subagent() {
        let e = parse_json_event(TASK, None);
        assert_eq!(e.len(), 3);
        assert!(matches!(&e[0], AgentEvent::ToolUse { name, .. } if name == "task"));
        assert_eq!(
            e[1],
            AgentEvent::SubagentStart {
                id: "call_7".into(),
                parent: None,
                description: "Listar".into(),
                subagent_type: "general".into(),
                prompt: "liste".into(),
            }
        );
        assert_eq!(e[2], AgentEvent::SubagentEnd { id: "call_7".into(), ok: true, result: "a.txt".into() });
        // Outras ferramentas continuam gerando um único evento.
        assert_eq!(parse_json_event(TOOL, None).len(), 1);
    }

    #[test]
    fn command_uses_stdin() {
        // O modelo vem da tarefa; o do provedor é só o valor inicial das conversas novas.
        let p = OpencodeProvider::with_model("openai/gpt");
        assert_eq!(p.default_model().as_deref(), Some("openai/gpt"));
        let opts = TurnOptions { model: Some("google/gemini".into()), system_note: Some("cópia isolada".into()) };
        let c = p.task_command("a \"b\" %PATH%\nc", Path::new("."), Some("ses_1"), &opts);
        assert_eq!(c.args, ["run", "--format", "json", "-m", "google/gemini", "--session", "ses_1"]);
        assert_eq!(c.cwd.as_deref(), Some(Path::new(".")));
        // Sessão existente: o aviso de ambiente já foi dado no primeiro pedido.
        assert_eq!(c.stdin.as_deref(), Some("a \"b\" %PATH%\nc"));
        let c = p.task_command("oi", Path::new("."), None, &opts);
        assert_eq!(c.stdin.as_deref(), Some("[Aviso do ambiente] cópia isolada\n\noi"));
        // Sem modelo na tarefa, vale o padrão da CLI (nenhum `-m`); nome inválido é ignorado.
        let c = p.task_command("oi", Path::new("."), None, &TurnOptions::default());
        assert_eq!(c.args, ["run", "--format", "json"]);
        let bad = TurnOptions { model: Some("--auto".into()), ..Default::default() };
        assert_eq!(p.task_command("oi", Path::new("."), None, &bad).args, ["run", "--format", "json"]);
    }

    #[test]
    fn models_list() {
        let out = "\u{1b}[90mgoogle/gemini-2.5-pro\u{1b}[0m\nopenai/gpt-6-luna\n\nopenai/gpt-6-luna\naviso solto\n";
        let m = parse_models(out);
        assert_eq!(m.len(), 2);
        assert_eq!((m[0].value.as_str(), m[0].label.as_str(), m[0].hint.as_str()), ("google/gemini-2.5-pro", "gemini-2.5-pro", "google"));
    }

    #[test]
    fn auth_list() {
        let out = "\u{1b}[90m┌  Credentials\u{1b}[0m\n│\n●  Google            API key                     stored\n●  OpenAI            OAuth                       stored\n└  2 credentials\n";
        assert_eq!(parse_auth_list(out), LoginStatus::LoggedIn);
        assert_eq!(parse_auth_list("┌  Credentials\n└  0 credentials\n"), LoginStatus::LoggedOut);
    }
}
