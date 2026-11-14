//! Terminal embutido com xterm.js (vendorizado em `assets/xterm`) ligado a um PTY do core.
//!
//! Saída: `PtyEvent::Output` -> base64 -> `term.write`.
//! Entrada: `term.onData` -> Rust -> `PtyManager::write`.

use super::editor::{assets_base_url, unique_dom_id};
use dioxus::prelude::*;
use argus_core::pty::{default_shell, CommandSpec, PtyEvent, PtyManager, SessionId};
use serde_json::{json, Value};
use std::path::PathBuf;

const TERM_JS: &str = r#"
const elId = __EL_ID__;
const base = __BASE__;
function loadScript(src) {
  return new Promise((ok, err) => {
    const s = document.createElement('script');
    s.src = src; s.onload = ok; s.onerror = () => err(new Error('falha ao carregar ' + src));
    document.head.appendChild(s);
  });
}
if (!window.__xtermReady) {
  window.__xtermReady = (async () => {
    const l = document.createElement('link');
    l.rel = 'stylesheet'; l.href = base + '/xterm/xterm.css';
    document.head.appendChild(l);
    // Evita que o loader AMD do Monaco capture os módulos UMD do xterm.
    const def = window.define; window.define = undefined;
    try {
      await loadScript(base + '/xterm/xterm.js');
      await loadScript(base + '/xterm/addon-fit.js');
    } finally { window.define = def; }
  })();
}
await window.__xtermReady;
let el = null;
for (let i = 0; i < 200 && !(el = document.getElementById(elId)); i++) {
  await new Promise(r => setTimeout(r, 25));
}
if (!el) return;
const term = new Terminal({ cursorBlink: true, fontSize: 13, fontFamily: 'Cascadia Mono, Consolas, monospace', theme: { background: '#1e1e1e' } });
const fit = new FitAddon.FitAddon();
term.loadAddon(fit);
term.open(el);
try { fit.fit(); } catch (_) {}
term.onData(d => dioxus.send({ ev: 'data', data: d }));
term.onResize(s => dioxus.send({ ev: 'resize', cols: s.cols, rows: s.rows }));
new ResizeObserver(() => { try { fit.fit(); } catch (_) {} }).observe(el);
dioxus.send({ ev: 'ready', cols: term.cols, rows: term.rows });
const dec = (b64) => Uint8Array.from(atob(b64), c => c.charCodeAt(0));
while (true) {
  const m = await dioxus.recv();
  if (!document.getElementById(elId)) { term.dispose(); return; }
  if (m.out !== undefined) term.write(dec(m.out));
  else if (m.exit !== undefined) term.write('\r\n[processo finalizado: ' + m.exit + ']\r\n');
}
"#;

/// Incrementado sempre que um processo de terminal termina (ex.: login da CLI).
pub static TERM_EXITS: GlobalSignal<u64> = Signal::global(|| 0);

/// Codificação base64 padrão (sem dependência externa).
pub fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Terminal com o shell padrão do sistema, aberto em `cwd`.
///
/// Usa o `PtyManager` do contexto, se houver (`use_context_provider`); senão cria um próprio.
/// O processo é encerrado quando o componente é desmontado.
#[component]
pub fn Terminal(cwd: PathBuf, command: Option<CommandSpec>) -> Element {
    let el_id = use_hook(|| unique_dom_id("xterm"));
    let pty = use_hook(|| try_consume_context::<PtyManager>().unwrap_or_default());
    let mut session = use_signal(|| None::<SessionId>);
    let mut error = use_signal(|| None::<String>);

    let bridge = use_hook({
        let el_id = el_id.clone();
        move || {
            let js = TERM_JS
                .replace("__EL_ID__", &json!(el_id).to_string())
                .replace("__BASE__", &json!(assets_base_url()).to_string());
            document::eval(&js)
        }
    });

    use_drop({
        let pty = pty.clone();
        move || {
            if let Some(id) = *session.peek() {
                let _ = pty.kill(id);
            }
        }
    });

    use_future({
        let pty = pty.clone();
        let bridge = bridge.clone();
        move || {
            let pty = pty.clone();
            let mut rx_js = bridge.clone();
            let tx_js = bridge.clone();
            let cwd = cwd.clone();
            let command = command.clone();
            async move {
                while let Ok(msg) = rx_js.recv::<Value>().await {
                    let size = || {
                        (
                            msg["cols"].as_u64().unwrap_or(80) as u16,
                            msg["rows"].as_u64().unwrap_or(24) as u16,
                        )
                    };
                    match msg["ev"].as_str() {
                        Some("ready") if session.peek().is_none() => {
                            let (cols, rows) = size();
                            let spec = CommandSpec {
                                cwd: Some(cwd.clone()),
                                ..command.clone().unwrap_or_else(default_shell)
                            };
                            match pty.spawn(spec, cols, rows) {
                                Ok((id, mut events)) => {
                                    session.set(Some(id));
                                    let tx = tx_js.clone();
                                    spawn(async move {
                                        while let Some(ev) = events.recv().await {
                                            match ev {
                                                PtyEvent::Output(bytes) => {
                                                    super::deps::on_terminal_output(&String::from_utf8_lossy(&bytes));
                                                    let _ = tx.send(
                                                        json!({"out": base64_encode(&bytes)}),
                                                    );
                                                }
                                                PtyEvent::Exit(code) => {
                                                    let c = code
                                                        .map(|c| c.to_string())
                                                        .unwrap_or_else(|| "?".into());
                                                    let _ = tx.send(json!({"exit": c}));
                                                    *TERM_EXITS.write() += 1;
                                                    break;
                                                }
                                            }
                                        }
                                    });
                                }
                                Err(e) => {
                                    error.set(Some(format!("Falha ao iniciar terminal: {e}")))
                                }
                            }
                        }
                        Some("data") => {
                            if let (Some(id), Some(d)) = (*session.peek(), msg["data"].as_str()) {
                                let _ = pty.write(id, d.as_bytes());
                            }
                        }
                        Some("resize") => {
                            if let Some(id) = *session.peek() {
                                let (cols, rows) = size();
                                let _ = pty.resize(id, cols, rows);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    rsx! {
        div {
            class: "terminal-root",
            style: "display:flex;flex-direction:column;height:100%;min-height:0;background:#1e1e1e;",
            if let Some(e) = error() {
                div { style: "color:#f37272;padding:4px 8px;font-size:12px;", "{e}" }
            }
            div { id: "{el_id}", style: "flex:1;min-height:0;padding:4px;" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::base64_encode;

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }
}
