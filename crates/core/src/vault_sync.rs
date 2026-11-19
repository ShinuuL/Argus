//! Atualização das notas de projeto do vault ao abrir o Argus.
//!
//! O trabalho mecânico é do Argus, sem agente: para cada projeto da raiz com
//! git, lê os commits desde a última sincronização (`git log`). Se nenhum
//! projeto mudou, termina sem chamar agente nenhum (custo zero). Se mudou,
//! abre UMA conversa no modelo mais barato, com o resumo dos commits já no
//! pedido, para o agente só escrever: ele não precisa explorar nada.
//!
//! A marca da última sincronização fica em `<raiz>/_Argus/.sincronizado`
//! (segundos desde 1970). Só roda se a pasta `_Argus` existir (memória ligada).

use crate::agents::orchestrator::{Orchestrator, SubmitOptions, WorkMode};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Modelo da sincronização: o mais barato do claude.
pub const MODEL: &str = "haiku";
/// Commits por projeto no pedido (o resto vira "e mais N").
const MAX_COMMITS: usize = 15;
const STAMP: &str = ".sincronizado";

/// Commits de um projeto desde a última sincronização.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    pub project: String,
    pub commits: Vec<String>,
    pub more: usize,
}

fn stamp_path(root: &Path) -> PathBuf {
    crate::memory::dir(root).join(STAMP)
}

/// Última sincronização (None = nunca: olha os últimos 7 dias).
pub fn last_sync(root: &Path) -> Option<i64> {
    std::fs::read_to_string(stamp_path(root)).ok()?.trim().parse().ok()
}

fn mark(root: &Path, t: i64) {
    let _ = std::fs::write(stamp_path(root), t.to_string());
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Projetos da raiz (pastas com `.git`) e seus commits desde `since`.
pub fn collect(root: &Path, since: i64) -> Vec<Changes> {
    let Ok(projects) = crate::workspace::scan(root) else { return Vec::new() };
    let mut out = Vec::new();
    for p in projects {
        if !p.path.join(".git").exists() {
            continue;
        }
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&p.path)
            .args(["log", "--no-merges", "--format=%s", &format!("--since=@{since}")]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let Ok(o) = cmd.output() else { continue };
        if !o.status.success() {
            continue;
        }
        let all: Vec<String> = String::from_utf8_lossy(&o.stdout).lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
        if all.is_empty() {
            continue;
        }
        let more = all.len().saturating_sub(MAX_COMMITS);
        out.push(Changes { project: p.name.clone(), commits: all.into_iter().take(MAX_COMMITS).collect(), more });
    }
    out
}

/// Pedido para o agente: só escrever, com tudo que ele precisa já aqui.
pub fn prompt(root: &Path, changes: &[Changes], date: &str) -> String {
    let mut s = format!(
        "Atualize as notas de projeto do vault em {}. Para cada projeto abaixo, acrescente ao FIM do arquivo \
         <nome do projeto>.md dessa pasta (crie se não existir, com o título # <nome>) uma seção `## {date}` com \
         de 2 a 5 marcadores curtos em português resumindo o que mudou, a partir só dos commits listados. \
         Não leia código, não rode comandos e não abra outros arquivos além dessas notas. Não altere seções antigas. \
         Ao terminar, responda só: ok.\n",
        root.display()
    );
    for c in changes {
        s.push_str(&format!("\n### {}\n", c.project));
        for m in &c.commits {
            s.push_str(&format!("- {m}\n"));
        }
        if c.more > 0 {
            s.push_str(&format!("- (e mais {} commits)\n", c.more));
        }
    }
    s
}

/// Roda uma vez: nada mudou → só marca e sai, sem agente. Devolve a tarefa aberta.
pub fn run(orch: &Orchestrator, root: &Path) -> Option<u64> {
    if !crate::memory::dir(root).is_dir() {
        return None;
    }
    let t = now();
    let since = last_sync(root).unwrap_or(t - 7 * 86_400);
    let changes = collect(root, since);
    if changes.is_empty() {
        mark(root, t);
        return None;
    }
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let opts = SubmitOptions { model: Some(MODEL.into()), mode: Some(WorkMode::Direct), ..Default::default() };
    match orch.submit_with(root.to_path_buf(), "claude", &prompt(root, &changes, &date), opts) {
        Ok(id) => {
            mark(root, t);
            Some(id)
        }
        Err(e) => {
            tracing::warn!("sincronização do vault não iniciou: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        assert!(Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success());
    }

    #[test]
    fn only_projects_with_new_commits_and_prompt_has_everything() {
        let root = tempfile::tempdir().unwrap();
        for name in ["a", "b"] {
            let p = root.path().join(name);
            std::fs::create_dir(&p).unwrap();
            git(&p, &["init", "-q"]);
            git(&p, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", &format!("inicio {name}")]);
        }
        // Nada depois de "agora": nenhum projeto mudou.
        assert!(collect(root.path(), now() + 60).is_empty());
        let c = collect(root.path(), now() - 3600);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].commits, vec!["inicio a".to_string()]);
        let p = prompt(root.path(), &c, "2026-11-19");
        assert!(p.contains("## 2026-11-19") && p.contains("### a") && p.contains("- inicio b"));
        assert!(p.contains("Não leia código"));
    }

    #[test]
    fn stamp_round_trip_and_skips_without_memory_dir() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(last_sync(root.path()), None);
        std::fs::create_dir(crate::memory::dir(root.path())).unwrap();
        mark(root.path(), 123);
        assert_eq!(last_sync(root.path()), Some(123));
    }
}
