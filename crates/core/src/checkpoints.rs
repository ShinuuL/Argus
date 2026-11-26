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

fn git_raw(dir: &Path, args: &[&str], index: Option<&Path>) -> Option<Vec<u8>> {
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
    out.status.success().then_some(out.stdout)
}

fn git(dir: &Path, args: &[&str], index: Option<&Path>) -> Option<String> {
    git_raw(dir, args, index).map(|b| String::from_utf8_lossy(&b).trim().to_string())
}

/// Caminhos que diferem do HEAD, incluindo os não versionados. `diff-index`
/// compara o HEAD com a pasta de trabalho; `ls-files --others` completa com o
/// que ainda não entrou no git. `-z` preserva nomes com espaços e acentos.
/// Qualquer falha do git aborta o retrato: uma lista parcial geraria um ponto
/// de retorno que não contém as mudanças do usuário.
fn changed_paths(dir: &Path, has_head: bool) -> Option<std::collections::BTreeSet<String>> {
    let mut out = std::collections::BTreeSet::new();
    let mut commands: Vec<&[&str]> = Vec::new();
    // Repositório sem commit não tem HEAD para comparar: o `ls-files` abaixo
    // já devolve todos os arquivos fora do .gitignore.
    if has_head {
        commands.push(&["diff-index", "--no-renames", "--name-only", "-z", "HEAD", "--"]);
    }
    commands.push(&["ls-files", "--others", "--exclude-standard", "-z"]);
    for args in commands {
        let bytes = git_raw(dir, args, None)?;
        for p in bytes.split(|b| *b == 0).filter(|p| !p.is_empty()) {
            out.insert(String::from_utf8_lossy(p).into_owned());
        }
    }
    Some(out)
}

/// Reescreve no índice temporário só os caminhos que mudaram. O índice já veio
/// do HEAD (`read-tree`), então os arquivos intactos não são relidos nem
/// rehasheados — no `git add -A` original todo o repositório era percorrido a
/// cada turno, o que dominava `prepare_ms` em pastas grandes.
fn apply_changes(dir: &Path, index: &Path, paths: &std::collections::BTreeSet<String>) -> Option<()> {
    for p in paths {
        let exists = std::fs::symlink_metadata(dir.join(p)).is_ok();
        let flag = if exists { "--add" } else { "--force-remove" };
        git(dir, &["update-index", flag, "--", p], Some(index))?;
    }
    Some(())
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
    apply_changes(dir, index, &changed_paths(dir, head.is_some())?)?;
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

    /// O retrato incremental tem de produzir exatamente a mesma árvore do
    /// `git add -A` de antes: editar, apagar, criar com espaço no nome e deixar
    /// o ignorado de fora. Sem esta prova, a otimização mudaria o "Voltar".
    #[test]
    fn incremental_snapshot_matches_full_add() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        sh(p, &["init", "-q"]);
        std::fs::write(p.join("a.txt"), "1").unwrap();
        std::fs::write(p.join("b.txt"), "2").unwrap();
        std::fs::write(p.join(".gitignore"), "ign/\n").unwrap();
        sh(p, &["add", "-A"]);
        sh(p, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "x"]);
        std::fs::write(p.join("a.txt"), "3").unwrap();
        std::fs::remove_file(p.join("b.txt")).unwrap();
        std::fs::write(p.join("novo arquivo.txt"), "n").unwrap();
        std::fs::create_dir(p.join("ign")).unwrap();
        std::fs::write(p.join("ign/x.txt"), "x").unwrap();

        let commit = snapshot(p, 9, 1).unwrap();
        let tree = git(p, &["rev-parse", &format!("{commit}^{{tree}}")], None).unwrap();

        let index = std::env::temp_dir().join(format!("argus-ponto-teste-{}-9-1.idx", std::process::id()));
        let _cleanup = Cleanup(index.clone());
        git(p, &["read-tree", "HEAD"], Some(&index)).unwrap();
        git(p, &["add", "-A", "."], Some(&index)).unwrap();
        let full = git(p, &["write-tree"], Some(&index)).unwrap();
        assert_eq!(tree, full);
    }

    /// Pasta git sem nenhum commit ainda: não há HEAD para `diff-index`, mas o
    /// retrato precisa capturar os arquivos como antes.
    #[test]
    fn snapshot_without_commit_still_captures_files() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        sh(p, &["init", "-q"]);
        std::fs::write(p.join("a.txt"), "1").unwrap();
        let commit = snapshot(p, 10, 1).unwrap();
        let tree = git(p, &["rev-parse", &format!("{commit}^{{tree}}")], None).unwrap();
        assert_eq!(git(p, &["ls-tree", "--name-only", &tree], None).unwrap(), "a.txt");
    }
}
