//! Provedor Claude Code (CLI oficial `claude`).

use super::provider::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// O parser guarda quais subagentes estão abertos (ids de tool_use, únicos
/// entre sessões), por isso o provedor pode ser compartilhado entre tarefas.
#[derive(Debug, Default, Clone)]
pub struct ClaudeProvider {
    parser: Arc<Mutex<StreamParser>>,
}

impl ClaudeProvider {
    pub fn new() -> Self {
        Self::default()
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
        // As pastas de anexos entram depois, com `--add-dir` (`attachments::apply_add_dirs`).
        c.arg("--permission-mode")
            .arg("acceptEdits")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .cwd(cwd)
            .stdin(prompt)
    }

    fn parse_line(&self, line: &str) -> Vec<AgentEvent> {
        self.parser.lock().unwrap_or_else(|e| e.into_inner()).parse(line)
    }

    fn begin_turn(&self, session: Option<&str>) {
        if let Some(s) = session {
            self.parser.lock().unwrap_or_else(|e| e.into_inner()).totals.remove(s);
        }
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

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

fn is_subagent_tool(name: &str) -> bool {
    matches!(name, "Task" | "Agent")
}

/// Junta os blocos de texto de um `content` (string ou lista de blocos),
/// sem o rodapé técnico "agentId: ..." que a CLI anexa para o modelo.
fn content_text(c: Option<&Value>) -> String {
    match c {
        Some(Value::String(t)) => t.clone(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .filter(|t| !t.starts_with("agentId:"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Parse de uma linha solta do `--output-format stream-json`, sem memória
/// entre linhas: marca os eventos internos e o início dos subagentes, mas não
/// reconhece o fim deles (isso exige o [`StreamParser`]).
pub fn parse_stream_json(line: &str) -> Vec<AgentEvent> {
    StreamParser::default().parse(line)
}

/// Parser com estado do stream-json do claude.
///
/// Formato conferido numa execução real (claude 2.1.220, fixture
/// `tests/fixtures/claude_subagent.jsonl`):
/// - o subagente nasce num bloco `tool_use` de nome `Agent` (versões antigas:
///   `Task`), com `input.{description, prompt, subagent_type}`; o `id` desse
///   bloco é o id do subagente;
/// - as mensagens `assistant`/`user` internas do subagente trazem
///   `parent_tool_use_id` igual a esse id (nulo no agente principal);
/// - o subagente termina no `tool_result` com o mesmo `tool_use_id`, numa
///   mensagem `user` do nível de quem o chamou; o topo dessa linha traz
///   `tool_use_result` com `status` e `content`.
///
/// Um mesmo processo pode imprimir MAIS DE UM `result`: quando um comando em
/// segundo plano termina depois da resposta, a CLI acorda sozinha, trabalha
/// mais um pouco e imprime outro `result`, cujo `modelUsage`/`total_cost_usd`
/// é o ACUMULADO do processo (visto numa execução real: 71055 tokens de cache
/// gravados no primeiro, 75008 no segundo). Por isso o parser guarda o último
/// total por sessão e emite só a diferença.
#[derive(Debug, Default)]
pub struct StreamParser {
    /// Subagentes abertos: id -> lançado em segundo plano (termina por
    /// `task_notification`, não pelo `tool_result`).
    open: HashMap<String, bool>,
    /// Último total acumulado informado por sessão, no processo em andamento
    /// (zerado por `begin_turn` quando um processo novo começa).
    totals: HashMap<String, SessionTotals>,
}

#[derive(Debug, Default)]
struct SessionTotals {
    /// Por modelo (None = uso agregado, sem modelo).
    models: HashMap<Option<String>, TokenUsage>,
    cost: f64,
}

/// `cur - prev`, se `cur` for o acumulado de `prev` (nenhum campo diminuiu);
/// senão `cur` como veio (processo novo).
fn usage_delta(cur: &TokenUsage, prev: Option<&TokenUsage>) -> TokenUsage {
    let Some(p) = prev else { return cur.clone() };
    let grew = cur.input_tokens >= p.input_tokens
        && cur.output_tokens >= p.output_tokens
        && cur.cache_read >= p.cache_read
        && cur.cache_write >= p.cache_write
        && cur.cost_usd.unwrap_or(0.0) + 1e-12 >= p.cost_usd.unwrap_or(0.0);
    if !grew {
        return cur.clone();
    }
    TokenUsage {
        input_tokens: cur.input_tokens - p.input_tokens,
        output_tokens: cur.output_tokens - p.output_tokens,
        cache_read: cur.cache_read - p.cache_read,
        cache_write: cur.cache_write - p.cache_write,
        model: cur.model.clone(),
        cost_usd: cur.cost_usd.map(|c| (c - p.cost_usd.unwrap_or(0.0)).max(0.0)),
    }
}

impl StreamParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(&mut self, line: &str) -> Vec<AgentEvent> {
        let line = line.trim();
        if line.is_empty() {
            return vec![];
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            return vec![AgentEvent::Raw(line.to_string())];
        };
        // Não nulo = a mensagem pertence ao subagente com esse id.
        let parent = v.get("parent_tool_use_id").and_then(Value::as_str);
        match v.get("type").and_then(Value::as_str) {
            Some("assistant") => {
                let Some(content) = v.pointer("/message/content").and_then(Value::as_array) else { return vec![] };
                let mut evs = Vec::new();
                for block in content {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            let t = s(block, "text");
                            if !t.is_empty() {
                                evs.push(AgentEvent::Text(t.to_string()).scoped(parent));
                            }
                        }
                        Some("tool_use") => {
                            let name = block.get("name").and_then(Value::as_str).unwrap_or("?");
                            let input = block.get("input").cloned().unwrap_or(Value::Null);
                            let start = match block.get("id").and_then(Value::as_str) {
                                Some(id) if is_subagent_tool(name) => {
                                    self.open.insert(id.to_string(), false);
                                    Some(AgentEvent::SubagentStart {
                                        id: id.to_string(),
                                        parent: parent.map(str::to_string),
                                        description: s(&input, "description").to_string(),
                                        subagent_type: s(&input, "subagent_type").to_string(),
                                        prompt: s(&input, "prompt").to_string(),
                                    })
                                }
                                _ => None,
                            };
                            evs.push(AgentEvent::ToolUse { name: name.to_string(), input }.scoped(parent));
                            evs.extend(start);
                        }
                        _ => {}
                    }
                }
                evs
            }
            // Mensagens `user` só interessam pelo `tool_result` que encerra um subagente.
            Some("user") => {
                let Some(content) = v.pointer("/message/content").and_then(Value::as_array) else { return vec![] };
                let tur = v.get("tool_use_result");
                let mut evs = Vec::new();
                for block in content {
                    if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                        continue;
                    }
                    let id = s(block, "tool_use_id");
                    if !self.open.contains_key(id) {
                        continue;
                    }
                    let status = tur.map(|t| s(t, "status")).unwrap_or_default();
                    let is_async = status.contains("async")
                        || tur.and_then(|t| t.get("isAsync")).and_then(Value::as_bool).unwrap_or(false);
                    if is_async {
                        // Lançado em segundo plano: continua aberto até o `task_notification`.
                        self.open.insert(id.to_string(), true);
                        continue;
                    }
                    self.open.remove(id);
                    let is_error = block.get("is_error").and_then(Value::as_bool).unwrap_or(false);
                    let mut result = content_text(tur.and_then(|t| t.get("content")));
                    if result.is_empty() {
                        result = content_text(block.get("content"));
                    }
                    evs.push(AgentEvent::SubagentEnd {
                        id: id.to_string(),
                        ok: !is_error && matches!(status, "" | "completed"),
                        result,
                    });
                }
                evs
            }
            Some("result") => {
                let mut evs = Vec::new();
                // Uso por modelo (preferido: já traz custo por modelo); senão o agregado.
                // Já inclui o consumo dos subagentes.
                let mut usages = Vec::new();
                if let Some(mu) = v.get("modelUsage").and_then(Value::as_object).filter(|m| !m.is_empty()) {
                    for (model, m) in mu {
                        usages.push(TokenUsage {
                            input_tokens: u(m, "inputTokens"),
                            output_tokens: u(m, "outputTokens"),
                            cache_read: u(m, "cacheReadInputTokens"),
                            cache_write: u(m, "cacheCreationInputTokens"),
                            model: Some(model.clone()),
                            cost_usd: m.get("costUSD").and_then(Value::as_f64),
                        });
                    }
                } else if let Some(us) = v.get("usage") {
                    usages.push(TokenUsage {
                        input_tokens: u(us, "input_tokens"),
                        output_tokens: u(us, "output_tokens"),
                        cache_read: u(us, "cache_read_input_tokens"),
                        cache_write: u(us, "cache_creation_input_tokens"),
                        model: None,
                        cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
                    });
                }
                // Os totais são acumulados no processo: desconta o que o `result`
                // anterior desta sessão já informou.
                let totals = self.totals.entry(s(&v, "session_id").to_string()).or_default();
                let usages: Vec<TokenUsage> = usages
                    .into_iter()
                    .map(|u| {
                        let delta = usage_delta(&u, totals.models.get(&u.model));
                        totals.models.insert(u.model.clone(), u);
                        delta
                    })
                    .collect();
                let cost = v.get("total_cost_usd").and_then(Value::as_f64).map(|c| {
                    let delta = if c + 1e-12 >= totals.cost { c - totals.cost } else { c };
                    totals.cost = c;
                    delta.max(0.0)
                });
                // Comando de barra que a própria CLI resolve (`/context`, `/cost`...): nenhum
                // modelo foi chamado, tudo vem zerado. Sem evento de uso, o turno não entra
                // nas métricas nem na contagem de turnos da tarefa.
                evs.extend(usages.into_iter().filter(|u| !u.is_empty()).map(AgentEvent::Usage));
                let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false);
                let sub_ok = v.get("subtype").and_then(Value::as_str).map_or(true, |s| s == "success");
                evs.push(AgentEvent::Result {
                    ok: !is_error && sub_ok,
                    summary: v.get("result").and_then(Value::as_str).unwrap_or_default().to_string(),
                    cost,
                });
                evs
            }
            Some("system") => match (v.get("subtype").and_then(Value::as_str), v.get("session_id").and_then(Value::as_str)) {
                (Some("init"), Some(sid)) => vec![AgentEvent::Session(sid.to_string())],
                // Fim de um subagente lançado em segundo plano.
                (Some("task_notification"), _) => {
                    let id = s(&v, "tool_use_id");
                    if self.open.get(id) == Some(&true) {
                        self.open.remove(id);
                        vec![AgentEvent::SubagentEnd {
                            id: id.to_string(),
                            ok: s(&v, "status") == "completed",
                            result: s(&v, "summary").to_string(),
                        }]
                    } else {
                        vec![]
                    }
                }
                // task_started/task_progress/task_updated e hooks: sem evento de UI.
                _ => vec![],
            },
            // rate_limit_event, stream_event: sem evento de UI.
            Some(_) => vec![],
            None => vec![AgentEvent::Raw(line.to_string())],
        }
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
        // Comando resolvido pela própria CLI (`/context`, `/cost`...): nenhum modelo
        // chamado, tudo zerado -> sem evento de uso.
        let evs = parse_stream_json(r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":412,"duration_api_ms":0,"num_turns":0,"result":"Context Usage","total_cost_usd":0,"usage":{"input_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":0},"modelUsage":{}}"#);
        assert_eq!(evs, vec![AgentEvent::Result { ok: true, summary: "Context Usage".into(), cost: Some(0.0) }]);
        // Sem modelUsage: usa o agregado.
        let evs = parse_stream_json(r#"{"type":"result","subtype":"error_max_turns","is_error":true,"usage":{"input_tokens":1,"output_tokens":2,"cache_read_input_tokens":3,"cache_creation_input_tokens":4}}"#);
        assert!(matches!(&evs[0], AgentEvent::Usage(u) if u.cache_read == 3 && u.cache_write == 4 && u.model.is_none()));
        assert!(matches!(&evs[1], AgentEvent::Result { ok: false, .. }));
    }

    /// Dois `result` no mesmo processo (a CLI acordou com o fim de um comando em
    /// segundo plano): o segundo traz o acumulado. Números de uma execução real.
    #[test]
    fn second_result_of_a_process_is_cumulative() {
        let first = r#"{"type":"result","subtype":"success","is_error":false,"session_id":"s1","result":"pronto.","total_cost_usd":0.144613,"modelUsage":{"claude-haiku-4-5-20251001":{"inputTokens":18,"outputTokens":497,"cacheReadInputTokens":0,"cacheCreationInputTokens":71055,"costUSD":0.144613}}}"#;
        let second = r#"{"type":"result","subtype":"success","is_error":false,"session_id":"s1","result":"Servidor iniciado.","total_cost_usd":0.1669749,"modelUsage":{"claude-haiku-4-5-20251001":{"inputTokens":44,"outputTokens":1080,"cacheReadInputTokens":115149,"cacheCreationInputTokens":75008,"costUSD":0.1669749}}}"#;
        let p = ClaudeProvider::new();
        let a = p.parse_line(first);
        assert!(matches!(&a[0], AgentEvent::Usage(u) if u.cache_write == 71055 && u.cost_usd == Some(0.144613)));
        let b = p.parse_line(second);
        match &b[0] {
            AgentEvent::Usage(u) => {
                assert_eq!((u.input_tokens, u.output_tokens, u.cache_read, u.cache_write), (26, 583, 115149, 3953));
                assert!((u.cost_usd.unwrap() - 0.0223619).abs() < 1e-9, "{:?}", u.cost_usd);
            }
            o => panic!("{o:?}"),
        }
        assert!(matches!(&b[1], AgentEvent::Result { cost: Some(c), .. } if (c - 0.0223619).abs() < 1e-9));
        // A soma dos eventos é o total real do processo, não o dobro.
        let total: f64 = [&a[0], &b[0]].iter().map(|e| match e { AgentEvent::Usage(u) => u.cost_usd.unwrap(), _ => 0.0 }).sum();
        assert!((total - 0.1669749).abs() < 1e-9);
        // Um terceiro `result` idêntico (nada novo) não gera uso.
        assert!(matches!(p.parse_line(second).as_slice(), [AgentEvent::Result { cost: Some(c), .. }] if *c == 0.0));
        // Processo novo da mesma sessão (`--resume`): os totais recomeçam do zero.
        p.begin_turn(Some("s1"));
        let c = p.parse_line(second);
        assert!(matches!(&c[0], AgentEvent::Usage(u) if u.cache_write == 75008 && u.cost_usd == Some(0.1669749)));
        // Outra sessão não é afetada pelos totais desta.
        let other = first.replace("\"s1\"", "\"s2\"");
        assert!(matches!(&p.parse_line(&other)[0], AgentEvent::Usage(u) if u.cache_write == 71055));
        // Sem `begin_turn`, um total menor que o anterior é tratado como processo novo.
        assert!(matches!(&p.parse_line(first)[0], AgentEvent::Usage(u) if u.cache_write == 71055));
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

    const FIXTURE: &str = include_str!("../../tests/fixtures/claude_subagent.jsonl");
    const SUB: &str = "toolu_01CsNFNQqgKLL88nwZezfZRE";

    /// Saída real (claude 2.1.220) de um turno que delega a listagem de arquivos a um subagente.
    #[test]
    fn real_subagent_stream() {
        let p = ClaudeProvider::new();
        let evs: Vec<AgentEvent> = FIXTURE.lines().flat_map(|l| p.parse_line(l)).collect();
        let kinds: Vec<&str> = evs.iter().map(|e| e.kind()).collect();
        assert_eq!(
            kinds,
            ["Session", "ToolUse", "SubagentStart", "ToolUse", "SubagentEnd", "Text", "Usage", "Result"],
            "{evs:#?}"
        );
        // O tool_use que cria o subagente continua aparecendo no chat principal.
        assert!(matches!(&evs[1], AgentEvent::ToolUse { name, .. } if name == "Agent"));
        match &evs[2] {
            AgentEvent::SubagentStart { id, parent, description, subagent_type, prompt } => {
                assert_eq!(id, SUB);
                assert_eq!(parent, &None);
                assert_eq!(description, "Listar arquivos da pasta");
                assert_eq!(subagent_type, "general-purpose");
                assert!(prompt.starts_with("Liste os arquivos da pasta"), "{prompt}");
            }
            o => panic!("{o:?}"),
        }
        // A ferramenta usada dentro do subagente vem marcada com o id dele.
        match &evs[3] {
            AgentEvent::Scoped { subagent, event } => {
                assert_eq!(subagent, SUB);
                assert!(matches!(&**event, AgentEvent::ToolUse { name, input } if name == "Glob" && input["pattern"] == "**/*"));
            }
            o => panic!("{o:?}"),
        }
        assert_eq!(evs[4], AgentEvent::SubagentEnd { id: SUB.into(), ok: true, result: "- alfa.txt\n- beta.md".into() });
        // O texto final é do agente principal (sem invólucro).
        assert_eq!(evs[5], AgentEvent::Text("A pasta contém dois arquivos: `alfa.txt` e `beta.md`.".into()));
        assert!(matches!(&evs[7], AgentEvent::Result { ok: true, .. }));
        let subs = subagents_of(&evs);
        assert_eq!(subs.len(), 1);
        assert!(subs[0].done && subs[0].ok == Some(true));
        assert!(p.parser.lock().unwrap().open.is_empty());
    }

    // Linhas sintéticas no mesmo formato da fixture real (subagente que cria outro).
    const N_A: &str = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"tA","name":"Task","input":{"description":"fora","prompt":"p1","subagent_type":"Explore"}}]},"parent_tool_use_id":null}"#;
    const N_TEXT: &str = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"vou delegar"},{"type":"tool_use","id":"tB","name":"Agent","input":{"description":"dentro","prompt":"p2","subagent_type":"general-purpose"}}]},"parent_tool_use_id":"tA"}"#;
    const N_INNER: &str = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"neto"}]},"parent_tool_use_id":"tB"}"#;
    const N_PROMPT: &str = r#"{"type":"user","message":{"content":[{"type":"text","text":"p2"}]},"parent_tool_use_id":"tB"}"#;
    const N_B_END: &str = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tB","is_error":true,"content":"falhou"}]},"parent_tool_use_id":"tA"}"#;
    const N_A_END: &str = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tA","content":[{"type":"text","text":"ok"},{"type":"text","text":"agentId: x"}]}]},"parent_tool_use_id":null,"tool_use_result":{"status":"completed","content":[{"type":"text","text":"ok"}]}}"#;

    #[test]
    fn nested_subagents() {
        let mut p = StreamParser::new();
        let evs: Vec<AgentEvent> =
            [N_A, N_TEXT, N_INNER, N_PROMPT, N_B_END, N_A_END].iter().flat_map(|l| p.parse(l)).collect();
        let tool = |name: &str, d: &str, pr: &str, t: &str| AgentEvent::ToolUse {
            name: name.into(),
            input: serde_json::json!({"description": d, "prompt": pr, "subagent_type": t}),
        };
        let start = |id: &str, parent: Option<&str>, d: &str, pr: &str, t: &str| AgentEvent::SubagentStart {
            id: id.into(),
            parent: parent.map(str::to_string),
            description: d.into(),
            subagent_type: t.into(),
            prompt: pr.into(),
        };
        assert_eq!(
            evs,
            vec![
                tool("Task", "fora", "p1", "Explore"),
                start("tA", None, "fora", "p1", "Explore"),
                AgentEvent::Text("vou delegar".into()).scoped(Some("tA")),
                tool("Agent", "dentro", "p2", "general-purpose").scoped(Some("tA")),
                start("tB", Some("tA"), "dentro", "p2", "general-purpose"),
                AgentEvent::Text("neto".into()).scoped(Some("tB")),
                AgentEvent::SubagentEnd { id: "tB".into(), ok: false, result: "falhou".into() },
                AgentEvent::SubagentEnd { id: "tA".into(), ok: true, result: "ok".into() },
            ]
        );
        let subs = subagents_of(&evs);
        assert_eq!(subs[1].parent.as_deref(), Some("tA"));
        assert_eq!(subs[1].ok, Some(false));
        // tool_result de subagente já encerrado (ou de ferramenta comum) não gera evento.
        assert!(p.parse(N_A_END).is_empty());
    }

    #[test]
    fn background_subagent_ends_on_notification() {
        let mut p = StreamParser::new();
        p.parse(N_A);
        let launched = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tA","content":"em segundo plano"}]},"parent_tool_use_id":null,"tool_use_result":{"status":"async_launched","isAsync":true}}"#;
        assert!(p.parse(launched).is_empty());
        let done = r#"{"type":"system","subtype":"task_notification","tool_use_id":"tA","status":"completed","summary":"feito"}"#;
        assert_eq!(p.parse(done), vec![AgentEvent::SubagentEnd { id: "tA".into(), ok: true, result: "feito".into() }]);
        assert!(p.parse(done).is_empty());
    }
}
