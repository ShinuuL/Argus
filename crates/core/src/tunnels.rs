//! Túneis que põem um servidor local na internet (cloudflared, ngrok, tailscale funnel).
//!
//! O agente às vezes sobe um túnel junto com o servidor de desenvolvimento ("abre
//! para eu ver no celular"). O túnel não escuta na porta pública — ele faz conexão
//! de saída —, então não aparece na varredura de portas: é preciso reconhecê-lo
//! pela linha de comando e descobrir a porta local que ele está publicando.
//!
//! O endereço público sai da API local da própria ferramenta, sempre em 127.0.0.1:
//! - cloudflared: `/quicktunnel` na porta de métricas (a única que ele escuta);
//! - ngrok: `/api/tunnels` na porta 4040.
//!
//! Nada aqui fala com a internet.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// Ferramenta de túnel reconhecida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelKind {
    Cloudflared,
    Ngrok,
    TailscaleFunnel,
}

impl TunnelKind {
    pub fn label(self) -> &'static str {
        match self {
            TunnelKind::Cloudflared => "Cloudflare",
            TunnelKind::Ngrok => "ngrok",
            TunnelKind::TailscaleFunnel => "Tailscale Funnel",
        }
    }
}

/// Um servidor local publicado para fora da máquina.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exposure {
    pub kind: TunnelKind,
    /// Nome da ferramenta, para a interface não precisar traduzir o enum.
    pub label: String,
    /// PID do processo do túnel (dá para derrubar só ele, sem matar o servidor).
    pub pid: u32,
    /// Endereço público, quando a ferramenta sabe dizer qual é.
    pub url: Option<String>,
}

/// Qual ferramenta de túnel é esta linha de comando (None = não é túnel).
pub fn kind_of(cmdline: &str) -> Option<TunnelKind> {
    let c = cmdline.to_ascii_lowercase();
    if c.contains("cloudflared") && c.contains(" tunnel") {
        return Some(TunnelKind::Cloudflared);
    }
    if c.contains("ngrok") && (c.contains(" http") || c.contains(" start") || c.contains(" tcp")) {
        return Some(TunnelKind::Ngrok);
    }
    if c.contains("tailscale") && c.contains(" funnel") {
        return Some(TunnelKind::TailscaleFunnel);
    }
    None
}

/// Porta local que o túnel está publicando (None = não deu para saber).
///
/// Aceita as formas usuais: `--url http://127.0.0.1:4173`, `--url localhost:4173`,
/// `ngrok http 3000`, `ngrok http http://localhost:3000`, `tailscale funnel 8080`.
pub fn target_port(cmdline: &str) -> Option<u16> {
    let args = split_args(cmdline);
    // Valor logo depois de --url / --addr / -url.
    for (i, a) in args.iter().enumerate() {
        let flag = a.trim_start_matches('-');
        let (flag, inline) = match flag.split_once('=') {
            Some((f, v)) => (f, Some(v.to_string())),
            None => (flag, None),
        };
        if !matches!(flag, "url" | "addr" | "address" | "target") {
            continue;
        }
        let value = inline.or_else(|| args.get(i + 1).cloned());
        if let Some(p) = value.as_deref().and_then(port_in) {
            return Some(p);
        }
    }
    // Senão, o último argumento que seja uma porta ou um endereço com porta.
    args.iter().rev().filter(|a| !a.starts_with('-')).find_map(|a| port_in(a))
}

/// Porta dentro de um argumento: `4173`, `:4173`, `localhost:4173`, `http://127.0.0.1:4173/x`.
fn port_in(value: &str) -> Option<u16> {
    let v = value.trim().trim_matches('"');
    // Tira o esquema (http://, tcp://) e o que vier depois do host.
    let v = v.split_once("://").map(|(_, rest)| rest).unwrap_or(v);
    let v = v.split(['/', '?', '#']).next().unwrap_or(v);
    let candidate = v.rsplit(':').next().unwrap_or(v);
    let candidate: String = candidate.chars().take_while(|c| c.is_ascii_digit()).collect();
    let port: u32 = candidate.parse().ok()?;
    // Porta de verdade; números pequenos costumam ser outra coisa (versão, timeout).
    if (1024..=65535).contains(&port) {
        Some(port as u16)
    } else {
        None
    }
}

/// Divide uma linha de comando respeitando aspas.
fn split_args(cmdline: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in cmdline.chars() {
        match ch {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// GET numa API local (127.0.0.1). Devolve só o corpo; None em qualquer problema.
fn local_get(port: u16, path: &str, timeout: Duration) -> Option<String> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut s = TcpStream::connect_timeout(&addr, timeout).ok()?;
    s.set_read_timeout(Some(timeout)).ok()?;
    s.set_write_timeout(Some(timeout)).ok()?;
    let req = format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nAccept: */*\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    // Resposta curta: o teto evita ler um corpo enorme de um serviço qualquer.
    s.take(256 * 1024).read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    let (head, body) = text.split_once("\r\n\r\n")?;
    if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
        return None;
    }
    Some(body.to_string())
}

/// Endereço do túnel rápido do cloudflared (`{"hostname":"x.trycloudflare.com"}`),
/// pela porta de métricas dele. Túnel nomeado não responde aqui: devolve None.
pub fn cloudflared_url(metrics_port: u16) -> Option<String> {
    let body = local_get(metrics_port, "/quicktunnel", Duration::from_millis(400))?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    let host = v.get("hostname")?.as_str()?.trim().to_string();
    if host.is_empty() {
        return None;
    }
    Some(if host.starts_with("http") { host } else { format!("https://{host}") })
}

/// Endereço público do ngrok que atende `local_port` (API local, porta 4040).
pub fn ngrok_url(local_port: u16) -> Option<String> {
    let body = local_get(4040, "/api/tunnels", Duration::from_millis(400))?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    let tunnels = v.get("tunnels")?.as_array()?;
    let want = format!(":{local_port}");
    let pick = tunnels.iter().find(|t| {
        t.get("config").and_then(|c| c.get("addr")).and_then(|a| a.as_str()).is_some_and(|a| a.ends_with(&want))
    })?;
    // Com http e https no mesmo endereço, o https vem primeiro na preferência.
    let url = pick.get("public_url")?.as_str()?;
    let https = tunnels
        .iter()
        .filter_map(|t| t.get("public_url").and_then(|u| u.as_str()))
        .find(|u| u.starts_with("https://") && u.trim_end_matches('/') != url.trim_end_matches('/'));
    Some(if url.starts_with("https://") { url.to_string() } else { https.unwrap_or(url).to_string() })
}

/// Endereço público de um túnel, pela ferramenta. `metrics_port` é a porta que o
/// próprio processo do túnel escuta (o cloudflared usa a dele para responder).
pub fn public_url(kind: TunnelKind, metrics_port: Option<u16>, local_port: Option<u16>) -> Option<String> {
    match kind {
        TunnelKind::Cloudflared => cloudflared_url(metrics_port?),
        TunnelKind::Ngrok => ngrok_url(local_port?),
        // O endereço do funnel é o nome da máquina na rede do usuário; sem API local.
        TunnelKind::TailscaleFunnel => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_the_tools() {
        assert_eq!(kind_of(r#""C:\cloudflared.exe" tunnel --url http://127.0.0.1:4173"#), Some(TunnelKind::Cloudflared));
        assert_eq!(kind_of("ngrok http 3000"), Some(TunnelKind::Ngrok));
        assert_eq!(kind_of("tailscale funnel 8080"), Some(TunnelKind::TailscaleFunnel));
        assert_eq!(kind_of("node server.js"), None);
        assert_eq!(kind_of("git commit -m 'tunnel'"), None);
        // O binário sozinho, sem subcomando, não é um túnel no ar.
        assert_eq!(kind_of("cloudflared --version"), None);
    }

    #[test]
    fn finds_the_local_port() {
        let real = r#""C:\Program Files (x86)\cloudflared\cloudflared.exe" tunnel --url http://127.0.0.1:4173"#;
        assert_eq!(target_port(real), Some(4173));
        assert_eq!(target_port("cloudflared tunnel --url=localhost:5173"), Some(5173));
        assert_eq!(target_port("ngrok http 3000"), Some(3000));
        assert_eq!(target_port("ngrok http http://localhost:8080/"), Some(8080));
        assert_eq!(target_port("tailscale funnel 1420"), Some(1420));
        assert_eq!(target_port("cloudflared tunnel run meu-tunel"), None);
    }

    #[test]
    fn splits_quoted_arguments() {
        let v = split_args(r#""C:\Program Files\x.exe" tunnel --url http://127.0.0.1:4173"#);
        assert_eq!(v[0], r"C:\Program Files\x.exe");
        assert_eq!(v[2], "--url");
        assert_eq!(v[3], "http://127.0.0.1:4173");
    }

    /// Porta fechada não trava o poll nem inventa endereço.
    #[test]
    fn missing_api_gives_nothing() {
        assert_eq!(cloudflared_url(1), None);
        assert_eq!(public_url(TunnelKind::TailscaleFunnel, None, Some(80)), None);
    }
}
