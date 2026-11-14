//! Provedor Claude Code (CLI oficial `claude`).

use super::provider::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct ClaudeProvider;

impl ClaudeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl AgentProvider for ClaudeProvider {
    fn id(&self) -> &str {
        "claude"
    }
    fn display_name(&self) -> &str {
        "Claude Code"
    }
    fn detect(&self) -> Option<PathBuf> {
        which_bin("claude")
    }

    fn login_status(&self) -> LoginStatus {
        let Some(bin) = self.detect() else { return LoginStatus::NotInstalled };
        if let Some((_, out)) = run_capture(&bin, &["auth", "status"]) {
            if let Some(b) = parse_auth_status(&out) {
                return if b { LoginStatus::LoggedIn } else { LoginStatus::LoggedOut };
            }
        }
        // Fallback: verifica só a EXISTÊNCIA do arquivo, nunca lê o conteúdo.
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
        match home {
            Some(ref h) if PathBuf::from(h).join(".claude").join(".credentials.json").exists() => {
                LoginStatus::LoggedIn
            }
            _ => LoginStatus::Unknown,
        }
    }

    fn login_command(&self) -> CommandSpec {
        CommandSpec::new("claude").arg("auth").arg("login")
    }

    fn task_command(&self, prompt: &str, cwd: &Path, session: Option<&str>) -> CommandSpec {
        // `claude -p` sem argumento de prompt lê o prompt da entrada padrão.
        let mut c = CommandSpec::new("claude").arg("-p");
        if let Some(s) = session {
            c = c.arg("--resume").arg(s);
        }
        // Em `-p` não há quem aprove permissões: sem isto toda edição de arquivo
        // é negada. `acceptEdits` libera só edições (Bash continua bloqueado).
        c.arg("--permission-mode")
            .arg("acceptEdits")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .cwd(cwd)
            .stdin(prompt)
    }

    fn parse_line(&self, line: &str) -> Vec<AgentEvent> {
        parse_stream_json(line)
    }
}

/// Lê `loggedIn` da saída JSON de `claude auth status` (só o booleano).
fn parse_auth_status(out: &str) -> Option<bool> {
    out.lines()
        .chain(std::iter::once(out))
        .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .find_map(|v| v.get("loggedIn").and_then(Value::as_bool))
}

fn u(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

/// Parse de uma linha do `--output-format stream-json`.
pub fn parse_stream_json(line: &str) -> Vec<AgentEvent> {
    let line = line.trim();
    if line.is_empty() {
        return vec![];
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![AgentEvent::Raw(line.to_string())];
    };
    match v.get("type").and_then(Value::as_str) {
        Some("assistant") => {
            let Some(content) = v.pointer("/message/content").and_then(Value::as_array) else { return vec![] };
            content
                .iter()
                .filter_map(|block| match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        let t = block.get("text").and_then(Value::as_str).unwrap_or_default();
                        (!t.is_empty()).then(|| AgentEvent::Text(t.to_string()))
                    }
                    Some("tool_use") => Some(AgentEvent::ToolUse {
                        name: block.get("name").and_then(Value::as_str).unwrap_or("?").to_string(),
                        input: block.get("input").cloned().unwrap_or(Value::Null),
                    }),
                    _ => None,
                })
                .collect()
        }
        Some("result") => {
            let mut evs = Vec::new();
            // Uso por modelo (preferido: já traz custo por modelo); senão o agregado.
            if let Some(mu) = v.get("modelUsage").and_then(Value::as_object).filter(|m| !m.is_empty()) {
                for (model, m) in mu {
                    evs.push(AgentEvent::Usage(TokenUsage {
                        input_tokens: u(m, "inputTokens"),
                        output_tokens: u(m, "outputTokens"),
                        cache_read: u(m, "cacheReadInputTokens"),
                        cache_write: u(m, "cacheCreationInputTokens"),
                        model: Some(model.clone()),
                        cost_usd: m.get("costUSD").and_then(Value::as_f64),
                    }));
                }
            } else if let Some(us) = v.get("usage") {
                evs.push(AgentEvent::Usage(TokenUsage {
                    input_tokens: u(us, "input_tokens"),
                    output_tokens: u(us, "output_tokens"),
                    cache_read: u(us, "cache_read_input_tokens"),
                    cache_write: u(us, "cache_creation_input_tokens"),
                    model: None,
                    cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
                }));
            }
            let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false);
            let sub_ok = v.get("subtype").and_then(Value::as_str).map_or(true, |s| s == "success");
            evs.push(AgentEvent::Result {
                ok: !is_error && sub_ok,
                summary: v.get("result").and_then(Value::as_str).unwrap_or_default().to_string(),
                cost: v.get("total_cost_usd").and_then(Value::as_f64),
            });
            evs
        }
        Some("system") => match (v.get("subtype").and_then(Value::as_str), v.get("session_id").and_then(Value::as_str)) {
            (Some("init"), Some(s)) => vec![AgentEvent::Session(s.to_string())],
            _ => vec![],
        },
        // user (tool_result), rate_limit_event, stream_event: sem evento de UI.
        Some(_) => vec![],
        None => vec![AgentEvent::Raw(line.to_string())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INIT: &str = r#"{"type":"system","subtype":"init","cwd":"C:\\x","session_id":"0730ecc5-b2da","model":"claude-haiku-4-5-20251001"}"#;
    const HOOK: &str = r#"{"type":"system","subtype":"hook_started","session_id":"0730ecc5-b2da"}"#;
    const MIXED: &str = r#"{"type":"assistant","message":{"model":"m","content":[{"type":"thinking","thinking":"..."},{"type":"text","text":"Vou listar"},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}],"usage":{"input_tokens":9}},"session_id":"s"}"#;
    // Recortado de uma execução real (claude 2.1.x, `--output-format stream-json --verbose`).
    const RESULT: &str = r#"{"is_error":false,"num_turns":1,"stop_reason":"end_turn","session_id":"0730ecc5-b2da","total_cost_usd":0.070944,"usage":{"input_tokens":9,"cache_creation_input_tokens":35305,"cache_read_input_tokens":0,"output_tokens":65},"modelUsage":{"claude-haiku-4-5-20251001":{"inputTokens":9,"outputTokens":65,"cacheReadInputTokens":0,"cacheCreationInputTokens":35305,"costUSD":0.070944}},"subtype":"success","result":"oi \"aspas\" 100%","type":"result"}"#;

    #[test]
    fn parses_fixtures() {
        let p = ClaudeProvider::new();
        assert_eq!(p.parse_line(INIT), vec![AgentEvent::Session("0730ecc5-b2da".into())]);
        assert!(p.parse_line(HOOK).is_empty());
        let evs = p.parse_line(MIXED);
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0], AgentEvent::Text("Vou listar".into()));
        match &evs[1] {
            AgentEvent::ToolUse { name, input } => {
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "ls");
            }
            o => panic!("{o:?}"),
        }
        assert!(p.parse_line(r#"{"type":"user","message":{"content":[{"type":"tool_result"}]}}"#).is_empty());
        assert!(p.parse_line(r#"{"type":"rate_limit_event"}"#).is_empty());
        assert_eq!(p.parse_line("texto solto"), vec![AgentEvent::Raw("texto solto".into())]);
        assert!(p.parse_line("  ").is_empty());
    }

    #[test]
    fn result_with_usage() {
        let evs = parse_stream_json(RESULT);
        assert_eq!(evs.len(), 2);
        assert_eq!(
            evs[0],
            AgentEvent::Usage(TokenUsage {
                input_tokens: 9,
                output_tokens: 65,
                cache_read: 0,
                cache_write: 35305,
                model: Some("claude-haiku-4-5-20251001".into()),
                cost_usd: Some(0.070944),
            })
        );
        assert_eq!(evs[1], AgentEvent::Result { ok: true, summary: "oi \"aspas\" 100%".into(), cost: Some(0.070944) });
        // Sem modelUsage: usa o agregado.
        let evs = parse_stream_json(r#"{"type":"result","subtype":"error_max_turns","is_error":true,"usage":{"input_tokens":1,"output_tokens":2,"cache_read_input_tokens":3,"cache_creation_input_tokens":4}}"#);
        assert!(matches!(&evs[0], AgentEvent::Usage(u) if u.cache_read == 3 && u.cache_write == 4 && u.model.is_none()));
        assert!(matches!(&evs[1], AgentEvent::Result { ok: false, .. }));
    }

    #[test]
    fn task_command_shape() {
        let c = ClaudeProvider::new().task_command("oi \"x\" 100%\nlinha", Path::new("."), None);
        let tail = ["--permission-mode", "acceptEdits", "--output-format", "stream-json", "--verbose"];
        assert_eq!(c.args, [&["-p"][..], &tail].concat());
        assert_eq!(c.stdin.as_deref(), Some("oi \"x\" 100%\nlinha"));
        let c = ClaudeProvider::new().task_command("e agora?", Path::new("."), Some("abc"));
        assert_eq!(c.args, [&["-p", "--resume", "abc"][..], &tail].concat());
    }

    #[test]
    fn auth_status() {
        assert_eq!(parse_auth_status("{\"loggedIn\": true, \"authMethod\":\"x\"}\n"), Some(true));
        assert_eq!(parse_auth_status("{\n \"loggedIn\": false\n}"), Some(false));
        assert_eq!(parse_auth_status("lixo"), None);
    }
}
