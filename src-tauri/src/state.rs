//! Estado global do backend, registrado com `app.manage` e lido pelos comandos.
use argus_core::agents::claude::ClaudeProvider;
use argus_core::agents::codex::CodexProvider;
use argus_core::agents::gemini::GeminiProvider;
use argus_core::agents::opencode::OpencodeProvider;
use argus_core::agents::orchestrator::Orchestrator;
use argus_core::agents::provider::AgentProvider;
use argus_core::autowork::AutoWork;
use argus_core::db::Db;
use argus_core::devices::ScreenMirror;
use argus_core::monitor::WatchHandle;
use argus_core::pty::PtyManager;
use argus_core::remote::RemoteServer;
use argus_core::schedules::Scheduler;
use argus_core::workspace::Project;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

pub struct AppCtx {
    pub pty: PtyManager,
    pub providers: Vec<Arc<dyn AgentProvider>>,
    pub orch: Orchestrator,
    pub autowork: AutoWork,
    /// Tarefas agendadas (o laço liga no `setup`, com o aviso para a janela).
    pub scheduler: Scheduler,
    pub remote: Arc<RemoteServer>,
    /// Caminho do banco em uso (None = histórico desligado).
    pub db_path: Option<PathBuf>,
    pub root: Mutex<Option<PathBuf>>,
    pub projects: Mutex<Vec<Project>>,
    pub watch: Mutex<Option<WatchHandle>>,
    /// Watcher do primeiro nível da pasta raiz (subpastas novas/removidas).
    pub root_watch: Mutex<Option<WatchHandle>>,
    /// Comandos ausentes já sugeridos a partir do terminal (evita repetir).
    pub suggested: Mutex<HashSet<String>>,
    pub sys: Arc<Mutex<Option<sysinfo::System>>>,
    pub audio: OnceLock<crate::commands::media::LocalAudio>,
    /// Screen-Mirror (celulares Android) rodando como processo auxiliar.
    pub mirror: ScreenMirror,
}

/// Caminho do banco: `ARGUS_DB` (testes) ou `%APPDATA%/argus/argus.db`.
pub fn db_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ARGUS_DB").map(PathBuf::from).filter(|p| !p.as_os_str().is_empty()) {
        return Some(p);
    }
    let base = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::current_dir().ok())?;
    Some(base.join(argus_core::paths::dir_name()).join("argus.db"))
}

/// Arquivo na pasta de dados do Argus (`%APPDATA%/argus/`).
fn data_file(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
    Some(base.join(argus_core::paths::dir_name()).join(name))
}

/// Abre o banco; None (com aviso no stderr) se não for possível.
fn open_db() -> Option<(Db, PathBuf)> {
    let path = db_path()?;
    if let Some(parent) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).ok()?;
    }
    match Db::open(&path) {
        Ok(db) => Some((db, path)),
        Err(e) => {
            eprintln!("banco indisponível ({}): {e}", path.display());
            None
        }
    }
}

impl AppCtx {
    /// Precisa rodar dentro do runtime tokio (o auto-work chama `tokio::spawn`).
    pub fn new() -> Self {
        // `ARGUS_OPENCODE_MODEL` (`provedor/modelo`, como em `opencode models`) é só o modelo
        // inicial das conversas novas do OpenCode; o de cada conversa é escolhido na interface
        // e fica gravado na tarefa. Sem ela, o inicial é o padrão da própria CLI.
        let opencode = match std::env::var("ARGUS_OPENCODE_MODEL").ok().map(|m| m.trim().to_string()).filter(|m| !m.is_empty()) {
            Some(model) => OpencodeProvider::with_model(model),
            None => OpencodeProvider::new(),
        };
        let providers: Vec<Arc<dyn AgentProvider>> = vec![
            Arc::new(ClaudeProvider::new()),
            Arc::new(opencode),
            Arc::new(CodexProvider::new()),
            Arc::new(GeminiProvider::new()),
        ];
        let (db, db_path) = match open_db() {
            Some((db, p)) => (Some(db), Some(p)),
            None => (None, None),
        };
        let orch = Orchestrator::with_db(providers.clone(), 2, db);
        let autowork = AutoWork::start(orch.clone());
        let scheduler = Scheduler::new(orch.clone());
        let remote = RemoteServer::new();
        remote.set_autowork(autowork.clone());
        Self {
            pty: PtyManager::new(),
            providers,
            orch,
            autowork,
            scheduler,
            remote: Arc::new(remote),
            db_path,
            root: Mutex::new(None),
            projects: Mutex::new(Vec::new()),
            watch: Mutex::new(None),
            root_watch: Mutex::new(None),
            suggested: Mutex::new(HashSet::new()),
            sys: Arc::new(Mutex::new(None)),
            audio: OnceLock::new(),
            mirror: ScreenMirror::new(data_file("screen_mirror.json")),
        }
    }

    pub fn provider(&self, id: &str) -> Result<Arc<dyn AgentProvider>, String> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .cloned()
            .ok_or_else(|| format!("provedor desconhecido: {id}"))
    }
}
