//! Servidores MCP da CLI do Claude: listar (`claude mcp list`), adicionar e
//! remover. O login (OAuth) é interativo e roda num terminal do Argus.
//!
//! Formato de `claude mcp list` (2.1.220), uma linha por servidor depois de
//! "Checking MCP server health…":
//!   `nome: alvo - ✔ Connected` / `- ! Needs authentication` / `- ✘ Failed to connect`

use serde::{Deserialize, Serialize};

use crate::agents::provider::{run_capture, strip_ansi, which_bin};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpStatus {
    Connected,
    NeedsAuth,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    /// Comando (stdio) ou URL (http/sse).
    pub target: String,
    pub status: McpStatus,
    /// Texto do estado como a CLI escreveu.
    pub status_text: String,
    /// Vem do claude.ai (conector da conta): não se remove pela CLI.
    pub remote: bool,
}

pub fn parse_list(out: &str) -> Vec<McpServer> {
    let out = strip_ansi(out);
    out.lines()
        .filter_map(|l| {
            let l = l.trim();
            let (head, status_text) = l.rsplit_once(" - ")?;
            let (name, target) = head.split_once(": ")?;
            let st = status_text.trim();
            let low = st.to_lowercase();
            let status = if low.contains("connected") && !low.contains("not") {
                McpStatus::Connected
            } else if low.contains("auth") {
                McpStatus::NeedsAuth
            } else if low.contains("fail") || low.contains("error") || st.starts_with('✘') {
                McpStatus::Failed
            } else {
                McpStatus::Unknown
            };
            let name = name.trim().to_string();
            Some(McpServer {
                remote: name.starts_with("claude.ai "),
                name,
                target: target.trim().to_string(),
                status,
                status_text: st.trim_start_matches(['✔', '✘', '!', ' ']).to_string(),
            })
        })
        .collect()
}

fn claude() -> Result<std::path::PathBuf, String> {
    which_bin("claude").ok_or_else(|| "A CLI claude não foi achada no PATH.".to_string())
}

fn run(args: &[&str]) -> Result<String, String> {
    let (ok, out) = run_capture(&claude()?, args).ok_or("Não consegui rodar a CLI claude.")?;
    let out = strip_ansi(&out).trim().to_string();
    if ok { Ok(out) } else { Err(if out.is_empty() { "A CLI devolveu erro.".into() } else { out }) }
}

/// Lista (confere a saúde de cada servidor: pode levar alguns segundos).
pub fn list() -> Result<Vec<McpServer>, String> {
    Ok(parse_list(&run(&["mcp", "list"])?))
}

/// Nome aceito: letras, números, `-`, `_` e `.`.
pub fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 64 && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn valid_scope(s: &str) -> bool {
    matches!(s, "local" | "user" | "project")
}

/// Adiciona. `transport`: "stdio", "http" ou "sse". Em stdio, `target` é o
/// comando e `args` os argumentos; em http/sse, `target` é a URL.
pub fn add(name: &str, transport: &str, target: &str, args: &[String], scope: &str, env: &[String]) -> Result<String, String> {
    if !valid_name(name) {
        return Err("Nome inválido: use letras, números, - _ ou .".into());
    }
    if !valid_scope(scope) {
        return Err(format!("escopo desconhecido: {scope}"));
    }
    let target = target.trim();
    if target.is_empty() {
        return Err("Informe o comando ou a URL.".into());
    }
    let mut a: Vec<&str> = vec!["mcp", "add", "--scope", scope];
    match transport {
        "http" | "sse" => {
            if !(target.starts_with("http://") || target.starts_with("https://")) {
                return Err("A URL precisa começar com http:// ou https://".into());
            }
            a.extend(["--transport", transport]);
        }
        "stdio" => {}
        t => return Err(format!("transporte desconhecido: {t}")),
    }
    for e in env {
        if !e.contains('=') {
            return Err(format!("variável sem '=': {e}"));
        }
        a.extend(["-e", e.as_str()]);
    }
    a.push(name);
    if transport == "stdio" {
        a.push("--");
    }
    a.push(target);
    a.extend(args.iter().map(String::as_str));
    run(&a)
}

pub fn remove(name: &str) -> Result<String, String> {
    if !valid_name(name) {
        return Err("Nome inválido.".into());
    }
    run(&["mcp", "remove", name])
}

// ------------------------------------------------------------ plugins

/// Plugin instalado na CLI (`claude plugin list --json`, conferido no 2.1.220).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plugin {
    /// "nome@marketplace".
    pub id: String,
    pub version: String,
    pub scope: String,
    pub enabled: bool,
    /// Servidores MCP que o plugin traz (só os nomes).
    pub mcp_servers: Vec<String>,
}

pub fn parse_plugins(json: &str) -> Result<Vec<Plugin>, String> {
    let v: serde_json::Value = serde_json::from_str(json.trim()).map_err(|e| format!("lista de plugins ilegível: {e}"))?;
    let arr = v.as_array().ok_or("lista de plugins ilegível")?;
    Ok(arr
        .iter()
        .filter_map(|p| {
            Some(Plugin {
                id: p.get("id")?.as_str()?.to_string(),
                version: p.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                scope: p.get("scope").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                enabled: p.get("enabled").and_then(|x| x.as_bool()).unwrap_or(false),
                mcp_servers: p.get("mcpServers").and_then(|m| m.as_object()).map(|m| m.keys().cloned().collect()).unwrap_or_default(),
            })
        })
        .collect())
}

pub fn plugins() -> Result<Vec<Plugin>, String> {
    let claude = claude()?;
    // Só a saída padrão: avisos no stderr quebrariam o JSON.
    let (ok, out) = crate::agents::provider::run_capture_stdout(&claude, &["plugin", "list", "--json"]).ok_or("Não consegui rodar a CLI claude.")?;
    if !ok {
        return Err("A CLI não listou os plugins.".into());
    }
    parse_plugins(&out)
}

/// Id aceito: "nome" ou "nome@marketplace", com as regras de `valid_name`.
pub fn valid_plugin_id(id: &str) -> bool {
    match id.split_once('@') {
        Some((n, m)) => valid_name(n) && valid_name(m),
        None => valid_name(id),
    }
}

pub fn set_plugin(id: &str, enabled: bool) -> Result<String, String> {
    if !valid_plugin_id(id) {
        return Err("Plugin inválido.".into());
    }
    run(&["plugin", if enabled { "enable" } else { "disable" }, id])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_list() {
        let out = "Checking MCP server health…\n\nclaude.ai Gmail: https://gmailmcp.googleapis.com/mcp/v1 - ✔ Connected\nclaude.ai Slack: https://mcp.slack.com/mcp - ! Needs authentication\nsentry: npx -y @sentry/mcp - ✘ Failed to connect\n";
        let l = parse_list(out);
        assert_eq!(l.len(), 3);
        assert_eq!((l[0].name.as_str(), &l[0].status, l[0].remote), ("claude.ai Gmail", &McpStatus::Connected, true));
        assert_eq!(l[1].status, McpStatus::NeedsAuth);
        assert_eq!((l[2].target.as_str(), &l[2].status, l[2].remote), ("npx -y @sentry/mcp", &McpStatus::Failed, false));
        assert_eq!(l[1].status_text, "Needs authentication");
    }

    #[test]
    fn parses_real_plugin_list() {
        let j = r#"[{"id":"vercel-plugin@vercel","version":"0.45.1","scope":"user","enabled":true,"installPath":"C:\\x","mcpServers":{"vercel":{"type":"http","url":"https://mcp.vercel.com"}}}]"#;
        let l = parse_plugins(j).unwrap();
        assert_eq!(l, vec![Plugin { id: "vercel-plugin@vercel".into(), version: "0.45.1".into(), scope: "user".into(), enabled: true, mcp_servers: vec!["vercel".into()] }]);
        assert!(parse_plugins("oi").is_err());
        assert!(valid_plugin_id("vercel-plugin@vercel") && !valid_plugin_id("a@b@c") && !valid_plugin_id("x; rm"));
    }

    #[test]
    fn validates_input() {
        assert!(valid_name("my-server.1") && !valid_name("a b") && !valid_name(""));
        assert!(add("x", "http", "ftp://a", &[], "local", &[]).unwrap_err().contains("http"));
        assert!(add("x", "stdio", "npx", &[], "global", &[]).unwrap_err().contains("escopo"));
        assert!(add("x", "stdio", "npx", &[], "local", &["SEMIGUAL".into()]).unwrap_err().contains("'='"));
    }
}
