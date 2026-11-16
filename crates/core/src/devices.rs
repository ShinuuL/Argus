//! Celulares Android pelo Screen-Mirror, rodando como processo auxiliar (sidecar).
//!
//! Fase A da integração (ver `PENDENCIAS.md`): o Argus não reimplementa o ADB nem o
//! protocolo do scrcpy. Ele inicia `node src/cli.ts serve` do projeto Screen-Mirror,
//! sempre em `127.0.0.1` e numa porta livre escolhida aqui, confere a saúde por
//! `GET /api/meta` e encerra o processo quando a janela fecha ou o usuário pede.
//!
//! A interface embute o painel do próprio Screen-Mirror num iframe. Dentro do iframe a
//! origem é `127.0.0.1:<porta>`, então a validação de Host/Origin do Screen-Mirror (que
//! barra sites externos) continua valendo sem nenhuma mudança no projeto.
//!
//! Onde fica o Screen-Mirror, em ordem: `ARGUS_SCREEN_MIRROR`, a pasta salva em
//! `screen_mirror.json` (na pasta de dados do Argus) e `<pasta raiz>/Screen-Mirror`.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Variável que aponta a pasta do Screen-Mirror (tem prioridade sobre o resto).
pub const DIR_VAR: &str = "ARGUS_SCREEN_MIRROR";
/// Versão mínima do Node exigida pelo Screen-Mirror (`engines` do package.json).
pub const MIN_NODE: (u32, u32) = (22, 18);
const HOST: &str = "127.0.0.1";
const START_TIMEOUT: Duration = Duration::from_secs(20);

/// Estado do sidecar, como a interface vê.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorStatus {
    /// Pasta do Screen-Mirror encontrada (None = não configurado).
    pub dir: Option<PathBuf>,
    /// Versão do Node encontrada no PATH (None = ausente).
    pub node: Option<String>,
    /// Node presente, mas mais antigo que o mínimo.
    pub node_too_old: bool,
    /// `node_modules` ausente: falta `npm install` no Screen-Mirror.
    pub needs_install: bool,
    /// URL do painel quando o servidor está no ar.
    pub url: Option<String>,
    pub pid: Option<u32>,
    /// Último erro de inicialização ou saída inesperada.
    pub error: Option<String>,
}

/// Aparelho listado pelo Screen-Mirror (`GET /api/devices`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub serial: String,
    pub state: String,
    #[serde(default)]
    pub model: Option<String>,
    /// `usb` ou `wifi`.
    #[serde(default)]
    pub connection: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DeviceList {
    pub devices: Vec<Device>,
    /// Seriais com espelhamento ativo.
    #[serde(default)]
    pub mirroring: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    dir: Option<PathBuf>,
}

struct Running {
    child: Child,
    port: u16,
}

/// Dono do processo do Screen-Mirror. Encerra o processo ao ser descartado.
pub struct ScreenMirror {
    config_file: Option<PathBuf>,
    running: Mutex<Option<Running>>,
    last_error: Mutex<Option<String>>,
}

impl ScreenMirror {
    /// `config_file`: onde a pasta escolhida pelo usuário fica salva (None = não salva).
    pub fn new(config_file: Option<PathBuf>) -> Self {
        Self { config_file, running: Mutex::new(None), last_error: Mutex::new(None) }
    }

    fn saved(&self) -> Saved {
        self.config_file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Salva a pasta escolhida na interface (falha se não for um Screen-Mirror).
    pub fn set_dir(&self, dir: &Path) -> anyhow::Result<()> {
        if !is_screen_mirror(dir) {
            anyhow::bail!("{} não parece o projeto Screen-Mirror (faltam package.json ou src/cli.ts).", dir.display());
        }
        let file = self.config_file.as_ref().ok_or_else(|| anyhow::anyhow!("sem pasta de dados para salvar a configuração"))?;
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, serde_json::to_string_pretty(&Saved { dir: Some(dir.to_path_buf()) })?)?;
        Ok(())
    }

    /// Pasta do Screen-Mirror, pela ordem descrita no topo do módulo.
    pub fn locate(&self, root: Option<&Path>) -> Option<PathBuf> {
        let env = std::env::var_os(DIR_VAR).map(PathBuf::from);
        let candidates = [env, self.saved().dir, root.map(|r| r.join("Screen-Mirror"))];
        candidates.into_iter().flatten().find(|d| is_screen_mirror(d))
    }

    /// Porta do servidor, se estiver rodando (limpa o estado se o processo morreu).
    fn port(&self) -> Option<u16> {
        let mut guard = self.running.lock().unwrap();
        let exited = match guard.as_mut() {
            Some(r) => match r.child.try_wait() {
                Ok(Some(code)) => Some(format!("O Screen-Mirror encerrou sozinho ({code}).")),
                Ok(None) => return Some(r.port),
                Err(e) => Some(e.to_string()),
            },
            None => return None,
        };
        *guard = None;
        *self.last_error.lock().unwrap() = exited;
        None
    }

    pub fn status(&self, root: Option<&Path>) -> MirrorStatus {
        let dir = self.locate(root);
        let node = node_version();
        let node_too_old = node.as_deref().and_then(parse_version).is_some_and(|v| v < MIN_NODE);
        let needs_install = dir.as_ref().is_some_and(|d| !d.join("node_modules").is_dir());
        let port = self.port();
        let pid = self.running.lock().unwrap().as_ref().map(|r| r.child.id());
        MirrorStatus {
            dir,
            node,
            node_too_old,
            needs_install,
            url: port.map(|p| format!("http://{HOST}:{p}")),
            pid,
            error: self.last_error.lock().unwrap().clone(),
        }
    }

    /// Inicia o servidor (se já estiver no ar, só devolve o estado). Bloqueia até
    /// o `/api/meta` responder ou o tempo esgotar.
    pub fn start(&self, root: Option<&Path>) -> anyhow::Result<MirrorStatus> {
        if self.port().is_some() {
            return Ok(self.status(root));
        }
        let result = self.spawn(root);
        *self.last_error.lock().unwrap() = result.as_ref().err().map(|e| e.to_string());
        result?;
        Ok(self.status(root))
    }

    fn spawn(&self, root: Option<&Path>) -> anyhow::Result<()> {
        let dir = self.locate(root).ok_or_else(|| {
            anyhow::anyhow!("Screen-Mirror não encontrado. Escolha a pasta do projeto ou defina {DIR_VAR}.")
        })?;
        match node_version().as_deref().map(|v| (v, parse_version(v))) {
            None => anyhow::bail!("Node.js não encontrado no PATH (o Screen-Mirror precisa do {}.{} ou mais novo).", MIN_NODE.0, MIN_NODE.1),
            Some((v, Some(parsed))) if parsed < MIN_NODE => {
                anyhow::bail!("Node.js {v} é antigo demais; o Screen-Mirror precisa do {}.{} ou mais novo.", MIN_NODE.0, MIN_NODE.1)
            }
            _ => {}
        }
        if !dir.join("node_modules").is_dir() {
            anyhow::bail!("Dependências do Screen-Mirror não instaladas: rode `npm install` em {}.", dir.display());
        }
        let port = free_port()?;
        let mut cmd = Command::new("node");
        cmd.args(["src/cli.ts", "serve", "--port", &port.to_string()])
            .current_dir(&dir)
            .env("SM_HOST", HOST)
            .env("SM_PORT", port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| anyhow::anyhow!("falha ao iniciar o node: {e}"))?;
        let deadline = Instant::now() + START_TIMEOUT;
        loop {
            if http_get(port, "/api/meta").is_ok() {
                break;
            }
            if let Some(code) = child.try_wait()? {
                let mut err = String::new();
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut err);
                }
                let err = err.trim();
                anyhow::bail!("O Screen-Mirror encerrou ao iniciar ({code}){}{}", if err.is_empty() { "" } else { ": " }, last_lines(err, 5));
            }
            if Instant::now() >= deadline {
                let _ = crate::procs::kill_tree(child.id());
                let _ = child.wait();
                anyhow::bail!("O Screen-Mirror não respondeu em {} s.", START_TIMEOUT.as_secs());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        // O stderr não é mais lido: descarta para o pipe não encher e travar o node.
        if let Some(mut s) = child.stderr.take() {
            std::thread::spawn(move || {
                let _ = std::io::copy(&mut s, &mut std::io::sink());
            });
        }
        *self.running.lock().unwrap() = Some(Running { child, port });
        Ok(())
    }

    /// Encerra o servidor (e o que ele tiver iniciado, como sessões do scrcpy).
    pub fn stop(&self) {
        if let Some(mut r) = self.running.lock().unwrap().take() {
            let _ = crate::procs::kill_tree(r.child.id());
            let _ = r.child.kill();
            let _ = r.child.wait();
        }
    }

    /// Aparelhos conectados (o servidor precisa estar no ar).
    pub fn devices(&self) -> anyhow::Result<DeviceList> {
        let port = self.port().ok_or_else(|| anyhow::anyhow!("O Screen-Mirror não está rodando."))?;
        Ok(serde_json::from_str(&http_get(port, "/api/devices")?)?)
    }
}

impl Drop for ScreenMirror {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Tem cara de Screen-Mirror: `package.json` com esse nome e o `src/cli.ts`.
pub fn is_screen_mirror(dir: &Path) -> bool {
    dir.join("src").join("cli.ts").is_file()
        && std::fs::read_to_string(dir.join("package.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .is_some_and(|v| v["name"] == "screen-mirror")
}

/// `v22.18.0` -> (22, 18).
pub fn parse_version(v: &str) -> Option<(u32, u32)> {
    let mut parts = v.trim().trim_start_matches('v').split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

fn node_version() -> Option<String> {
    let mut cmd = Command::new("node");
    cmd.arg("--version").stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok().filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

/// Porta livre em 127.0.0.1 (o SO escolhe; liberada logo em seguida para o node).
fn free_port() -> anyhow::Result<u16> {
    Ok(TcpListener::bind((HOST, 0))?.local_addr()?.port())
}

/// GET mínimo em HTTP/1.0 para o servidor local. Sem cabeçalho Origin, com o Host
/// que o Screen-Mirror aceita. Devolve o corpo se o status for 2xx.
fn http_get(port: u16, path: &str) -> anyhow::Result<String> {
    let addr: SocketAddr = format!("{HOST}:{port}").parse()?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(500))?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(s, "GET {path} HTTP/1.0\r\nHost: {HOST}:{port}\r\nAccept: application/json\r\n\r\n")?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw)?;
    parse_response(&String::from_utf8_lossy(&raw))
}

fn parse_response(raw: &str) -> anyhow::Result<String> {
    let (head, body) = raw.split_once("\r\n\r\n").ok_or_else(|| anyhow::anyhow!("resposta HTTP incompleta"))?;
    let status: u16 = head.split_whitespace().nth(1).and_then(|c| c.parse().ok()).ok_or_else(|| anyhow::anyhow!("status HTTP inválido"))?;
    if !(200..300).contains(&status) {
        anyhow::bail!("HTTP {status}: {}", body.trim());
    }
    Ok(body.to_string())
}

fn last_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v22.18.0"), Some((22, 18)));
        assert_eq!(parse_version("24.1.3\n"), Some((24, 1)));
        assert_eq!(parse_version("abc"), None);
        assert!(parse_version("v22.17.1").unwrap() < MIN_NODE);
        assert!(parse_version("v23.0.0").unwrap() >= MIN_NODE);
    }

    #[test]
    fn http_parsing() {
        assert_eq!(parse_response("HTTP/1.1 200 OK\r\nA: b\r\n\r\n{\"x\":1}").unwrap(), "{\"x\":1}");
        assert!(parse_response("HTTP/1.1 403 Forbidden\r\n\r\nnão").is_err());
        assert!(parse_response("lixo").is_err());
    }

    #[test]
    fn device_list_json() {
        let l: DeviceList = serde_json::from_str(
            r#"{"devices":[{"serial":"abc","state":"device","model":"Pixel"},{"serial":"x","state":"unauthorized"}],"mirroring":["abc"]}"#,
        )
        .unwrap();
        assert_eq!(l.devices.len(), 2);
        assert_eq!(l.devices[1].model, None);
        assert_eq!(l.mirroring, vec!["abc"]);
    }

    #[test]
    fn recognizes_project() {
        let d = tempfile::tempdir().unwrap();
        assert!(!is_screen_mirror(d.path()));
        std::fs::create_dir(d.path().join("src")).unwrap();
        std::fs::write(d.path().join("src/cli.ts"), "").unwrap();
        std::fs::write(d.path().join("package.json"), r#"{"name":"outro"}"#).unwrap();
        assert!(!is_screen_mirror(d.path()));
        std::fs::write(d.path().join("package.json"), r#"{"name":"screen-mirror"}"#).unwrap();
        assert!(is_screen_mirror(d.path()));

        let sm = ScreenMirror::new(Some(d.path().join("cfg.json")));
        assert!(sm.set_dir(&d.path().join("src")).is_err());
        sm.set_dir(d.path()).unwrap();
        // A variável de ambiente tem prioridade; só confere a pasta salva sem ela.
        if std::env::var_os(DIR_VAR).is_none() {
            assert_eq!(sm.locate(None).as_deref(), Some(d.path()));
        }
    }
}
