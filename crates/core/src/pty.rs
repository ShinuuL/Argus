//! Gerenciador de pseudo-terminais (ConPTY no Windows via portable-pty).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

pub type SessionId = u64;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtyEvent {
    Output(Vec<u8>),
    Exit(Option<u32>),
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: SessionId,
    pub program: String,
    pub cols: u16,
    pub rows: u16,
}

struct Session {
    info: SessionInfo,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
}

#[derive(Clone, Default)]
pub struct PtyManager {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    next_id: AtomicU64,
    sessions: Mutex<HashMap<SessionId, Session>>,
}

/// Shell padrão do sistema.
pub fn default_shell() -> CommandSpec {
    #[cfg(windows)]
    let program = if which::which("pwsh").is_ok() {
        "pwsh".to_string()
    } else {
        "powershell.exe".to_string()
    };
    #[cfg(not(windows))]
    let program = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    CommandSpec { program, ..Default::default() }
}

/// O que o PTY executa de fato para `program args`.
///
/// O ConPTY chama `CreateProcessW` com o nome como veio, e a busca do Windows
/// pega o primeiro arquivo com esse nome no PATH. As CLIs instaladas pelo npm
/// deixam três lado a lado (`opencode`, um script de shell sem extensão;
/// `opencode.cmd`; às vezes um `.exe` em outra pasta), e o primeiro não é um
/// executável do Windows ("não é um aplicativo Win32 válido", erro 193). Por
/// isso o nome é resolvido antes (ordem do PATH e, em cada pasta, da PATHEXT;
/// ver `which_bin`), e um `.cmd`/`.bat` roda por `cmd /c`.
pub fn launch_target(program: &str, args: &[String]) -> (String, Vec<String>) {
    let resolved = crate::agents::orchestrator::resolve_program(program);
    wrap_script(&resolved.to_string_lossy(), args)
}

/// `.cmd`/`.bat` no Windows viram `cmd /c <script> args`; o resto passa direto.
fn wrap_script(program: &str, args: &[String]) -> (String, Vec<String>) {
    let ext = std::path::Path::new(program).extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    if cfg!(windows) && matches!(ext.as_deref(), Some("cmd") | Some("bat")) {
        let mut all = vec!["/c".to_string(), program.to_string()];
        all.extend(args.iter().cloned());
        return ("cmd.exe".to_string(), all);
    }
    (program.to_string(), args.to_vec())
}

impl PtyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(
        &self,
        cmd: CommandSpec,
        cols: u16,
        rows: u16,
    ) -> Result<(SessionId, UnboundedReceiver<PtyEvent>)> {
        let pair = native_pty_system()
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| anyhow!("openpty: {e}"))?;

        let (program, args) = launch_target(&cmd.program, &cmd.args);
        let mut builder = CommandBuilder::new(&program);
        builder.args(&args);
        if let Some(cwd) = &cmd.cwd {
            builder.cwd(cwd);
            // `PWD` herdado de quem abriu o Argus enganaria as CLIs rodadas no terminal.
            builder.env("PWD", cwd);
        }
        for (k, v) in &cmd.env {
            builder.env(k, v);
        }

        let child = pair
            .slave
            .spawn_command(builder)
            .map_err(|e| anyhow!("spawn {}: {e}", cmd.program))?;
        drop(pair.slave);
        let child: Arc<Mutex<Box<dyn Child + Send + Sync>>> = Arc::new(Mutex::new(child));

        let mut reader = pair.master.try_clone_reader().map_err(|e| anyhow!("reader: {e}"))?;
        let writer = pair.master.take_writer().map_err(|e| anyhow!("writer: {e}"))?;

        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(pid) = child.lock().ok().and_then(|c| c.process_id()) {
            crate::procs::tracker().track(pid, crate::procs::Owner::Terminal(id));
        }
        let (tx, rx) = unbounded_channel();

        // Thread de leitura: bloqueante, envia para o canal; ao EOF, aguarda o processo.
        let child_r = child.clone();
        let inner = Arc::downgrade(&self.inner);
        std::thread::Builder::new()
            .name(format!("pty-read-{id}"))
            .spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send(PtyEvent::Output(buf[..n].to_vec())).is_err() {
                                break;
                            }
                        }
                    }
                }
                let code = child_r.lock().ok().and_then(|mut c| c.wait().ok()).map(|s| s.exit_code());
                let _ = tx.send(PtyEvent::Exit(code));
                if let Some(inner) = inner.upgrade() {
                    if let Ok(mut s) = inner.sessions.lock() {
                        s.remove(&id);
                    }
                }
            })
            .context("thread de leitura")?;

        // No Windows (ConPTY) o reader só recebe EOF quando o master é fechado;
        // thread de espera remove a sessão (fechando o master) quando o processo sai.
        #[cfg(windows)]
        {
            let child_w = child.clone();
            let inner = Arc::downgrade(&self.inner);
            std::thread::spawn(move || loop {
                let done = child_w.lock().map(|mut c| !matches!(c.try_wait(), Ok(None))).unwrap_or(true);
                if done {
                    // pequena folga para drenar a saída restante
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    if let Some(inner) = inner.upgrade() {
                        if let Ok(mut s) = inner.sessions.lock() {
                            s.remove(&id);
                        }
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            });
        }

        let info = SessionInfo { id, program: cmd.program.clone(), cols, rows };
        self.inner
            .sessions
            .lock()
            .map_err(|_| anyhow!("lock"))?
            .insert(id, Session { info, master: pair.master, writer, child });
        Ok((id, rx))
    }

    fn with<T>(&self, id: SessionId, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        let mut map = self.inner.sessions.lock().map_err(|_| anyhow!("lock"))?;
        let s = map.get_mut(&id).ok_or_else(|| anyhow!("sessão {id} não existe"))?;
        f(s)
    }

    pub fn write(&self, id: SessionId, data: &[u8]) -> Result<()> {
        self.with(id, |s| {
            s.writer.write_all(data)?;
            s.writer.flush()?;
            Ok(())
        })
    }

    pub fn resize(&self, id: SessionId, cols: u16, rows: u16) -> Result<()> {
        self.with(id, |s| {
            s.master
                .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
                .map_err(|e| anyhow!("resize: {e}"))?;
            s.info.cols = cols;
            s.info.rows = rows;
            Ok(())
        })
    }

    pub fn kill(&self, id: SessionId) -> Result<()> {
        let session = self
            .inner
            .sessions
            .lock()
            .map_err(|_| anyhow!("lock"))?
            .remove(&id)
            .ok_or_else(|| anyhow!("sessão {id} não existe"))?;
        if let Ok(mut c) = session.child.lock() {
            let _ = c.kill();
        }
        Ok(())
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        self.inner
            .sessions
            .lock()
            .map(|m| m.values().map(|s| s.info.clone()).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn scripts_run_through_cmd_and_names_are_resolved() {
        let args = vec!["auth".to_string(), "login".to_string()];
        let (p, a) = wrap_script(r"C:
pm\opencode.cmd", &args);
        if cfg!(windows) {
            assert_eq!(p, "cmd.exe");
            assert_eq!(a, ["/c", r"C:
pm\opencode.cmd", "auth", "login"]);
        } else {
            assert_eq!((p.as_str(), a.len()), (r"C:
pm\opencode.cmd", 2));
        }
        // Executável de verdade (ou nome desconhecido) passa direto.
        assert_eq!(wrap_script(r"C:un\opencode.exe", &args), (r"C:un\opencode.exe".to_string(), args.clone()));
        assert_eq!(launch_target("programa-que-nao-existe-xyz", &args).0, "programa-que-nao-existe-xyz");
        // Um nome do PATH vira caminho completo de um executável do sistema, nunca um arquivo sem extensão.
        #[cfg(windows)]
        {
            let (p, _) = launch_target("cmd", &[]);
            assert!(p.to_ascii_lowercase().ends_with("cmd.exe"), "{p}");
        }
    }

    #[tokio::test]
    async fn echo_output() {
        let mgr = PtyManager::new();
        #[cfg(windows)]
        let spec = CommandSpec { program: "cmd".into(), args: vec!["/c".into(), "echo oi".into()], ..Default::default() };
        #[cfg(not(windows))]
        let spec = CommandSpec { program: "sh".into(), args: vec!["-c".into(), "echo oi".into()], ..Default::default() };
        let (id, mut rx) = mgr.spawn(spec, 80, 24).unwrap();
        let mut out = Vec::new();
        let res = tokio::time::timeout(Duration::from_secs(15), async {
            while let Some(ev) = rx.recv().await {
                match ev {
                    PtyEvent::Output(b) => {
                        // ConPTY pode pedir a posição do cursor (DSR); um terminal real responde.
                        if b.windows(4).any(|w| w == b"\x1b[6n") {
                            let _ = mgr.write(id, b"\x1b[1;1R");
                        }
                        out.extend(b);
                        if String::from_utf8_lossy(&out).contains("oi") {
                            return;
                        }
                    }
                    PtyEvent::Exit(_) => return,
                }
            }
        })
        .await;
        assert!(res.is_ok(), "timeout");
        assert!(String::from_utf8_lossy(&out).contains("oi"), "saída: {:?}", String::from_utf8_lossy(&out));
    }
}
