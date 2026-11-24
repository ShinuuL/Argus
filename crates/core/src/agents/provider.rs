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
    pub fn args(mut self, a: impl IntoIterator<Item = String>) -> Self {
        self.args.extend(a);
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

impl TokenUsage {
    /// Nenhum token e nenhum custo: o turno não chamou modelo algum.
    pub fn is_empty(&self) -> bool {
        self.input_tokens == 0
            && self.output_tokens == 0
            && self.cache_read == 0
            && self.cache_write == 0
            && self.cost_usd.unwrap_or(0.0) <= 0.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentEvent {
    /// Id de sessão da CLI (para continuar a conversa).
    Session(String),
    /// Mensagem enviada pelo usuário (turnos de resposta).
    User(String),
    Text(String),
    /// A conversa foi compactada (resumo no lugar do histórico): tamanho do
    /// contexto antes e depois, e se foi pedida (`manual`) ou feita pela CLI (`auto`).
    Compacted { pre_tokens: u64, post_tokens: u64, trigger: String },
    /// O Argus vai compactar a conversa antes de mandar a mensagem. `reason` é
    /// `size` (passou do limite) ou `idle` (parada além da vida do cache).
    ///
    /// Só ao vivo, não vai ao histórico: serve para a tela dizer por que a CLI
    /// vai ficar muda por alguns minutos antes de começar a responder. O fim
    /// vem no [`AgentEvent::Compacted`].
    Compacting { reason: String },
    /// Pedaço de texto ainda sendo gerado (claude com `--include-partial-messages`).
    /// Só vai ao vivo para a tela: não é gravado, e o `Text` completo vem depois.
    TextDelta(String),
    /// Pedaço do raciocínio ainda sendo gerado (`thinking_delta`). Como o
    /// `TextDelta`: só ao vivo, o `Thinking` completo vem depois.
    ThinkingDelta(String),
    /// Raciocínio do modelo (bloco `thinking`), mostrado recolhido no chat.
    Thinking(String),
    ToolUse { name: String, input: serde_json::Value },
    /// Saída de uma ferramenta comum (não de subagente). Vem na mesma ordem dos
    /// `ToolUse` do mesmo dono; `output` é cortado em [`TOOL_OUTPUT_MAX`] caracteres.
    ToolResult { ok: bool, output: String },
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
    /// Números que a CLI informa no fim do subagente `id` (vem logo antes do
    /// `SubagentEnd`): duração total, tokens (claude: `totalTokens`, o tamanho
    /// do contexto da última chamada do subagente) e quantas ferramentas usou.
    SubagentStats { id: String, duration_ms: Option<u64>, tokens: Option<u64>, tool_uses: Option<u64> },
    /// Evento interno do subagente `subagent` (texto, ferramenta...). Os eventos
    /// do agente principal continuam vindo sem este invólucro.
    Scoped { subagent: String, event: Box<AgentEvent> },
    /// O usuário trocou o agente (provedor) da conversa: `from` e `to` são ids de
    /// provedor. Vira a linha divisória "Agente trocado: X → Y" no chat.
    ProviderSwitch { from: String, to: String },
    /// A CLI negou uma ferramenta por falta de permissão. `tool` é o nome da
    /// ferramenta ("Bash", "PowerShell"...) e `command` o comando pedido (vazio
    /// quando a ferramenta não é de shell).
    Denied { tool: String, command: String },
    /// Ocupação da janela de contexto: tokens do último pedido do agente
    /// principal ao modelo. Não vai para o histórico: o último valor fica na tarefa.
    Context(crate::agents::context::ContextUsage),
    /// Limite de uso do plano informado pela CLI (claude: `rate_limit_event`):
    /// janela (`five_hour`, `seven_day`...), fração usada (0 a 1), quando
    /// reinicia (segundos desde 1970) e o estado (`allowed`, `allowed_warning`,
    /// `rejected`). Não vai para o histórico: o último valor de cada janela fica
    /// nas configurações.
    /// Aviso da CLI que vale mostrar: nova tentativa na API, hook que falhou,
    /// notificação. `level`: "info", "warn" ou "err".
    Notice { level: String, text: String },
    /// Andamento de um subagente (claude `task_progress`): o que ele está
    /// fazendo agora e os números até aqui. Só ao vivo, não vai ao histórico.
    SubagentProgress { id: String, activity: String, tokens: Option<u64>, tool_uses: Option<u64>, duration_ms: Option<u64> },
    RateLimit { window: String, utilization: Option<f64>, resets_at: Option<u64>, status: String },
}

/// Máximo de caracteres guardados da saída de uma ferramenta (`ToolResult`).
pub const TOOL_OUTPUT_MAX: usize = 4000;

/// Opções de um turno, escolhidas por tarefa.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TurnOptions {
    /// Modelo pedido à CLI (claude: `--model`; opencode: `-m`). None = padrão da CLI.
    pub model: Option<String>,
    /// Aviso curto sobre o ambiente (cópia isolada do projeto). O claude o recebe
    /// por `--append-system-prompt`; quem não tem isso prefixa o primeiro pedido.
    pub system_note: Option<String>,
    /// Modo de acesso da conversa (Planejar / normal / Bypass).
    pub access: crate::permissions::AccessMode,
    /// Nível de esforço (claude: `--effort`). None = padrão da CLI.
    pub effort: Option<String>,
    /// Teto de custo do turno em US$ (claude: `--max-budget-usd`).
    pub max_budget_usd: Option<f64>,
    /// Modelo reserva quando o principal está sobrecarregado (claude: `--fallback-model`).
    pub fallback_model: Option<String>,
    /// Retoma a sessão numa cópia nova (claude: `--fork-session`): a original fica como estava.
    pub fork: bool,
}

/// Um modelo que a CLI aceita.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Valor passado à CLI.
    pub value: String,
    /// Nome para exibir.
    pub label: String,
    /// Segunda linha: descrição informada pela CLI ou o provedor (pode ser vazia).
    pub hint: String,
    /// Grupo na lista (provedor do prefixo "provedor/"; vazio = sem grupo).
    #[serde(default)]
    pub group: String,
    /// Selo curto ao lado do nome (ex.: "1M" para janela de 1M tokens).
    #[serde(default)]
    pub badge: String,
    /// O item representa o padrão da CLI (não é passado em `--model`): só o
    /// rótulo e a dica (qual modelo ele resolve) são usados.
    #[serde(default)]
    pub default: bool,
}

impl ModelInfo {
    pub fn new(value: impl Into<String>, label: impl Into<String>, hint: impl Into<String>) -> Self {
        ModelInfo { value: value.into(), label: label.into(), hint: hint.into(), group: String::new(), badge: String::new(), default: false }
    }
}

/// Nome legível a partir do id de um modelo, sem inventar nada além de caixa e
/// separadores: "openai/gpt-6-luna" -> "GPT-6 Luna", "claude-sonnet-4-5" ->
/// "Claude Sonnet 4.5", "llama-3.1-70b-instruct" -> "Llama 3.1 70B Instruct".
pub fn pretty_model_name(id: &str) -> String {
    const UPPER: &[&str] = &["gpt", "glm", "oss", "ai", "vl", "llm", "qwq", "moe", "api", "tts"];
    let name = id.rsplit('/').next().unwrap_or(id);
    let mut out = String::new();
    let mut prev: Option<&str> = None;
    for raw in name.split(['-', '_', ' ']).filter(|t| !t.is_empty()) {
        let lower = raw.to_ascii_lowercase();
        let digit_start = raw.starts_with(|c: char| c.is_ascii_digit());
        let word = if UPPER.contains(&lower.as_str()) || (digit_start && raw.len() <= 5 && raw.ends_with(['b', 'k', 'm'])) {
            raw.to_ascii_uppercase()
        } else {
            let mut c = raw.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        };
        let all_digits = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit());
        let sep = match prev {
            None => "",
            Some(p) if all_digits(raw) && all_digits(p) && raw.len() <= 2 && p.len() <= 2 => ".",
            Some(p) if digit_start && UPPER.contains(&p.to_ascii_lowercase().as_str()) => "-",
            _ => " ",
        };
        out.push_str(sep);
        out.push_str(&word);
        prev = Some(raw);
    }
    if out.is_empty() { id.to_string() } else { out }
}

#[cfg(test)]
mod pretty_tests {
    use super::pretty_model_name as p;
    #[test]
    fn pretty_names() {
        assert_eq!(p("openai/gpt-6-luna"), "GPT-6 Luna");
        assert_eq!(p("anthropic/claude-sonnet-4-5"), "Claude Sonnet 4.5");
        assert_eq!(p("gemini-2.5-pro"), "Gemini 2.5 Pro");
        assert_eq!(p("meta/llama-3.1-70b-instruct"), "Llama 3.1 70B Instruct");
        assert_eq!(p("x/gpt-oss-120b"), "GPT OSS-120B");
        assert_eq!(p("claude-haiku-4-5-20251001"), "Claude Haiku 4.5 20251001");
    }
}

/// Um nome de modelo pode virar argumento de linha de comando? Recusa vazio,
/// espaços, caracteres de controle e o que começa com `-` (seria lido como opção).
pub fn valid_model_name(m: &str) -> bool {
    !m.is_empty()
        && m.len() <= 200
        && !m.starts_with('-')
        && m.chars().all(|c| c.is_ascii_alphanumeric() || "._-/:[]#@+".contains(c))
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
            AgentEvent::SubagentStart { id, .. }
            | AgentEvent::SubagentEnd { id, .. }
            | AgentEvent::SubagentStats { id, .. } => Some(id),
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
            AgentEvent::SubagentStats { .. } => "SubagentStats",
            AgentEvent::Scoped { event, .. } => event.kind(),
            AgentEvent::Session(_) => "Session",
            AgentEvent::User(_) => "User",
            AgentEvent::Text(_) => "Text",
            AgentEvent::TextDelta(_) => "TextDelta",
            AgentEvent::Compacted { .. } => "Compacted",
            AgentEvent::Compacting { .. } => "Compacting",
            AgentEvent::RateLimit { .. } => "RateLimit",
            AgentEvent::ThinkingDelta(_) => "ThinkingDelta",
            AgentEvent::Notice { .. } => "Notice",
            AgentEvent::SubagentProgress { .. } => "SubagentProgress",
            AgentEvent::ToolUse { .. } => "ToolUse",
            AgentEvent::Thinking(_) => "Thinking",
            AgentEvent::ToolResult { .. } => "ToolResult",
            AgentEvent::Result { .. } => "Result",
            AgentEvent::Usage(_) => "Usage",
            AgentEvent::Raw(_) => "Raw",
            AgentEvent::ProviderSwitch { .. } => "ProviderSwitch",
            AgentEvent::Denied { .. } => "Denied",
            AgentEvent::Context(_) => "Context",
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
    fn task_command(&self, prompt: &str, cwd: &Path, session: Option<&str>, opts: &TurnOptions) -> CommandSpec;
    /// Comando de um host que fica vivo entre turnos. `None` (padrão) = um
    /// processo por turno, como sempre. Quando há host, o prompt NÃO vai em
    /// `stdin`: cada turno é uma mensagem enviada por [`Self::host_message`].
    fn host_command(&self, _cwd: &Path, _session: Option<&str>, _opts: &TurnOptions) -> Option<CommandSpec> {
        None
    }
    /// Linha enviada ao host para começar um turno. `id` correlaciona a mensagem
    /// (a CLI do claude aceita `uuid` e o devolve no `result` quando o campo vem).
    fn host_message(&self, prompt: &str, id: &str) -> String {
        serde_json::json!({
            "type": "user",
            "uuid": id,
            "message": { "role": "user", "content": prompt },
            "parent_tool_use_id": null
        })
        .to_string()
    }
    /// Verdadeiro quando uma linha do host fecha o turno atual.
    fn host_turn_ended(&self, _line: &str) -> bool {
        false
    }
    /// Aquece o que a CLI carrega ao iniciar (flags do `--help`, lista de
    /// modelos). Roda em background na abertura do app. Falha nunca atrapalha
    /// o turno: é só precaução de latência, não parte da conversa.
    fn warmup(&self) {}

    /// Modelo inicial das conversas novas deste provedor (None = padrão da CLI).
    fn default_model(&self) -> Option<String> {
        None
    }
    /// Modelos que a CLI aceita, perguntando a ela (bloqueante; pode levar
    /// segundos). Não inclui o "padrão da CLI".
    fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        Ok(Vec::new())
    }
    /// Uma linha de saída pode gerar vários eventos.
    fn parse_line(&self, line: &str) -> Vec<AgentEvent>;
    /// Avisa que um processo novo da CLI vai começar para a sessão `session`
    /// (None = sessão nova). Provedores cujo parser guarda estado por processo
    /// (totais acumulados) zeram esse estado aqui.
    fn begin_turn(&self, _session: Option<&str>) {}
    /// Acompanhante do turno: vê cada linha da CLI e, quando o processo acaba
    /// (normalmente ou cancelado), devolve os eventos que faltaram (uso parcial,
    /// resultado que a CLI não imprimiu). Um por processo.
    fn turn_tracker(&self) -> Box<dyn TurnTracker> {
        Box::new(NoTracker)
    }
}

/// Como o processo do turno terminou, para o [`TurnTracker`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnEnd {
    /// Saiu sozinho; `ok` = código de saída zero.
    Exited { ok: bool },
    /// Morto pelo cancelamento do turno.
    Cancelled,
}

/// Estado de um único processo de turno (ver [`AgentProvider::turn_tracker`]).
pub trait TurnTracker: Send {
    /// Linha crua da CLI e os eventos que o parser tirou dela.
    fn observe(&mut self, line: &str, events: &[AgentEvent]);
    /// Ajusta os eventos de uma linha antes de gravar (chamado depois de
    /// `observe`): juntar pedaços de texto (gemini), trocar totais acumulados por
    /// diferenças (codex). Padrão: não mexe.
    fn transform(&mut self, events: Vec<AgentEvent>) -> Vec<AgentEvent> {
        events
    }
    /// Eventos a gravar no fim do processo.
    fn finish(&mut self, end: TurnEnd) -> Vec<AgentEvent>;
}

/// Acompanhante que não acrescenta nada.
pub struct NoTracker;

impl TurnTracker for NoTracker {
    fn observe(&mut self, _line: &str, _events: &[AgentEvent]) {}
    fn finish(&mut self, _end: TurnEnd) -> Vec<AgentEvent> {
        Vec::new()
    }
}

/// Localiza um executável no PATH, como o shell faria: percorre o PATH em
/// ordem e, em cada diretório, tenta as extensões na ordem de PATHEXT. O
/// primeiro achado vence, então um `.cmd` numa pasta anterior do PATH ganha de
/// um `.exe` numa pasta posterior.
pub fn which_bin(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let pathext = std::env::var("PATHEXT").ok();
    which_in(name, &path, pathext.as_deref())
}

/// Extensões que o Windows consegue executar (o resto da PATHEXT, como `.ps1`
/// ou `.vbs`, não roda por `CreateProcess` nem por `cmd /c` de forma útil).
const RUNNABLE_EXTS: [&str; 4] = ["com", "exe", "bat", "cmd"];

/// Núcleo de `which_bin`, com PATH e PATHEXT por parâmetro (para testes).
/// `pathext` = None usa o padrão do Windows. Fora do Windows, a PATHEXT é
/// ignorada e vale o primeiro arquivo com o nome exato.
pub(crate) fn which_in(name: &str, path: &std::ffi::OsStr, pathext: Option<&str>) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    let exts: Vec<String> = pathext
        .unwrap_or(".COM;.EXE;.BAT;.CMD")
        .split(';')
        .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|e| RUNNABLE_EXTS.contains(&e.as_str()))
        .collect();
    // Nome que já traz uma extensão executável (`git.exe`) é procurado como veio.
    let has_ext = Path::new(name)
        .extension()
        .is_some_and(|e| RUNNABLE_EXTS.contains(&e.to_string_lossy().to_ascii_lowercase().as_str()));
    for dir in std::env::split_paths(path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        if cfg!(windows) {
            if has_ext {
                let c = dir.join(name);
                if c.is_file() {
                    return Some(c);
                }
                continue;
            }
            for e in &exts {
                let c = dir.join(format!("{name}.{e}"));
                if c.is_file() {
                    return Some(c);
                }
            }
        } else {
            let c = dir.join(name);
            if c.is_file() {
                return Some(c);
            }
        }
    }
    None
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

/// Como `run_capture`, devolvendo só a saída padrão (stderr descartado).
pub(crate) fn run_capture_stdout(program: &Path, args: &[&str]) -> Option<(bool, String)> {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    Some((out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned()))
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

    /// Cria `dir/arquivo` vazio.
    fn touch(dir: &Path, file: &str) -> PathBuf {
        let p = dir.join(file);
        std::fs::write(&p, b"").unwrap();
        p
    }

    fn join(dirs: &[&Path]) -> std::ffi::OsString {
        std::env::join_paths(dirs).unwrap()
    }

    #[cfg(windows)]
    #[test]
    fn which_respects_path_order_over_exe() {
        let npm = tempfile::tempdir().unwrap();
        let bun = tempfile::tempdir().unwrap();
        touch(npm.path(), "opencode"); // script de shell sem extensão: ignorado
        let cmd = touch(npm.path(), "opencode.cmd");
        let exe = touch(bun.path(), "opencode.exe");
        let ext = Some(".COM;.EXE;.BAT;.CMD");
        assert_eq!(which_in("opencode", &join(&[npm.path(), bun.path()]), ext), Some(cmd.clone()));
        assert_eq!(which_in("opencode", &join(&[bun.path(), npm.path()]), ext), Some(exe));
        assert_eq!(which_in("nada", &join(&[npm.path(), bun.path()]), ext), None);
        // Só o script sem extensão: não é executável no Windows.
        let so = tempfile::tempdir().unwrap();
        touch(so.path(), "opencode");
        assert_eq!(which_in("opencode", &join(&[so.path()]), ext), None);
        let _ = cmd;
    }

    #[cfg(windows)]
    #[test]
    fn which_follows_pathext_order_within_dir() {
        let d = tempfile::tempdir().unwrap();
        let exe = touch(d.path(), "tool.exe");
        let cmd = touch(d.path(), "tool.cmd");
        touch(d.path(), "tool.ps1");
        let p = join(&[d.path()]);
        assert_eq!(which_in("tool", &p, Some(".COM;.EXE;.BAT;.CMD")), Some(exe.clone()));
        assert_eq!(which_in("tool", &p, Some(".cmd;.exe")), Some(cmd.clone()));
        // `.ps1` na PATHEXT não conta: não roda direto.
        assert_eq!(which_in("tool", &p, Some(".PS1;.CMD")), Some(cmd));
        // Sem PATHEXT: padrão do Windows (.EXE antes de .CMD).
        assert_eq!(which_in("tool", &p, None), Some(exe.clone()));
        // Nome com extensão é procurado como veio.
        assert_eq!(which_in("tool.exe", &p, Some(".CMD")), Some(exe));
    }

    #[cfg(not(windows))]
    #[test]
    fn which_takes_first_in_path_unix() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let fb = touch(b.path(), "tool");
        assert_eq!(which_in("tool", &join(&[a.path(), b.path()]), None), Some(fb));
        let fa = touch(a.path(), "tool");
        assert_eq!(which_in("tool", &join(&[a.path(), b.path()]), Some(".EXE")), Some(fa));
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
    fn model_names_are_checked() {
        for ok in ["opus", "opus[1m]", "claude-fable-5-2[1m]", "openai/gpt-6-luna", "provedor/modelo#variante"] {
            assert!(valid_model_name(ok), "{ok}");
        }
        for bad in ["", "--dangerously", "a b", "a\nb", "x;y", "a\"b"] {
            assert!(!valid_model_name(bad), "{bad}");
        }
        let sw = AgentEvent::ProviderSwitch { from: "claude".into(), to: "opencode".into() };
        assert_eq!(serde_json::to_string(&sw).unwrap(), r#"{"ProviderSwitch":{"from":"claude","to":"opencode"}}"#);
        assert_eq!(sw.kind(), "ProviderSwitch");
    }

    #[test]
    fn strips_ansi() {
        assert_eq!(strip_ansi("\u{1b}[1;32mok\u{1b}[0m x"), "ok x");
    }
}
