//! Provedor Claude Code (CLI oficial `claude`).

use super::context::ContextUsage;
use super::provider::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// O parser guarda quais subagentes estão abertos (ids de tool_use, únicos
/// entre sessões), por isso o provedor pode ser compartilhado entre tarefas.
#[derive(Debug, Default, Clone)]
pub struct ClaudeProvider {
    parser: Arc<Mutex<StreamParser>>,
    /// Opções que a CLI instalada aceita (lidas do `--help` na primeira vez).
    flags: Arc<OnceLock<CliFlags>>,
}

/// Opções recentes do `claude` que o Argus usa quando existem. Uma CLI antiga
/// recusaria a opção desconhecida e todo turno falharia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CliFlags {
    pub exclude_dynamic: bool,
    pub partial_messages: bool,
    /// `--permission-mode plan` (modo Planejar da conversa).
    pub plan: bool,
    /// `--permission-mode bypassPermissions`; sem ele o Bypass usa
    /// `--dangerously-skip-permissions`.
    pub bypass: bool,
}

impl CliFlags {
    pub const ALL: CliFlags = CliFlags { exclude_dynamic: true, partial_messages: true, plan: true, bypass: true };

    /// Lê do texto do `claude --help`.
    pub fn from_help(help: &str) -> Self {
        CliFlags {
            exclude_dynamic: help.contains("--exclude-dynamic-system-prompt-sections"),
            partial_messages: help.contains("--include-partial-messages"),
            plan: help.contains("\"plan\""),
            bypass: help.contains("bypassPermissions"),
        }
    }
}

impl ClaudeProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// Provedor com as opções já decididas (testes).
    pub fn with_flags(flags: CliFlags) -> Self {
        let p = Self::default();
        let _ = p.flags.set(flags);
        p
    }

    fn flags(&self) -> CliFlags {
        *self.flags.get_or_init(|| {
            let help = which_bin("claude").and_then(|b| run_capture(&b, &["--help"])).map(|(_, out)| out).unwrap_or_default();
            CliFlags::from_help(&help)
        })
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

    fn task_command(&self, prompt: &str, cwd: &Path, session: Option<&str>, opts: &TurnOptions) -> CommandSpec {
        // `claude -p` sem argumento de prompt lê o prompt da entrada padrão.
        let mut c = CommandSpec::new("claude").arg("-p");
        if let Some(s) = session {
            c = c.arg("--resume").arg(s);
        }
        // O modelo vale por processo: acompanha também o `--resume`.
        if let Some(m) = opts.model.as_deref().filter(|m| valid_model_name(m)) {
            c = c.arg("--model").arg(m);
        }
        // O aviso de ambiente também vale por processo (todo turno o repete).
        if let Some(n) = opts.system_note.as_deref().filter(|n| !n.is_empty()) {
            c = c.arg("--append-system-prompt").arg(n);
        }
        // Em `-p` não há quem aprove permissões: sem isto toda edição de arquivo
        // é negada. `acceptEdits` libera só edições (Bash continua bloqueado).
        // No modo de permissão "Auto", `permissions::apply` troca para `auto`.
        // As pastas de anexos entram depois, com `--add-dir` (`attachments::apply_add_dirs`).
        let flags = self.flags();
        // As seções do prompt de sistema que mudam a cada turno (git status, cwd,
        // memória) vão para a primeira mensagem. Sem isto, o git status muda quando
        // o agente edita um arquivo, o prompt de sistema muda junto e o cache da
        // conversa inteira é regravado no turno seguinte.
        if flags.exclude_dynamic {
            c = c.arg("--exclude-dynamic-system-prompt-sections");
        }
        use crate::permissions::AccessMode;
        c = match opts.access {
            AccessMode::Plan if flags.plan => c.arg("--permission-mode").arg("plan"),
            AccessMode::Bypass if flags.bypass => c.arg("--permission-mode").arg("bypassPermissions"),
            AccessMode::Bypass => c.arg("--dangerously-skip-permissions"),
            // Sem `plan` na CLI, Planejar cai no padrão (`acceptEdits`).
            _ => c.arg("--permission-mode").arg("acceptEdits"),
        };
        c = c
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose");
        // O texto chega aos pedaços enquanto é gerado (vira `TextDelta`).
        if flags.partial_messages {
            c = c.arg("--include-partial-messages");
        }
        c.cwd(cwd).stdin(prompt)
    }

    fn parse_line(&self, line: &str) -> Vec<AgentEvent> {
        self.parser.lock().unwrap_or_else(|e| e.into_inner()).parse(line)
    }

    fn begin_turn(&self, session: Option<&str>) {
        if let Some(s) = session {
            self.parser.lock().unwrap_or_else(|e| e.into_inner()).totals.remove(s);
        }
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        probe_models()
    }

    fn turn_tracker(&self) -> Box<dyn TurnTracker> {
        Box::new(PartialUsage::default())
    }
}

/// Uso parcial de um processo do claude: soma o `usage` das mensagens
/// `assistant` vistas depois do último `result`. Se o processo morre sem
/// `result` (turno cancelado, CLI que caiu), esse uso vira linhas de consumo
/// sem custo informado (o custo equivalente é calculado em `metrics`).
///
/// O stream-json repete a mesma mensagem (mesmo `message.id`, mesmo `usage`)
/// numa linha por bloco de conteúdo, por isso o uso é guardado por id; cada
/// campo fica com o maior valor visto (a contagem de saída só cresce). Inclui
/// as mensagens dos subagentes, como o `modelUsage` do `result`.
#[derive(Debug, Default)]
pub struct PartialUsage {
    /// Ordem de chegada das mensagens.
    order: Vec<String>,
    by_msg: HashMap<String, TokenUsage>,
}

impl PartialUsage {
    fn take_by_model(&mut self) -> Vec<TokenUsage> {
        let mut out: Vec<TokenUsage> = Vec::new();
        for id in self.order.drain(..) {
            let Some(u) = self.by_msg.remove(&id) else { continue };
            match out.iter_mut().find(|o| o.model == u.model) {
                Some(o) => {
                    o.input_tokens += u.input_tokens;
                    o.output_tokens += u.output_tokens;
                    o.cache_read += u.cache_read;
                    o.cache_write += u.cache_write;
                }
                None => out.push(u),
            }
        }
        self.by_msg.clear();
        out.into_iter().filter(|u| !u.is_empty()).collect()
    }
}

impl TurnTracker for PartialUsage {
    fn observe(&mut self, line: &str, _events: &[AgentEvent]) {
        let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { return };
        match v.get("type").and_then(Value::as_str) {
            // O `result` já informa o acumulado do processo (o parser desconta o
            // anterior): o que foi visto até aqui está contado.
            Some("result") => {
                self.order.clear();
                self.by_msg.clear();
            }
            Some("assistant") => {
                let Some(us) = v.pointer("/message/usage") else { return };
                let id = v.pointer("/message/id").and_then(Value::as_str).unwrap_or_default();
                // Sem id não há como juntar as repetições: conta a linha sozinha.
                let id = if id.is_empty() { format!("#{}", self.order.len()) } else { id.to_string() };
                let model = v.pointer("/message/model").and_then(Value::as_str).filter(|m| !m.starts_with('<'));
                let cur = TokenUsage {
                    input_tokens: u(us, "input_tokens"),
                    output_tokens: u(us, "output_tokens"),
                    cache_read: u(us, "cache_read_input_tokens"),
                    cache_write: u(us, "cache_creation_input_tokens"),
                    model: model.map(str::to_string),
                    cost_usd: None,
                };
                match self.by_msg.get_mut(&id) {
                    Some(p) => {
                        p.input_tokens = p.input_tokens.max(cur.input_tokens);
                        p.output_tokens = p.output_tokens.max(cur.output_tokens);
                        p.cache_read = p.cache_read.max(cur.cache_read);
                        p.cache_write = p.cache_write.max(cur.cache_write);
                    }
                    None => {
                        self.order.push(id.clone());
                        self.by_msg.insert(id, cur);
                    }
                }
            }
            _ => {}
        }
    }

    fn finish(&mut self, _end: TurnEnd) -> Vec<AgentEvent> {
        // Só sobra uso quando nenhum `result` cobriu as últimas mensagens.
        self.take_by_model().into_iter().map(AgentEvent::Usage).collect()
    }
}

/// Resposta ao pedido de controle `initialize`, lida de uma linha do stream-json.
#[derive(Debug, Clone, PartialEq)]
pub enum InitReply {
    /// Lista de modelos aceitos (sem o "default").
    Models(Vec<ModelInfo>),
    /// A CLI respondeu com erro (ou sem a lista de modelos).
    Error(String),
}

/// Lê a lista de modelos da resposta ao pedido de controle `initialize` do
/// stream-json (conferido no claude 2.1.220: `response.response.models`, cada
/// item com `value`, `resolvedModel`, `displayName` e `description`). O item
/// "default" vem marcado (`default: true`), com o modelo que ele resolve na dica. Nenhum outro campo da resposta
/// (conta, organização) é lido.
pub fn models_from_initialize(v: &Value) -> Option<Vec<ModelInfo>> {
    match init_reply(v)? {
        InitReply::Models(m) => Some(m),
        InitReply::Error(_) => None,
    }
}

/// Classifica uma linha como resposta ao `initialize`: lista de modelos ou erro
/// (`subtype: "error"`, ou sucesso sem `models`). Outras linhas: None.
pub fn init_reply(v: &Value) -> Option<InitReply> {
    if v.get("type").and_then(Value::as_str) != Some("control_response") {
        return None;
    }
    let r = v.get("response")?;
    if r.get("subtype").and_then(Value::as_str) == Some("error") {
        let msg = s(r, "error");
        return Some(InitReply::Error(if msg.is_empty() {
            "a CLI recusou o pedido de inicialização".into()
        } else {
            msg.to_string()
        }));
    }
    let Some(list) = r.pointer("/response/models").and_then(Value::as_array) else {
        return Some(InitReply::Error("a resposta da CLI não traz a lista de modelos (versão da CLI diferente?)".into()));
    };
    Some(InitReply::Models(
        list.iter()
            .filter_map(|m| {
                let value = m.get("value")?.as_str()?;
                let is_default = value == "default";
                if !is_default && !valid_model_name(value) {
                    return None;
                }
                // A descrição vem como "Opus 5 with 1M context · Best for everyday...":
                // antes do " · " o nome com versão, depois a descrição. O
                // `displayName` ("Opus (1M context)") não traz a versão.
                let desc = s(m, "description");
                let (head, tail) = desc.split_once(" · ").unwrap_or((desc, ""));
                let wide = value.ends_with("[1m]") || head.contains("1M context");
                let mut name = head.replace(" with 1M context", "").trim().to_string();
                if name.is_empty() {
                    name = s(m, "displayName").replace(" (1M context)", "").trim().to_string();
                }
                if name.is_empty() {
                    name = value.to_string();
                }
                if is_default {
                    // Qual modelo o padrão resolve, se a CLI disse.
                    let which = if head.is_empty() {
                        "o padrão da CLI".to_string()
                    } else if wide {
                        format!("{name} · 1M")
                    } else {
                        name
                    };
                    return Some(ModelInfo {
                        default: true,
                        ..ModelInfo::new(value, "Padrão (recomendado)", format!("{which} — usado quando nenhum é escolhido"))
                    });
                }
                let badge = if wide { "1M" } else { "" };
                Some(ModelInfo { badge: badge.into(), ..ModelInfo::new(value, name, tail.trim()) })
            })
            .collect(),
    ))
}

/// Quanto esperar a resposta ao `initialize`. A CLI roda os hooks de início de
/// sessão do usuário antes de responder (visto: de 5 a 18 s numa máquina com
/// três hooks), por isso a folga é grande.
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

/// Sobe um `claude -p` que não grava sessão, manda só o pedido de controle
/// `initialize` (nenhuma mensagem: nenhum modelo é chamado), lê a resposta e
/// mata o processo. Falhas (CLI que encerra antes, resposta de erro, tempo
/// esgotado) voltam com o fim da saída de erro da CLI, para a interface mostrar.
fn probe_models() -> Result<Vec<ModelInfo>, String> {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::process::{Command, Stdio};
    use std::sync::mpsc::RecvTimeoutError;

    let exe = which_bin("claude").ok_or_else(|| "A CLI claude não está instalada (não foi achada no PATH).".to_string())?;
    let mut cmd = Command::new(&exe);
    cmd.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--no-session-persistence"])
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("Não foi possível executar {}: {e}", exe.display()))?;
    // O stdin fica aberto até a resposta: fechá-lo antes encerraria a CLI.
    let mut stdin = child.stdin.take();
    if let Some(stdin) = stdin.as_mut() {
        let line = serde_json::json!({ "type": "control_request", "request_id": "argus-modelos", "request": { "subtype": "initialize" } });
        let _ = stdin.write_all(format!("{line}\n").as_bytes());
        let _ = stdin.flush();
    }
    // Só o fim da saída de erro é guardado, para explicar uma falha.
    let errbuf = Arc::new(Mutex::new(String::new()));
    if let Some(mut stderr) = child.stderr.take() {
        let errbuf = errbuf.clone();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(n) = stderr.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                let mut b = errbuf.lock().unwrap_or_else(|e| e.into_inner());
                b.push_str(&String::from_utf8_lossy(&chunk[..n]));
                if b.len() > 8192 {
                    let mut cut = b.len() - 4096;
                    while !b.is_char_boundary(cut) {
                        cut += 1;
                    }
                    b.drain(..cut);
                }
            }
        });
    }
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("O claude não abriu a saída.".into());
    };
    let (tx, rx) = std::sync::mpsc::channel::<Option<InitReply>>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if let Some(reply) = init_reply(&v) {
                let _ = tx.send(Some(reply));
                return;
            }
        }
        // A saída fechou sem resposta: a CLI encerrou.
        let _ = tx.send(None);
    });
    let got = rx.recv_timeout(PROBE_TIMEOUT);
    let _ = child.kill();
    let _ = child.wait();
    drop(stdin);
    let tail = || {
        let b = errbuf.lock().unwrap_or_else(|e| e.into_inner());
        let t = strip_ansi(&b).split_whitespace().collect::<Vec<_>>().join(" ");
        let n = t.chars().count();
        let t: String = t.chars().skip(n.saturating_sub(400)).collect();
        if t.is_empty() {
            String::new()
        } else {
            format!(" Saída da CLI: {t}")
        }
    };
    match got {
        Ok(Some(InitReply::Models(m))) if !m.is_empty() => Ok(m),
        Ok(Some(InitReply::Models(_))) => Err("A CLI claude respondeu sem nenhum modelo.".into()),
        Ok(Some(InitReply::Error(e))) => Err(format!("A CLI claude recusou a consulta: {e}.")),
        Ok(None) | Err(RecvTimeoutError::Disconnected) => Err(format!("A CLI claude encerrou sem informar os modelos.{}", tail())),
        Err(RecvTimeoutError::Timeout) => {
            Err(format!("O claude não informou a lista de modelos em {} s.{}", PROBE_TIMEOUT.as_secs(), tail()))
        }
    }
}

fn ev_is_empty_denial(ev: &AgentEvent) -> bool {
    matches!(ev, AgentEvent::Denied { tool, .. } if tool.is_empty())
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
/// Os primeiros `max` caracteres de `t`, com "…" quando cortado.
fn truncate_chars(t: &str, max: usize) -> String {
    match t.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &t[..i]),
        None => t.to_string(),
    }
}

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

/// `SubagentStats` a partir do `tool_use_result` do fim de um subagente
/// (`totalDurationMs`, `totalTokens`, `totalToolUseCount`) ou do `usage` de um
/// `task_notification` (`duration_ms`, `total_tokens`, `tool_uses`). None se
/// nenhum número vier.
fn subagent_stats(id: &str, v: &Value) -> Option<AgentEvent> {
    let num = |keys: &[&str]| keys.iter().find_map(|k| v.get(*k).and_then(Value::as_u64));
    let duration_ms = num(&["totalDurationMs", "duration_ms"]);
    let tokens = num(&["totalTokens", "total_tokens"]);
    let tool_uses = num(&["totalToolUseCount", "tool_uses"]);
    if duration_ms.is_none() && tokens.is_none() && tool_uses.is_none() {
        return None;
    }
    Some(AgentEvent::SubagentStats { id: id.to_string(), duration_ms, tokens, tool_uses })
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
    /// Ids de todo tool_use que criou subagente: o `tool_result` deles não vira `ToolResult`.
    spawned: std::collections::HashSet<String>,
    /// Último total acumulado informado por sessão, no processo em andamento
    /// (zerado por `begin_turn` quando um processo novo começa).
    totals: HashMap<String, SessionTotals>,
    /// Último pedido do agente principal por sessão: (id da mensagem, uso).
    ctx: HashMap<String, (String, ContextUsage)>,
    /// Tamanho da janela por modelo, informado pela CLI (`modelUsage.*.contextWindow`).
    windows: HashMap<String, u64>,
}

/// Janela informada pela CLI para `model`: o nome exato ou, como a chave do
/// `modelUsage` pode trazer sufixos (`[1m]`, data), o que começar igual.
fn window_of(windows: &HashMap<String, u64>, model: &str) -> Option<u64> {
    if let Some(w) = windows.get(model) {
        return Some(*w);
    }
    let base = |m: &str| m.trim_end_matches("[1m]").to_string();
    let m = base(model);
    windows
        .iter()
        .filter(|(k, _)| {
            let k = base(k);
            !k.is_empty() && !m.is_empty() && (k.starts_with(&m) || m.starts_with(&k))
        })
        .map(|(_, w)| *w)
        .max()
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

    /// Ocupação de contexto de uma mensagem `assistant` do agente principal. A
    /// mesma mensagem sai numa linha por bloco (mesmo `message.id`): cada campo
    /// fica com o maior valor visto. Devolve Some só quando o valor mudou.
    fn context_of(&mut self, v: &Value) -> Option<ContextUsage> {
        let us = v.pointer("/message/usage")?;
        let session = v.get("session_id").and_then(Value::as_str).unwrap_or_default().to_string();
        let id = v.pointer("/message/id").and_then(Value::as_str).unwrap_or_default().to_string();
        let model = v
            .pointer("/message/model")
            .and_then(Value::as_str)
            .filter(|m| !m.is_empty() && !m.starts_with('<'))
            .map(str::to_string);
        // Mensagem sintética (erro local, sem chamada ao modelo): não muda o contexto.
        model.as_ref()?;
        let mut cur = ContextUsage {
            input_tokens: u(us, "input_tokens"),
            cache_read: u(us, "cache_read_input_tokens"),
            cache_write: u(us, "cache_creation_input_tokens"),
            output_tokens: u(us, "output_tokens"),
            window: model.as_deref().and_then(|m| window_of(&self.windows, m)),
            model,
        };
        if cur.total() == 0 && cur.output_tokens == 0 {
            return None;
        }
        if let Some((pid, p)) = self.ctx.get(&session) {
            if !id.is_empty() && *pid == id {
                cur.input_tokens = cur.input_tokens.max(p.input_tokens);
                cur.cache_read = cur.cache_read.max(p.cache_read);
                cur.cache_write = cur.cache_write.max(p.cache_write);
                cur.output_tokens = cur.output_tokens.max(p.output_tokens);
            }
            if *p == cur && *pid == id {
                return None;
            }
        }
        self.ctx.insert(session, (id, cur.clone()));
        Some(cur)
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
                let ctx = if parent.is_none() { self.context_of(&v) } else { None };
                let Some(content) = v.pointer("/message/content").and_then(Value::as_array) else {
                    return ctx.map(AgentEvent::Context).into_iter().collect();
                };
                let mut evs = Vec::new();
                for block in content {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            let t = s(block, "text");
                            if !t.is_empty() {
                                evs.push(AgentEvent::Text(t.to_string()).scoped(parent));
                            }
                        }
                        Some("thinking") => {
                            let t = s(block, "thinking").trim();
                            if !t.is_empty() {
                                evs.push(AgentEvent::Thinking(t.to_string()).scoped(parent));
                            }
                        }
                        Some("tool_use") => {
                            let name = block.get("name").and_then(Value::as_str).unwrap_or("?");
                            let input = block.get("input").cloned().unwrap_or(Value::Null);
                            let start = match block.get("id").and_then(Value::as_str) {
                                Some(id) if is_subagent_tool(name) => {
                                    self.open.insert(id.to_string(), false);
                                    self.spawned.insert(id.to_string());
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
                evs.extend(ctx.map(AgentEvent::Context));
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
                        if !self.spawned.contains(id) {
                            let ok = !block.get("is_error").and_then(Value::as_bool).unwrap_or(false);
                            let output = truncate_chars(&content_text(block.get("content")), TOOL_OUTPUT_MAX);
                            evs.push(AgentEvent::ToolResult { ok, output }.scoped(parent));
                        }
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
                    if let Some(stats) = tur.and_then(|t| subagent_stats(id, t)) {
                        evs.push(stats);
                    }
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
                        if let Some(w) = m.get("contextWindow").and_then(Value::as_u64).filter(|w| *w > 0) {
                            self.windows.insert(model.clone(), w);
                        }
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
                // Ferramentas negadas por falta de permissão (conferido no claude 2.1.220:
                // `permission_denials: [{ tool_name, tool_use_id, tool_input }]`).
                if let Some(denials) = v.get("permission_denials").and_then(Value::as_array) {
                    for d in denials {
                        let ev = AgentEvent::Denied {
                            tool: s(d, "tool_name").to_string(),
                            command: d.pointer("/tool_input/command").and_then(Value::as_str).unwrap_or_default().to_string(),
                        };
                        if !ev_is_empty_denial(&ev) && !evs.contains(&ev) {
                            evs.push(ev);
                        }
                    }
                }
                let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false);
                let sub_ok = v.get("subtype").and_then(Value::as_str).map_or(true, |s| s == "success");
                let ok = !is_error && sub_ok;
                let summary = v.get("result").and_then(Value::as_str).unwrap_or_default().to_string();
                // `result` vazio: sucesso sem texto, sem uso novo e sem negação. É o
                // que a CLI imprime quando nada novo aconteceu (num processo retomado,
                // um `result` de zero turnos antes de chamar o modelo; ou um `result`
                // repetido com o mesmo acumulado). Gravá-lo virava um "resultado ok,
                // custo 0" fantasma no meio do turno; o `result` de verdade vem depois.
                if ok && summary.trim().is_empty() && evs.is_empty() && cost.unwrap_or(0.0) <= 0.0 {
                    return evs;
                }
                // A janela informada no `result` completa o último pedido da sessão.
                if let Some((_, c)) = v.get("session_id").and_then(Value::as_str).and_then(|sid| self.ctx.get_mut(sid)) {
                    let w = c.model.as_deref().and_then(|m| window_of(&self.windows, m));
                    if w.is_some() && w != c.window {
                        c.window = w;
                        evs.push(AgentEvent::Context(c.clone()));
                    }
                }
                evs.push(AgentEvent::Result { ok, summary, cost });
                evs
            }
            Some("system") => match (v.get("subtype").and_then(Value::as_str), v.get("session_id").and_then(Value::as_str)) {
                (Some("init"), Some(sid)) => vec![AgentEvent::Session(sid.to_string())],
                // Fim de um subagente lançado em segundo plano.
                (Some("task_notification"), _) => {
                    let id = s(&v, "tool_use_id");
                    if self.open.get(id) == Some(&true) {
                        self.open.remove(id);
                        let stats = v.get("usage").and_then(|u| subagent_stats(id, u));
                        stats
                            .into_iter()
                            .chain([AgentEvent::SubagentEnd {
                            id: id.to_string(),
                            ok: s(&v, "status") == "completed",
                            result: s(&v, "summary").to_string(),
                            }])
                            .collect()
                    } else {
                        vec![]
                    }
                }
                // Fim de uma compactação (pedida pelo Argus ou automática da CLI).
                (Some("compact_boundary"), _) => {
                    let m = v.get("compact_metadata");
                    let n = |k: &str| m.and_then(|m| m.get(k)).and_then(Value::as_u64).unwrap_or(0);
                    vec![AgentEvent::Compacted {
                        pre_tokens: n("pre_tokens"),
                        post_tokens: n("post_tokens"),
                        trigger: m.and_then(|m| m.get("trigger")).and_then(Value::as_str).unwrap_or("auto").to_string(),
                    }]
                }
                // task_started/task_progress/task_updated e hooks: sem evento de UI.
                _ => vec![],
            },
            // Texto parcial do agente principal (os subagentes trazem `parent_tool_use_id`).
            Some("stream_event") => {
                let main = v.get("parent_tool_use_id").map_or(true, Value::is_null);
                let ev = v.get("event");
                let is_text = ev.and_then(|e| e.get("type")).and_then(Value::as_str) == Some("content_block_delta")
                    && ev.and_then(|e| e.pointer("/delta/type")).and_then(Value::as_str) == Some("text_delta");
                match ev.and_then(|e| e.pointer("/delta/text")).and_then(Value::as_str) {
                    Some(t) if main && is_text && !t.is_empty() => vec![AgentEvent::TextDelta(t.to_string())],
                    _ => vec![],
                }
            }
            // rate_limit_event: sem evento de UI.
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
        let mut evs = p.parse_line(MIXED);
        assert_eq!(evs.len(), 4);
        assert_eq!(evs.remove(0), AgentEvent::Thinking("...".into()));
        assert!(matches!(&evs[2], AgentEvent::Context(c) if c.input_tokens == 9 && c.model.as_deref() == Some("m")));
        assert_eq!(evs[0], AgentEvent::Text("Vou listar".into()));
        match &evs[1] {
            AgentEvent::ToolUse { name, input } => {
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "ls");
            }
            o => panic!("{o:?}"),
        }
        assert_eq!(
            p.parse_line(r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","is_error":true,"content":"falhou"}]}}"#),
            vec![AgentEvent::ToolResult { ok: false, output: "falhou".into() }]
        );
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
        let c = ClaudeProvider::with_flags(CliFlags::ALL).task_command("oi \"x\" 100%\nlinha", Path::new("."), None, &TurnOptions::default());
        let tail = [
            "--exclude-dynamic-system-prompt-sections",
            "--permission-mode",
            "acceptEdits",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
        ];
        assert_eq!(c.args, [&["-p"][..], &tail].concat());
        assert_eq!(c.stdin.as_deref(), Some("oi \"x\" 100%\nlinha"));
        let c = ClaudeProvider::with_flags(CliFlags::ALL).task_command("e agora?", Path::new("."), Some("abc"), &TurnOptions::default());
        assert_eq!(c.args, [&["-p", "--resume", "abc"][..], &tail].concat());
        // Modelo e aviso de ambiente acompanham também a retomada; nunca entra `--add-dir` do projeto.
        let opts = TurnOptions { model: Some("opus[1m]".into()), system_note: Some("cópia isolada do projeto x".into()), ..Default::default() };
        let c = ClaudeProvider::with_flags(CliFlags::ALL).task_command("oi", Path::new("."), Some("abc"), &opts);
        let extra = ["-p", "--resume", "abc", "--model", "opus[1m]", "--append-system-prompt", "cópia isolada do projeto x"];
        assert_eq!(c.args, [&extra[..], &tail].concat());
        assert!(!c.args.iter().any(|a| a == "--add-dir"));
        let bad = TurnOptions { model: Some("--dangerously-skip-permissions".into()), ..Default::default() };
        assert_eq!(ClaudeProvider::with_flags(CliFlags::ALL).task_command("oi", Path::new("."), None, &bad).args, [&["-p"][..], &tail].concat());
    }

    #[test]
    fn access_modes_args() {
        use crate::permissions::AccessMode;
        let mode = |flags: CliFlags, access| {
            let c = ClaudeProvider::with_flags(flags).task_command("oi", Path::new("."), None, &TurnOptions { access, ..Default::default() });
            let i = c.args.iter().position(|a| a == "--permission-mode");
            (i.map(|i| c.args[i + 1].clone()), c.args.iter().any(|a| a == "--dangerously-skip-permissions"))
        };
        assert_eq!(mode(CliFlags::ALL, AccessMode::Plan), (Some("plan".into()), false));
        assert_eq!(mode(CliFlags::ALL, AccessMode::Bypass), (Some("bypassPermissions".into()), false));
        assert_eq!(mode(CliFlags::ALL, AccessMode::Normal), (Some("acceptEdits".into()), false));
        let old = CliFlags { plan: false, bypass: false, ..CliFlags::ALL };
        assert_eq!(mode(old, AccessMode::Bypass), (None, true));
        assert_eq!(mode(old, AccessMode::Plan), (Some("acceptEdits".into()), false));
        let f = CliFlags::from_help(r#"--permission-mode <mode> (choices: "acceptEdits", "auto", "bypassPermissions", "manual", "dontAsk", "plan")"#);
        assert!(f.plan && f.bypass);
    }

    /// Linha real (claude 2.1.220) do fim de um `/compact` com `--resume`.
    #[test]
    fn compact_boundary_becomes_event() {
        let l = r#"{"type":"system","subtype":"compact_boundary","session_id":"s","compact_metadata":{"trigger":"manual","pre_tokens":39252,"post_tokens":2663}}"#;
        assert!(matches!(&ClaudeProvider::new().parse_line(l)[..],
            [AgentEvent::Compacted { pre_tokens: 39252, post_tokens: 2663, trigger }] if trigger == "manual"));
    }

    #[test]
    fn old_cli_gets_no_new_flags() {
        let old = CliFlags::from_help("--model <model>
--verbose");
        assert_eq!(old, CliFlags { exclude_dynamic: false, partial_messages: false, plan: false, bypass: false });
        let c = ClaudeProvider::with_flags(old).task_command("oi", Path::new("."), None, &TurnOptions::default());
        assert!(!c.args.iter().any(|a| a == "--include-partial-messages" || a == "--exclude-dynamic-system-prompt-sections"));
    }

    #[test]
    fn text_delta_only_from_main_agent() {
        let p = ClaudeProvider::new();
        let main = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Olá"}},"parent_tool_use_id":null,"session_id":"s"}"#;
        assert!(matches!(&p.parse_line(main)[..], [AgentEvent::TextDelta(t)] if t == "Olá"));
        let sub = main.replace("\"parent_tool_use_id\":null", "\"parent_tool_use_id\":\"toolu_1\"");
        assert!(p.parse_line(&sub).is_empty());
        let thinking = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hm"}},"parent_tool_use_id":null}"#;
        assert!(p.parse_line(thinking).is_empty());
        let start = r#"{"type":"stream_event","event":{"type":"message_start","message":{}},"parent_tool_use_id":null}"#;
        assert!(p.parse_line(start).is_empty());
    }

    /// Linha real (claude 2.1.220, encurtada): `git push` negado com o padrão.
    #[test]
    fn denied_tools_become_events() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"session_id":"s9","result":"feito","total_cost_usd":0.1,"modelUsage":{},"permission_denials":[{"tool_name":"PowerShell","tool_use_id":"toolu_01","tool_input":{"command":"git push .","description":"Execute git push ."}},{"tool_name":"PowerShell","tool_use_id":"toolu_02","tool_input":{"command":"git push .","description":"de novo"}},{"tool_name":"Read","tool_use_id":"toolu_03","tool_input":{"file_path":"C:\\fora.txt"}}]}"#;
        let evs = parse_stream_json(line);
        let denied: Vec<&AgentEvent> = evs.iter().filter(|e| matches!(e, AgentEvent::Denied { .. })).collect();
        assert_eq!(denied.len(), 2, "o mesmo comando negado duas vezes aparece uma vez: {evs:?}");
        assert_eq!(denied[0], &AgentEvent::Denied { tool: "PowerShell".into(), command: "git push .".into() });
        assert_eq!(denied[1], &AgentEvent::Denied { tool: "Read".into(), command: String::new() });
        assert!(matches!(evs.last(), Some(AgentEvent::Result { ok: true, .. })));
    }

    #[test]
    fn models_come_from_the_initialize_response() {
        // Forma real da resposta (claude 2.1.220), encurtada.
        let v = serde_json::json!({"type":"control_response","response":{"subtype":"success","request_id":"r1","response":{
            "account": {"qualquer": "coisa"},
            "models":[
                {"value":"default","displayName":"Default (recommended)","description":"d"},
                {"value":"opus[1m]","displayName":"Opus (1M context)","description":"Opus 5 with 1M context · Best for everyday, complex tasks"},
                {"value":"haiku","displayName":"Haiku","description":""},
                {"value":"com espaço","displayName":"x","description":""}
            ]}}});
        let m = models_from_initialize(&v).unwrap();
        assert_eq!(m.len(), 3, "sem nome inválido: {m:?}");
        assert!(m[0].default && m[0].hint == "d — usado quando nenhum é escolhido", "{:?}", m[0]);
        assert_eq!((m[1].value.as_str(), m[1].label.as_str(), m[1].hint.as_str(), m[1].badge.as_str()), ("opus[1m]", "Opus 5", "Best for everyday, complex tasks", "1M"));
        assert_eq!((m[2].value.as_str(), m[2].label.as_str(), m[2].hint.as_str(), m[2].badge.as_str()), ("haiku", "Haiku", "", ""));
        assert_eq!(models_from_initialize(&serde_json::json!({"type":"system","subtype":"init"})), None);
    }

    /// Consulta real à CLI instalada (só o pedido `initialize`: nenhum modelo é
    /// chamado). `cargo test -p argus-core -- --ignored probe_models_live`.
    #[test]
    #[ignore]
    fn probe_models_live() {
        let t = std::time::Instant::now();
        let r = probe_models();
        eprintln!("{:?} em {:?}", r, t.elapsed());
        assert!(!r.unwrap().is_empty());
    }

    /// Resposta real do claude 2.1.220 ao `initialize`, observada em 2026-11-16
    /// (só `models` e campos inofensivos; `account`, `commands` e o resto saíram).
    const REAL_INIT: &str = r#"{"type":"control_response","response":{"subtype":"success","request_id":"argus-modelos","response":{"output_style":"default","models":[{"value":"default","resolvedModel":"claude-opus-5[1m]","displayName":"Default (recommended)","description":"Opus 5 with 1M context · Best for everyday, complex tasks","supportsEffort":true,"supportedEffortLevels":["low","medium","high","xhigh","max"],"supportsAdaptiveThinking":true,"supportsFastMode":true,"supportsAutoMode":true},{"value":"opus[1m]","resolvedModel":"claude-opus-5[1m]","displayName":"Opus (1M context)","description":"Opus 5 with 1M context · Best for everyday, complex tasks","supportsEffort":true,"supportsFastMode":true},{"value":"claude-fable-5-2[1m]","resolvedModel":"claude-fable-5-2","displayName":"Fable","description":"Fable 5.2 · Most capable for your hardest and longest-running tasks","supportsEffort":true},{"value":"sonnet","resolvedModel":"claude-sonnet-5","displayName":"Sonnet","description":"Sonnet 5 · Efficient for routine tasks","supportsEffort":true},{"value":"haiku","resolvedModel":"claude-haiku-4-5-20251001","displayName":"Haiku","description":"Haiku 4.5 · Fastest for quick answers"}],"pid":1234}}}"#;

    #[test]
    fn real_initialize_response_is_parsed() {
        let v: Value = serde_json::from_str(REAL_INIT).unwrap();
        let m = models_from_initialize(&v).unwrap();
        let vals: Vec<&str> = m.iter().map(|m| m.value.as_str()).collect();
        assert_eq!(vals, ["default", "opus[1m]", "claude-fable-5-2[1m]", "sonnet", "haiku"]);
        assert!(m[0].default);
        assert_eq!(m[0].label, "Padrão (recomendado)");
        assert_eq!(m[0].hint, "Opus 5 · 1M — usado quando nenhum é escolhido");
        assert_eq!((m[2].label.as_str(), m[2].badge.as_str()), ("Fable 5.2", "1M"));
        assert_eq!(m[2].hint, "Most capable for your hardest and longest-running tasks");
        assert_eq!((m[3].label.as_str(), m[3].badge.as_str()), ("Sonnet 5", ""));
        assert_eq!(m[4].label, "Haiku 4.5");
        assert!(m.iter().skip(1).all(|m| !m.default));
    }

    #[test]
    fn initialize_errors_are_reported() {
        let err = serde_json::json!({"type":"control_response","response":{"subtype":"error","request_id":"x","error":"Already initialized"}});
        assert_eq!(init_reply(&err), Some(InitReply::Error("Already initialized".into())));
        assert_eq!(models_from_initialize(&err), None);
        // Sucesso sem `models` (formato mudou): erro explícito, nunca lista vazia calada.
        let other = serde_json::json!({"type":"control_response","response":{"subtype":"success","request_id":"x","response":{"commands":[]}}});
        assert!(matches!(init_reply(&other), Some(InitReply::Error(_))));
        assert_eq!(init_reply(&serde_json::json!({"type":"system","subtype":"hook_started"})), None);
    }

    #[test]
    fn auth_status() {
        assert_eq!(parse_auth_status("{\"loggedIn\": true, \"authMethod\":\"x\"}\n"), Some(true));
        assert_eq!(parse_auth_status("{\n \"loggedIn\": false\n}"), Some(false));
        assert_eq!(parse_auth_status("lixo"), None);
    }

    const FIXTURE: &str = include_str!("../../tests/fixtures/claude_subagent.jsonl");
    const SUB: &str = "toolu_01CsNFNQqgKLL88nwZezfZRE";

    /// Contexto na saída real: só as mensagens do agente principal contam (a do
    /// subagente, 27245 de cache gravado, não); a repetição da mesma mensagem
    /// não gera evento novo; o `result` traz a janela (`contextWindow`).
    #[test]
    fn context_from_real_stream() {
        let p = ClaudeProvider::new();
        let ctx: Vec<ContextUsage> = FIXTURE
            .lines()
            .flat_map(|l| p.parse_line(l))
            .filter_map(|e| match e {
                AgentEvent::Context(c) => Some(c),
                _ => None,
            })
            .collect();
        assert_eq!(ctx.len(), 3, "{ctx:#?}");
        assert_eq!((ctx[0].total(), ctx[0].window), (39408, None));
        assert_eq!((ctx[1].input_tokens, ctx[1].cache_write, ctx[1].output_tokens), (2, 40536, 31));
        assert_eq!((ctx[2].total(), ctx[2].window, ctx[2].model.as_deref()), (40538, Some(1_000_000), Some("claude-fable-5")));
        // A janela aprendida vale para os próximos pedidos do mesmo modelo.
        let next = r#"{"type":"assistant","message":{"id":"msg_X","model":"claude-fable-5","content":[],"usage":{"input_tokens":1,"cache_read_input_tokens":40538,"cache_creation_input_tokens":10,"output_tokens":3}},"parent_tool_use_id":null,"session_id":"s2"}"#;
        assert!(matches!(p.parse_line(next).as_slice(), [AgentEvent::Context(c)] if c.window == Some(1_000_000) && c.total() == 40549));
    }

    /// Troca de modelo: o contexto passa a ser o do pedido novo, com a janela do
    /// modelo novo; a soma do turno nunca entra.
    #[test]
    fn context_follows_last_call_and_model_switch() {
        let mut p = StreamParser::new();
        let ctx = |evs: Vec<AgentEvent>| evs.into_iter().find_map(|e| match e { AgentEvent::Context(c) => Some(c), _ => None });
        let a = ctx(p.parse(A1_TEXT)).unwrap();
        assert_eq!((a.total(), a.output_tokens), (17003, 8));
        // Mesma mensagem, mais saída: atualiza só a saída.
        let a = ctx(p.parse(A1_TOOL)).unwrap();
        assert_eq!((a.total(), a.output_tokens), (17003, 61));
        assert!(ctx(p.parse(A1_TOOL)).is_none(), "repetição idêntica não emite");
        // Subagente não conta.
        assert!(ctx(p.parse(A_SUB)).is_none());
        let b = ctx(p.parse(A2)).unwrap();
        assert_eq!(b.total(), 17301);
        let sw = r#"{"type":"assistant","message":{"id":"msg_01C","model":"claude-opus-4-5[1m]","content":[],"usage":{"input_tokens":5,"cache_creation_input_tokens":0,"cache_read_input_tokens":300000,"output_tokens":2}},"parent_tool_use_id":null,"session_id":"s1"}"#;
        let c = ctx(p.parse(sw)).unwrap();
        assert_eq!((c.total(), c.model.as_deref()), (300005, Some("claude-opus-4-5[1m]")));
        let mut c2 = c.clone();
        crate::agents::context::resolve_window(&mut c2, None);
        assert_eq!(c2.window, Some(1_000_000));
    }

    /// Saída real (claude 2.1.220) de um turno que delega a listagem de arquivos a um subagente.
    #[test]
    fn real_subagent_stream() {
        let p = ClaudeProvider::new();
        let all: Vec<AgentEvent> = FIXTURE.lines().flat_map(|l| p.parse_line(l)).filter(|e| !matches!(e, AgentEvent::Context(_))).collect();
        // A saída do Glob de dentro do subagente vem marcada com o id dele.
        assert!(all.iter().any(|e| matches!(e, AgentEvent::Scoped { subagent, event } if subagent == SUB && matches!(&**event, AgentEvent::ToolResult { ok: true, .. }))));
        let evs: Vec<AgentEvent> = all.into_iter().filter(|e| e.kind() != "ToolResult").collect();
        let kinds: Vec<&str> = evs.iter().map(|e| e.kind()).collect();
        assert_eq!(
            kinds,
            ["Session", "ToolUse", "SubagentStart", "ToolUse", "SubagentStats", "SubagentEnd", "Text", "Usage", "Result"],
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
        // Duração e tokens que a CLI informa no fim do subagente.
        assert_eq!(
            evs[4],
            AgentEvent::SubagentStats { id: SUB.into(), duration_ms: Some(14989), tokens: Some(29039), tool_uses: Some(1) }
        );
        assert_eq!(evs[5], AgentEvent::SubagentEnd { id: SUB.into(), ok: true, result: "- alfa.txt\n- beta.md".into() });
        // O texto final é do agente principal (sem invólucro).
        assert_eq!(evs[6], AgentEvent::Text("A pasta contém dois arquivos: `alfa.txt` e `beta.md`.".into()));
        assert!(matches!(&evs[8], AgentEvent::Result { ok: true, .. }));
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
        // Com `usage` no aviso, os números vêm antes do fim.
        p.parse(N_A);
        p.parse(launched);
        let with_usage = r#"{"type":"system","subtype":"task_notification","tool_use_id":"tA","status":"completed","summary":"ok","usage":{"total_tokens":120,"tool_uses":3,"duration_ms":4000}}"#;
        assert_eq!(
            p.parse(with_usage),
            vec![
                AgentEvent::SubagentStats { id: "tA".into(), duration_ms: Some(4000), tokens: Some(120), tool_uses: Some(3) },
                AgentEvent::SubagentEnd { id: "tA".into(), ok: true, result: "ok".into() },
            ]
        );
    }

    // Mensagem `assistant` no formato do stream-json (claude 2.1.x): a mesma
    // mensagem sai numa linha por bloco, repetindo id e `usage`.
    const A1_TEXT: &str = r#"{"type":"assistant","message":{"id":"msg_01A","model":"claude-sonnet-4-5","content":[{"type":"text","text":"Vou ler"}],"usage":{"input_tokens":3,"cache_creation_input_tokens":2000,"cache_read_input_tokens":15000,"output_tokens":8}},"parent_tool_use_id":null,"session_id":"s1"}"#;
    const A1_TOOL: &str = r#"{"type":"assistant","message":{"id":"msg_01A","model":"claude-sonnet-4-5","content":[{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"a.txt"}}],"usage":{"input_tokens":3,"cache_creation_input_tokens":2000,"cache_read_input_tokens":15000,"output_tokens":61}},"parent_tool_use_id":null,"session_id":"s1"}"#;
    const U1: &str = r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":"a"}]},"parent_tool_use_id":null,"session_id":"s1"}"#;
    const A2: &str = r#"{"type":"assistant","message":{"id":"msg_01B","model":"claude-sonnet-4-5","content":[{"type":"tool_use","id":"toolu_2","name":"Bash","input":{"command":"ls"}}],"usage":{"input_tokens":1,"cache_creation_input_tokens":300,"cache_read_input_tokens":17000,"output_tokens":40}},"parent_tool_use_id":null,"session_id":"s1"}"#;
    const A_SUB: &str = r#"{"type":"assistant","message":{"id":"msg_01S","model":"claude-haiku-4-5-20251001","content":[{"type":"text","text":"sub"}],"usage":{"input_tokens":5,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":7}},"parent_tool_use_id":"toolu_9","session_id":"s1"}"#;

    fn feed(t: &mut dyn TurnTracker, p: &mut StreamParser, lines: &[&str]) {
        for l in lines {
            let evs = p.parse(l);
            t.observe(l, &evs);
        }
    }

    #[test]
    fn cancelled_turn_keeps_partial_usage() {
        let prov = ClaudeProvider::new();
        let mut t = prov.turn_tracker();
        let mut p = StreamParser::new();
        feed(t.as_mut(), &mut p, &[INIT, A1_TEXT, A1_TOOL, U1, A2, A_SUB]);
        let evs = t.finish(TurnEnd::Cancelled);
        let sonnet = TokenUsage {
            input_tokens: 4,
            output_tokens: 101, // 61 (maior valor da msg_01A) + 40
            cache_read: 32000,
            cache_write: 2300,
            model: Some("claude-sonnet-4-5".into()),
            cost_usd: None,
        };
        let haiku = TokenUsage { input_tokens: 5, output_tokens: 7, model: Some("claude-haiku-4-5-20251001".into()), ..Default::default() };
        assert_eq!(evs, vec![AgentEvent::Usage(sonnet), AgentEvent::Usage(haiku)]);
        // Nada é devolvido duas vezes.
        assert!(t.finish(TurnEnd::Cancelled).is_empty());
    }

    #[test]
    fn result_covers_partial_usage() {
        let prov = ClaudeProvider::new();
        let mut p = StreamParser::new();
        // Turno completo: o `result` já informa tudo, nada sobra no fim.
        let mut t = prov.turn_tracker();
        feed(t.as_mut(), &mut p, &[INIT, A1_TEXT, A1_TOOL, U1, RESULT]);
        assert!(t.finish(TurnEnd::Exited { ok: true }).is_empty());
        // Depois do `result` a CLI acordou (comando em segundo plano) e foi cancelada
        // antes do segundo `result`: só o uso visto depois do primeiro entra.
        let mut t = prov.turn_tracker();
        feed(t.as_mut(), &mut p, &[A1_TEXT, RESULT, A2]);
        let evs = t.finish(TurnEnd::Cancelled);
        assert_eq!(evs.len(), 1);
        assert!(matches!(&evs[0], AgentEvent::Usage(u) if u.output_tokens == 40 && u.cache_read == 17000));
    }

    #[test]
    fn empty_result_is_not_an_event() {
        // Processo retomado: `result` de zero turnos, sem texto e sem custo, antes
        // de o modelo ser chamado.
        let empty = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":4,"num_turns":0,"result":"","stop_reason":null,"session_id":"s2","total_cost_usd":0,"usage":{"input_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":0},"modelUsage":{},"permission_denials":[]}"#;
        let mut p = StreamParser::new();
        assert!(p.parse(empty).is_empty());
        // `result` repetido com o mesmo acumulado do processo, sem texto: também some.
        let r2 = RESULT.replace(r#""result":"oi \"aspas\" 100%""#, r#""result":"""#);
        assert_ne!(r2, RESULT);
        assert!(p.parse(RESULT).iter().any(|e| matches!(e, AgentEvent::Result { .. })));
        assert!(p.parse(&r2).is_empty());
        // Resposta ao `initialize` e linhas de sistema nunca viram resultado.
        let ctl = r#"{"type":"control_response","response":{"subtype":"success","request_id":"r1","response":{"models":[]}}}"#;
        let st = r#"{"type":"system","subtype":"status","status":null,"session_id":"s2"}"#;
        assert!(p.parse(ctl).is_empty());
        assert!(p.parse(st).is_empty());
        // Falha sem texto continua sendo resultado (o turno falhou).
        let err = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"num_turns":0,"result":"","session_id":"s3","total_cost_usd":0,"modelUsage":{}}"#;
        assert!(matches!(p.parse(err).as_slice(), [AgentEvent::Result { ok: false, .. }]));
        // Comando resolvido pela CLI (`/cost`): sem uso, mas com texto.
        let local = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":0,"result":"Total cost: $0.00","session_id":"s4","total_cost_usd":0,"modelUsage":{}}"#;
        assert!(matches!(p.parse(local).as_slice(), [AgentEvent::Result { ok: true, .. }]));
    }
}
