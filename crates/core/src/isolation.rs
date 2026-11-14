//! O que o agente NÃO enxerga quando trabalha numa cópia isolada (git worktree).
//!
//! O worktree nasce do último commit: só tem os arquivos versionados. Pastas e
//! arquivos não versionados, ignorados (`.env`, dados locais) e alterações ainda
//! não commitadas ficam só na pasta original do projeto. Este módulo compara as
//! duas coisas antes de a conversa começar, para a interface avisar o usuário.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// Pastas e arquivos gerados que não vale listar (dependências, saída de build, cache).
const NOISE: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".git",
    ".argus-worktrees",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".nuxt",
    ".cache",
    ".gradle",
    ".dart_tool",
    ".turbo",
    ".parcel-cache",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "coverage",
    ".idea",
    ".vs",
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
];
const NOISE_EXT: &[&str] = &["log", "tmp", "pyc", "tsbuildinfo"];

/// Quantos itens de cada lista vão para a interface (o total vai à parte).
pub const MAX_ITEMS: usize = 40;

/// O que existe na pasta do projeto e não existirá na cópia isolada.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsolationReport {
    /// A pasta é um repositório git (sem isso não há cópia isolada possível).
    pub is_git: bool,
    /// Arquivos e pastas não versionados (pasta termina com `/`).
    pub untracked: Vec<String>,
    /// Ignorados pelo git que parecem relevantes (sem dependências nem saída de build).
    pub ignored: Vec<String>,
    /// Arquivos versionados com alterações não commitadas (o agente verá a versão do commit).
    pub modified: Vec<String>,
    /// Caminhos citados no pedido que existem na pasta do projeto e não existirão na cópia.
    pub mentioned: Vec<String>,
    /// Totais antes do corte em `MAX_ITEMS`.
    pub untracked_total: usize,
    pub ignored_total: usize,
    pub modified_total: usize,
}

impl IsolationReport {
    /// Nada ficaria de fora da cópia isolada.
    pub fn is_clean(&self) -> bool {
        self.untracked_total == 0 && self.ignored_total == 0 && self.modified_total == 0 && self.mentioned.is_empty()
    }
}

/// A pasta é a raiz de um repositório git?
pub fn is_git_repo(project: &Path) -> bool {
    project.join(".git").exists()
}

fn git(dir: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let mut cmd = std::process::Command::new(crate::agents::orchestrator::resolve_program("git"));
    cmd.args(args).current_dir(dir).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    out.status.success().then_some(out.stdout)
}

pub(crate) fn is_noise(path: &str) -> bool {
    let p = path.trim_end_matches('/');
    if p.split('/').any(|seg| NOISE.contains(&seg)) {
        return true;
    }
    let name = p.rsplit('/').next().unwrap_or(p);
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => NOISE_EXT.contains(&ext.to_ascii_lowercase().as_str()),
        _ => false,
    }
}

/// Separa a saída de `git status --porcelain -z --ignored` em
/// (não versionados, ignorados, alterados). Cada registro é `XY caminho`,
/// terminado por NUL; renomeações trazem o caminho antigo no registro seguinte.
pub fn parse_status(raw: &[u8]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let text = String::from_utf8_lossy(raw);
    let (mut untracked, mut ignored, mut modified) = (Vec::new(), Vec::new(), Vec::new());
    let mut it = text.split('\0');
    while let Some(rec) = it.next() {
        if rec.len() < 4 {
            continue;
        }
        let (code, path) = (&rec[..2], rec[3..].to_string());
        if code.starts_with('R') || code.starts_with('C') {
            it.next(); // caminho de origem
        }
        match code {
            "??" => untracked.push(path),
            "!!" => ignored.push(path),
            _ => modified.push(path),
        }
    }
    (untracked, ignored, modified)
}

/// Nome real (com as maiúsculas do disco) de `rel` dentro de `project`, se existir.
/// Só aceita caminhos relativos que não saem da pasta.
fn real_relative(project: &Path, rel: &str) -> Option<String> {
    let rel = rel.replace('\\', "/");
    let mut cur = project.to_path_buf();
    let mut parts = Vec::new();
    for comp in Path::new(&rel).components() {
        let Component::Normal(seg) = comp else {
            if matches!(comp, Component::CurDir) {
                continue;
            }
            return None;
        };
        let want = seg.to_string_lossy().to_lowercase();
        let found = std::fs::read_dir(&cur).ok()?.flatten().find_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name.to_lowercase() == want).then_some(name)
        })?;
        cur.push(&found);
        parts.push(found);
    }
    if parts.is_empty() {
        return None;
    }
    let dir = cur.is_dir();
    Some(parts.join("/") + if dir { "/" } else { "" })
}

/// Trechos do pedido que podem ser um caminho: palavras sem as aspas, parênteses
/// e a pontuação das pontas (`"comite"`, `@docs/notas.md,`, `` `dados/` ``).
fn path_candidates(prompt: &str) -> Vec<String> {
    let edge = |c: char| "\"'`()[]{}<>,;:!?*".contains(c) || c == '“' || c == '”' || c == '‘' || c == '’';
    let mut out: Vec<String> = Vec::new();
    for word in prompt.split_whitespace() {
        let w = word.trim_matches(edge).trim_start_matches('@').trim_end_matches('.').trim_matches(edge);
        if w.chars().count() < 2 || w.len() > 260 || w.contains("://") || out.iter().any(|o| o == w) {
            continue;
        }
        out.push(w.to_string());
        if out.len() >= 200 {
            break;
        }
    }
    out
}

/// Caminhos citados em `prompt` que existem em `project` e não têm nenhum arquivo
/// versionado (logo, não existirão no worktree).
pub fn mentioned_missing(project: &Path, prompt: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for cand in path_candidates(prompt) {
        // Caminho absoluto dentro do projeto vira relativo; fora dele, não interessa.
        let rel = match Path::new(&cand).is_absolute() {
            true => match PathBuf::from(&cand).strip_prefix(project) {
                Ok(r) => r.to_string_lossy().into_owned(),
                Err(_) => continue,
            },
            false => cand,
        };
        let Some(real) = real_relative(project, &rel) else { continue };
        if real.trim_end_matches('/') == ".git" || out.contains(&real) {
            continue;
        }
        let tracked = git(project, &["ls-files", "-z", "--", real.trim_end_matches('/')]);
        if tracked.is_some_and(|t| t.is_empty()) {
            out.push(real);
        }
        if out.len() >= 10 {
            break;
        }
    }
    out
}

/// Compara a pasta do projeto com o que um worktree novo teria. `prompt` é o
/// pedido do usuário (para destacar caminhos citados que ficariam de fora).
pub fn check(project: &Path, prompt: &str) -> IsolationReport {
    if !is_git_repo(project) {
        return IsolationReport::default();
    }
    let mut rep = IsolationReport { is_git: true, ..Default::default() };
    if let Some(raw) = git(project, &["status", "--porcelain=v1", "-z", "--untracked-files=normal", "--ignored=traditional"]) {
        let (untracked, ignored, modified) = parse_status(&raw);
        let keep = |v: Vec<String>| -> Vec<String> { v.into_iter().filter(|p| !is_noise(p)).collect() };
        let (untracked, ignored) = (keep(untracked), keep(ignored));
        rep.untracked_total = untracked.len();
        rep.ignored_total = ignored.len();
        rep.modified_total = modified.len();
        let cut = |mut v: Vec<String>| -> Vec<String> {
            // Pastas primeiro: costumam ser o que mais faz falta.
            v.sort_by_key(|p| (!p.ends_with('/'), p.to_lowercase()));
            v.truncate(MAX_ITEMS);
            v
        };
        rep.untracked = cut(untracked);
        rep.ignored = cut(ignored);
        rep.modified = cut(modified);
    }
    rep.mentioned = mentioned_missing(project, prompt);
    rep
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dir: &Path, args: &[&str]) {
        let mut all = vec!["-c", "user.email=t@t", "-c", "user.name=t"];
        all.extend_from_slice(args);
        assert!(git(dir, &all).is_some(), "git {args:?}");
    }

    #[test]
    fn parses_porcelain_z() {
        let raw = b"?? comite/\0!! .env\0 M src/a.rs\0R  novo.txt\0velho.txt\0!! node_modules/\0";
        let (u, i, m) = parse_status(raw);
        assert_eq!(u, ["comite/"]);
        assert_eq!(i, [".env", "node_modules/"]);
        assert_eq!(m, ["src/a.rs", "novo.txt"]);
        assert!(is_noise("node_modules/") && is_noise("a/target/") && is_noise("saida.log"));
        assert!(!is_noise(".env") && !is_noise("comite/") && !is_noise("src/target.rs"));
    }

    /// O cenário real: uma pasta que existe no projeto mas não está no git.
    #[test]
    fn detects_what_the_worktree_will_not_have() {
        if which::which("git").is_err() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path();
        std::fs::create_dir_all(p.join("src")).unwrap();
        std::fs::write(p.join("src/main.txt"), "a\n").unwrap();
        std::fs::write(p.join("README.md"), "leia\n").unwrap();
        std::fs::write(p.join(".gitignore"), ".env\nnode_modules/\n*.log\n").unwrap();
        run(p, &["init", "-q"]);
        run(p, &["add", "."]);
        run(p, &["commit", "-q", "-m", "init"]);
        // Limpo: nada a avisar, mesmo citando um caminho versionado.
        let rep = check(p, "explique a pasta src e o README.md");
        assert!(rep.is_git && rep.is_clean(), "{rep:?}");

        std::fs::create_dir_all(p.join("comite/atas")).unwrap();
        std::fs::write(p.join("comite/atas/a.md"), "x").unwrap();
        std::fs::write(p.join("rascunho.txt"), "x").unwrap();
        std::fs::write(p.join(".env"), "CHAVE=1").unwrap();
        std::fs::write(p.join("saida.log"), "x").unwrap();
        std::fs::create_dir_all(p.join("node_modules/x")).unwrap();
        std::fs::write(p.join("node_modules/x/i.js"), "x").unwrap();
        std::fs::write(p.join("README.md"), "leia de novo\n").unwrap();

        let rep = check(p, "Melhore a arquitetura da pasta \"Comite\" e veja o README.md, o src/main.txt e a node_modules.");
        assert_eq!(rep.untracked, ["comite/", "rascunho.txt"]);
        assert_eq!(rep.ignored, [".env"], "dependências e logs ficam de fora da lista");
        assert_eq!(rep.modified, ["README.md"]);
        // Citados que não existirão na cópia: a pasta fora do git (com o nome real) e a ignorada.
        assert_eq!(rep.mentioned, ["comite/", "node_modules/"]);
        assert!(!rep.is_clean());
        // Caminho que sai da pasta, endereço e palavra comum não contam.
        assert!(mentioned_missing(p, "veja ../fora, https://exemplo.test/comite e a pasta").is_empty());

        // Pasta que não é repositório: sem relatório (só aceita "Direto na pasta").
        let plain = tempfile::tempdir().unwrap();
        let rep = check(plain.path(), "qualquer");
        assert!(!rep.is_git && rep.is_clean());
    }
}
