//! Pontos de retorno por turno ("Voltar a este ponto").
//!
//! Antes de cada turno de uma tarefa, o Argus tira um retrato da pasta de
//! trabalho (a do projeto no modo Direto, o worktree no Isolado) e guarda num
//! ref do git: `refs/argus/t<tarefa>/<n>`, onde `n` é o número do pedido na
//! conversa (1 = o primeiro). O retrato inclui os arquivos não versionados (o
//! que o `.gitignore` não exclui) e não mexe no índice, no stash nem na branch:
//! é feito com um índice temporário (`GIT_INDEX_FILE`), `write-tree` e
//! `commit-tree`.
//!
//! Voltar restaura só a pasta de trabalho: os arquivos ficam como no retrato e
//! os que surgiram depois (fora do `.gitignore`) são apagados. O índice e os
//! commits não mudam.
use std::path::Path;
use std::process::Command;

fn ref_name(task: u64, n: u32) -> String {
    format!("refs/argus/t{task}/{n}")
}

fn git(dir: &Path, args: &[&str], index: Option<&Path>) -> Option<String> {
    let mut cmd = Command::new(crate::agents::orchestrator::resolve_program("git"));
    cmd.args(args).current_dir(dir).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    if let Some(i) = index {
        cmd.env("GIT_INDEX_FILE", i);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

struct Cleanup(std::path::PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Tira o retrato de `dir` antes do pedido `n` da tarefa. None se não for git.
pub fn snapshot(dir: &Path, task: u64, n: u32) -> Option<String> {
    if !crate::isolation::is_git_repo(dir) {
        return None;
    }
    // Índice temporário próprio (apagado no fim), fora do repositório.
    let index_path = std::env::temp_dir().join(format!("argus-ponto-{}-{task}-{n}.idx", std::process::id()));
    let _cleanup = Cleanup(index_path.clone());
    let index = index_path.as_path();
    // Índice temporário a partir do HEAD (repositório sem commit: vazio).
    let head = git(dir, &["rev-parse", "--verify", "-q", "HEAD"], None);
    if head.is_some() {
        git(dir, &["read-tree", "HEAD"], Some(index))?;
    }
    git(dir, &["add", "-A", "."], Some(index))?;
    let tree = git(dir, &["write-tree"], Some(index))?;
    let mut args = vec!["-c", "user.name=Argus", "-c", "user.email=argus@localhost", "commit-tree", tree.as_str(), "-m", "argus: ponto de retorno"];
    if let Some(h) = head.as_deref() {
        args.extend(["-p", h]);
    }
    let commit = git(dir, &args, None)?;
    git(dir, &["update-ref", &ref_name(task, n), &commit], None)?;
    Some(commit)
}

/// Pedidos da tarefa que têm ponto de retorno, em ordem.
pub fn list(dir: &Path, task: u64) -> Vec<u32> {
    let prefix = format!("refs/argus/t{task}/");
    let Some(out) = git(dir, &["for-each-ref", "--format=%(refname)", &prefix], None) else { return Vec::new() };
    let mut v: Vec<u32> = out.lines().filter_map(|l| l.strip_prefix(&prefix)?.parse().ok()).collect();
    v.sort_unstable();
    v
}

/// Arquivos da pasta de trabalho fora do `.gitignore` (versionados ou não).
fn files_now(dir: &Path) -> Vec<String> {
    git(dir, &["ls-files", "-co", "--exclude-standard", "-z"], None)
        .map(|s| s.split('\0').filter(|f| !f.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

/// Volta a pasta de trabalho ao retrato do pedido `n`. Devolve quantos arquivos
/// foram apagados (os que surgiram depois do retrato).
pub fn restore(dir: &Path, task: u64, n: u32) -> Result<usize, String> {
    let commit = git(dir, &["rev-parse", "--verify", "-q", &ref_name(task, n)], None).ok_or("Esse ponto de retorno não existe mais.")?;
    let in_snap: std::collections::HashSet<String> = git(dir, &["ls-tree", "-r", "--name-only", "-z", &commit], None)
        .map(|s| s.split('\0').filter(|f| !f.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    let mut removed = 0;
    for f in files_now(dir) {
        if !in_snap.contains(&f) && std::fs::remove_file(dir.join(&f)).is_ok() {
            removed += 1;
        }
    }
    git(dir, &["restore", &format!("--source={commit}"), "--worktree", "--", "."], None).ok_or("O git não conseguiu restaurar os arquivos.")?;
    Ok(removed)
}

/// Apaga os pontos de retorno da tarefa (ao excluir, aplicar ou descartar).
pub fn forget(dir: &Path, task: u64) {
    for n in list(dir, task) {
        let _ = git(dir, &["update-ref", "-d", &ref_name(task, n)], None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(dir: &Path, args: &[&str]) {
        assert!(Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success());
    }

    #[test]
    fn snapshot_and_restore_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        sh(p, &["init", "-q"]);
        std::fs::write(p.join("a.txt"), "1").unwrap();
        std::fs::write(p.join(".gitignore"), "ign/\n").unwrap();
        sh(p, &["add", "-A"]);
        sh(p, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "x"]);
        // Alteração não commitada e arquivo novo antes do retrato: entram nele.
        std::fs::write(p.join("a.txt"), "2").unwrap();
        std::fs::write(p.join("novo.txt"), "n").unwrap();
        assert!(snapshot(p, 7, 1).is_some());
        assert_eq!(list(p, 7), vec![1]);
        // O índice e o status não mudaram.
        let st = git(p, &["status", "--porcelain"], None).unwrap();
        assert!(st.contains("M a.txt") && st.contains("?? novo.txt") && !st.contains("A  novo.txt"), "{st}");
        // O agente mexe: edita, apaga e cria; o ignorado fica.
        std::fs::write(p.join("a.txt"), "3").unwrap();
        std::fs::remove_file(p.join("novo.txt")).unwrap();
        std::fs::write(p.join("depois.txt"), "d").unwrap();
        std::fs::create_dir(p.join("ign")).unwrap();
        std::fs::write(p.join("ign/x"), "x").unwrap();
        assert_eq!(restore(p, 7, 1).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(p.join("a.txt")).unwrap(), "2");
        assert_eq!(std::fs::read_to_string(p.join("novo.txt")).unwrap(), "n");
        assert!(!p.join("depois.txt").exists() && p.join("ign/x").exists());
        forget(p, 7);
        assert!(list(p, 7).is_empty());
    }

    #[test]
    fn not_git_is_none() {
        let d = tempfile::tempdir().unwrap();
        assert!(snapshot(d.path(), 1, 1).is_none());
    }
}
