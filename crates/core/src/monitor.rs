//! Monitoramento de projetos: status git (via git2) e watcher de arquivos (via notify).

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use git2::{Repository, Status, StatusOptions};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

/// Estado de um arquivo alterado no working tree/index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    New,
    Modified,
    Deleted,
    Renamed,
    TypeChange,
    Conflicted,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author: String,
    /// Segundos desde a época Unix.
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectStatus {
    /// Nome do branch atual (None se HEAD destacado ou repo vazio sem nome).
    pub branch: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub changed_files: Vec<(String, FileStatus)>,
    pub last_commit: Option<CommitInfo>,
}

fn map_status(s: Status) -> Option<FileStatus> {
    if s.is_conflicted() {
        Some(FileStatus::Conflicted)
    } else if s.is_wt_new() && !s.is_index_new() {
        Some(FileStatus::Untracked)
    } else if s.is_index_new() {
        Some(FileStatus::New)
    } else if s.intersects(Status::INDEX_RENAMED | Status::WT_RENAMED) {
        Some(FileStatus::Renamed)
    } else if s.intersects(Status::INDEX_DELETED | Status::WT_DELETED) {
        Some(FileStatus::Deleted)
    } else if s.intersects(Status::INDEX_TYPECHANGE | Status::WT_TYPECHANGE) {
        Some(FileStatus::TypeChange)
    } else if s.intersects(Status::INDEX_MODIFIED | Status::WT_MODIFIED) {
        Some(FileStatus::Modified)
    } else {
        None
    }
}

/// Lê o status git de `path`. Retorna `Ok(None)` se não for um repositório.
pub fn status(path: &Path) -> Result<Option<ProjectStatus>> {
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let head = repo.head().ok();
    let branch = match &head {
        Some(h) if h.is_branch() => h.shorthand().map(str::to_owned),
        Some(_) => None,
        // Repo sem commits: nome do branch vem do symbolic ref.
        None => repo
            .find_reference("HEAD")
            .ok()
            .and_then(|r| r.symbolic_target().map(|t| t.trim_start_matches("refs/heads/").to_owned())),
    };

    let head_commit = head.as_ref().and_then(|h| h.peel_to_commit().ok());
    let last_commit = head_commit.as_ref().map(|c| {
        let id = c.id().to_string();
        CommitInfo {
            id: id[..7.min(id.len())].to_owned(),
            message: c.summary().unwrap_or("").to_owned(),
            author: c.author().name().unwrap_or("").to_owned(),
            time: c.time().seconds(),
        }
    });

    // ahead/behind em relação ao upstream, se houver.
    let (mut ahead, mut behind) = (0, 0);
    if let (Some(h), Some(local)) = (&head, &head_commit) {
        if h.is_branch() {
            if let Some(name) = h.shorthand() {
                if let Ok(b) = repo.find_branch(name, git2::BranchType::Local) {
                    if let Ok(up) = b.upstream() {
                        if let Some(up_oid) = up.get().target() {
                            if let Ok((a, bh)) = repo.graph_ahead_behind(local.id(), up_oid) {
                                ahead = a;
                                behind = bh;
                            }
                        }
                    }
                }
            }
        }
    }

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .renames_head_to_index(true);
    let mut changed_files = Vec::new();
    for entry in repo.statuses(Some(&mut opts))?.iter() {
        if let (Some(p), Some(st)) = (entry.path(), map_status(entry.status())) {
            changed_files.push((p.to_owned(), st));
        }
    }

    Ok(Some(ProjectStatus { branch, ahead, behind, changed_files, last_commit }))
}

/// Mantém o watcher vivo; ao ser descartado, o monitoramento para.
pub struct WatchHandle {
    _watcher: RecommendedWatcher,
}

fn is_ignored(p: &Path) -> bool {
    let comps: Vec<String> = p
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    comps.iter().enumerate().any(|(i, c)| {
        c == "target"
            || c == "node_modules"
            || (c == ".git" && comps.get(i + 1).map(|n| n == "objects").unwrap_or(false))
    })
}

/// Observa `paths` (raízes de projeto) e emite a raiz do projeto alterado,
/// com debounce de ~300ms por projeto.
pub fn watch(paths: Vec<PathBuf>) -> Result<(WatchHandle, UnboundedReceiver<PathBuf>)> {
    const DEBOUNCE: Duration = Duration::from_millis(300);
    let (raw_tx, raw_rx) = std_mpsc::channel::<PathBuf>();
    let (tx, rx) = unbounded_channel();

    let roots = paths.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            for p in ev.paths {
                if let Some(root) = roots.iter().filter(|r| p.starts_with(r)).max_by_key(|r| r.as_os_str().len()) {
                    let rel = p.strip_prefix(root).unwrap_or(&p);
                    if !is_ignored(rel) {
                        let _ = raw_tx.send(root.clone());
                    }
                }
            }
        }
    })?;
    for p in &paths {
        watcher.watch(p, RecursiveMode::Recursive)?;
    }

    // Thread de debounce: emite cada projeto após 300ms sem novos eventos.
    std::thread::spawn(move || {
        let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
        loop {
            let timeout = pending
                .values()
                .map(|t| (*t + DEBOUNCE).saturating_duration_since(Instant::now()))
                .min()
                .unwrap_or(Duration::from_secs(3600));
            match raw_rx.recv_timeout(timeout) {
                Ok(root) => {
                    pending.insert(root, Instant::now());
                }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                Err(std_mpsc::RecvTimeoutError::Disconnected) => break,
            }
            let now = Instant::now();
            let ready: Vec<PathBuf> =
                pending.iter().filter(|(_, t)| now >= **t + DEBOUNCE).map(|(p, _)| p.clone()).collect();
            for p in ready {
                pending.remove(&p);
                if tx.send(p).is_err() {
                    return;
                }
            }
        }
    });

    Ok((WatchHandle { _watcher: watcher }, rx))
}

/// Observa só o primeiro nível de `root` (subpastas criadas, removidas ou
/// renomeadas) e avisa, com debounce de ~500ms, que a lista de projetos pode ter
/// mudado. Mudanças dentro dos projetos ficam com `watch`.
pub fn watch_root(root: PathBuf) -> Result<(WatchHandle, UnboundedReceiver<()>)> {
    const DEBOUNCE: Duration = Duration::from_millis(500);
    let (raw_tx, raw_rx) = std_mpsc::channel::<()>();
    let (tx, rx) = unbounded_channel();
    let base = root.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            use notify::EventKind;
            if matches!(ev.kind, EventKind::Access(_)) {
                return;
            }
            // Só entradas diretas da raiz (a própria raiz conta: pode ter sumido).
            if ev.paths.iter().any(|p| p == &base || p.parent() == Some(base.as_path())) {
                let _ = raw_tx.send(());
            }
        }
    })?;
    watcher.watch(&root, RecursiveMode::NonRecursive)?;
    std::thread::spawn(move || loop {
        // Espera o primeiro sinal e depois o silêncio de DEBOUNCE.
        if raw_rx.recv().is_err() {
            return;
        }
        loop {
            match raw_rx.recv_timeout(DEBOUNCE) {
                Ok(()) => continue,
                Err(std_mpsc::RecvTimeoutError::Timeout) => break,
                Err(std_mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        if tx.send(()).is_err() {
            return;
        }
    });
    Ok((WatchHandle { _watcher: watcher }, rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;

    #[test]
    fn not_a_repo_returns_none() {
        let d = tempfile::tempdir().unwrap();
        assert!(status(d.path()).unwrap().is_none());
    }

    #[test]
    fn status_reports_commit_and_changes() {
        let d = tempfile::tempdir().unwrap();
        let repo = Repository::init(d.path()).unwrap();
        std::fs::write(d.path().join("a.txt"), "1").unwrap();
        let mut idx = repo.index().unwrap();
        idx.add_path(Path::new("a.txt")).unwrap();
        idx.write().unwrap();
        let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Teste", "t@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "primeiro commit", &tree, &[]).unwrap();

        std::fs::write(d.path().join("a.txt"), "2").unwrap();
        std::fs::write(d.path().join("b.txt"), "x").unwrap();

        let st = status(d.path()).unwrap().unwrap();
        assert!(st.branch.is_some());
        let c = st.last_commit.unwrap();
        assert_eq!(c.message, "primeiro commit");
        assert_eq!(c.author, "Teste");
        assert_eq!(c.id.len(), 7);
        assert!(st.changed_files.contains(&("a.txt".into(), FileStatus::Modified)));
        assert!(st.changed_files.contains(&("b.txt".into(), FileStatus::Untracked)));
    }

    #[test]
    fn ignored_paths() {
        assert!(is_ignored(Path::new("target/debug/x")));
        assert!(is_ignored(Path::new("web/node_modules/a")));
        assert!(is_ignored(Path::new(".git/objects/ab")));
        assert!(!is_ignored(Path::new(".git/HEAD")));
        assert!(!is_ignored(Path::new("src/main.rs")));
    }

    #[tokio::test]
    async fn watch_emits_project_root() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().canonicalize().unwrap();
        let (_h, mut rx) = watch(vec![root.clone()]).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        std::fs::write(root.join("f.txt"), "x").unwrap();
        let got = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
        assert_eq!(got, root);
    }

    #[tokio::test]
    async fn watch_root_sees_new_and_removed_subfolders() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("a")).unwrap();
        let (_h, mut rx) = watch_root(root.clone()).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Várias mudanças seguidas viram um aviso só.
        std::fs::create_dir(root.join("b")).unwrap();
        std::fs::create_dir(root.join("c")).unwrap();
        std::fs::write(root.join("c").join("Cargo.toml"), "").unwrap();
        tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(900), rx.recv()).await.is_err(), "debounce");
        std::fs::rename(root.join("b"), root.join("b2")).unwrap();
        tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
        std::fs::remove_dir(root.join("a")).unwrap();
        tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
    }
}
