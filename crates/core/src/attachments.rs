//! Pasta de anexos do chat e a liberação de leitura dela para o agente.
//!
//! Os anexos (imagens e arquivos soltos, colados ou escolhidos no clipe) ficam em
//! `%APPDATA%/argus/anexos/<tarefa>/` (ou `novo/`, antes de a tarefa existir), fora
//! da pasta de trabalho do agente. O `claude -p` só lê fora da pasta de trabalho o
//! que vier em `--add-dir`; sem isso, a leitura do anexo é negada.

use crate::agents::provider::CommandSpec;
use std::path::{Path, PathBuf};

/// `%APPDATA%/argus/anexos` (ou a pasta temporária, se não houver APPDATA).
pub fn root() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(crate::paths::dir_name())
        .join("anexos")
}

/// Forma comparável de um caminho: barras iguais e, no Windows, sem diferença de caixa.
fn norm(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.trim_end_matches('/').to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

/// A subpasta direta de `root` que contém `path` (`root/<pasta>/arquivo`), se houver.
/// Caminho fora da raiz, com `..` ou direto na raiz não devolve nada.
fn subdir_of(root: &Path, path: &Path) -> Option<PathBuf> {
    if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return None;
    }
    let (r, p) = (norm(root), norm(path));
    let rest = p.strip_prefix(&r)?.strip_prefix('/')?;
    let (first, tail) = rest.split_once('/')?;
    if first.is_empty() || tail.is_empty() {
        return None;
    }
    Some(root.join(first))
}

/// Caminhos citados nas linhas "- <caminho> (<tipo>)" do bloco de anexos de um pedido.
fn cited_paths(prompt: &str) -> impl Iterator<Item = PathBuf> + '_ {
    prompt.lines().filter_map(|l| {
        let l = l.trim().strip_prefix("- ")?.trim();
        // tira o rótulo final "(imagem)", "(texto)"...
        let path = match l.rsplit_once(" (") {
            Some((p, kind)) if kind.ends_with(')') && !kind.contains(['/', '\\']) => p,
            _ => l,
        };
        Some(PathBuf::from(path.trim()))
    })
}

/// Pastas de anexos que o agente precisa ler neste turno: a da tarefa
/// (`root/<id>`) e as dos anexos citados nos pedidos dados (o primeiro pedido
/// de uma conversa grava em `root/novo`). Só entram subpastas diretas de `root`
/// que existem; nada fora da pasta de anexos.
pub fn dirs_for(root: &Path, task: u64, prompts: &[&str]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |d: PathBuf| {
        if d.is_dir() && !out.iter().any(|o| norm(o) == norm(&d)) {
            out.push(d);
        }
    };
    push(root.join(task.to_string()));
    for prompt in prompts {
        for p in cited_paths(prompt) {
            if let Some(d) = subdir_of(root, &p) {
                push(d);
            }
        }
    }
    out
}

/// Acrescenta `--add-dir <pastas...>` a um comando do `claude`. Outras CLIs não mudam.
pub fn apply_add_dirs(mut spec: CommandSpec, dirs: &[PathBuf]) -> CommandSpec {
    let is_claude = Path::new(&spec.program)
        .file_stem()
        .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("claude"));
    if is_claude && !dirs.is_empty() {
        spec.args.push("--add-dir".into());
        spec.args.extend(dirs.iter().map(|d| d.to_string_lossy().into_owned()));
    }
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_attachment_folders_of_a_turn() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("anexos");
        for d in ["7", "novo", "9"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let img = root.join("novo").join("print-1.png");
        let first = format!(
            "descreva a imagem\n\nArquivos anexados pelo caminho (abra com as ferramentas se precisar):\n- {} (imagem)\n- {} (texto)",
            img.display(),
            root.join("novo").join("nota (1).txt").display()
        );
        // A pasta da tarefa e a do primeiro pedido, sem repetir; a de outra tarefa fica de fora.
        assert_eq!(dirs_for(&root, 7, &[&first, "e agora?"]), vec![root.join("7"), root.join("novo")]);
        // Tarefa sem pasta própria: só o que o pedido cita.
        assert_eq!(dirs_for(&root, 8, &[&first]), vec![root.join("novo")]);
        assert!(dirs_for(&root, 8, &["sem anexos\n- item de lista\n- outro (x)"]).is_empty());
        // Nada fora da pasta de anexos, nem subindo com `..`, nem a própria raiz.
        let outside = tmp.path().join("fora").join("a.png");
        std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
        let evil = format!(
            "- {} (imagem)\n- {} (imagem)\n- {} (imagem)",
            outside.display(),
            root.join("9").join("..").join("..").join("fora").join("a.png").display(),
            root.join("solto.png").display()
        );
        assert!(dirs_for(&root, 8, &[&evil]).is_empty());
        // Pasta citada que não existe não entra.
        assert!(dirs_for(&root, 8, &[&format!("- {} (imagem)", root.join("123").join("a.png").display())]).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn folder_match_ignores_case_and_slashes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("anexos");
        std::fs::create_dir_all(root.join("novo")).unwrap();
        let cited = root.join("novo").join("a.png").to_string_lossy().replace('\\', "/").to_uppercase();
        let dirs = dirs_for(&root, 1, &[&format!("- {cited} (imagem)")]);
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].is_dir());
    }

    #[test]
    fn add_dir_only_for_claude() {
        let dirs = vec![PathBuf::from("/x/anexos/7"), PathBuf::from("/x/anexos/novo")];
        let c = apply_add_dirs(CommandSpec::new("claude").arg("-p"), &dirs);
        assert_eq!(c.args.len(), 4);
        assert_eq!(&c.args[..2], ["-p", "--add-dir"]);
        assert!(c.args[2].ends_with('7') && c.args[3].ends_with("novo"));
        assert_eq!(apply_add_dirs(CommandSpec::new("claude").arg("-p"), &[]).args, ["-p"]);
        assert_eq!(apply_add_dirs(CommandSpec::new("opencode").arg("run"), &dirs).args, ["run"]);
    }
}
