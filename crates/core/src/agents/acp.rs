//! Cliente do Agent Client Protocol (ACP) para `opencode acp`.
//!
//! O `opencode run` não é interativo: uma regra de permissão `ask` é recusada
//! pela própria CLI ("cannot ask the user for permission") e a pergunta chega
//! ao Argus já como respondida. O `opencode acp` mantém um processo vivo e
//! manda `session/request_permission` e `elicitation/create` para o cliente;
//! aqui o Argus é esse cliente e pode esperar a escolha do usuário.
//!
//! Transporte: JSON-RPC 2.0, uma mensagem por linha, em stdin/stdout. A sessão
//! é criada com `cwd` absoluto; o modelo é escolhido por
//! `session/set_config_option`. Nenhum token é lido: a CLI cuida do login.
//!
//! Formatos conferidos em https://opencode.ai/docs/acp/ e no schema v1 do ACP.

use super::context::ContextUsage;
use super::provider::{AgentEvent, AgentProvider, CommandSpec, LoginStatus, ModelInfo, TokenUsage, TurnOptions, which_bin};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStdin};
use tokio::sync::mpsc;

/// Versão do protocolo que o Argus fala (a que o opencode acp implementa).
pub const PROTOCOL_VERSION: u32 = 1;
/// Nome da ferramenta sintética que vira cartão de perguntas na interface.
pub const ASK_TOOL: &str = "AskUserQuestion";

/// Uma mensagem recebida do agente. Pedidos têm `id` **e** `method`; respostas
/// têm `id` e `result`/`error`; notificações só têm `method`.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Request { id: String, method: String, params: Value },
    Response { id: String, result: Value, error: Option<String> },
    Notification { method: String, params: Value },
}

/// Id do JSON-RPC como texto: aceita número e string, sem perder a chave.
pub fn id_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Null => Some("null".into()),
        _ => None,
    }
}

/// Lê uma linha do ACP. Linhas que não são JSON-RPC 2.0 (log, aviso) são
/// descartadas: stdout do `opencode acp` só deve conter protocolo.
pub fn parse_message(line: &str) -> Option<Message> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    if v.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return None;
    }
    let method = v.get("method").and_then(Value::as_str).map(str::to_string);
    let id = v.get("id").and_then(id_text);
    let params = v.get("params").cloned().unwrap_or(Value::Null);
    match (id, method) {
        (Some(id), Some(method)) => Some(Message::Request { id, method, params }),
        (Some(id), None) => {
            let error = v.get("error").and_then(|e| {
                e.get("message").and_then(Value::as_str).map(str::to_string).or_else(|| Some(e.to_string()))
            });
            Some(Message::Response { id, result: v.get("result").cloned().unwrap_or(Value::Null), error })
        }
        (None, Some(method)) => Some(Message::Notification { method, params }),
        (None, None) => None,
    }
}

fn rpc(method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "method": method, "params": params }).to_string()
}

fn request(id: &str, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

/// JSON-RPC exige o mesmo tipo de `id` do pedido. O opencode usa número (`0`);
/// devolver string (`"0"`) faz a CLI descartar a resposta como "unknown request".
fn respond(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

/// `initialize`: pede perguntas em formulário para o opencode usar
/// `elicitation/create` em vez de recusar a pergunta.
pub fn initialize_line(id: &str) -> String {
    request(
        id,
        "initialize",
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "clientInfo": { "name": "Argus", "version": env!("CARGO_PKG_VERSION") },
            "clientCapabilities": {
                // ACP 1: `form` é um objeto vazio, não um booleano. Com `true`
                // o opencode 2.3.0 não oferece a ferramenta de pergunta e
                // responde "The question couldn't be shown to the user".
                "elicitation": { "form": {} },
                "fs": { "readTextFile": false, "writeTextFile": false },
                "terminal": false,
                "auth": { "terminal": false }
            }
        }),
    )
}

fn session_params(cwd: &Path, dirs: &[PathBuf], session: Option<&str>) -> Value {
    let mut v = json!({ "cwd": cwd.to_string_lossy(), "mcpServers": [] });
    if let Some(id) = session {
        v["sessionId"] = json!(id);
    }
    if !dirs.is_empty() {
        v["additionalDirectories"] = json!(dirs.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>());
    }
    v
}

pub fn new_session_line(id: &str, cwd: &Path, dirs: &[PathBuf]) -> String {
    request(id, "session/new", session_params(cwd, dirs, None))
}

pub fn load_session_line(id: &str, session: &str, cwd: &Path, dirs: &[PathBuf]) -> String {
    request(id, "session/load", session_params(cwd, dirs, Some(session)))
}

pub fn set_model_line(id: &str, session: &str, model: &str) -> String {
    request(id, "session/set_config_option", json!({
        "sessionId": session,
        "configId": "model",
        "value": model
    }))
}

pub fn prompt_line(id: &str, session: &str, text: &str) -> String {
    request(
        id,
        "session/prompt",
        json!({ "sessionId": session, "prompt": [{ "type": "text", "text": text }] }),
    )
}

/// Cancelamento é notificação: o agente responde ao prompt com
/// `stopReason: "cancelled"` e continua vivo para o próximo turno.
pub fn cancel_line(session: &str) -> String {
    rpc("session/cancel", json!({ "sessionId": session }))
}

/// O resultado de `session/request_permission` embrulha o desfecho em
/// `outcome`: `{ "outcome": { "outcome": "selected", "optionId": "..." } }`.
/// Plano, o opencode 2.3.0 interpreta como recusa e a ferramenta falha com
/// "The user declined this tool call" (confirmado com sonda ACP ao vivo).
pub fn permission_answer(id: &Value, option_id: &str) -> String {
    respond(id, json!({ "outcome": { "outcome": "selected", "optionId": option_id } }))
}

pub fn permission_cancelled(id: &Value) -> String {
    respond(id, json!({ "outcome": { "outcome": "cancelled" } }))
}

/// Resposta de `elicitation/create`. `content` é o objeto do schema pedido;
/// None recusa a pergunta (o agente continua sem resposta).
pub fn elicitation_answer(id: &Value, content: Option<Value>) -> String {
    match content {
        Some(content) => respond(id, json!({ "action": "accept", "content": content })),
        None => respond(id, json!({ "action": "decline" })),
    }
}

/// Texto de um bloco de conteúdo do ACP (texto, raciocínio ou recurso com texto).
fn block_text(v: &Value) -> String {
    match v.get("type").and_then(Value::as_str) {
        Some("text") | Some("thinking") => v.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
        Some("resource") => v.pointer("/resource/text").and_then(Value::as_str).unwrap_or("").to_string(),
        _ => String::new(),
    }
}

/// Converte uma notificação `session/update` em eventos do Argus. Update que
/// não tem representação no chat (comandos disponíveis, retry, plano) é
/// ignorado: não invencionar mensagem para o usuário.
pub fn update_events(update: &Value) -> Vec<AgentEvent> {
    let kind = update.get("sessionUpdate").and_then(Value::as_str).unwrap_or("");
    // O texto vem em `content` (um bloco), não na raiz do update.
    let chunk = update.get("content").unwrap_or(update);
    match kind {
        "agent_message_chunk" => {
            let text = block_text(chunk);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![AgentEvent::TextDelta(text)]
            }
        }
        "agent_thought_chunk" => {
            let text = block_text(chunk);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![AgentEvent::ThinkingDelta(text)]
            }
        }
        "user_message_chunk" => {
            let text = block_text(chunk);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![AgentEvent::User(text)]
            }
        }
        "tool_call" | "tool_call_update" => {
            let title = update.get("title").and_then(Value::as_str).unwrap_or("");
            let name = if title.is_empty() {
                update.get("kind").and_then(Value::as_str).unwrap_or("tool").to_string()
            } else {
                title.to_string()
            };
            if kind == "tool_call" {
                let mut input = json!({});
                if let Some(raw) = update.get("rawInput") {
                    input["input"] = raw.clone();
                }
                if let Some(meta) = update.get("_meta") {
                    input["meta"] = meta.clone();
                }
                return vec![AgentEvent::ToolUse { name, input }];
            }
            let status = update.get("status").and_then(Value::as_str).unwrap_or("");
            if status == "completed" || status == "failed" {
                let output = match update.get("content") {
                    Some(Value::Array(list)) => list.iter().map(block_text).collect::<String>(),
                    Some(v) => block_text(v),
                    None => String::new(),
                };
                vec![AgentEvent::ToolResult { ok: status == "completed", output }]
            } else {
                Vec::new()
            }
        }
        "usage_update" => {
            let used = update.get("used").and_then(Value::as_u64).unwrap_or(0);
            let size = update.get("size").and_then(Value::as_u64);
            if used == 0 {
                Vec::new()
            } else {
                vec![AgentEvent::Context(ContextUsage {
                    input_tokens: used,
                    model: None,
                    window: size,
                    ..Default::default()
                })]
            }
        }
        "compaction_update" => vec![AgentEvent::Notice { level: "info".into(), text: "Conversa compactada pelo agente.".into() }],
        _ => Vec::new(),
    }
}

/// Uma pergunta pendente do ACP. Um pedido `elicitation/create` pode trazer
/// várias perguntas no mesmo formulário: elas viram abas de um único cartão e
/// são respondidas juntas, numa só resposta JSON-RPC.
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    /// Id do pedido JSON-RPC como texto (chave do cartão na interface).
    pub id: String,
    /// Id original, com o tipo JSON do pedido (número ou string).
    pub id_json: Value,
    pub kind: AskKind,
    pub parts: Vec<AskPart>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AskPart {
    pub question: String,
    pub options: Vec<AskChoice>,
    pub multi: bool,
    /// O schema oferece `<campo>_custom` para texto livre nesta pergunta.
    pub custom: bool,
    /// Chave do campo no schema (só em `elicitation`).
    pub field: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AskChoice {
    /// Valor enviado de volta ao ACP (`optionId` ou o `const` do schema).
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskKind {
    Permission,
    Elicitation,
}

/// `session/request_permission`: o toolCall diz o que será feito; as opções
/// vêm em `options` (`once`, `always`, `reject` no opencode).
pub fn permission_ask(id: &Value, params: &Value) -> Option<Ask> {
    let call = params.get("toolCall");
    let title = call.and_then(|c| c.get("title")).and_then(Value::as_str).unwrap_or("");
    let kind = call.and_then(|c| c.get("kind")).and_then(Value::as_str).unwrap_or("ferramenta");
    let options = params.get("options")?.as_array()?;
    let choices: Vec<AskChoice> = options
        .iter()
        .filter_map(|o| {
            let value = o.get("optionId")?.as_str()?;
            let label = o.get("name").and_then(Value::as_str).unwrap_or(value);
            Some(AskChoice {
                value: value.to_string(),
                label: label.to_string(),
                description: o.get("description").and_then(Value::as_str).map(str::to_string),
            })
        })
        .collect();
    if choices.is_empty() {
        return None;
    }
    let what = if title.is_empty() { kind } else { title };
    Some(Ask {
        id: id_text(id).unwrap_or_default(),
        id_json: id.clone(),
        kind: AskKind::Permission,
        parts: vec![AskPart {
            question: format!("Permitir {what}?"),
            options: choices,
            multi: false,
            custom: false,
            field: None,
        }],
    })
}

/// `elicitation/create` em modo formulário: cada propriedade com `oneOf` vira
/// uma pergunta. Todas as perguntas do pedido viram um cartão só, porque o ACP
/// espera **uma** resposta com o conteúdo completo do formulário.
pub fn elicitation_ask(id: &Value, params: &Value) -> Option<Ask> {
    let message = params.get("message").and_then(Value::as_str).unwrap_or("O agente pediu uma decisão.");
    let props = params.pointer("/requestedSchema/properties")?.as_object()?;
    let mut parts = Vec::new();
    for (field, prop) in props {
        if field.ends_with("_custom") {
            continue;
        }
        let Some(list) = prop.get("oneOf").and_then(Value::as_array) else { continue };
        let choices: Vec<AskChoice> = list
            .iter()
            .filter_map(|o| {
                let value = o.get("const")?.as_str()?;
                let label = o.get("title").and_then(Value::as_str).unwrap_or(value);
                Some(AskChoice {
                    value: value.to_string(),
                    label: label.to_string(),
                    description: o.get("description").and_then(Value::as_str).map(str::to_string),
                })
            })
            .collect();
        if !choices.is_empty() {
            parts.push(AskPart {
                question: if parts.is_empty() { message.to_string() } else { format!("{message} — {field}") },
                options: choices,
                multi: prop.get("type").and_then(Value::as_str) == Some("array"),
                custom: props.contains_key(&format!("{field}_custom")),
                field: Some(field.clone()),
            });
        }
    }
    (!parts.is_empty()).then(|| Ask { id: id_text(id).unwrap_or_default(), id_json: id.clone(), kind: AskKind::Elicitation, parts })
}

/// Evento `ToolUse` com os campos extras que a interface usa para responder
/// pelo ACP em vez de criar um novo turno. `questions` é o formato que
/// `parseAsk` já entende (um cartão com uma aba por pergunta).
pub fn ask_event(ask: &Ask) -> AgentEvent {
    let questions = ask
        .parts
        .iter()
        .map(|p| {
            let options = p
                .options
                .iter()
                .map(|o| json!({ "label": o.label, "description": o.description }))
                .collect::<Vec<_>>();
            json!({ "question": p.question, "options": options, "multi": p.multi })
        })
        .collect::<Vec<_>>();
    AgentEvent::ToolUse {
        name: ASK_TOOL.to_string(),
        input: json!({
            "questions": questions,
            "acp_request": ask.id,
            "acp_kind": if ask.kind == AskKind::Permission { "permission" } else { "elicitation" }
        }),
    }
}

/// Conteúdo do schema devolvido ao `elicitation/create`: um valor por campo,
/// na mesma ordem das perguntas do cartão. Texto livre vai para `<campo>_custom`
/// quando o schema oferece esse campo.
pub fn elicitation_content(ask: &Ask, groups: &[Vec<String>], extras: &[String]) -> Option<Value> {
    let mut content = serde_json::Map::new();
    for (part, (picked, extra)) in ask.parts.iter().zip(groups.iter().zip(extras.iter())) {
        let Some(field) = part.field.as_deref() else { continue };
        let values: Vec<&str> = picked
            .iter()
            .filter_map(|label| part.options.iter().find(|o| &o.label == label).map(|o| o.value.as_str()))
            .collect();
        if !values.is_empty() {
            content.insert(field.to_string(), if part.multi || values.len() > 1 {
                json!(values)
            } else {
                json!(values[0])
            });
        }
        if part.custom && !extra.trim().is_empty() {
            content.insert(format!("{field}_custom"), json!(extra.trim()));
        }
    }
    (!content.is_empty()).then(|| Value::Object(content))
}

/// Uso do prompt: o ACP devolve o total acumulado da sessão. O orquestrador
/// subtrai o último total para não contar duas vezes a mesma chamada.
pub fn prompt_usage(result: &Value) -> (u64, u64, Option<f64>) {
    let usage = result.get("usage").unwrap_or(&Value::Null);
    let input = usage.get("inputTokens").and_then(Value::as_u64).unwrap_or(0);
    let output = usage.get("outputTokens").and_then(Value::as_u64).unwrap_or(0);
    let cost = result.get("cost").and_then(|c| c.get("amount")).and_then(Value::as_f64);
    (input, output, cost)
}

/// `user_cancelled` distingue `stopReason: "cancelled"` pedido pelo usuário de
/// uma interrupção da própria CLI. Cancelamento pedido não é falha: sem isso o
/// chat mostra "Erro: o turno falhou. cancelled" depois de um "Parar" correto.
pub fn prompt_events(result: &Value, error: &Option<String>, previous: Option<(u64, u64)>, user_cancelled: bool) -> Vec<AgentEvent> {
    if let Some(msg) = error {
        return vec![AgentEvent::Result { ok: false, summary: msg.clone(), cost: None }];
    }
    let stop = result.get("stopReason").and_then(Value::as_str).unwrap_or("end_turn");
    let (input, output, cost) = prompt_usage(result);
    let delta = match previous {
        Some((pi, po)) => (input.saturating_sub(pi), output.saturating_sub(po)),
        None => (input, output),
    };
    let usage = TokenUsage { input_tokens: delta.0, output_tokens: delta.1, model: None, cost_usd: cost, ..Default::default() };
    let mut out = Vec::new();
    if !usage.is_empty() {
        out.push(AgentEvent::Usage(usage));
    }
    if !(user_cancelled && stop == "cancelled") {
        out.push(AgentEvent::Result { ok: stop != "cancelled", summary: stop.to_string(), cost });
    }
    out
}

/// Uma linha lida do host ACP. `Eof` impede um turno preso se a CLI sair.
pub enum Wire {
    Line(String),
    Eof,
}

/// Processo ACP vivo por conversa. Guarda a sessão e o último total de uso
/// para transformar o total acumulado do ACP em consumo por turno.
pub struct AcpHost {
    pub child: Child,
    pub stdin: Arc<tokio::sync::Mutex<ChildStdin>>,
    pub rx: Option<mpsc::UnboundedReceiver<Wire>>,
    pub session: Option<String>,
    /// Id do `initialize` enviado ao abrir o processo (para reconhecer a resposta).
    pub init_id: Option<String>,
    pub initialized: bool,
    pub next_id: u64,
    pub last_usage: Option<(u64, u64)>,
    pub cancelled: bool,
    /// Perguntas vivas deste host, por id do pedido JSON-RPC.
    pub asks: HashMap<String, Ask>,
    /// Arquivo onde as leituras escrevem; trocado no começo de cada turno.
    pub log: Arc<std::sync::Mutex<crate::agentlog::TurnLog>>,
}

impl AcpHost {
    pub fn set_log(&self, log: crate::agentlog::TurnLog) {
        *self.log.lock().unwrap_or_else(|e| e.into_inner()) = log;
    }

    pub fn new_id(&mut self) -> String {
        self.next_id += 1;
        format!("argus-acp-{}", self.next_id)
    }

    pub async fn write_line(&self, line: &str) -> std::io::Result<()> {
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(format!("{line}\n").as_bytes()).await?;
        stdin.flush().await
    }
}

/// Codifica as escolhas do cartão (uma lista por pergunta) na resposta ACP.
pub fn answer_line(ask: &Ask, groups: &[Vec<String>], extras: &[String]) -> Option<String> {
    match ask.kind {
        AskKind::Permission => {
            let value = groups.first()?.first()?;
            let option = ask.parts.first()?.options.iter().find(|o| &o.label == value)?;
            Some(permission_answer(&ask.id_json, &option.value))
        }
        AskKind::Elicitation => elicitation_content(ask, groups, extras).map(|content| elicitation_answer(&ask.id_json, Some(content))),
    }
}

/// Resposta de recusa (o agente continua sem uma escolha).
pub fn decline_line(ask: &Ask) -> String {
    match ask.kind {
        AskKind::Permission => permission_cancelled(&ask.id_json),
        AskKind::Elicitation => elicitation_answer(&ask.id_json, None),
    }
}

/// Provedor ACP do opencode. É registrado como `opencode-acp` para não mudar o
/// `opencode` atual (`opencode run`), que continua funcionando como antes.
#[derive(Debug, Default, Clone)]
pub struct AcpProvider {
    model: Option<String>,
}

impl AcpProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_model(model: impl Into<String>) -> Self {
        Self { model: Some(model.into()) }
    }
}

impl AgentProvider for AcpProvider {
    fn id(&self) -> &str {
        "opencode-acp"
    }

    fn display_name(&self) -> &str {
        "OpenCode (ACP)"
    }

    fn detect(&self) -> Option<PathBuf> {
        which_bin("opencode")
    }

    fn login_status(&self) -> LoginStatus {
        super::opencode::OpencodeProvider::new().login_status()
    }

    fn login_command(&self) -> CommandSpec {
        CommandSpec::new("opencode").arg("auth").arg("login")
    }

    /// Comando do host. O prompt NÃO vai em stdin: cada turno é uma requisição
    /// `session/prompt` dentro do mesmo processo.
    fn task_command(&self, _prompt: &str, cwd: &Path, _session: Option<&str>, _opts: &TurnOptions) -> CommandSpec {
        CommandSpec::new("opencode").arg("acp").cwd(cwd)
    }

    fn acp(&self) -> bool {
        true
    }

    fn parse_line(&self, line: &str) -> Vec<AgentEvent> {
        match parse_message(line) {
            Some(Message::Notification { method, params }) if method == "session/update" => {
                update_events(params.get("update").unwrap_or(&params))
            }
            _ => Vec::new(),
        }
    }

    fn default_model(&self) -> Option<String> {
        self.model.clone()
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        super::opencode::OpencodeProvider::new().list_models()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_requests_responses_and_notifications() {
        let req = parse_message(r#"{"jsonrpc":"2.0","id":7,"method":"session/request_permission","params":{"x":1}}"#);
        assert_eq!(req, Some(Message::Request { id: "7".into(), method: "session/request_permission".into(), params: json!({"x":1}) }));
        let res = parse_message(r#"{"jsonrpc":"2.0","id":"a","result":{"stopReason":"end_turn"}}"#);
        assert_eq!(res, Some(Message::Response { id: "a".into(), result: json!({"stopReason":"end_turn"}), error: None }));
        let err = parse_message(r#"{"jsonrpc":"2.0","id":2,"error":{"message":"auth_required"}}"#);
        assert_eq!(err, Some(Message::Response { id: "2".into(), result: Value::Null, error: Some("auth_required".into()) }));
        let note = parse_message(r#"{"jsonrpc":"2.0","method":"session/update","params":{}}"#);
        assert_eq!(note, Some(Message::Notification { method: "session/update".into(), params: json!({}) }));
        assert_eq!(parse_message("log solto"), None);
    }

    #[test]
    fn session_requests_carry_absolute_cwd_and_model() {
        let dir = Path::new("D:\\proj");
        let line = new_session_line("n1", dir, &[]);
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "session/new");
        assert_eq!(v["params"]["cwd"], "D:\\proj");
        let load: Value = serde_json::from_str(&load_session_line("n2", "ses_1", dir, &[])).unwrap();
        assert_eq!(load["method"], "session/load");
        assert_eq!(load["params"]["sessionId"], "ses_1");
        let model: Value = serde_json::from_str(&set_model_line("m1", "ses_1", "openai/gpt-6-luna")).unwrap();
        assert_eq!(model["method"], "session/set_config_option");
        assert_eq!(model["params"]["configId"], "model");
        assert_eq!(model["params"]["value"], "openai/gpt-6-luna");
        let prompt: Value = serde_json::from_str(&prompt_line("p1", "ses_1", "olá\nmundo")).unwrap();
        assert_eq!(prompt["params"]["prompt"][0]["text"], "olá\nmundo");
        let cancel: Value = serde_json::from_str(&cancel_line("ses_1")).unwrap();
        assert_eq!(cancel["method"], "session/cancel");
        assert!(cancel.get("id").is_none(), "cancel é notificação");
    }

    #[test]
    fn initialize_asks_for_form_elicitation() {
        let v: Value = serde_json::from_str(&initialize_line("i1")).unwrap();
        assert_eq!(v["params"]["protocolVersion"], 1);
        assert!(v["params"]["clientCapabilities"]["elicitation"]["form"].is_object());
    }

    #[test]
    fn updates_become_stream_events() {
        let text = update_events(&json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"oi"}}));
        assert_eq!(text, vec![AgentEvent::TextDelta("oi".into())]);
        let thought = update_events(&json!({"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"hm"}}));
        assert_eq!(thought, vec![AgentEvent::ThinkingDelta("hm".into())]);
        let tool = update_events(&json!({"sessionUpdate":"tool_call","title":"Ler arquivo","kind":"read","toolCallId":"c1","rawInput":{"path":"a.rs"}}));
        assert_eq!(tool, vec![AgentEvent::ToolUse { name: "Ler arquivo".into(), input: json!({"input":{"path":"a.rs"}}) }]);
        let done = update_events(&json!({"sessionUpdate":"tool_call_update","status":"completed","content":[{"type":"text","text":"ok"}]}));
        assert_eq!(done, vec![AgentEvent::ToolResult { ok: true, output: "ok".into() }]);
        assert!(update_events(&json!({"sessionUpdate":"available_commands_update","availableCommands":[]})).is_empty());
    }

    #[test]
    fn permission_becomes_answering_card() {
        let params = json!({
            "sessionId":"ses_1",
            "toolCall":{"toolCallId":"c1","title":"Rodar git status","kind":"execute"},
            "options":[
                {"optionId":"once","kind":"allow_once","name":"Permitir uma vez"},
                {"optionId":"always","kind":"allow_always","name":"Sempre permitir"},
                {"optionId":"reject","kind":"reject_once","name":"Negar"}
            ]
        });
        let ask = permission_ask(&json!("req-1"), &params).unwrap();
        assert_eq!(ask.parts[0].question, "Permitir Rodar git status?");
        assert_eq!(ask.parts[0].options.len(), 3);
        assert_eq!(ask.parts[0].options[0].value, "once");
        let ev = ask_event(&ask);
        let AgentEvent::ToolUse { name, input } = ev else { panic!("esperava ToolUse") };
        assert_eq!(name, ASK_TOOL);
        assert_eq!(input["acp_request"], "req-1");
        assert_eq!(input["questions"][0]["options"][0]["label"], "Permitir uma vez");
        let line = answer_line(&ask, &[vec!["Permitir uma vez".into()]], &[]).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["result"]["outcome"]["optionId"], "once");
        let v: Value = serde_json::from_str(&permission_answer(&json!("req-1"), "once")).unwrap();
        assert_eq!(v["result"]["outcome"]["outcome"], "selected");
        assert_eq!(v["result"]["outcome"]["optionId"], "once");
        let v: Value = serde_json::from_str(&permission_cancelled(&json!("req-1"))).unwrap();
        assert_eq!(v["result"]["outcome"]["outcome"], "cancelled");
    }

    #[test]
    fn elicitation_form_becomes_question() {
        let params = json!({
            "mode":"form",
            "sessionId":"ses_1",
            "message":"Qual gerenciador?",
            "requestedSchema":{"type":"object","properties":{
                "pm":{"type":"string","oneOf":[
                    {"const":"bun","title":"Bun"},
                    {"const":"npm","title":"npm"}
                ]}
            },"required":["pm"]}
        });
        let ask = elicitation_ask(&json!("req-2"), &params).unwrap();
        assert_eq!(ask.parts.len(), 1);
        assert_eq!(ask.parts[0].question, "Qual gerenciador?");
        assert_eq!(ask.parts[0].options[0].value, "bun");
        let line = answer_line(&ask, &[vec!["npm".into()]], &["".into()]).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["result"]["action"], "accept");
        assert_eq!(v["result"]["content"]["pm"], "npm");
    }

    #[test]
    fn numeric_request_id_stays_numeric_in_the_answer() {
        let params = json!({
            "message":"Questions",
            "requestedSchema":{"type":"object","properties":{
                "q0":{"type":"string","oneOf":[
                    {"const":"Alpha","title":"Alpha"},
                    {"const":"Beta","title":"Beta"}
                ]}
            }}
        });
        let ask = elicitation_ask(&json!(0), &params).unwrap();
        assert_eq!(ask.id, "0");
        let line = answer_line(&ask, &[vec!["Alpha".into()]], &["".into()]).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        assert!(v["id"].is_number(), "opencode espera o mesmo tipo de id do pedido");
        assert_eq!(v["id"], json!(0));
        assert_eq!(v["result"]["content"]["q0"], "Alpha");
        let v: Value = serde_json::from_str(&decline_line(&ask)).unwrap();
        assert!(v["id"].is_number());
        assert_eq!(v["result"]["action"], "decline");
    }

    #[test]
    fn one_card_answers_every_field_of_a_form() {
        let params = json!({
            "message":"Escolha duas coisas",
            "requestedSchema":{"type":"object","properties":{
                "pm":{"type":"string","oneOf":[{"const":"bun","title":"Bun"},{"const":"npm","title":"npm"}]},
                "pm_custom":{"type":"string"},
                "shell":{"type":"string","oneOf":[{"const":"pwsh","title":"PowerShell"},{"const":"bash","title":"Bash"}]}
            },"required":["pm","shell"]}
        });
        let ask = elicitation_ask(&json!("req-3"), &params).unwrap();
        assert_eq!(ask.parts.len(), 2, "o campo _custom não é pergunta");
        assert!(ask.parts[0].custom, "texto livre do primeiro campo");
        let line = answer_line(&ask, &[vec!["Bun".into()], vec!["Bash".into()]], &["minha nota".into(), "".into()]).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["result"]["content"]["pm"], "bun");
        assert_eq!(v["result"]["content"]["pm_custom"], "minha nota");
        assert_eq!(v["result"]["content"]["shell"], "bash");
    }

    #[test]
    fn prompt_usage_is_a_delta_not_the_session_total() {
        let result = json!({"stopReason":"end_turn","usage":{"inputTokens":1200,"outputTokens":340,"totalTokens":1540}});
        let evs = prompt_events(&result, &None, Some((900, 100)), false);
        assert!(matches!(&evs[0], AgentEvent::Usage(u) if u.input_tokens == 300 && u.output_tokens == 240));
        assert!(matches!(&evs[1], AgentEvent::Result { ok: true, summary, .. } if summary == "end_turn"));
        let cancelled = json!({"stopReason":"cancelled","usage":{}});
        assert!(matches!(&prompt_events(&cancelled, &None, None, false)[0], AgentEvent::Result { ok: false, .. }));
        // "Parar" confirmado pela CLI não vira linha de erro na conversa.
        assert!(prompt_events(&cancelled, &None, None, true).is_empty());
    }
}
