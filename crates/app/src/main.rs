mod state;
mod ui;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use argus_core::agents::claude::ClaudeProvider;
use argus_core::agents::opencode::OpencodeProvider;
use argus_core::agents::orchestrator::{Orchestrator, OrchestratorEvent, TaskState};
use argus_core::agents::provider::{AgentEvent, AgentProvider};
use argus_core::db::{Db, TimeRange};
use argus_core::metrics::Pricing;
use argus_core::autowork::{AutoWork, AutoWorkStatus, STOP_BY_USER};
use argus_core::remote::RemoteServer;
use argus_core::{monitor, pty::PtyManager, workspace};
use state::AppState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use ui::agents::{AgentsView, Providers};
use ui::deps::{DepsPanel, InstallPrompt};
use ui::pet::{use_system_usage, Pet, UsageSnapshot};
use ui::player::MusicPlayer;
use ui::remote::{Remote, RemotePanel};
use ui::sidebar::Sidebar;
use ui::terminal::Terminal;
use ui::workbench::{provide_workbench, Activity, Workbench};

fn main() {
    // No Windows o handler nativo de arrastar arquivos (WebView2) bloqueia o
    // drag-and-drop HTML5 usado para mover abas entre grupos.
    let cfg = Config::new()
        .with_disable_drag_drop_handler(true)
        .with_window(
            WindowBuilder::new()
                .with_title("Argus")
                .with_inner_size(LogicalSize::new(1400.0, 900.0)),
        );
    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(App);
}

/// Banco em %APPDATA%/argus/argus.db, ou no caminho de `ARGUS_DB` (testes).
/// None se não for possível abrir.
fn open_db() -> Option<Db> {
    if let Some(p) = std::env::var_os("ARGUS_DB").map(PathBuf::from).filter(|p| !p.as_os_str().is_empty()) {
        if let Some(parent) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).ok()?;
        }
        return match Db::open(&p) {
            Ok(db) => Some(db),
            Err(e) => {
                eprintln!("banco indisponível ({}): {e}", p.display());
                None
            }
        };
    }
    let base = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::current_dir().ok())?;
    let dir = base.join("argus");
    std::fs::create_dir_all(&dir).ok()?;
    match Db::open(dir.join("argus.db")) {
        Ok(db) => Some(db),
        Err(e) => {
            eprintln!("banco indisponível: {e}");
            None
        }
    }
}

fn describe(ev: &OrchestratorEvent) -> Option<(u64, String)> {
    Some(match ev {
        OrchestratorEvent::StateChanged { task, state } => (*task, format!("[estado] {state:?}")),
        OrchestratorEvent::Stderr { task, line } => (*task, format!("[stderr] {line}")),
        OrchestratorEvent::AutoWork(e) => (e.task, ui::autowork::describe(e)),
        OrchestratorEvent::Agent { task, event } => (
            *task,
            match event {
                AgentEvent::Text(t) => t.clone(),
                AgentEvent::ToolUse { name, input } => format!("[ferramenta] {name} {input}"),
                AgentEvent::Result { ok, summary, cost } => format!(
                    "[resultado {}] {summary}{}",
                    if *ok { "ok" } else { "erro" },
                    cost.map(|c| format!(" (custo ${c:.4})")).unwrap_or_default()
                ),
                AgentEvent::Session(_) => return None,
                AgentEvent::User(u) => format!("> {u}"),
                AgentEvent::Usage(u) => format!("[uso] {} entrada / {} saída", u.input_tokens, u.output_tokens),
                AgentEvent::Raw(r) => r.clone(),
            },
        ),
    })
}

/// Recalcula o status git de um projeto fora da thread da UI.
fn refresh_status(app: AppState, path: PathBuf) {
    let mut statuses = app.statuses;
    spawn(async move {
        let p = path.clone();
        let st = tokio::task::spawn_blocking(move || monitor::status(&p).ok().flatten())
            .await
            .ok()
            .flatten();
        statuses.write().insert(path, st);
    });
}

/// Início do dia atual (UTC) em segundos.
fn today_start() -> i64 {
    let n = argus_core::db::now();
    n - n.rem_euclid(86_400)
}

#[component]
fn App() -> Element {
    use_context_provider(PtyManager::new);
    let providers = use_context_provider(|| {
        Providers(vec![
            Arc::new(ClaudeProvider::new()) as Arc<dyn AgentProvider>,
            Arc::new(OpencodeProvider::new()),
        ])
    });
    let orch = use_context_provider({
        let providers = providers.clone();
        move || Orchestrator::with_db(providers.0.clone(), 2, true, open_db())
    });
    let autowork = use_context_provider({
        let orch = orch.clone();
        move || AutoWork::start(orch)
    });
    use_context_provider({
        let aw = autowork.clone();
        move || {
            let r = RemoteServer::new();
            r.set_autowork(aw);
            Remote(Arc::new(r))
        }
    });
    let mut app = use_context_provider(|| AppState {
        root: Signal::new(None),
        projects: Signal::new(Vec::new()),
        statuses: Signal::new(HashMap::new()),
        selected: Signal::new(None),
        open_file: Signal::new(None),
        terms: Signal::new(Vec::new()),
        active_term: Signal::new(None),
        next_term: Signal::new(0),
        task_log: Signal::new(HashMap::new()),
        task_rev: Signal::new(0),
    });
    let wb = provide_workbench();
    let mut error = use_signal(|| None::<String>);
    let mut watch_handle = use_signal(|| None::<monitor::WatchHandle>);
    let mut last_ok = use_signal(|| None::<Instant>);
    let mut last_err = use_signal(|| None::<Instant>);
    let mut now_tick = use_signal(Instant::now);
    let mut term_open = use_signal(|| true);

    // Eventos do orquestrador -> log por tarefa + humor do pet.
    use_future({
        let orch = orch.clone();
        move || {
            let mut rx = orch.subscribe();
            async move {
                loop {
                    match rx.recv().await {
                        Ok(ev) => {
                            if let OrchestratorEvent::AutoWork(e) = &ev {
                                match &e.status {
                                    AutoWorkStatus::Completed => last_ok.set(Some(Instant::now())),
                                    AutoWorkStatus::Stopped(r) if r != STOP_BY_USER => last_err.set(Some(Instant::now())),
                                    _ => {}
                                }
                            }
                            if let OrchestratorEvent::StateChanged { state, .. } = &ev {
                                match state {
                                    TaskState::Completed => last_ok.set(Some(Instant::now())),
                                    TaskState::Failed(_) => last_err.set(Some(Instant::now())),
                                    _ => {}
                                }
                            }
                            if let Some((id, line)) = describe(&ev) {
                                app.task_log.write().entry(id).or_default().push(line);
                            }
                            *app.task_rev.write() += 1;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            }
        }
    });
    // Relógio para expirar "sucesso/erro recente".
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            now_tick.set(Instant::now());
        }
    });

    // Tokens e custo de hoje (recalculado a cada evento de tarefa).
    let today = use_resource({
        let orch = orch.clone();
        move || {
            let _ = (app.task_rev)();
            let orch = orch.clone();
            async move {
                tokio::task::spawn_blocking(move || {
                    let db = orch.db()?;
                    let rows = db.usage_rows(TimeRange { from: Some(today_start()), to: None }).ok()?;
                    let pricing = Pricing::default_path().map(|p| Pricing::load(&p)).unwrap_or_default();
                    let tokens: u64 = rows
                        .iter()
                        .map(|r| r.usage.input_tokens + r.usage.output_tokens + r.usage.cache_read + r.usage.cache_write)
                        .sum();
                    let cost: f64 = rows.iter().map(|r| pricing.cost(&r.usage)).sum();
                    Some((tokens, cost))
                })
                .await
                .ok()
                .flatten()
            }
        }
    });
    let sys = use_system_usage();

    let mut open_root = move |dir: PathBuf| {
        match workspace::scan(&dir) {
            Ok(list) => {
                error.set(None);
                app.statuses.write().clear();
                let paths: Vec<PathBuf> = list.iter().map(|p| p.path.clone()).collect();
                for p in &paths {
                    refresh_status(app, p.clone());
                }
                app.projects.set(list);
                app.selected.set(None);
                watch_handle.set(None);
                match monitor::watch(paths) {
                    Ok((h, mut rx)) => {
                        watch_handle.set(Some(h));
                        spawn(async move {
                            while let Some(p) = rx.recv().await {
                                refresh_status(app, p);
                            }
                        });
                    }
                    Err(e) => error.set(Some(format!("Monitoramento indisponível: {e}"))),
                }
            }
            Err(e) => error.set(Some(e.to_string())),
        }
        app.root.set(Some(dir));
    };
    // ARGUS_ROOT: abre a pasta raiz automaticamente (útil para testes).
    use_hook(move || {
        if let Some(d) = std::env::var_os("ARGUS_ROOT").map(PathBuf::from).filter(|d| d.is_dir()) {
            open_root(d);
        }
    });
    // Diálogo assíncrono: o síncrono bloqueia a thread da UI (WebView2) e derruba o app no Windows.
    let pick = move |_| {
        spawn(async move {
            if let Some(dir) = rfd::AsyncFileDialog::new().set_title("Pasta raiz").pick_folder().await {
                open_root(dir.path().to_path_buf());
            }
        });
    };

    // Uso real para o pet.
    let _ = (app.task_rev)();
    let now = now_tick();
    let recent = |t: Option<Instant>| t.is_some_and(|t| t > now || now.duration_since(t) < Duration::from_secs(10));
    let (tokens_today, cost_today) = today.read().clone().flatten().unwrap_or((0, 0.0));
    let s = sys();
    let usage = UsageSnapshot {
        running_agents: orch.tasks().iter().filter(|t| t.state.is_active()).count(),
        tokens_today,
        cost_today,
        cpu: s.cpu,
        mem_used_mb: s.mem_used_mb,
        mem_total_mb: s.mem_total_mb,
        recent_success: recent(last_ok()),
        recent_error: recent(last_err()),
    };

    let side = match (wb.activity)() {
        Activity::Explorer => rsx! { Sidebar { on_pick: pick } },
        Activity::Agents => rsx! { AgentsView {} },
        Activity::Deps => {
            let sel = app.selected.read().clone();
            let sel_str = sel.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            let projects = app.projects.read().clone();
            rsx! {
                div { class: "side-pad",
                    select {
                        value: "{sel_str}",
                        onchange: move |e| {
                            let v = e.value();
                            app.selected.set(if v.is_empty() { None } else { Some(PathBuf::from(v)) });
                        },
                        option { value: "", "Escolha um projeto..." }
                        for p in projects {
                            option { key: "{p.path.display()}", value: "{p.path.display()}", "{p.name}" }
                        }
                    }
                    if let Some(p) = sel {
                        DepsPanel { key: "{p.display()}", project: p.clone() }
                    } else {
                        p { class: "muted small", "Selecione um projeto (abra uma pasta raiz no Explorador)." }
                    }
                }
            }
        }
        Activity::Metrics => rsx! {
            div { class: "side-pad",
                button { onclick: move |_| wb.open_metrics(), "Abrir métricas de tokens e custo" }
                button { onclick: move |_| wb.open_dashboard(), "Abrir painel de projetos" }
            }
        },
        Activity::Remote => rsx! { RemotePanel {} },
    };

    let terms = app.terms.read().clone();
    let active_term = (app.active_term)();
    let new_term = move |_| {
        let cwd = app
            .selected
            .read()
            .clone()
            .or_else(|| app.root.read().clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let title = ui::sidebar::dir_name(&cwd);
        app.open_terminal(title, cwd, None);
    };
    // Terminal novo (ex.: login, instalação) reabre o painel.
    use_effect(move || {
        let _ = (app.next_term)();
        term_open.set(true);
    });

    let bottom = rsx! {
        div { class: if term_open() { "bottom" } else { "bottom collapsed" },
            div { class: "tabs",
                button { class: "tab", title: "Mostrar/ocultar terminais", onclick: move |_| term_open.toggle(),
                    if term_open() { "\u{25BE} Terminal" } else { "\u{25B8} Terminal" }
                }
                for t in terms.iter().cloned() {
                    div { key: "{t.id}", class: if active_term == Some(t.id) { "tab active" } else { "tab" },
                        span { onclick: move |_| { app.active_term.set(Some(t.id)); term_open.set(true); }, "{t.title}" }
                        span { class: "close", onclick: move |e| {
                                e.stop_propagation();
                                app.terms.write().retain(|x| x.id != t.id);
                                if (app.active_term)() == Some(t.id) {
                                    let last = app.terms.read().last().map(|x| x.id);
                                    app.active_term.set(last);
                                }
                            }, "×" }
                    }
                }
                button { class: "tab", title: "Novo terminal", onclick: new_term, "+" }
                button { class: "tab", title: "Abrir um terminal como aba do editor",
                    onclick: move |_| {
                        let cwd = app.selected.read().clone()
                            .or_else(|| app.root.read().clone())
                            .or_else(|| std::env::current_dir().ok())
                            .unwrap_or_default();
                        wb.open_terminal(ui::sidebar::dir_name(&cwd), cwd, None);
                    },
                    "\u{21F1}"
                }
            }
            div { class: "terms",
                if terms.is_empty() {
                    div { class: "pad muted small", "Nenhum terminal aberto. Clique em + para abrir um terminal na pasta do projeto." }
                }
                for t in terms {
                    div { key: "{t.id}", class: "pane", style: if active_term == Some(t.id) { "display:block" } else { "display:none" },
                        Terminal { cwd: t.cwd.clone(), command: t.command.clone() }
                    }
                }
            }
        }
    };

    rsx! {
        style { {STYLE} }
        style { {ui::autowork::CSS} }
        div { class: "shell",
            if let Some(e) = error() { div { class: "err banner", "{e}" } }
            div { style: "flex:1;min-height:0;",
                Workbench {
                    side,
                    bottom,
                    status_left: rsx! { MusicPlayer {} },
                    status_right: rsx! { Pet { usage } },
                }
            }
        }
        InstallPrompt {}
    }
}

const STYLE: &str = r#"
:root { color-scheme: dark; }
* { box-sizing: border-box; }
body { margin: 0; font: 13px system-ui, sans-serif; background: #16181d; color: #d6d9e0; overflow: hidden; }
button { background: #2a2e37; color: inherit; border: 1px solid #353a45; border-radius: 5px; padding: 5px 10px; cursor: pointer; font: inherit; }
button:hover { background: #333845; }
button.primary { background: #3b6ef5; border-color: #3b6ef5; color: #fff; }
select, textarea, input { background: #1d2027; color: inherit; border: 1px solid #353a45; border-radius: 5px; padding: 6px; font: inherit; }
.shell { display: flex; flex-direction: column; height: 100vh; }
.row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
.sidebar > * { min-width: 0; max-width: 100%; }
.remote { min-width: 0; max-width: 100%; overflow-wrap: anywhere; }
.remote select { width: 100%; max-width: 100%; min-width: 0; text-overflow: ellipsis; }
.remote .row > * { min-width: 0; max-width: 100%; }
.qr svg { max-width: 100%; width: 220px; height: 220px; box-sizing: border-box; background: #fff; padding: 8px; border-radius: 6px; }
.side-pad { padding: 8px; display: flex; flex-direction: column; gap: 8px; }
.bottom.collapsed { height: auto; min-height: 0; }
.bottom.collapsed .terms { display: none; }
.sidebar { width: 270px; flex: none; background: #1d2027; padding: 12px; overflow-wrap: anywhere; overflow-x: hidden; overflow-y: auto; box-sizing: border-box; border-right: 1px solid #2a2e37; display: flex; flex-direction: column; gap: 6px; }
.sidebar h4 { margin: 10px 0 2px; font-size: 11px; text-transform: uppercase; opacity: .6; }
.projects { list-style: none; padding: 0; margin: 0; }
.projects li { padding: 6px; border-radius: 4px; display: flex; flex-direction: column; cursor: pointer; }
.projects li:hover { background: #262a33; }
.projects li.sel { background: #2b3550; }
.tree .node { padding: 2px 4px; cursor: pointer; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.tree .node:hover { background: #262a33; }
.main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.tabs { display: flex; gap: 2px; background: #1a1d23; border-bottom: 1px solid #2a2e37; padding: 4px 6px 0; flex: none; }
.tab { border-radius: 5px 5px 0 0; border-bottom: 0; background: transparent; display: flex; gap: 6px; align-items: center; padding: 5px 10px; cursor: pointer; }
.tab.active { background: #262a33; }
.tab .close { opacity: .5; } .tab .close:hover { opacity: 1; }
.center { flex: 1; min-height: 0; position: relative; }
.bottom { flex: none; height: 32%; min-height: 120px; display: flex; flex-direction: column; border-top: 1px solid #2a2e37; }
.terms { flex: 1; min-height: 0; position: relative; }
.pane { position: absolute; inset: 0; }
.pad { padding: 16px; } .scroll { overflow: auto; height: 100%; }
.small { font-size: 11px; } .muted { opacity: .6; }
.err { color: #f37272; } .warn { color: #e8b04a; } .ok { color: #5fd38d; }
.banner { padding: 6px 12px; background: #3a1f24; }
table.grid { border-collapse: collapse; width: 100%; }
.grid th, .grid td { text-align: left; padding: 6px 10px; border-bottom: 1px solid #2a2e37; }
.grid tbody tr:hover { background: #20242c; cursor: pointer; }
.agents section { margin-bottom: 20px; }
.providers { display: flex; gap: 10px; margin-top: 8px; flex-wrap: wrap; }
.card { background: #1d2027; border: 1px solid #2a2e37; border-radius: 6px; padding: 10px; display: flex; gap: 10px; align-items: center; }
.task-form { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 8px; max-width: 760px; }
.task-form textarea { grid-column: 1 / -1; min-width: 0; }
.task-form select { min-width: 0; }
.task { border: 1px solid #2a2e37; border-radius: 6px; margin-bottom: 8px; background: #1b1e24; min-width: 0; }
.task-head { padding: 8px 10px; cursor: pointer; overflow-wrap: anywhere; }
.ellipsis { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
.task-actions { display: flex; flex-wrap: wrap; gap: 6px; padding: 0 10px 8px; }
.task-actions button, .row button, .card button { white-space: nowrap; flex: none; }
.card { flex-wrap: wrap; min-width: 0; }
div.log { margin: 0 10px 10px; padding: 8px; background: #111317; border-radius: 4px; max-height: 320px; overflow: auto; }
div.log pre.log-text { margin: 0; white-space: pre-wrap; word-break: break-word; font: 12px Consolas, monospace; }
.wb-notice { color: #7d8290; font-size: 11px; margin: 2px 0; }
.wb-notice summary { cursor: pointer; list-style: none; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; opacity: .75; }
.wb-notice summary::-webkit-details-marker { display: none; }
.wb-notice pre { margin: 4px 0 0; padding: 6px; background: #0d0f12; border-radius: 4px; white-space: pre-wrap; word-break: break-word; font: 11px Consolas, monospace; color: #8b90a0; }
.deps-panel { padding: 10px; min-width: 0; }
.deps-top h3 { margin: 0; flex: 1; }
.deps-list { display: flex; flex-direction: column; gap: 8px; }
.dep-item { border: 1px solid #2a2e37; border-radius: 6px; background: #1b1e24; padding: 8px; display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.dep-head { display: flex; gap: 6px; align-items: center; min-width: 0; }
.dep-head b { flex: 1; min-width: 0; }
.dep-head button { flex: none; white-space: nowrap; padding: 3px 8px; }
.dep-cmd { display: block; background: #111317; padding: 4px 6px; border-radius: 4px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
pre.diff { margin: 0 10px 10px; padding: 8px; background: #111317; border-radius: 4px; max-height: 320px; overflow: auto; white-space: pre-wrap; font: 12px Consolas, monospace; }
"#;
