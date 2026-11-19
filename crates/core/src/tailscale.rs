//! Controle do Tailscale para o acesso remoto.
//!
//! Dois modos de publicação, ambos chamando a CLI `tailscale`:
//! - **Serve** (`Exposure::Tailnet`): HTTPS só para os aparelhos do seu tailnet.
//! - **Funnel** (`Exposure::Funnel`): HTTPS aberto na internet — qualquer um com a
//!   URL entra, e a URL do Argus carrega o token. Só use se o celular precisar
//!   acessar de fora do tailnet.
//!
//! Nos dois casos o servidor do Argus escuta só em 127.0.0.1: quem fala com a rede
//! é o Tailscale, e o Firewall do Windows não pede liberação.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// Como o servidor remoto fica acessível ao celular.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
pub enum Exposure {
    /// Escuta em 0.0.0.0 e parea por IP da rede local (http, sem Tailscale).
    Lan,
    /// `tailscale serve`: HTTPS só dentro do tailnet.
    Tailnet,
    /// `tailscale funnel`: HTTPS publicado na internet.
    #[default]
    Funnel,
}

impl Exposure {
    /// O Tailscale é quem publica? Nesses modos o Argus escuta só no loopback.
    pub fn via_tailscale(self) -> bool {
        matches!(self, Exposure::Tailnet | Exposure::Funnel)
    }

    /// Subcomando da CLI correspondente.
    fn verb(self) -> &'static str {
        match self {
            Exposure::Funnel => "funnel",
            _ => "serve",
        }
    }
}

/// Estado do Tailscale nesta máquina.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TailscaleStatus {
    /// A CLI existe no computador.
    pub installed: bool,
    /// O daemon está logado e rodando (`BackendState == "Running"`).
    pub running: bool,
    /// Nome DNS da máquina no tailnet, sem o ponto final (ex.: `pc.tail1234.ts.net`).
    pub dns_name: String,
}

/// Quanto tempo esperar cada chamada da CLI antes de desistir.
const CLI_TIMEOUT: Duration = Duration::from_secs(20);

/// Caminho da CLI `tailscale` (PATH ou instalação padrão do Windows).
pub fn cli() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "tailscale.exe" } else { "tailscale" };
    let fixed: &[&str] = if cfg!(windows) {
        &[r"C:\Program Files\Tailscale\tailscale.exe", r"C:\Program Files (x86)\Tailscale\tailscale.exe"]
    } else {
        &["/usr/bin/tailscale", "/usr/local/bin/tailscale", "/Applications/Tailscale.app/Contents/MacOS/Tailscale"]
    };
    for p in fixed {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    // Sem caminho fixo: confia no PATH se a CLI responder.
    let ok = Command::new(exe).arg("version").output().map(|o| o.status.success()).unwrap_or(false);
    ok.then(|| PathBuf::from(exe))
}

/// Roda a CLI e devolve (stdout, stderr) quando o comando termina bem.
fn run(args: &[&str]) -> anyhow::Result<(String, String)> {
    let bin = cli().ok_or_else(|| anyhow::anyhow!("Tailscale não encontrado neste computador."))?;
    let mut child = Command::new(&bin)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("não foi possível executar o Tailscale: {e}"))?;

    // A CLI pode travar esperando interação; o tempo limite evita prender a interface.
    let deadline = std::time::Instant::now() + CLI_TIMEOUT;
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            anyhow::bail!("o Tailscale não respondeu em {}s (`tailscale {}`).", CLI_TIMEOUT.as_secs(), args.join(" "));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !out.status.success() {
        let msg = if stderr.is_empty() { stdout } else { stderr };
        anyhow::bail!("{}", if msg.is_empty() { format!("`tailscale {}` falhou", args.join(" ")) } else { msg });
    }
    Ok((stdout, stderr))
}

#[derive(Deserialize)]
struct StatusJson {
    #[serde(rename = "BackendState")]
    backend_state: Option<String>,
    #[serde(rename = "Self")]
    self_node: Option<SelfJson>,
}

#[derive(Deserialize)]
struct SelfJson {
    #[serde(rename = "DNSName")]
    dns_name: Option<String>,
}

/// Lê o estado do Tailscale. Nunca falha: sem CLI, devolve tudo desligado.
pub fn status() -> TailscaleStatus {
    if cli().is_none() {
        return TailscaleStatus::default();
    }
    let Ok((out, _)) = run(&["status", "--json"]) else {
        return TailscaleStatus { installed: true, ..Default::default() };
    };
    let parsed: StatusJson = match serde_json::from_str(&out) {
        Ok(v) => v,
        Err(_) => return TailscaleStatus { installed: true, ..Default::default() },
    };
    TailscaleStatus {
        installed: true,
        running: parsed.backend_state.as_deref() == Some("Running"),
        dns_name: parsed
            .self_node
            .and_then(|s| s.dns_name)
            .map(|d| d.trim_end_matches('.').to_string())
            .unwrap_or_default(),
    }
}

/// Publica `127.0.0.1:port` e devolve a URL HTTPS (`https://host.tailnet.ts.net`).
///
/// Erros da CLI (Funnel não liberado na ACL, HTTPS desativado, máquina deslogada)
/// sobem como estão: a mensagem do Tailscale já diz o que fazer.
pub fn expose(mode: Exposure, port: u16) -> anyhow::Result<String> {
    if !mode.via_tailscale() {
        anyhow::bail!("modo {mode:?} não usa Tailscale");
    }
    let st = status();
    if !st.installed {
        anyhow::bail!("Tailscale não encontrado neste computador.");
    }
    if !st.running {
        anyhow::bail!("O Tailscale está instalado, mas não conectado. Abra o Tailscale e entre na sua conta.");
    }
    if st.dns_name.is_empty() {
        anyhow::bail!("O Tailscale não informou o nome da máquina (MagicDNS desligado?).");
    }
    let target = format!("http://127.0.0.1:{port}");
    run(&[mode.verb(), "--bg", "--https=443", &target])?;
    Ok(format!("https://{}", st.dns_name))
}

/// Desfaz a publicação da porta 443. Chamada no desligar e no Drop: só registra o erro.
pub fn unexpose(mode: Exposure) {
    if !mode.via_tailscale() {
        return;
    }
    if let Err(e) = run(&[mode.verb(), "--https=443", "off"]) {
        tracing::warn!("não foi possível desligar o {} do Tailscale: {e}", mode.verb());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn via_tailscale_only_for_serve_and_funnel() {
        assert!(!Exposure::Lan.via_tailscale());
        assert!(Exposure::Tailnet.via_tailscale());
        assert!(Exposure::Funnel.via_tailscale());
    }

    #[test]
    fn verbs_match_the_cli() {
        assert_eq!(Exposure::Funnel.verb(), "funnel");
        assert_eq!(Exposure::Tailnet.verb(), "serve");
    }

    #[test]
    fn status_json_drops_the_trailing_dot() {
        let json = r#"{"BackendState":"Running","Self":{"DNSName":"pc.tail1234.ts.net."}}"#;
        let p: StatusJson = serde_json::from_str(json).unwrap();
        assert_eq!(p.backend_state.as_deref(), Some("Running"));
        assert_eq!(p.self_node.unwrap().dns_name.unwrap().trim_end_matches('.'), "pc.tail1234.ts.net");
    }

    #[test]
    fn expose_refuses_lan() {
        assert!(expose(Exposure::Lan, 8787).is_err());
    }

    #[test]
    fn funnel_is_the_default_choice() {
        assert_eq!(Exposure::default(), Exposure::Funnel);
    }
}
