//! Acesso remoto (celular) às sessões de agentes.
//!
//! Servidor HTTP/WebSocket (axum) desligado por padrão, iniciado pela IDE sob
//! demanda. Toda rota exige o token de pareamento (header `Authorization:
//! Bearer <token>` ou cookie definido em `/pair?token=...`). Revogar =
//! regenerar o token: cookies e QR antigos deixam de valer na hora.
//! Fora da LAN, use uma VPN como Tailscale em vez de expor a porta.

use crate::autowork::AutoWork;
use crate::agents::orchestrator::{Orchestrator, OrchestratorEvent, TaskId};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, Request, State,
    },
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::{broadcast, oneshot};

const COOKIE: &str = "argus_token";

const INDEX_HTML: &str = include_str!("../../app/assets/remote/index.html");
const APP_JS: &str = include_str!("../../app/assets/remote/app.js");
const STYLE_CSS: &str = include_str!("../../app/assets/remote/style.css");
const MANIFEST: &str = include_str!("../../app/assets/remote/manifest.webmanifest");
const ICON_SVG: &str = include_str!("../../app/assets/remote/icon.svg");

/// Dados para exibir ao usuário depois de iniciar o servidor.
#[derive(Debug, Clone, Serialize)]
pub struct RemoteInfo {
    /// URL de pareamento (contém o token).
    pub url: String,
    /// QR code da `url` em SVG.
    pub qr_svg: String,
    pub port: u16,
    /// IP usado na URL.
    pub host: String,
    /// IPs disponíveis para escolha (LAN primeiro).
    pub hosts: Vec<LanAddr>,
}

/// Endereço IPv4 local candidato para o pareamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanAddr {
    pub ip: String,
    pub iface: String,
    pub kind: AddrKind,
}

/// Classificação do endereço (ordem = preferência).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum AddrKind {
    /// 192.168/16, 10/8, 172.16/12 em interface física.
    Lan,
    /// 100.64/10 (Tailscale / CGNAT).
    Tailscale,
    /// VPN/virtual (Radmin 26.x, Hyper-V, WSL, VirtualBox, VMware, Docker...).
    Virtual,
}

impl AddrKind {
    pub fn label(self) -> &'static str {
        match self {
            AddrKind::Lan => "LAN",
            AddrKind::Tailscale => "Tailscale",
            AddrKind::Virtual => "VPN/virtual",
        }
    }
}

fn is_virtual_iface(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["vethernet", "virtualbox", "vmware", "vbox", "hyper-v", "wsl", "docker", "radmin", "hamachi", "zerotier", "loopback", "vpn", "tap", "tun", "wireguard", "npcap"]
        .iter()
        .any(|k| n.contains(k))
}

/// Classifica um IPv4; `None` = descartar (loopback, link-local, público...).
pub fn classify(ip: Ipv4Addr, iface: &str) -> Option<AddrKind> {
    let o = ip.octets();
    if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() {
        return None;
    }
    if o[0] == 100 && (64..128).contains(&o[1]) || iface.to_ascii_lowercase().contains("tailscale") {
        return Some(AddrKind::Tailscale);
    }
    if o[0] == 26 {
        return Some(AddrKind::Virtual); // Radmin VPN
    }
    if ip.is_private() {
        return Some(if is_virtual_iface(iface) { AddrKind::Virtual } else { AddrKind::Lan });
    }
    None
}

/// Ordena candidatos: LAN (192.168 > 10 > 172) antes de Tailscale e virtuais.
pub fn rank(mut v: Vec<LanAddr>) -> Vec<LanAddr> {
    fn sub(ip: &str) -> u8 {
        if ip.starts_with("192.168.") { 0 } else if ip.starts_with("10.") { 1 } else { 2 }
    }
    v.sort_by(|a, b| (a.kind, sub(&a.ip)).cmp(&(b.kind, sub(&b.ip))));
    v.dedup_by(|a, b| a.ip == b.ip);
    v
}

/// IPv4 locais candidatos, melhor primeiro.
pub fn lan_addrs() -> Vec<LanAddr> {
    let list = local_ip_address::list_afinet_netifas().unwrap_or_default();
    rank(
        list.into_iter()
            .filter_map(|(iface, ip)| match ip {
                IpAddr::V4(v4) => classify(v4, &iface).map(|kind| LanAddr { ip: v4.to_string(), iface, kind }),
                _ => None,
            })
            .collect(),
    )
}

/// Gera token aleatório de 32 bytes em base64url (sem padding).
pub fn new_token() -> String {
    let bytes: [u8; 32] = rand::random();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// SVG do QR code de `text`.
pub fn qr_svg(text: &str) -> anyhow::Result<String> {
    let code = qrcode::QrCode::new(text.as_bytes())?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .build())
}

fn lan_ip() -> IpAddr {
    lan_addrs()
        .first()
        .and_then(|a| a.ip.parse().ok())
        .or_else(|| local_ip_address::local_ip().ok())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[derive(Clone)]
struct AppState {
    orch: Orchestrator,
    aw: Arc<RwLock<Option<AutoWork>>>,
    token: Arc<RwLock<String>>,
}

impl AppState {
    fn check(&self, candidate: &str) -> bool {
        let t = self.token.read().unwrap();
        !t.is_empty() && ct_eq(t.as_bytes(), candidate.as_bytes())
    }
}

fn token_from_headers(h: &HeaderMap) -> Option<String> {
    if let Some(v) = h.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(t) = v.strip_prefix("Bearer ") {
            return Some(t.trim().to_string());
        }
    }
    for v in h.get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()) {
        for part in v.split(';') {
            if let Some((k, val)) = part.trim().split_once('=') {
                if k == COOKIE {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

async fn auth(State(st): State<AppState>, req: Request, next: Next) -> Response {
    match token_from_headers(req.headers()) {
        Some(t) if st.check(&t) => next.run(req).await,
        _ => (StatusCode::UNAUTHORIZED, "não autorizado: escaneie o QR code na IDE").into_response(),
    }
}

#[derive(Deserialize)]
struct PairQuery {
    token: String,
}

async fn pair(State(st): State<AppState>, Query(q): Query<PairQuery>) -> Response {
    if !st.check(&q.token) {
        return (StatusCode::UNAUTHORIZED, "token inválido ou revogado").into_response();
    }
    let cookie = format!("{COOKIE}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000", q.token);
    ([(header::SET_COOKIE, cookie)], Redirect::to("/")).into_response()
}

fn asset(ct: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, ct), (header::CACHE_CONTROL, "no-cache")], body).into_response()
}

fn err(code: StatusCode, msg: impl ToString) -> Response {
    (code, Json(json!({ "error": msg.to_string() }))).into_response()
}

fn orch_err(e: crate::agents::orchestrator::OrchestratorError) -> Response {
    use crate::agents::orchestrator::OrchestratorError as E;
    let code = match e {
        E::UnknownTask(_) => StatusCode::NOT_FOUND,
        E::UnknownProvider(_) => StatusCode::BAD_REQUEST,
        E::Busy | E::NoSession | E::Closed | E::NoWorktree => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    err(code, e)
}

async fn list_tasks(State(st): State<AppState>) -> Response {
    Json(st.orch.tasks()).into_response()
}

async fn get_task(State(st): State<AppState>, Path(id): Path<TaskId>) -> Response {
    match st.orch.task(id) {
        Some(t) => Json(json!({ "task": t, "events": st.orch.history(id) })).into_response(),
        None => err(StatusCode::NOT_FOUND, "tarefa não encontrada"),
    }
}

#[derive(Deserialize)]
struct ReplyBody {
    message: String,
}

async fn reply(State(st): State<AppState>, Path(id): Path<TaskId>, Json(b): Json<ReplyBody>) -> Response {
    if b.message.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "mensagem vazia");
    }
    match st.orch.reply(id, &b.message) {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(e) => orch_err(e),
    }
}

async fn cancel(State(st): State<AppState>, Path(id): Path<TaskId>) -> Response {
    match st.orch.cancel(id) {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(e) => orch_err(e),
    }
}

#[derive(Deserialize)]
struct NewTask {
    project: String,
    provider: String,
    prompt: String,
}

async fn create_task(State(st): State<AppState>, Json(b): Json<NewTask>) -> Response {
    if b.prompt.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "pedido vazio");
    }
    let p = std::path::PathBuf::from(&b.project);
    if !p.is_dir() {
        return err(StatusCode::BAD_REQUEST, "projeto inexistente");
    }
    match st.orch.submit(p, &b.provider, &b.prompt) {
        Ok(id) => (StatusCode::CREATED, Json(json!({ "id": id }))).into_response(),
        Err(e) => orch_err(e),
    }
}

/// Projetos conhecidos (das tarefas existentes), para o formulário de nova tarefa.
async fn projects(State(st): State<AppState>) -> Response {
    let mut v: Vec<String> = st.orch.tasks().into_iter().map(|t| t.project.to_string_lossy().into_owned()).collect();
    v.sort();
    v.dedup();
    Json(v).into_response()
}

fn autowork_of(st: &AppState) -> Result<AutoWork, Response> {
    st.aw.read().unwrap().clone().ok_or_else(|| err(StatusCode::NOT_FOUND, "auto-work indisponível"))
}

/// Configuração por projeto + execuções em andamento.
async fn autowork_get(State(st): State<AppState>) -> Response {
    let aw = match autowork_of(&st) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let settings: Vec<Value> =
        aw.all_settings().into_iter().map(|(p, s)| json!({ "project": p, "settings": s })).collect();
    Json(json!({ "settings": settings, "runs": aw.all_status() })).into_response()
}

#[derive(Deserialize)]
struct AutoWorkToggle {
    project: String,
    enabled: bool,
}

async fn autowork_set(State(st): State<AppState>, Json(b): Json<AutoWorkToggle>) -> Response {
    let aw = match autowork_of(&st) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let p = std::path::PathBuf::from(&b.project);
    aw.set_enabled(&p, b.enabled);
    Json(json!({ "project": p, "settings": aw.settings(&p) })).into_response()
}

async fn autowork_action(State(st): State<AppState>, Path((id, action)): Path<(TaskId, String)>) -> Response {
    let aw = match autowork_of(&st) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let ok = match action.as_str() {
        "start" => aw.start_task(id),
        "pause" => aw.pause(id),
        "resume" => aw.resume(id),
        "stop" => aw.stop(id),
        _ => return err(StatusCode::BAD_REQUEST, "ação inválida"),
    };
    if ok {
        StatusCode::ACCEPTED.into_response()
    } else {
        err(StatusCode::CONFLICT, "auto-work não está nesse estado")
    }
}

/// Converte evento do orquestrador em JSON para o WebSocket.
pub fn event_json(ev: &OrchestratorEvent) -> Value {
    match ev {
        OrchestratorEvent::StateChanged { task, state } => json!({ "type": "state", "task": task, "state": state }),
        OrchestratorEvent::Agent { task, event } => json!({ "type": "agent", "task": task, "event": event }),
        OrchestratorEvent::Stderr { task, line } => json!({ "type": "stderr", "task": task, "line": line }),
        OrchestratorEvent::AutoWork(e) => json!({ "type": "autowork", "task": e.task, "autowork": e }),
    }
}

async fn stream(State(st): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let rx = st.orch.subscribe();
    let token = st.token.clone();
    let initial = token.read().unwrap().clone();
    ws.on_upgrade(move |sock| ws_loop(sock, rx, token, initial))
}

async fn ws_loop(
    mut sock: WebSocket,
    mut rx: broadcast::Receiver<OrchestratorEvent>,
    token: Arc<RwLock<String>>,
    initial: String,
) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(ev) => {
                    if sock.send(Message::Text(event_json(&ev).to_string().into())).await.is_err() { break; }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // cliente atrasou: pede para recarregar
                    if sock.send(Message::Text(json!({"type":"resync"}).to_string().into())).await.is_err() { break; }
                }
                Err(_) => break,
            },
            msg = sock.recv() => match msg {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            },
            _ = tick.tick() => {
                // token revogado: encerra conexões abertas
                if *token.read().unwrap() != initial { break; }
            }
        }
    }
    let _ = sock.send(Message::Close(None)).await;
}

fn router(st: AppState) -> Router {
    let protected = Router::new()
        .route("/", get(|| async { asset("text/html; charset=utf-8", INDEX_HTML) }))
        .route("/app.js", get(|| async { asset("application/javascript; charset=utf-8", APP_JS) }))
        .route("/style.css", get(|| async { asset("text/css; charset=utf-8", STYLE_CSS) }))
        .route("/manifest.webmanifest", get(|| async { asset("application/manifest+json", MANIFEST) }))
        .route("/icon.svg", get(|| async { asset("image/svg+xml", ICON_SVG) }))
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route("/api/projects", get(projects))
        .route("/api/tasks/{id}", get(get_task))
        .route("/api/tasks/{id}/reply", post(reply))
        .route("/api/tasks/{id}/cancel", post(cancel))
        .route("/api/stream", get(stream))
        .route("/api/autowork", get(autowork_get).post(autowork_set))
        .route("/api/tasks/{id}/autowork/{action}", post(autowork_action))
        .route_layer(middleware::from_fn_with_state(st.clone(), auth));
    Router::new().route("/pair", get(pair)).merge(protected).with_state(st)
}

struct Running {
    shutdown: oneshot::Sender<()>,
    port: u16,
}

/// Servidor remoto. Desligado até `start`.
pub struct RemoteServer {
    token: Arc<RwLock<String>>,
    running: Mutex<Option<Running>>,
    /// IP escolhido pelo usuário (None = automático).
    host: RwLock<Option<String>>,
    /// Controlador do auto-work exposto pela API (opcional).
    aw: Arc<RwLock<Option<AutoWork>>>,
}

impl Default for RemoteServer {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteServer {
    pub fn new() -> Self {
        Self { token: Arc::new(RwLock::new(new_token())), running: Mutex::new(None), host: RwLock::new(None), aw: Arc::new(RwLock::new(None)) }
    }

    /// Liga os endpoints de auto-work (`/api/autowork`).
    pub fn set_autowork(&self, aw: AutoWork) {
        *self.aw.write().unwrap() = Some(aw);
    }

    pub fn is_running(&self) -> bool {
        self.running.lock().unwrap().is_some()
    }

    /// Inicia em 0.0.0.0:`port` (0 = porta livre). Se já rodando, reinicia.
    pub async fn start(&self, orch: Orchestrator, port: u16) -> anyhow::Result<RemoteInfo> {
        self.stop();
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await?;
        let port = listener.local_addr()?.port();
        let app = router(AppState { orch, aw: self.aw.clone(), token: self.token.clone() });
        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let r = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
            if let Err(e) = r {
                tracing::warn!("servidor remoto: {e}");
            }
        });
        *self.running.lock().unwrap() = Some(Running { shutdown: tx, port });
        self.info()
    }

    pub fn stop(&self) {
        if let Some(r) = self.running.lock().unwrap().take() {
            let _ = r.shutdown.send(());
        }
    }

    /// Revoga o token atual (todos os aparelhos pareados perdem acesso) e gera outro.
    pub fn regenerate_token(&self) -> anyhow::Result<RemoteInfo> {
        *self.token.write().unwrap() = new_token();
        self.info()
    }

    /// Fixa o IP usado na URL (None = automático) e devolve a nova info.
    pub fn set_host(&self, host: Option<String>) -> anyhow::Result<RemoteInfo> {
        *self.host.write().unwrap() = host;
        self.info()
    }

    /// URL + QR atuais (erro se parado).
    pub fn info(&self) -> anyhow::Result<RemoteInfo> {
        let port = self.running.lock().unwrap().as_ref().map(|r| r.port).ok_or_else(|| anyhow::anyhow!("servidor parado"))?;
        let host = self.host.read().unwrap().clone().unwrap_or_else(|| lan_ip().to_string());
        let url = format!("http://{host}:{port}/pair?token={}", self.token.read().unwrap());
        Ok(RemoteInfo { qr_svg: qr_svg(&url)?, url, port, host, hosts: lan_addrs() })
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn http(port: u16, req: String) -> String {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = Vec::new();
        s.read_to_end(&mut out).await.unwrap();
        String::from_utf8_lossy(&out).into_owned()
    }

    fn get(path: &str, extra: &str) -> String {
        format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n{extra}\r\n")
    }

    #[test]
    fn classifies_and_ranks_addrs() {
        let c = |s: &str, i: &str| classify(s.parse().unwrap(), i);
        assert_eq!(c("192.168.0.5", "Wi-Fi"), Some(AddrKind::Lan));
        assert_eq!(c("172.20.1.1", "vEthernet (WSL)"), Some(AddrKind::Virtual));
        assert_eq!(c("26.1.2.3", "Radmin VPN"), Some(AddrKind::Virtual));
        assert_eq!(c("100.101.1.1", "Tailscale"), Some(AddrKind::Tailscale));
        assert_eq!(c("127.0.0.1", "lo"), None);
        assert_eq!(c("169.254.1.1", "Ethernet"), None);
        assert_eq!(c("8.8.8.8", "Ethernet"), None);
        let a = |ip: &str, kind| LanAddr { ip: ip.into(), iface: "x".into(), kind };
        let r = rank(vec![a("26.0.0.1", AddrKind::Virtual), a("100.64.0.1", AddrKind::Tailscale), a("10.0.0.2", AddrKind::Lan), a("192.168.1.2", AddrKind::Lan)]);
        let ips: Vec<_> = r.iter().map(|x| x.ip.as_str()).collect();
        assert_eq!(ips, ["192.168.1.2", "10.0.0.2", "100.64.0.1", "26.0.0.1"]);
    }

    #[test]
    fn token_and_qr() {
        let t = new_token();
        assert_eq!(t.len(), 43);
        assert_ne!(t, new_token());
        assert!(qr_svg("http://1.2.3.4:1/pair?token=abc").unwrap().contains("<svg"));
        assert!(ct_eq(b"abc", b"abc") && !ct_eq(b"abc", b"abd") && !ct_eq(b"a", b"ab"));
    }

    #[test]
    fn parses_bearer_and_cookie() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer xyz".parse().unwrap());
        assert_eq!(token_from_headers(&h).as_deref(), Some("xyz"));
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, format!("a=1; {COOKIE}=tok; b=2").parse().unwrap());
        assert_eq!(token_from_headers(&h).as_deref(), Some("tok"));
    }

    #[tokio::test]
    async fn auth_flow() {
        let srv = RemoteServer::new();
        let orch = Orchestrator::new(vec![], 1, false);
        let info = srv.start(orch, 0).await.unwrap();
        assert!(info.qr_svg.contains("<svg"));
        let port = info.port;
        let token = srv.token.read().unwrap().clone();

        // sem token
        assert!(http(port, get("/api/tasks", "")).await.starts_with("HTTP/1.1 401"));
        assert!(http(port, get("/", "")).await.starts_with("HTTP/1.1 401"));
        // token errado
        assert!(http(port, get("/api/tasks", "Authorization: Bearer nope\r\n")).await.starts_with("HTTP/1.1 401"));
        assert!(http(port, get("/pair?token=nope", "")).await.starts_with("HTTP/1.1 401"));
        // bearer certo
        let r = http(port, get("/api/tasks", &format!("Authorization: Bearer {token}\r\n"))).await;
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
        assert!(r.ends_with("[]"));
        // pareamento define cookie
        let r = http(port, get(&format!("/pair?token={token}"), "")).await;
        assert!(r.starts_with("HTTP/1.1 303"), "{r}");
        assert!(r.contains("HttpOnly"));
        let ck = format!("Cookie: {COOKIE}={token}\r\n");
        assert!(http(port, get("/", &ck)).await.starts_with("HTTP/1.1 200"));
        assert!(http(port, get("/api/tasks/99", &ck)).await.starts_with("HTTP/1.1 404"));
        // revogação
        srv.regenerate_token().unwrap();
        assert!(http(port, get("/", &ck)).await.starts_with("HTTP/1.1 401"));
        srv.stop();
    }
}
