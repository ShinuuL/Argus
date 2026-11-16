//! Servidores vinculados a sessões/terminais.
//!
//! Rastreia a árvore de processos de cada dono (turno do agente ou terminal do PTY),
//! detecta portas TCP em LISTEN desses processos e permite derrubá-los.
//!
//! - Árvore: poll (~2 s) com `sysinfo`, descendentes por parent pid. PIDs vistos
//!   enquanto o pai vivia ficam guardados: no Windows um filho órfão mantém o parent
//!   pid antigo (que pode até ser reutilizado), então só a memória garante o vínculo.
//! - Portas: Windows via `netstat -ano` (parse independente de idioma); unix via
//!   `lsof -nP -iTCP -sTCP:LISTEN -Fpn`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Dono de uma árvore de processos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id")]
pub enum Owner {
    /// Sessão do agente (id da tarefa).
    Session(u64),
    /// Terminal do PTY (id da sessão de terminal).
    Terminal(u64),
}

impl Owner {
    pub fn label(&self) -> String {
        match self {
            Owner::Session(id) => format!("Sessão #{id}"),
            Owner::Terminal(id) => format!("Terminal {id}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerInfo {
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
    pub port: u16,
    pub url: String,
    pub owner: Owner,
    /// Segundos desde a época Unix.
    pub started_at: u64,
    /// Diretório de trabalho (para reiniciar), se conhecido.
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}

/// Uma linha da tabela de processos (independente do `sysinfo`, para testes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcRow {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub cmdline: String,
    pub started_at: u64,
    pub cwd: Option<PathBuf>,
}

/// Fonte de dados do sistema (real ou falsa nos testes).
pub trait ProcSource: Send + Sync {
    fn processes(&self) -> Vec<ProcRow>;
    /// (pid, porta) em LISTEN.
    fn listening(&self) -> Vec<(u32, u16)>;
}

// ---------------------------------------------------------------- parsers

/// Parse de `netstat -ano -p tcp` / `-p tcpv6` (Windows). Não depende do idioma:
/// linha em escuta = 5 colunas e endereço remoto terminando em `:0`.
pub fn parse_netstat(out: &str) -> Vec<(u32, u16)> {
    let mut v = Vec::new();
    for l in out.lines() {
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.len() != 5 || !c[0].eq_ignore_ascii_case("TCP") || !c[2].ends_with(":0") {
            continue;
        }
        let port = c[1].rsplit(':').next().and_then(|p| p.parse::<u16>().ok());
        let pid = c[4].parse::<u32>().ok();
        if let (Some(port), Some(pid)) = (port, pid) {
            if pid != 0 && !v.contains(&(pid, port)) {
                v.push((pid, port));
            }
        }
    }
    v
}

/// Parse de `lsof -nP -iTCP -sTCP:LISTEN -Fpn` (unix).
pub fn parse_lsof(out: &str) -> Vec<(u32, u16)> {
    let mut v = Vec::new();
    let mut pid = None;
    for l in out.lines() {
        if let Some(p) = l.strip_prefix('p') {
            pid = p.parse::<u32>().ok();
        } else if let (Some(n), Some(pid)) = (l.strip_prefix('n'), pid) {
            if let Some(port) = n.rsplit(':').next().and_then(|p| p.parse::<u16>().ok()) {
                if !v.contains(&(pid, port)) {
                    v.push((pid, port));
                }
            }
        }
    }
    v
}

/// PIDs descendentes de `root` (inclui o próprio, se existir).
pub fn descendants(root: u32, rows: &[ProcRow]) -> HashSet<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for r in rows {
        if let Some(p) = r.parent {
            if p != r.pid {
                children.entry(p).or_default().push(r.pid);
            }
        }
    }
    let mut out = HashSet::new();
    let mut stack = vec![root];
    while let Some(p) = stack.pop() {
        if !out.insert(p) {
            continue;
        }
        if let Some(cs) = children.get(&p) {
            stack.extend(cs.iter().copied());
        }
    }
    out
}

/// Caminho normalizado para comparação: sem `\\?\`, barras `/`, minúsculas,
/// sem barra final.
fn norm_path(s: &str) -> String {
    let s = s.strip_prefix(r"\\?\").unwrap_or(s);
    s.replace('\\', "/").to_lowercase().trim_end_matches('/').to_string()
}

/// `path` é `dir` ou está dentro dele (ambos normalizados).
fn inside(path: &str, dir: &str) -> bool {
    path == dir || path.strip_prefix(dir).is_some_and(|r| r.starts_with('/'))
}

/// A linha de comando (normalizada) cita `dir` ou algo dentro dele.
fn cmdline_mentions(cmd: &str, dir: &str) -> bool {
    let mut from = 0;
    while let Some(i) = cmd[from..].find(dir) {
        let end = from + i + dir.len();
        match cmd[end..].chars().next() {
            None | Some('/') | Some('"') | Some('\'') | Some(' ') | Some('\t') => return true,
            _ => from = end,
        }
    }
    false
}

/// Processos que prendem o worktree `dir` e podem ser encerrados:
/// - PIDs já vinculados ao dono (`owned`, a árvore do processo da tarefa), exceto
///   os que sabidamente rodam fora de `dir` (cwd conhecido e fora dele);
/// - qualquer processo cujo cwd está dentro de `dir`;
/// - qualquer processo cuja linha de comando cita `dir` (ou caminho dentro dele).
///
/// `exclude` (o próprio Argus) nunca entra. Caminho curto demais (raiz) não casa nada.
pub fn worktree_pids(rows: &[ProcRow], owned: &HashSet<u32>, dir: &Path, exclude: u32) -> Vec<u32> {
    let d = norm_path(&dir.to_string_lossy());
    let depth = d.split('/').filter(|c| !c.is_empty()).count();
    if depth < 2 {
        return Vec::new();
    }
    let mut out: Vec<u32> = rows
        .iter()
        .filter(|r| r.pid != exclude && r.pid != 0)
        .filter(|r| {
            let cwd = r.cwd.as_ref().map(|c| norm_path(&c.to_string_lossy())).filter(|c| !c.is_empty());
            let cwd_in = cwd.as_deref().is_some_and(|c| inside(c, &d));
            let by_owner = owned.contains(&r.pid) && cwd.as_deref().map_or(true, |c| inside(c, &d));
            cwd_in || by_owner || cmdline_mentions(&norm_path(&r.cmdline), &d)
        })
        .map(|r| r.pid)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

// ---------------------------------------------------------------- fonte real

/// Fonte real: `sysinfo` + netstat/lsof.
pub struct SystemSource {
    sys: Mutex<Option<sysinfo::System>>,
}

impl Default for SystemSource {
    fn default() -> Self {
        Self { sys: Mutex::new(sysinfo::System::new().ok()) }
    }
}

impl ProcSource for SystemSource {
    fn processes(&self) -> Vec<ProcRow> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
        let mut g = self.sys.lock().unwrap_or_else(|e| e.into_inner());
        let Some(sys) = g.as_mut() else { return Vec::new() };
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet).with_cwd(UpdateKind::OnlyIfNotSet),
        );
        sys.processes()
            .values()
            .map(|p| ProcRow {
                pid: p.pid().as_u32(),
                parent: p.parent().map(|x| x.as_u32()),
                name: p.name().to_string_lossy().into_owned(),
                cmdline: p.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" "),
                started_at: p.start_time(),
                cwd: p.cwd().map(Path::to_path_buf),
            })
            .collect()
    }

    fn listening(&self) -> Vec<(u32, u16)> {
        #[cfg(windows)]
        {
            // Fallback sem a crate `windows` no core (GetExtendedTcpTable): netstat.
            let mut v = Vec::new();
            for proto in ["tcp", "tcpv6"] {
                if let Some((_, out)) = crate::agents::provider::run_capture(Path::new("netstat"), &["-ano", "-p", proto]) {
                    for x in parse_netstat(&out) {
                        if !v.contains(&x) {
                            v.push(x);
                        }
                    }
                }
            }
            v
        }
        #[cfg(not(windows))]
        {
            crate::agents::provider::run_capture(Path::new("lsof"), &["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpn"])
                .map(|(_, o)| parse_lsof(&o))
                .unwrap_or_default()
        }
    }
}

// ---------------------------------------------------------------- rastreador

#[derive(Default)]
struct Inner {
    /// Raízes registradas (pid -> dono).
    roots: HashMap<u32, Owner>,
    /// Todo PID já visto na árvore de um dono (com o start_time, contra reuso de PID).
    known: HashMap<u32, (Owner, u64)>,
    servers: Vec<ServerInfo>,
    rev: u64,
}

/// Rastreador de processos/servidores. Clonável (compartilhado).
#[derive(Clone)]
pub struct ProcTracker {
    inner: Arc<Mutex<Inner>>,
    src: Arc<dyn ProcSource>,
}

impl ProcTracker {
    pub fn new(src: Arc<dyn ProcSource>) -> Self {
        Self { inner: Arc::new(Mutex::new(Inner::default())), src }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Vincula a árvore de `pid` a `owner`.
    pub fn track(&self, pid: u32, owner: Owner) {
        self.lock().roots.insert(pid, owner);
    }

    /// Contador que muda a cada alteração da lista de servidores.
    pub fn rev(&self) -> u64 {
        self.lock().rev
    }

    /// Atualiza a árvore e os servidores. Devolve a lista nova.
    pub fn poll(&self) -> Vec<ServerInfo> {
        let rows = self.src.processes();
        let by_pid: HashMap<u32, &ProcRow> = rows.iter().map(|r| (r.pid, r)).collect();
        let mut g = self.lock();
        // Raízes mortas saem; os PIDs conhecidos continuam valendo.
        g.roots.retain(|pid, _| by_pid.contains_key(pid));
        let roots: Vec<(u32, Owner)> = g.roots.iter().map(|(p, o)| (*p, *o)).collect();
        for (root, owner) in roots {
            for pid in descendants(root, &rows) {
                if let Some(r) = by_pid.get(&pid) {
                    g.known.entry(pid).or_insert((owner, r.started_at));
                }
            }
        }
        // Esquece PIDs mortos ou reutilizados (start_time diferente).
        g.known.retain(|pid, (_, st)| by_pid.get(pid).is_some_and(|r| r.started_at == *st));
        drop(g);

        let listen = if self.lock().known.is_empty() { Vec::new() } else { self.src.listening() };
        let mut g = self.lock();
        let mut servers: Vec<ServerInfo> = listen
            .into_iter()
            .filter_map(|(pid, port)| {
                let (owner, _) = *g.known.get(&pid)?;
                let r = by_pid.get(&pid)?;
                Some(ServerInfo {
                    pid,
                    name: r.name.clone(),
                    cmdline: r.cmdline.clone(),
                    port,
                    url: format!("http://localhost:{port}"),
                    owner,
                    started_at: r.started_at,
                    cwd: r.cwd.clone(),
                })
            })
            .collect();
        servers.sort_by_key(|s| (s.port, s.pid));
        if servers != g.servers {
            g.servers = servers.clone();
            g.rev += 1;
        }
        servers
    }

    pub fn servers(&self) -> Vec<ServerInfo> {
        self.lock().servers.clone()
    }

    pub fn servers_of(&self, owner: Owner) -> Vec<ServerInfo> {
        self.lock().servers.iter().filter(|s| s.owner == owner).cloned().collect()
    }

    /// Dono de um PID conhecido.
    pub fn owner_of(&self, pid: u32) -> Option<Owner> {
        self.lock().known.get(&pid).map(|(o, _)| *o)
    }

    /// Mata a árvore de `pid` (só PIDs rastreados, por segurança).
    pub fn kill(&self, pid: u32) -> anyhow::Result<()> {
        if self.owner_of(pid).is_none() {
            anyhow::bail!("processo {pid} não pertence a nenhuma sessão");
        }
        kill_tree(pid)?;
        let mut g = self.lock();
        g.servers.retain(|s| s.pid != pid);
        g.rev += 1;
        Ok(())
    }

    /// Derruba todos os servidores de `owner`. Devolve quantos.
    pub fn stop_all(&self, owner: Owner) -> usize {
        let pids: HashSet<u32> = self.servers_of(owner).into_iter().map(|s| s.pid).collect();
        pids.iter().filter(|p| self.kill(**p).is_ok()).count()
    }

    /// Encerra os processos que prendem o worktree `dir` da sessão `owner`
    /// (critério em `worktree_pids`). Devolve os PIDs encerrados.
    pub fn kill_in_dir(&self, owner: Owner, dir: &Path) -> Vec<u32> {
        self.poll();
        let rows = self.src.processes();
        let owned: HashSet<u32> =
            self.lock().known.iter().filter(|(_, (o, _))| *o == owner).map(|(p, _)| *p).collect();
        let pids = worktree_pids(&rows, &owned, dir, std::process::id());
        // Um `taskkill /T` no pai já leva os filhos: o erro dos seguintes é esperado.
        let killed: Vec<u32> = pids.into_iter().filter(|p| kill_tree(*p).is_ok()).collect();
        if !killed.is_empty() {
            let mut g = self.lock();
            g.servers.retain(|s| !killed.contains(&s.pid));
            g.rev += 1;
        }
        killed
    }

    /// Esquece um dono (ex.: terminal fechado).
    pub fn forget(&self, owner: Owner) {
        let mut g = self.lock();
        g.roots.retain(|_, o| *o != owner);
        g.known.retain(|_, (o, _)| *o != owner);
        g.servers.retain(|s| s.owner != owner);
        g.rev += 1;
    }

    /// Inicia o poll em segundo plano (thread própria, a cada `every`).
    pub fn start_polling(&self, every: Duration) {
        let me = self.clone();
        let _ = std::thread::Builder::new().name("procs-poll".into()).spawn(move || loop {
            let has_work = {
                let g = me.lock();
                !g.roots.is_empty() || !g.known.is_empty() || !g.servers.is_empty()
            };
            if has_work {
                me.poll();
            }
            std::thread::sleep(every);
        });
    }
}

/// Rastreador global (fonte real, poll de 2 s iniciado no primeiro uso).
pub fn tracker() -> &'static ProcTracker {
    static T: OnceLock<ProcTracker> = OnceLock::new();
    T.get_or_init(|| {
        let t = ProcTracker::new(Arc::new(SystemSource::default()));
        t.start_polling(Duration::from_secs(2));
        t
    })
}

/// Mata `pid` e todos os descendentes.
pub fn kill_tree(pid: u32) -> anyhow::Result<()> {
    #[cfg(windows)]
    let r = crate::agents::provider::run_capture(Path::new("taskkill"), &["/PID", &pid.to_string(), "/T", "/F"]);
    #[cfg(not(windows))]
    let r = {
        // Filhos primeiro (pkill -P), depois o próprio.
        let _ = crate::agents::provider::run_capture(Path::new("pkill"), &["-KILL", "-P", &pid.to_string()]);
        crate::agents::provider::run_capture(Path::new("kill"), &["-KILL", &pid.to_string()])
    };
    match r {
        Some((true, _)) => Ok(()),
        Some((false, out)) => anyhow::bail!("falha ao encerrar {pid}: {}", out.trim()),
        None => anyhow::bail!("falha ao encerrar {pid}"),
    }
}

/// Reinicia um servidor: mata a árvore e roda a mesma linha de comando no mesmo cwd,
/// vinculada ao mesmo dono. Só funciona quando o comando foi capturado.
pub fn restart(s: &ServerInfo) -> anyhow::Result<u32> {
    let parts = split_cmdline(&s.cmdline);
    let (prog, args) = parts.split_first().ok_or_else(|| anyhow::anyhow!("linha de comando desconhecida"))?;
    let _ = kill_tree(s.pid);
    let mut cmd = std::process::Command::new(prog);
    cmd.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    if let Some(cwd) = &s.cwd {
        cmd.current_dir(cwd);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let child = cmd.spawn()?;
    let pid = child.id();
    tracker().track(pid, s.owner);
    Ok(pid)
}

/// Divide uma linha de comando respeitando aspas duplas.
pub fn split_cmdline(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut q, mut any) = (Vec::new(), String::new(), false, false);
    for c in s.chars() {
        match c {
            '"' => {
                q = !q;
                any = true;
            }
            c if c.is_whitespace() && !q => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

// ---------------------------------------------------------------- comandos permitidos

/// "Comandos permitidos ao agente" por projeto. Com `acceptEdits` o Bash do claude é
/// bloqueado; cada entrada vira `--allowedTools "Bash(<padrão>)"`. Vazio por padrão.
fn allowed_store() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static S: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(load_allowed().unwrap_or_default()))
}

fn allowed_file() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ARGUS_ALLOWED_TOOLS") {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join(crate::paths::dir_name()).join("allowed_tools.json"))
}

fn load_allowed() -> Option<HashMap<String, Vec<String>>> {
    serde_json::from_str(&std::fs::read_to_string(allowed_file()?).ok()?).ok()
}

fn key(project: &Path) -> String {
    project.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_ascii_lowercase()
}

pub fn allowed_commands(project: &Path) -> Vec<String> {
    allowed_store().lock().unwrap_or_else(|e| e.into_inner()).get(&key(project)).cloned().unwrap_or_default()
}

pub fn set_allowed_commands(project: &Path, cmds: Vec<String>) -> anyhow::Result<()> {
    let cmds: Vec<String> = cmds.into_iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    let snapshot = {
        let mut m = allowed_store().lock().unwrap_or_else(|e| e.into_inner());
        if cmds.is_empty() {
            m.remove(&key(project));
        } else {
            m.insert(key(project), cmds);
        }
        m.clone()
    };
    if let Some(f) = allowed_file() {
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(f, serde_json::to_string_pretty(&snapshot)?)?;
    }
    Ok(())
}

/// Argumentos `--allowedTools` para a lista dada. Entradas já no formato
/// `Ferramenta(...)` passam direto; o resto vira `Bash(<cmd>)`.
pub fn allowed_tools_args(cmds: &[String]) -> Vec<String> {
    let tools: Vec<String> = cmds
        .iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .map(|c| if c.ends_with(')') && c.contains('(') { c.to_string() } else { format!("Bash({c})") })
        .collect();
    if tools.is_empty() {
        return Vec::new();
    }
    let mut v = vec!["--allowedTools".to_string()];
    v.extend(tools);
    v
}

/// Acrescenta os comandos permitidos do projeto a um comando do `claude`.
pub fn apply_allowed_tools(mut spec: crate::agents::provider::CommandSpec, project: &Path) -> crate::agents::provider::CommandSpec {
    let is_claude = Path::new(&spec.program)
        .file_stem()
        .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("claude"));
    if is_claude {
        spec.args.extend(allowed_tools_args(&allowed_commands(project)));
    }
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETSTAT: &str = "
Conexões ativas

  Proto  Endereço local         Endereço externo       Estado          PID
  TCP    0.0.0.0:135            0.0.0.0:0              ESCUTANDO       1100
  TCP    0.0.0.0:5173           0.0.0.0:0              LISTENING       4242
  TCP    127.0.0.1:5173         127.0.0.1:50000        ESTABLISHED     4242
  TCP    [::]:3000              [::]:0                 LISTENING       777
  TCP    [::1]:3000             [::]:0                 LISTENING       777
  TCP    0.0.0.0:445            0.0.0.0:0              LISTENING       0
";

    #[test]
    fn netstat_fixture() {
        assert_eq!(parse_netstat(NETSTAT), vec![(1100, 135), (4242, 5173), (777, 3000)]);
    }

    #[test]
    fn lsof_fixture() {
        let out = "p321\nf12\nn*:8080\np99\nf3\nn127.0.0.1:5432\nn[::1]:5432\n";
        assert_eq!(parse_lsof(out), vec![(321, 8080), (99, 5432)]);
    }

    fn row(pid: u32, parent: Option<u32>, st: u64) -> ProcRow {
        ProcRow { pid, parent, name: format!("p{pid}"), cmdline: format!("cmd {pid}"), started_at: st, cwd: None }
    }

    struct Fake {
        rows: Mutex<Vec<ProcRow>>,
        listen: Mutex<Vec<(u32, u16)>>,
    }
    impl ProcSource for Fake {
        fn processes(&self) -> Vec<ProcRow> {
            self.rows.lock().unwrap().clone()
        }
        fn listening(&self) -> Vec<(u32, u16)> {
            self.listen.lock().unwrap().clone()
        }
    }

    #[test]
    fn tree_descendants() {
        let rows = vec![row(1, None, 0), row(10, Some(1), 0), row(11, Some(10), 0), row(12, Some(11), 0), row(20, Some(2), 0)];
        let d = descendants(10, &rows);
        assert_eq!(d, [10, 11, 12].into_iter().collect());
    }

    #[test]
    fn owner_binding_survives_orphans() {
        let fake = Arc::new(Fake {
            rows: Mutex::new(vec![row(100, None, 1), row(101, Some(100), 2), row(102, Some(101), 3), row(200, None, 4), row(201, Some(200), 5)]),
            listen: Mutex::new(vec![(102, 5173), (201, 3000), (999, 80)]),
        });
        let t = ProcTracker::new(fake.clone());
        t.track(100, Owner::Session(7));
        t.track(200, Owner::Terminal(1));
        let s = t.poll();
        assert_eq!(s.len(), 2);
        assert_eq!(t.servers_of(Owner::Session(7))[0].port, 5173);
        assert_eq!(t.servers_of(Owner::Session(7))[0].url, "http://localhost:5173");
        assert_eq!(t.servers_of(Owner::Terminal(1))[0].pid, 201);

        // O claude encerra o turno: 100 e 101 morrem, 102 fica órfão (parent antigo).
        fake.rows.lock().unwrap().retain(|r| r.pid != 100 && r.pid != 101);
        t.poll();
        assert_eq!(t.owner_of(102), Some(Owner::Session(7)));
        assert_eq!(t.servers_of(Owner::Session(7)).len(), 1);

        // PID reutilizado por outro processo (start_time diferente): perde o vínculo.
        fake.rows.lock().unwrap().retain(|r| r.pid != 102);
        fake.rows.lock().unwrap().push(row(102, None, 99));
        t.poll();
        assert_eq!(t.owner_of(102), None);
        assert!(t.servers_of(Owner::Session(7)).is_empty());
    }

    #[test]
    fn kill_refuses_untracked() {
        let t = ProcTracker::new(Arc::new(Fake { rows: Mutex::new(vec![]), listen: Mutex::new(vec![]) }));
        assert!(t.kill(4).is_err());
    }

    #[test]
    fn worktree_pids_selects_only_worktree_processes() {
        let wt = Path::new(r"C:\Proj\.argus-worktrees\app-7");
        let mk = |pid: u32, cmd: &str, cwd: Option<&str>| ProcRow {
            pid,
            parent: None,
            name: format!("p{pid}"),
            cmdline: cmd.into(),
            started_at: 0,
            cwd: cwd.map(PathBuf::from),
        };
        let rows = vec![
            mk(1, "opencode serve", Some(r"C:\Proj\.argus-worktrees\app-7")), // cwd = worktree
            mk(2, "node x.js", Some(r"c:\proj\.argus-worktrees\APP-7\src\")), // cwd dentro (outra caixa)
            mk(3, r#"node "C:\Proj\.argus-worktrees\app-7\srv.js""#, Some(r"C:\")), // linha de comando cita
            mk(4, "node y.js", Some(r"C:\Proj\.argus-worktrees\app-70")), // prefixo parecido: fora
            mk(5, r"code C:\Proj\.argus-worktrees\app-70\a", None), // idem na linha de comando
            mk(6, "filho", None), // da sessão, cwd desconhecido
            mk(7, "filho", Some(r"C:\Outro")), // da sessão, mas roda fora
            mk(8, "explorer", Some(r"C:\Users")), // alheio
            mk(9, "argus", Some(r"C:\Proj\.argus-worktrees\app-7")), // o próprio Argus
            mk(10, r"\\?\C:\Proj\.argus-worktrees\app-7\bin\x.exe", None), // prefixo \\?\
        ];
        let owned: HashSet<u32> = [6, 7].into_iter().collect();
        assert_eq!(worktree_pids(&rows, &owned, wt, 9), vec![1, 2, 3, 6, 10]);
        // Raiz de disco nunca casa nada.
        assert!(worktree_pids(&rows, &owned, Path::new(r"C:\"), 9).is_empty());
    }

    #[test]
    fn allowed_tools() {
        assert!(allowed_tools_args(&[]).is_empty());
        let a = allowed_tools_args(&["npm run dev:*".into(), " ".into(), "Bash(cargo test:*)".into()]);
        assert_eq!(a, vec!["--allowedTools", "Bash(npm run dev:*)", "Bash(cargo test:*)"]);
    }

    #[test]
    fn split_cmd() {
        assert_eq!(split_cmdline(r#""C:\Program Files\node.exe" vite --port 5173"#), vec![r"C:\Program Files\node.exe", "vite", "--port", "5173"]);
    }
}
