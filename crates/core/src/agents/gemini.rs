//! Provedor Gemini (Google Gemini CLI, pacote npm `@google/gemini-cli`, binário `gemini`).
//!
//! Login pela própria CLI: `gemini` interativo, opção "Login with Google" (ou
//! `/auth`). A chave do Google AI Studio (`GEMINI_API_KEY`), se o usuário quiser,
//! é configurada por ele na CLI; o Argus não a lê nem a pede.
//!
//! Turno: `gemini --output-format stream-json --approval-mode auto_edit
//! --skip-trust [-m modelo] [--resume <session_id>]`, prompt pela entrada padrão.
//! Fontes (repositório github.com/google-gemini/gemini-cli):
//! - `docs/cli/headless.md`: modo sem interface quando a entrada não é um
//!   terminal ou com `-p`; formatos `json` e `stream-json`; eventos `init`,
//!   `message`, `tool_use`, `tool_result`, `error`, `result`;
//! - `packages/core/src/output/types.ts`: campos de cada evento e `StreamStats`;
//! - `packages/cli/src/config/config.ts`: `-m/--model`, `--approval-mode`
//!   (`default|auto_edit|yolo|plan`), `--resume` (`latest`, índice ou UUID),
//!   `--skip-trust` (confia na pasta só nesta sessão; sem isso, pasta não
//!   confiável força `default`), `--list-models`; e, sem interface, decisões
//!   "perguntar" viram "negar" (Policy Engine);
//! - `packages/cli/src/commands/models/list.ts`: saída JSON de `--list-models`.
//!
//! Permissões em `auto_edit` sem interface: edições de arquivo aprovadas sozinhas
//! (ferramentas de arquivo da CLI só alcançam a pasta de trabalho e as
//! `--include-directories`, que o Argus só passa para a pasta `_Argus` da memória); shell e qualquer outra
//! ferramenta que pediria aprovação é NEGADA (modo "Restrito").
//! No modo "Auto" (`permissions::apply`) vira `--approval-mode yolo` com
//! `--admin-policy <arquivo>`: regras `deny` de `run_shell_command` para os
//! comandos nocivos (`crate::harmful`). A política admin fica no nível 5.x,
//! acima do "permitir tudo" do yolo (1.998; `packages/core/src/policy/policies/yolo.toml`).
//! O arquivo é do Argus (pasta temporária), sem tocar no `~/.gemini`. Se não
//! der para gravar o arquivo, o turno segue em `auto_edit`.

use super::provider::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct GeminiProvider;

impl GeminiProvider {
    pub fn new() -> Self {
        Self
    }
}

/// Lista curta para quando `--list-models` falhar. Apelidos e nomes de
/// `packages/core/src/config/models.ts` (consultado em 2026-11-16).
const KNOWN_MODELS: &[(&str, &str)] = &[
    ("auto", "Automático"),
    ("pro", "Pro (apelido)"),
    ("flash", "Flash (apelido)"),
    ("flash-lite", "Flash-Lite (apelido)"),
    ("gemini-3.1-pro-preview", "Gemini 3.1 Pro (preview)"),
    ("gemini-3.8-flash", "Gemini 3.8 Flash"),
];

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

impl AgentProvider for GeminiProvider {
    fn id(&self) -> &str {
        "gemini"
    }
    fn display_name(&self) -> &str {
        "Gemini"
    }
    fn detect(&self) -> Option<PathBuf> {
        which_bin("gemini")
    }

    /// A CLI não tem comando de status. Lê só o TIPO de login escolhido em
    /// `~/.gemini/settings.json` (`security.auth.selectedType`, não é segredo) e
    /// confere a EXISTÊNCIA do arquivo de credenciais do login Google ou da
    /// variável `GEMINI_API_KEY` (nunca o conteúdo).
    fn login_status(&self) -> LoginStatus {
        if self.detect().is_none() {
            return LoginStatus::NotInstalled;
        }
        let Some(h) = home() else { return LoginStatus::Unknown };
        let dir = h.join(".gemini");
        let settings = std::fs::read_to_string(dir.join("settings.json")).ok();
        auth_status(
            settings.as_deref(),
            dir.join("oauth_creds.json").exists(),
            std::env::var_os("GEMINI_API_KEY").is_some() || dir.join(".env").exists(),
        )
    }

    fn login_command(&self) -> CommandSpec {
        // Abre a interface da CLI, que pede o login quando não há nenhum.
        CommandSpec::new("gemini")
    }

    fn task_command(&self, prompt: &str, cwd: &Path, session: Option<&str>, opts: &TurnOptions) -> CommandSpec {
        let mut c = CommandSpec::new("gemini")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--approval-mode")
            .arg(match opts.access {
                crate::permissions::AccessMode::Plan => "plan",
                crate::permissions::AccessMode::Bypass => "yolo",
                crate::permissions::AccessMode::Normal => "auto_edit",
            })
            .arg("--skip-trust");
        if let Some(m) = opts.model.as_deref().filter(|m| valid_model_name(m)) {
            c = c.arg("-m").arg(m);
        }
        if let Some(s) = session.filter(|s| valid_model_name(s)) {
            c = c.arg("--resume").arg(s);
        }
        // Sem opção de prompt de sistema: o aviso vai na frente do primeiro pedido.
        let text = match opts.system_note.as_deref().filter(|n| !n.is_empty() && session.is_none()) {
            Some(n) => format!("[Aviso do ambiente] {n}\n\n{prompt}"),
            None => prompt.to_string(),
        };
        c.cwd(cwd).stdin(text)
    }

    fn parse_line(&self, line: &str) -> Vec<AgentEvent> {
        parse_json_event(line)
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        let fixed = || {
            KNOWN_MODELS
                .iter()
                .map(|(v, l)| ModelInfo::new(*v, *l, "lista fixa do Argus"))
                .collect::<Vec<_>>()
        };
        let Some(bin) = self.detect() else { return Err("A CLI gemini não está instalada.".into()) };
        match run_capture_stdout(&bin, &["--list-models", "--output-format", "json"]) {
            Some((true, out)) => {
                let list = parse_models(&out);
                Ok(if list.is_empty() { fixed() } else { list })
            }
            _ => Ok(fixed()),
        }
    }

    fn turn_tracker(&self) -> Box<dyn TurnTracker> {
        Box::new(GeminiTracker::default())
    }
}

/// Estado de login a partir do `settings.json` (texto) e da existência das credenciais.
pub fn auth_status(settings: Option<&str>, has_oauth_file: bool, has_key: bool) -> LoginStatus {
    let v: Value = settings.and_then(|s| serde_json::from_str(s).ok()).unwrap_or(Value::Null);
    let kind = v
        .pointer("/security/auth/selectedType")
        .or_else(|| v.get("selectedAuthType"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    match kind {
        "oauth-personal" => if has_oauth_file { LoginStatus::LoggedIn } else { LoginStatus::LoggedOut },
        "gemini-api-key" => if has_key { LoginStatus::LoggedIn } else { LoginStatus::LoggedOut },
        "" if has_oauth_file || has_key => LoginStatus::LoggedIn,
        "" => LoginStatus::LoggedOut,
        // vertex-ai, cloud-shell e outros: depende do ambiente.
        _ => LoginStatus::Unknown,
    }
}

/// Saída JSON de `gemini --list-models --output-format json`.
pub fn parse_models(out: &str) -> Vec<ModelInfo> {
    let start = out.find('[').unwrap_or(0);
    let Ok(Value::Array(a)) = serde_json::from_str::<Value>(out[start..].trim()) else { return vec![] };
    a.iter()
        .filter_map(|m| {
            let id = m.get("modelId").or_else(|| m.get("id")).and_then(Value::as_str)?;
            if !valid_model_name(id) {
                return None;
            }
            let label = m.get("displayName").and_then(Value::as_str).unwrap_or(id).to_string();
            let ctx = m.pointer("/contextLimits/inputTokens").and_then(Value::as_u64);
            let desc = m.get("description").and_then(Value::as_str).unwrap_or_default();
            let hint = match ctx {
                Some(c) if desc.is_empty() => format!("{c} tokens de contexto"),
                Some(c) => format!("{desc} · {c} tokens de contexto"),
                None => desc.to_string(),
            };
            Some(ModelInfo::new(id, label, hint))
        })
        .collect()
}

fn u(v: Option<&Value>) -> u64 {
    v.and_then(Value::as_u64).unwrap_or(0)
}

/// Uso a partir de `StreamStats`/`ModelStreamStats`: `input_tokens` inclui os de
/// cache (`cached`); `input` é a parte sem cache.
fn usage_of(s: &Value, model: Option<&str>) -> TokenUsage {
    let cached = u(s.get("cached"));
    let input = s.get("input").and_then(Value::as_u64).unwrap_or_else(|| u(s.get("input_tokens")).saturating_sub(cached));
    TokenUsage {
        input_tokens: input,
        output_tokens: u(s.get("output_tokens")),
        cache_read: cached,
        cache_write: 0,
        model: model.map(str::to_string),
        cost_usd: None,
    }
}

/// Parse de uma linha do `--output-format stream-json`.
pub fn parse_json_event(line: &str) -> Vec<AgentEvent> {
    let line = line.trim();
    if line.is_empty() {
        return vec![];
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![AgentEvent::Raw(line.to_string())];
    };
    let st = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    match v.get("type").and_then(Value::as_str) {
        Some("init") => {
            let s = st("session_id");
            if s.is_empty() { vec![] } else { vec![AgentEvent::Session(s)] }
        }
        Some("message") if st("role") == "assistant" => {
            let t = st("content");
            if t.is_empty() { vec![] } else { vec![AgentEvent::Text(t)] }
        }
        Some("tool_use") => vec![AgentEvent::ToolUse { name: st("tool_name"), input: v.get("parameters").cloned().unwrap_or(Value::Null) }],
        Some("tool_result") if st("status") == "error" => {
            let msg = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("a ferramenta falhou");
            vec![AgentEvent::Raw(format!("Ferramenta {}: {msg}", st("tool_id")))]
        }
        Some("error") => vec![AgentEvent::Raw(st("message"))],
        Some("result") => {
            let mut evs = Vec::new();
            if let Some(stats) = v.get("stats") {
                // Uma linha de uso por modelo (melhor preço); sem a divisão, uma só
                // (o orquestrador preenche o modelo da tarefa).
                let models = stats.get("models").and_then(Value::as_object).filter(|m| !m.is_empty());
                match models {
                    Some(m) => evs.extend(m.iter().map(|(name, s)| AgentEvent::Usage(usage_of(s, Some(name))))),
                    None => evs.push(AgentEvent::Usage(usage_of(stats, None))),
                }
                // Os números são a soma dos pedidos do turno: só valem como
                // ocupação da janela quando houve um único pedido (sem ferramentas).
                let single = u(stats.get("tool_calls")) == 0 && models.is_none_or(|m| m.len() == 1);
                let total = usage_of(stats, None);
                if single && total.input_tokens + total.cache_read > 0 {
                    evs.push(AgentEvent::Context(crate::agents::context::ContextUsage {
                        input_tokens: total.input_tokens,
                        cache_read: total.cache_read,
                        cache_write: 0,
                        output_tokens: total.output_tokens,
                        model: models.and_then(|m| m.keys().next().cloned()),
                        window: None,
                    }));
                }
            }
            let ok = st("status") == "success";
            let summary = v.pointer("/error/message").and_then(Value::as_str).unwrap_or_default().to_string();
            evs.push(AgentEvent::Result { ok, summary, cost: None });
            evs
        }
        Some(_) => vec![],
        None => vec![AgentEvent::Raw(line.to_string())],
    }
}

/// Junta os pedaços de texto (`delta: true`) numa só mensagem e sintetiza o
/// resultado quando a CLI sai sem o evento `result`.
#[derive(Debug, Default)]
pub struct GeminiTracker {
    text: String,
    saw_result: bool,
    last_error: Option<String>,
}

impl GeminiTracker {
    fn flush(&mut self) -> Option<AgentEvent> {
        (!self.text.is_empty()).then(|| AgentEvent::Text(std::mem::take(&mut self.text)))
    }
}

impl TurnTracker for GeminiTracker {
    fn observe(&mut self, _line: &str, events: &[AgentEvent]) {
        for e in events {
            match e {
                AgentEvent::Result { .. } => self.saw_result = true,
                AgentEvent::Raw(m) if !m.is_empty() => self.last_error = Some(m.clone()),
                _ => {}
            }
        }
    }

    fn transform(&mut self, events: Vec<AgentEvent>) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        for e in events {
            match e {
                AgentEvent::Text(t) => self.text.push_str(&t),
                o => {
                    out.extend(self.flush());
                    out.push(o);
                }
            }
        }
        out
    }

    fn finish(&mut self, end: TurnEnd) -> Vec<AgentEvent> {
        let mut out: Vec<AgentEvent> = self.flush().into_iter().collect();
        if let TurnEnd::Exited { ok } = end {
            if !self.saw_result {
                let summary = if ok { String::new() } else { self.last_error.take().unwrap_or_default() };
                out.push(AgentEvent::Result { ok, summary, cost: None });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Linhas REAIS dos testes oficiais do formato (JSON.stringify dos objetos de
    // `packages/core/src/output/stream-json-formatter.test.ts`, gemini-cli, main
    // em 2026-11-16). Não vieram de uma execução da CLI.
    const INIT: &str = r#"{"type":"init","timestamp":"2025-10-10T12:00:00.000Z","session_id":"test-session-123","model":"gemini-2.0-flash-exp"}"#;
    const USER: &str = r#"{"type":"message","timestamp":"2025-10-10T12:00:00.000Z","role":"user","content":"What is 2+2?"}"#;
    const DELTA: &str = r#"{"type":"message","timestamp":"2025-10-10T12:00:00.000Z","role":"assistant","content":"4","delta":true}"#;
    const TOOL: &str = r#"{"type":"tool_use","timestamp":"2025-10-10T12:00:00.000Z","tool_name":"Read","tool_id":"read-123","parameters":{"file_path":"/path/to/file.txt"}}"#;
    const TOOL_OK: &str = r#"{"type":"tool_result","timestamp":"2025-10-10T12:00:00.000Z","tool_id":"read-123","status":"success","output":"File contents here"}"#;
    const TOOL_ERR: &str = r#"{"type":"tool_result","timestamp":"2025-10-10T12:00:00.000Z","tool_id":"read-123","status":"error","error":{"type":"FILE_NOT_FOUND","message":"File not found"}}"#;
    const WARN: &str = r#"{"type":"error","timestamp":"2025-10-10T12:00:00.000Z","severity":"warning","message":"Loop detected, stopping execution"}"#;
    const RESULT: &str = r#"{"type":"result","timestamp":"2025-10-10T12:00:00.000Z","status":"success","stats":{"total_tokens":100,"input_tokens":50,"output_tokens":50,"cached":0,"input":50,"duration_ms":1200,"tool_calls":2,"models":{}}}"#;
    const RESULT_ERR: &str = r#"{"type":"result","timestamp":"2025-10-10T12:00:00.000Z","status":"error","error":{"type":"MaxSessionTurnsError","message":"Maximum session turns exceeded"},"stats":{"total_tokens":100,"input_tokens":50,"output_tokens":50,"cached":0,"input":50,"duration_ms":1200,"tool_calls":0,"models":{}}}"#;

    #[test]
    fn parses_official_examples() {
        assert_eq!(parse_json_event(INIT), vec![AgentEvent::Session("test-session-123".into())]);
        assert!(parse_json_event(USER).is_empty());
        assert_eq!(parse_json_event(DELTA), vec![AgentEvent::Text("4".into())]);
        assert!(matches!(parse_json_event(TOOL).as_slice(), [AgentEvent::ToolUse { name, input }] if name == "Read" && input["file_path"] == "/path/to/file.txt"));
        assert!(parse_json_event(TOOL_OK).is_empty());
        assert!(matches!(parse_json_event(TOOL_ERR).as_slice(), [AgentEvent::Raw(m)] if m.contains("File not found")));
        assert_eq!(parse_json_event(WARN), vec![AgentEvent::Raw("Loop detected, stopping execution".into())]);
        let e = parse_json_event(RESULT);
        // Com ferramentas no turno: uso, sem contexto, resultado.
        assert!(matches!(&e[0], AgentEvent::Usage(u) if u.input_tokens == 50 && u.output_tokens == 50 && u.model.is_none()));
        assert!(matches!(&e[1], AgentEvent::Result { ok: true, .. }));
        assert_eq!(e.len(), 2);
        let e = parse_json_event(RESULT_ERR);
        assert!(e.iter().any(|x| matches!(x, AgentEvent::Context(c) if c.total() == 50)));
        assert!(matches!(e.last(), Some(AgentEvent::Result { ok: false, summary, .. }) if summary == "Maximum session turns exceeded"));
    }

    #[test]
    fn per_model_usage() {
        // SINTÉTICA: `models` preenchido no formato de `ModelStreamStats`.
        let l = r#"{"type":"result","timestamp":"t","status":"success","stats":{"total_tokens":1300,"input_tokens":1200,"output_tokens":100,"cached":1000,"input":200,"duration_ms":1,"tool_calls":0,"models":{"gemini-3.8-flash":{"total_tokens":1300,"input_tokens":1200,"output_tokens":100,"cached":1000,"input":200}}}}"#;
        let e = parse_json_event(l);
        assert!(matches!(&e[0], AgentEvent::Usage(u) if u.model.as_deref() == Some("gemini-3.8-flash") && u.cache_read == 1000 && u.input_tokens == 200));
        assert!(matches!(&e[1], AgentEvent::Context(c) if c.total() == 1200 && c.model.as_deref() == Some("gemini-3.8-flash")));
    }

    #[test]
    fn deltas_are_joined() {
        let mut t = GeminiProvider::new().turn_tracker();
        let mut all = Vec::new();
        let d2 = DELTA.replace(r#""4""#, r#"" é a resposta""#);
        for l in [INIT, DELTA, d2.as_str(), TOOL, DELTA, RESULT] {
            let evs = parse_json_event(l);
            t.observe(l, &evs);
            all.extend(t.transform(evs));
        }
        all.extend(t.finish(TurnEnd::Exited { ok: true }));
        let texts: Vec<_> = all.iter().filter_map(|e| if let AgentEvent::Text(s) = e { Some(s.as_str()) } else { None }).collect();
        assert_eq!(texts, ["4 é a resposta", "4"]);
        assert_eq!(all.iter().filter(|e| matches!(e, AgentEvent::Result { .. })).count(), 1);
        // Sem `result`: sintetiza no fim.
        let mut t = GeminiProvider::new().turn_tracker();
        let evs = parse_json_event(DELTA);
        t.observe(DELTA, &evs);
        assert!(t.transform(evs).is_empty());
        let end = t.finish(TurnEnd::Exited { ok: false });
        assert_eq!(end[0], AgentEvent::Text("4".into()));
        assert!(matches!(end[1], AgentEvent::Result { ok: false, .. }));
    }

    #[test]
    fn command_line() {
        let p = GeminiProvider::new();
        let opts = TurnOptions { model: Some("gemini-3.8-flash".into()), system_note: Some("cópia".into()), ..Default::default() };
        let c = p.task_command("oi", Path::new("w"), None, &opts);
        assert_eq!(c.args, ["--output-format", "stream-json", "--approval-mode", "auto_edit", "--skip-trust", "-m", "gemini-3.8-flash"]);
        assert_eq!(c.stdin.as_deref(), Some("[Aviso do ambiente] cópia\n\noi"));
        let c = p.task_command("oi", Path::new("w"), Some("test-session-123"), &TurnOptions::default());
        assert_eq!(c.args[c.args.len() - 2..], ["--resume", "test-session-123"]);
        assert_eq!(c.stdin.as_deref(), Some("oi"));
        assert!(!c.args.iter().any(|a| a == "--yolo" || a == "yolo" || a == "--include-directories"));
        use crate::permissions::AccessMode;
        let mode = |access| p.task_command("oi", Path::new("w"), None, &TurnOptions { access, ..Default::default() }).args[3].clone();
        assert_eq!(mode(AccessMode::Plan), "plan");
        assert_eq!(mode(AccessMode::Bypass), "yolo");
        assert_eq!(mode(AccessMode::Normal), "auto_edit");
    }

    #[test]
    fn auth_and_models() {
        let s = r#"{"security":{"auth":{"selectedType":"oauth-personal"}}}"#;
        assert_eq!(auth_status(Some(s), true, false), LoginStatus::LoggedIn);
        assert_eq!(auth_status(Some(s), false, false), LoginStatus::LoggedOut);
        assert_eq!(auth_status(Some(r#"{"security":{"auth":{"selectedType":"gemini-api-key"}}}"#), false, true), LoginStatus::LoggedIn);
        assert_eq!(auth_status(None, false, false), LoginStatus::LoggedOut);
        assert_eq!(auth_status(Some(r#"{"security":{"auth":{"selectedType":"vertex-ai"}}}"#), false, false), LoginStatus::Unknown);
        // Forma de `ModelListing` (models/list.ts).
        let out = r#"[{"modelId":"gemini-3.8-flash","id":"gemini-3.8-flash","name":"Flash","displayName":"Flash","description":"","contextLimits":{"inputTokens":1048576}}]"#;
        let m = parse_models(out);
        assert_eq!((m[0].value.as_str(), m[0].hint.as_str()), ("gemini-3.8-flash", "1048576 tokens de contexto"));
    }
}
