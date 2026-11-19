//! Builds e servidores de dev com um clique (detecção em `argus_core::runs`).
//!
//! Tudo roda num terminal da IDE: `runs_prepare` valida e devolve o comando, a
//! interface abre uma aba de terminal com ele e o `pty_spawn` vincula o PTY à
//! execução (`on_pty_spawn`). O fim do PTY fecha a execução com o código de saída.
//!
//! Builds digitados num terminal da IDE ou rodados por agentes são reconhecidos
//! (melhor esforço) pelo watcher: a cada 2 s compara a linha de comando dos processos
//! das árvores rastreadas (`procs::tracker().owned()`) com os comandos detectados no
//! projeto. Desses não há código de saída: terminam como `ok` se surgiu um artefato
//! novo, senão `ended`.
//!
//! Eventos: `runs-changed` (`RunInfo[]`) e `run-finished` (`RunInfo`).
use super::{blocking, err};
use crate::events::{RUNS_CHANGED, RUN_FINISHED};
use crate::state::AppCtx;
use argus_core::db::BuildRecord;
use argus_core::procs::{self, Owner};
use argus_core::pty::CommandSpec;
use argus_core::runs::{self, Artifact, ArtifactHint, DetectedCommand, RunKind};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

/// Builds guardados no histórico por projeto.
const KEEP_BUILDS: usize = 30;
/// Execuções encerradas mantidas na memória (as em andamento sempre ficam).
const KEEP_FINISHED: usize = 40;
/// Prazo para o terminal aberto pela interface pegar o comando preparado.
const PENDING_TTL: Duration = Duration::from_secs(60);

/// Uma execução (build ou servidor), em andamento ou encerrada nesta sessão do app.
#[derive(Debug, Clone, Serialize)]
pub struct RunInfo {
    pub id: u64,
    /// Id no histórico (`build_runs`), só para builds.
    pub db_id: Option<i64>,
    pub project: PathBuf,
    pub command_id: Option<String>,
    pub label: String,
    pub cmdline: String,
    pub kind: RunKind,
    /// `ide`, `terminal` ou `agente`.
    pub origin: String,
    /// `running`, `ready` (servidor respondendo), `ok`, `failed`, `stopped` ou `ended`.
    pub state: String,
    pub exit_code: Option<i64>,
    /// Milissegundos Unix.
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub port: Option<u16>,
    pub url: Option<String>,
    /// Terminal da IDE que roda o comando.
    pub pty: Option<u64>,
    pub artifact: Option<Artifact>,
    #[serde(skip)]
    hints: Vec<ArtifactHint>,
    /// Pasta onde procurar o artefato.
    #[serde(skip)]
    root: PathBuf,
    /// Processos reconhecidos (execuções de fora da IDE).
    #[serde(skip)]
    pids: HashSet<u32>,
    #[serde(skip)]
    stop_requested: bool,
    /// Desde quando (ms) a execução está sem nenhum processo vivo. Ela só
    /// termina depois de [`EMPTY_GRACE_MS`]: um elo curto da corrente pode ter
    /// nascido e morrido entre duas varreduras.
    #[serde(skip)]
    empty_since: Option<i64>,
}

/// Tempo sem processos antes de dar uma execução de fora como encerrada.
const EMPTY_GRACE_MS: i64 = 3000;

impl RunInfo {
    fn active(&self) -> bool {
        matches!(self.state.as_str(), "running" | "ready")
    }
}

struct Pending {
    project: PathBuf,
    cmd: DetectedCommand,
    program: String,
    args: Vec<String>,
    port: Option<u16>,
    at: Instant,
}

#[derive(Default)]
struct Registry {
    next: u64,
    runs: Vec<RunInfo>,
    pending: Vec<Pending>,
    rev: u64,
    /// Comandos detectados por projeto (cache curto do watcher).
    detected: HashMap<PathBuf, (Instant, Vec<DetectedCommand>)>,
}

fn reg() -> MutexGuard<'static, Registry> {
    static R: OnceLock<Mutex<Registry>> = OnceLock::new();
    R.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

fn now_ms() -> i64 {
    argus_core::db::now() * 1000 + i64::from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_millis()).unwrap_or(0))
}

fn same_path(a: &Path, b: &Path) -> bool {
    runs::inside(a, b) && runs::inside(b, a)
}

fn project_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

fn record_of(r: &RunInfo) -> BuildRecord {
    BuildRecord {
        id: r.db_id.unwrap_or(0),
        project: r.project.to_string_lossy().into_owned(),
        command_id: r.command_id.clone(),
        label: r.label.clone(),
        cmdline: r.cmdline.clone(),
        origin: r.origin.clone(),
        state: r.state.clone(),
        exit_code: r.exit_code,
        started_at: r.started_at,
        ended_at: r.ended_at,
        artifact_path: r.artifact.as_ref().map(|a| a.path.to_string_lossy().into_owned()),
        artifact_size: r.artifact.as_ref().map(|a| i64::try_from(a.size).unwrap_or(i64::MAX)),
    }
}

fn emit_changed(app: &AppHandle, g: &mut Registry) {
    g.rev += 1;
    let _ = app.emit(RUNS_CHANGED, g.runs.clone());
}

/// Cria a execução (e a linha do histórico, se for build).
fn start_run(app: &AppHandle, g: &mut Registry, project: &Path, cmd: &DetectedCommand, origin: &str, pty: Option<u64>, root: PathBuf, cmdline: String, port: Option<u16>) -> u64 {
    g.next += 1;
    let mut run = RunInfo {
        id: g.next,
        db_id: None,
        project: project.to_path_buf(),
        command_id: Some(cmd.id.clone()),
        label: cmd.label.clone(),
        cmdline,
        kind: cmd.kind,
        origin: origin.into(),
        state: "running".into(),
        exit_code: None,
        started_at: now_ms(),
        ended_at: None,
        port,
        url: None,
        pty,
        artifact: None,
        hints: cmd.artifacts.clone(),
        root,
        pids: HashSet::new(),
        stop_requested: false,
        empty_since: None,
    };
    if run.kind == RunKind::Build {
        if let Some(db) = app.state::<AppCtx>().orch.db() {
            run.db_id = db.insert_build(&record_of(&run)).ok();
        }
    }
    let id = run.id;
    g.runs.push(run);
    // Só as encerradas mais recentes ficam na memória.
    let finished: Vec<u64> = g.runs.iter().filter(|r| !r.active()).map(|r| r.id).collect();
    if finished.len() > KEEP_FINISHED {
        let drop: HashSet<u64> = finished[..finished.len() - KEEP_FINISHED].iter().copied().collect();
        g.runs.retain(|r| !drop.contains(&r.id));
    }
    emit_changed(app, g);
    id
}

fn format_duration(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    if s < 60 {
        format!("{s}s")
    } else {
        format!("{}min{:02}s", s / 60, s % 60)
    }
}

/// Encerra a execução `id`: estado final, artefato, histórico e avisos.
fn finish(app: &AppHandle, g: &mut Registry, id: u64, code: Option<i64>) {
    let Some(run) = g.runs.iter_mut().find(|r| r.id == id && r.active()) else { return };
    run.ended_at = Some(now_ms());
    run.exit_code = code;
    if run.kind == RunKind::Build {
        let since = u64::try_from(run.started_at / 1000).unwrap_or(0).saturating_sub(2);
        run.artifact = runs::find_artifact(&run.root, &run.hints, since);
    }
    run.state = match (run.stop_requested, code) {
        (true, _) => "stopped",
        (_, Some(0)) => "ok",
        (_, Some(_)) => "failed",
        (_, None) if run.artifact.is_some() => "ok",
        (_, None) => "ended",
    }
    .into();
    let run = run.clone();
    if run.kind == RunKind::Build {
        if let (Some(db), Some(_)) = (app.state::<AppCtx>().orch.db(), run.db_id) {
            let _ = db.finish_build(&record_of(&run), KEEP_BUILDS);
        }
        let dur = format_duration(run.ended_at.unwrap_or(run.started_at) - run.started_at);
        let name = project_name(&run.project);
        let text = match run.state.as_str() {
            "ok" => format!("Build concluído em {name}: {} ({dur})", run.label),
            "failed" => format!("Build falhou em {name}: {} (código {})", run.label, code.unwrap_or(-1)),
            "stopped" => format!("Build interrompido em {name}: {}", run.label),
            _ => format!("Build encerrado em {name}: {} ({dur})", run.label),
        };
        app.state::<AppCtx>().remote.notify(serde_json::json!({
            "type": "build",
            "ok": run.state == "ok",
            "state": run.state,
            "project": run.project,
            "label": run.label,
            "text": text,
        }));
    }
    let _ = app.emit(RUN_FINISHED, run);
    emit_changed(app, g);
}

/// Chamado pelo `pty_spawn`: se o comando é um preparado por `runs_prepare` (ou casa
/// com um comando detectado no projeto do `cwd`), vincula o terminal a uma execução.
pub fn on_pty_spawn(app: &AppHandle, pty: u64, spec: &CommandSpec) {
    let mut g = reg();
    g.pending.retain(|p| p.at.elapsed() < PENDING_TTL);
    let cwd = spec.cwd.clone();
    let pos = g.pending.iter().position(|p| {
        p.program == spec.program && p.args == spec.args && cwd.as_deref().is_none_or(|c| same_path(c, &p.project))
    });
    let cmdline = std::iter::once(spec.program.as_str()).chain(spec.args.iter().map(String::as_str)).collect::<Vec<_>>().join(" ");
    if let Some(i) = pos {
        let p = g.pending.remove(i);
        start_run(app, &mut g, &p.project, &p.cmd, "ide", Some(pty), p.project.clone(), cmdline, p.port);
        return;
    }
    // Comando de fora do fluxo dos botões (ex.: outra tela abrindo um terminal com `npm run build`).
    let Some(cwd) = cwd else { return };
    let projects = app.state::<AppCtx>().projects.lock().map(|v| v.clone()).unwrap_or_default();
    let Some(project) = projects.iter().find(|p| runs::inside(&p.path, &cwd)).map(|p| p.path.clone()) else { return };
    let cmds = runs::detect(&project);
    if let Some(cmd) = runs::match_cmdline(&cmdline, &cmds) {
        let cmd = cmd.clone();
        start_run(app, &mut g, &project, &cmd, "ide", Some(pty), cwd, cmdline, cmd.port);
    }
}

/// Chamado quando o PTY termina.
pub fn on_pty_exit(app: &AppHandle, pty: u64, code: Option<u32>) {
    let mut g = reg();
    if let Some(id) = g.runs.iter().find(|r| r.pty == Some(pty) && r.active()).map(|r| r.id) {
        finish(app, &mut g, id, code.map(i64::from));
    }
}

fn detected_cached(g: &mut Registry, project: &Path) -> Vec<DetectedCommand> {
    if let Some((at, v)) = g.detected.get(project) {
        if at.elapsed() < Duration::from_secs(15) {
            return v.clone();
        }
    }
    let v = runs::detect(project);
    g.detected.insert(project.to_path_buf(), (Instant::now(), v.clone()));
    v
}

/// Uma volta do watcher: servidores que começaram a responder e builds de fora da IDE.
fn tick(app: &AppHandle, scan: bool) {
    // Portas: o connect pode demorar, então fora do lock.
    let waiting: Vec<(u64, u16)> = reg()
        .runs
        .iter()
        .filter(|r| r.kind == RunKind::Servidor && r.state == "running")
        .filter_map(|r| r.port.map(|p| (r.id, p)))
        .collect();
    let ready: Vec<(u64, u16)> = waiting.into_iter().filter(|(_, p)| runs::port_open(*p)).collect();
    if !ready.is_empty() {
        let mut g = reg();
        for (id, port) in ready {
            if let Some(r) = g.runs.iter_mut().find(|r| r.id == id && r.state == "running") {
                r.state = "ready".into();
                r.url = Some(format!("http://localhost:{port}"));
            }
        }
        emit_changed(app, &mut g);
    }
    if !scan {
        return;
    }

    let owned = procs::tracker().owned();
    let ctx = app.state::<AppCtx>();
    let projects: Vec<PathBuf> =
        ctx.projects.lock().map(|v| v.iter().filter(|p| !p.missing).map(|p| p.path.clone()).collect()).unwrap_or_default();
    let mut g = reg();
    let alive: HashSet<u32> = owned.iter().map(|p| p.pid).collect();
    // Terminais da IDE que já são de uma execução: os processos deles não contam de novo.
    let run_ptys: HashSet<u64> = g.runs.iter().filter_map(|r| r.pty).collect();

    // Um build é uma corrente de processos: `npm run build` chama o `cargo`, que
    // chama um `rustc` por alvo. Cada elo morre quando o seguinte começa. Antes,
    // a execução era dada como encerrada no primeiro elo que sumia — e o build
    // continuava rodando com a tela dizendo "encerrado" (visto com o APK, que
    // roda um `cargo build` por alvo do Android). Por isso a execução adota os
    // descendentes dos processos dela e só termina quando a corrente inteira some.
    let parent_of: HashMap<u32, Option<u32>> = owned.iter().map(|p| (p.pid, p.parent)).collect();
    /// Primeiro ancestral de `pid` que já pertence a uma execução.
    fn run_of_ancestor(parent_of: &HashMap<u32, Option<u32>>, runs: &[RunInfo], pid: u32) -> Option<u64> {
        let mut cur = parent_of.get(&pid).copied().flatten();
        // Teto baixo de propósito: a corrente de um build é curta e um ciclo
        // (pid reaproveitado apontando para si) não pode prender o laço.
        for _ in 0..16 {
            let p = cur?;
            if let Some(r) = runs.iter().find(|r| r.active() && r.pids.contains(&p)) {
                return Some(r.id);
            }
            cur = parent_of.get(&p).copied().flatten();
        }
        None
    }

    // Descendentes vivos entram na execução do ancestral antes de decidir quem acabou.
    // Processo mais antigo que a execução não é dela (PID reaproveitado).
    let started_ms = |id: u64| g.runs.iter().find(|r| r.id == id).map_or(0, |r| r.started_at);
    let mut adotar: Vec<(u64, u32)> = owned
        .iter()
        .filter(|p| !g.runs.iter().any(|r| r.pids.contains(&p.pid)))
        .filter_map(|p| run_of_ancestor(&parent_of, &g.runs, p.pid).map(|id| (id, p)))
        .filter(|(id, p)| (p.started_at as i64) * 1000 + 1000 >= started_ms(*id))
        .map(|(id, p)| (id, p.pid))
        .collect();
    // Execução que ficou sem processos (um elo intermediário morreu entre duas
    // varreduras e o filho perdeu o ancestral): adota processo novo que roda na
    // pasta do projeto dela e nasceu depois dela.
    for r in g.runs.iter().filter(|r| r.active() && r.pty.is_none() && !r.pids.iter().any(|p| alive.contains(p))) {
        for p in &owned {
            let free = !g.runs.iter().any(|x| x.pids.contains(&p.pid)) && !adotar.iter().any(|(_, q)| *q == p.pid);
            let here = p.cwd.as_deref().is_some_and(|c| c.starts_with(&r.root) || c.starts_with(&r.project));
            if free && here && (p.started_at as i64) * 1000 + 1000 >= r.started_at {
                adotar.push((r.id, p.pid));
            }
        }
    }
    for (id, pid) in adotar {
        if let Some(r) = g.runs.iter_mut().find(|r| r.id == id) {
            r.pids.insert(pid);
        }
    }

    // Execuções de fora: terminam quando todos os processos reconhecidos somem
    // e continuam sumidos por EMPTY_GRACE_MS.
    let now = now_ms();
    let mut gone: Vec<u64> = Vec::new();
    for r in g.runs.iter_mut().filter(|r| r.active() && r.pty.is_none()) {
        if r.pids.iter().any(|p| alive.contains(p)) {
            r.empty_since = None;
        } else {
            let since = *r.empty_since.get_or_insert(now);
            if now - since >= EMPTY_GRACE_MS {
                gone.push(r.id);
            }
        }
    }
    for id in gone {
        finish(app, &mut g, id, None);
    }

    for p in owned {
        if let Owner::Terminal(t) = p.owner {
            if run_ptys.contains(&t) {
                continue;
            }
        }
        if g.runs.iter().any(|r| r.pids.contains(&p.pid)) {
            continue;
        }
        let project = p
            .cwd
            .as_deref()
            .and_then(|c| projects.iter().find(|pr| runs::inside(pr, c)).cloned())
            .or_else(|| match p.owner {
                Owner::Session(task) => ctx
                    .orch
                    .db()
                    .and_then(|db| db.get_task(task as i64).ok().flatten())
                    .map(|t| PathBuf::from(t.project)),
                Owner::Terminal(_) => None,
            });
        let Some(project) = project else { continue };
        let cmds = detected_cached(&mut g, &project);
        let Some(cmd) = runs::match_cmdline(&p.cmdline, &cmds).filter(|c| c.kind == RunKind::Build).cloned() else { continue };
        let existing = g
            .runs
            .iter_mut()
            .find(|r| r.active() && r.pty.is_none() && same_path(&r.project, &project) && r.command_id.as_deref() == Some(&cmd.id));
        if let Some(r) = existing {
            r.pids.insert(p.pid);
            continue;
        }
        let origin = if matches!(p.owner, Owner::Session(_)) { "agente" } else { "terminal" };
        let root = p.cwd.clone().unwrap_or_else(|| project.clone());
        let id = start_run(app, &mut g, &project, &cmd, origin, None, root, p.cmdline.clone(), None);
        if let Some(r) = g.runs.iter_mut().find(|r| r.id == id) {
            r.pids.insert(p.pid);
        }
    }
}

/// Inicia o watcher (thread própria): portas a cada 1 s, processos a cada 2 s.
pub fn start_watcher(app: &AppHandle) {
    if let Some(db) = app.state::<AppCtx>().orch.db() {
        let _ = db.close_stale_builds();
    }
    let app = app.clone();
    let _ = std::thread::Builder::new().name("argus-runs".into()).spawn(move || {
        let mut n: u64 = 0;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            n += 1;
            tick(&app, n % 2 == 0);
        }
    });
}

// ---------------------------------------------------------------- comandos

#[derive(Debug, Clone, Serialize)]
pub struct ProjectCommands {
    pub project: PathBuf,
    pub commands: Vec<DetectedCommand>,
}

/// Comando pronto para abrir no terminal.
#[derive(Debug, Clone, Serialize)]
pub struct PreparedRun {
    pub command: super::pty::CmdSpec,
    /// Título da aba do terminal.
    pub title: String,
    /// Aviso para mostrar (ex.: porta trocada).
    pub notice: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Remembered {
    pub project: PathBuf,
    pub command_id: String,
}

fn setting_key(project: &Path) -> String {
    format!("devserver:{}", project.to_string_lossy())
}

/// Comandos de build e de servidor detectados no projeto.
#[tauri::command]
pub async fn runs_detect(project: PathBuf) -> Result<Vec<DetectedCommand>, String> {
    blocking(move || Ok(runs::detect(&project))).await
}

/// Comandos detectados em todos os projetos da pasta raiz (só os que têm algum).
#[tauri::command]
pub async fn runs_detect_all(ctx: State<'_, AppCtx>) -> Result<Vec<ProjectCommands>, String> {
    let projects: Vec<PathBuf> = ctx.projects.lock().map_err(err)?.iter().filter(|p| !p.missing).map(|p| p.path.clone()).collect();
    blocking(move || {
        Ok(projects
            .into_iter()
            .map(|project| ProjectCommands { commands: runs::detect(&project), project })
            .filter(|p| !p.commands.is_empty())
            .collect())
    })
    .await
}

/// Valida e prepara um comando para rodar num terminal da IDE.
///
/// Servidor: recusa se houver um build do projeto em andamento, se o servidor do
/// projeto já estiver no ar, ou se a porta esperada estiver ocupada e o comando não
/// aceitar outra (aceitando, usa a livre mais próxima). Lembra o comando do projeto.
#[tauri::command]
pub async fn runs_prepare(ctx: State<'_, AppCtx>, project: PathBuf, command_id: String) -> Result<PreparedRun, String> {
    let cmds = {
        let p = project.clone();
        blocking(move || Ok(runs::detect(&p))).await?
    };
    let name = project_name(&project);
    let cmd = cmds
        .into_iter()
        .find(|c| c.id == command_id)
        .ok_or_else(|| format!("O comando {command_id} não foi encontrado em {name}."))?;
    let mut notice = None;
    {
        let g = reg();
        let in_project = |r: &&RunInfo| r.active() && same_path(&r.project, &project);
        if let Some(r) = g.runs.iter().filter(in_project).find(|r| r.command_id.as_deref() == Some(&cmd.id)) {
            return Err(format!("{} já está rodando em {name} (desde o terminal {}).", r.label, r.pty.map(|p| p.to_string()).unwrap_or_else(|| "externo".into())));
        }
        match cmd.kind {
            RunKind::Servidor => {
                if let Some(b) = g.runs.iter().filter(in_project).find(|r| r.kind == RunKind::Build) {
                    return Err(format!("Há um build de {name} em andamento ({}). Espere terminar para iniciar o servidor.", b.label));
                }
                if let Some(s) = g.runs.iter().filter(in_project).find(|r| r.kind == RunKind::Servidor) {
                    return Err(format!("O servidor de {name} já está rodando ({}).", s.label));
                }
            }
            RunKind::Build => {
                if g.runs.iter().filter(in_project).any(|r| r.kind == RunKind::Servidor) {
                    notice = Some(format!("O servidor de dev de {name} está rodando; o build pode disputar arquivos com ele."));
                }
            }
        }
    }
    let (mut args, mut env, mut port) = (cmd.args.clone(), Vec::new(), cmd.port);
    if let (RunKind::Servidor, Some(p)) = (cmd.kind, cmd.port) {
        let c = cmd.clone();
        let (a, e, chosen, msg) = blocking(move || {
            if !runs::port_busy(p) {
                return Ok((c.args.clone(), Vec::new(), p, None));
            }
            if c.port_style.is_none() {
                return Err(format!(
                    "A porta {p} já está em uso e {} não aceita outra porta. Pare o que está nela (painel Servidores) e tente de novo.",
                    c.label
                ));
            }
            let free = runs::nearest_free(p.saturating_add(1)).ok_or_else(|| format!("Nenhuma porta livre perto da {p}."))?;
            let (a, e) = c.with_port(free).ok_or_else(|| "O comando não aceita porta.".to_string())?;
            Ok((a, e, free, Some(format!("A porta {p} está ocupada; usando a {free}."))))
        })
        .await?;
        args = a;
        env = e;
        port = Some(chosen);
        notice = msg.or(notice);
    }
    if cmd.kind == RunKind::Servidor {
        if let Some(db) = ctx.orch.db() {
            let _ = db.set_setting(&setting_key(&project), Some(&cmd.id));
        }
    }
    let mut g = reg();
    g.pending.push(Pending { project: project.clone(), cmd: cmd.clone(), program: cmd.program.clone(), args: args.clone(), port, at: Instant::now() });
    Ok(PreparedRun {
        command: super::pty::CmdSpec { program: cmd.program.clone(), args, cwd: Some(project), env },
        title: format!("{name}: {}", cmd.label),
        notice,
        port,
    })
}

/// Execuções em andamento e as encerradas nesta sessão do app.
#[tauri::command]
pub async fn runs_list() -> Result<Vec<RunInfo>, String> {
    Ok(reg().runs.clone())
}

/// Histórico de builds (do projeto, ou de todos), mais recentes primeiro.
#[tauri::command]
pub async fn runs_history(ctx: State<'_, AppCtx>, project: Option<PathBuf>, limit: Option<usize>) -> Result<Vec<BuildRecord>, String> {
    let Some(db) = ctx.orch.db() else { return Ok(Vec::new()) };
    let p = project.map(|p| p.to_string_lossy().into_owned());
    db.list_builds(p.as_deref(), limit.unwrap_or(KEEP_BUILDS)).map_err(err)
}

/// Para uma execução: derruba a árvore de processos e fecha o PTY.
#[tauri::command]
pub async fn runs_stop(app: AppHandle, ctx: State<'_, AppCtx>, id: u64) -> Result<(), String> {
    let (pty, pids) = {
        let mut g = reg();
        let r = g.runs.iter_mut().find(|r| r.id == id && r.active()).ok_or("Essa execução já terminou.")?;
        r.stop_requested = true;
        (r.pty, r.pids.iter().copied().collect::<Vec<_>>())
    };
    let pids2 = pids.clone();
    blocking(move || {
        if let Some(root) = pty.and_then(|t| procs::tracker().root_of(Owner::Terminal(t))) {
            let _ = procs::kill_tree(root);
        }
        for p in pids2 {
            let _ = procs::kill_tree(p);
        }
        Ok(())
    })
    .await?;
    if let Some(t) = pty {
        let _ = ctx.pty.kill(t);
    }
    // Sem PTY (processo de fora) ou PTY que já tinha saído: fecha aqui mesmo.
    let mut g = reg();
    if g.runs.iter().any(|r| r.id == id && r.active()) && (pty.is_none() || !ctx.pty.list().iter().any(|s| Some(s.id) == pty)) {
        finish(&app, &mut g, id, None);
    }
    Ok(())
}

/// Último servidor iniciado em cada projeto da pasta raiz ("Iniciar todos").
#[tauri::command]
pub async fn runs_remembered(ctx: State<'_, AppCtx>) -> Result<Vec<Remembered>, String> {
    let Some(db) = ctx.orch.db() else { return Ok(Vec::new()) };
    let projects: Vec<PathBuf> = ctx.projects.lock().map_err(err)?.iter().filter(|p| !p.missing).map(|p| p.path.clone()).collect();
    Ok(projects
        .into_iter()
        .filter_map(|project| {
            let command_id = db.get_setting(&setting_key(&project)).ok().flatten()?;
            Some(Remembered { project, command_id })
        })
        .collect())
}

/// Abre a pasta (ou a pasta do arquivo, selecionando-o) no gerenciador de arquivos.
#[tauri::command]
pub async fn runs_open_folder(path: PathBuf) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("{} não existe mais.", path.display()));
    }
    blocking(move || {
        #[cfg(windows)]
        let r = {
            let mut c = std::process::Command::new("explorer");
            if path.is_file() {
                c.arg(format!("/select,{}", path.display()));
            } else {
                c.arg(&path);
            }
            c.spawn()
        };
        #[cfg(target_os = "macos")]
        let r = std::process::Command::new("open").arg(if path.is_file() { path.parent().unwrap_or(&path) } else { &path }).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let r = std::process::Command::new("xdg-open").arg(if path.is_file() { path.parent().unwrap_or(&path) } else { &path }).spawn();
        r.map(|_| ()).map_err(|e| format!("Não foi possível abrir a pasta: {e}"))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_and_paths() {
        assert_eq!(format_duration(4_200), "4s");
        assert_eq!(format_duration(125_000), "2min05s");
        assert!(same_path(Path::new(r"C:\a\b"), Path::new("c:/a/b/")));
        assert!(!same_path(Path::new(r"C:\a\b"), Path::new(r"C:\a\b\c")));
        assert_eq!(setting_key(Path::new("/p")), "devserver:/p");
    }

    #[test]
    fn run_info_serializes_for_the_ui() {
        let r = RunInfo {
            id: 1,
            db_id: Some(3),
            project: "/p".into(),
            command_id: Some("npm:dev".into()),
            label: "npm run dev".into(),
            cmdline: "npm run dev".into(),
            kind: RunKind::Servidor,
            origin: "ide".into(),
            state: "ready".into(),
            exit_code: None,
            started_at: 1,
            ended_at: None,
            port: Some(5173),
            url: Some("http://localhost:5173".into()),
            pty: Some(2),
            artifact: None,
            hints: Vec::new(),
            root: "/p".into(),
            pids: HashSet::new(),
            stop_requested: false,
            empty_since: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["kind"], "servidor");
        assert_eq!(v["port"], 5173);
        assert!(v.get("pids").is_none() && v.get("hints").is_none());
    }
}
