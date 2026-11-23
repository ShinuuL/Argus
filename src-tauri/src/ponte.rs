//! Ponte de validação: deixa um agente (ou um script) *mexer* na janela.
//!
//! O problema que isto resolve está escrito no PROXIMOS_PASSOS: dá para
//! fotografar a janela do Argus de fora (`ffmpeg -f gdigrab`), mas não dá para
//! operá-la — nem `SendKeys` nem clique sintético do Win32 chegam ao WebView2.
//! Resultado: dezenas de itens de validação travados em "falta ver na janela",
//! esperando uma pessoa clicar.
//!
//! A ponte entra por dentro, não por fora. Com a variável `ARGUS_PONTE`
//! definida, o Argus sobe um servidor minúsculo em `127.0.0.1` que repassa cada
//! pedido para a interface (`src/dev/ponte.ts`), que executa de dentro da
//! página — clique de verdade, no mesmo caminho de um clique humano — e devolve
//! a resposta.
//!
//! ```text
//! node scripts/ui.mjs ver
//! node scripts/ui.mjs clicar "Configurações"
//! ```
//!
//! Três travas, porque isto é controle remoto da interface:
//! 1. só existe com `ARGUS_PONTE` definida (fora disso nada é escutado);
//! 2. escuta em `127.0.0.1`, nunca em `0.0.0.0`;
//! 3. não mexe em arquivo nem em banco: tudo que ela faz, a própria janela faz.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

/// Variável que liga a ponte: `1` usa a porta padrão, um número usa aquela porta.
pub const VAR: &str = "ARGUS_PONTE";
pub const PORTA_PADRAO: u16 = 8787;
/// Evento que leva o pedido até a janela.
const EVENTO: &str = "argus://ponte";
/// Quanto esperar a janela responder. Generoso de propósito: a ação pode ser um
/// "espere o texto X aparecer", que legitimamente leva segundos.
const ESPERA: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Ponte {
    pendentes: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    seq: AtomicU64,
}

fn ponte() -> &'static Ponte {
    static P: OnceLock<Ponte> = OnceLock::new();
    P.get_or_init(Ponte::default)
}

/// Porta pedida, ou `None` quando a ponte está desligada.
pub fn porta() -> Option<u16> {
    let v = std::env::var(VAR).ok()?;
    let v = v.trim();
    match v {
        "" | "0" | "nao" | "não" => None,
        "1" | "sim" => Some(PORTA_PADRAO),
        n => n.parse().ok(),
    }
}

/// A ponte está ligada? A interface pergunta ao abrir e só então escuta.
#[tauri::command]
pub fn ponte_ligada() -> bool {
    porta().is_some()
}

/// Resposta da janela a um pedido (ver `src/dev/ponte.ts`).
#[tauri::command]
pub fn ponte_resposta(id: u64, resultado: Value) {
    if let Some(tx) = ponte().pendentes.lock().unwrap().remove(&id) {
        let _ = tx.send(resultado);
    }
}

/// Sobe o servidor, se a variável mandar. Silencioso quando desligada.
pub fn start(app: &AppHandle) {
    let Some(porta) = porta() else { return };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let rotas = axum::Router::new()
            .route("/cmd", axum::routing::post(cmd))
            .route("/saude", axum::routing::get(saude))
            .with_state(app);
        match tokio::net::TcpListener::bind(("127.0.0.1", porta)).await {
            Ok(l) => {
                eprintln!("ponte de validação em http://127.0.0.1:{porta}");
                let _ = axum::serve(l, rotas).await;
            }
            // Erro aqui não derruba o app: a ponte é ferramenta de teste.
            Err(e) => eprintln!("ponte de validação não subiu na porta {porta}: {e}"),
        }
    });
}

async fn saude() -> axum::Json<Value> {
    axum::Json(json!({ "ok": true, "versao": env!("CARGO_PKG_VERSION") }))
}

async fn cmd(axum::extract::State(app): axum::extract::State<AppHandle>, axum::Json(mut corpo): axum::Json<Value>) -> axum::Json<Value> {
    let id = ponte().seq.fetch_add(1, Ordering::SeqCst) + 1;
    let (tx, rx) = oneshot::channel();
    ponte().pendentes.lock().unwrap().insert(id, tx);
    if !corpo.is_object() {
        corpo = json!({ "acao": corpo });
    }
    corpo["id"] = json!(id);
    if let Err(e) = app.emit(EVENTO, corpo) {
        ponte().pendentes.lock().unwrap().remove(&id);
        return axum::Json(json!({ "erro": format!("a janela não recebeu o pedido: {e}") }));
    }
    match tokio::time::timeout(ESPERA, rx).await {
        Ok(Ok(v)) => axum::Json(v),
        _ => {
            ponte().pendentes.lock().unwrap().remove(&id);
            axum::Json(json!({ "erro": format!("a janela não respondeu em {}s", ESPERA.as_secs()) }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porta_le_a_variavel() {
        // A variável é global ao processo: um teste só, em sequência.
        let antes = std::env::var(VAR).ok();
        std::env::remove_var(VAR);
        assert_eq!(porta(), None, "sem a variável a ponte não existe");
        std::env::set_var(VAR, "1");
        assert_eq!(porta(), Some(PORTA_PADRAO));
        std::env::set_var(VAR, "9123");
        assert_eq!(porta(), Some(9123));
        std::env::set_var(VAR, "nao");
        assert_eq!(porta(), None);
        std::env::set_var(VAR, "qualquer coisa");
        assert_eq!(porta(), None, "valor ilegível desliga, não adivinha porta");
        match antes {
            Some(v) => std::env::set_var(VAR, v),
            None => std::env::remove_var(VAR),
        }
    }

    #[tokio::test]
    async fn resposta_sem_dono_nao_quebra() {
        // Resposta atrasada, depois do tempo esgotado: o id já não existe.
        ponte_resposta(999_999, json!({ "ok": true }));
    }
}
