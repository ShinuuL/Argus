//! Preferências globais e memória dos agentes, em Markdown dentro da pasta raiz
//! (que pode ser parte de um vault do Obsidian):
//!
//! - `<raiz>/_Argus/Preferencias.md`: o prompt que vale para todos os projetos;
//! - `<raiz>/_Argus/Memoria.md`: índice da memória, uma linha por nota;
//! - `<raiz>/_Argus/Memoria/*.md`: uma nota por fato, com frontmatter
//!   (`tipo`, `projetos`, `atualizado`) e links `[[nota]]`;
//! - `<raiz>/<Projeto>.md`: nota do projeto, se existir.
//!
//! O conteúdo não vai no aviso de sistema: o aviso só aponta os arquivos (texto
//! estável entre turnos, para não regravar o cache) e a pasta `Argus` é liberada
//! com `--add-dir`.
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Pasta da memória dentro da raiz. Não pode ser `Argus`: na raiz real essa é
/// a pasta do próprio repositório do Argus. O `_` a põe no topo do Obsidian, e
/// `workspace::scan` não a lista como projeto.
pub const DIR: &str = "_Argus";
pub const PREFS: &str = "Preferencias.md";
pub const INDEX: &str = "Memoria.md";
pub const NOTES: &str = "Memoria";

/// Pasta raiz aberta no Argus (definida ao abrir a raiz).
static ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn set_root(root: Option<PathBuf>) {
    *ROOT.lock().unwrap() = root;
}

pub fn root() -> Option<PathBuf> {
    ROOT.lock().unwrap().clone()
}

pub fn dir(root: &Path) -> PathBuf {
    root.join(DIR)
}
pub fn prefs_path(root: &Path) -> PathBuf {
    dir(root).join(PREFS)
}
pub fn index_path(root: &Path) -> PathBuf {
    dir(root).join(INDEX)
}
pub fn notes_dir(root: &Path) -> PathBuf {
    dir(root).join(NOTES)
}

/// Caminho que pode ir num argumento de linha de comando (no Windows, pelo cmd).
fn arg_safe(p: &Path) -> bool {
    let s = p.to_string_lossy();
    !s.chars().any(|c| c.is_control() || matches!(c, '"' | '%' | '^' | '&' | '|' | '<' | '>' | '!' | '`'))
}

/// A memória está ativa quando a pasta `<raiz>/Argus` existe.
pub fn active(root: &Path) -> bool {
    dir(root).is_dir() && arg_safe(&dir(root))
}

/// Aviso de sistema que aponta as preferências, o índice da memória e a nota do
/// projeto. Uma linha só, sem aspas, e só depende de caminhos (estável entre turnos).
pub fn note(root: &Path, project: &Path) -> Option<String> {
    if !active(root) {
        return None;
    }
    let prefs = prefs_path(root);
    let index = index_path(root);
    let notes = notes_dir(root);
    let mut s = format!(
        "Antes de começar, leia as preferências do usuário em {} e o índice da memória em {}; essas preferências valem para todos os projetos. \
         Quando aprender algo durável (uma preferência do usuário ou uma decisão de projeto), grave ou atualize uma nota em {} (um fato por nota, \
         com frontmatter tipo, projetos e atualizado no formato AAAA-MM-DD, e links [[nota]] para notas relacionadas) e mantenha a linha dela no índice. \
         Não grave segredos nem credenciais na memória.",
        prefs.display(),
        index.display(),
        notes.display()
    );
    if let Some(p) = project_note(root, project) {
        s.push_str(&format!(" A nota deste projeto fica em {}.", p.display()));
    }
    Some(s)
}

/// `<raiz>/<Projeto>.md`, se existir.
pub fn project_note(root: &Path, project: &Path) -> Option<PathBuf> {
    let name = project.file_name()?.to_string_lossy().into_owned();
    let p = root.join(format!("{name}.md"));
    (p.is_file() && arg_safe(&p)).then_some(p)
}

/// Aviso e pasta a liberar para o projeto, com a raiz atual.
pub fn for_project(project: &Path) -> Option<(String, PathBuf)> {
    let root = root()?;
    note(&root, project).map(|n| (n, dir(&root)))
}

const PREFS_TEMPLATE: &str = "---\ntipo: preferencias\natualizado: \n---\n# Preferências\n\nInstruções que valem para todos os projetos e agentes.\n";
const INDEX_TEMPLATE: &str = "# Memória\n\nUma linha por nota: `- [[nome-da-nota]]: resumo em uma frase`.\n";

/// Cria a pasta `Argus`, as preferências, o índice e a pasta de notas, sem
/// sobrescrever o que já existe.
pub fn ensure(root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(notes_dir(root))?;
    for (p, t) in [(prefs_path(root), PREFS_TEMPLATE), (index_path(root), INDEX_TEMPLATE)] {
        if !p.exists() {
            std::fs::write(p, t)?;
        }
    }
    Ok(())
}

pub fn read_prefs(root: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(prefs_path(root)) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn write_prefs(root: &Path, text: &str) -> std::io::Result<()> {
    ensure(root)?;
    std::fs::write(prefs_path(root), text)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MemoryNote {
    /// Nome do arquivo sem `.md` (o alvo de `[[nome]]`).
    pub name: String,
    pub path: PathBuf,
    pub tipo: Option<String>,
    pub projetos: Vec<String>,
    pub atualizado: Option<String>,
    /// Primeira linha de texto depois do frontmatter (título ou resumo).
    pub summary: String,
}

/// Frontmatter mínimo: `chave: valor`, listas `[a, b]` ou `- item` nas linhas seguintes.
fn parse_note(name: String, path: PathBuf, text: &str) -> MemoryNote {
    let mut n = MemoryNote { name, path, tipo: None, projetos: vec![], atualizado: None, summary: String::new() };
    let mut body = text;
    if let Some(rest) = text.strip_prefix("---").and_then(|r| r.strip_prefix('\n').or_else(|| r.strip_prefix("\r\n"))) {
        if let Some(end) = rest.find("\n---") {
            let fm = &rest[..end];
            body = rest[end + 4..].trim_start_matches(['-', '\r', '\n']);
            let mut key = String::new();
            for line in fm.lines() {
                let t = line.trim();
                if let Some(item) = t.strip_prefix("- ") {
                    if key == "projetos" {
                        n.projetos.push(unquote(item));
                    }
                    continue;
                }
                let Some((k, v)) = t.split_once(':') else { continue };
                key = k.trim().to_string();
                let v = v.trim();
                match key.as_str() {
                    "tipo" if !v.is_empty() => n.tipo = Some(unquote(v)),
                    "atualizado" if !v.is_empty() => n.atualizado = Some(unquote(v)),
                    "projetos" if !v.is_empty() => {
                        let v = v.trim_start_matches('[').trim_end_matches(']');
                        n.projetos.extend(v.split(',').map(unquote).filter(|s| !s.is_empty()));
                    }
                    _ => {}
                }
            }
        }
    }
    n.summary = body
        .lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect();
    n
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches(['"', '\'']).trim_start_matches("[[").trim_end_matches("]]").to_string()
}

/// Notas de `<raiz>/_Argus/Memoria/`, as mais recentes primeiro.
pub fn list_notes(root: &Path) -> std::io::Result<Vec<MemoryNote>> {
    let rd = match std::fs::read_dir(notes_dir(root)) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    let mut v = vec![];
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() || p.extension().is_none_or(|x| !x.eq_ignore_ascii_case("md")) {
            continue;
        }
        let name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        v.push(parse_note(name, p, &text));
    }
    v.sort_by(|a, b| b.atualizado.cmp(&a.atualizado).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(v)
}

/// `path` está dentro de `<raiz>/Argus`? (para abrir no Obsidian só o que é da memória)
pub fn inside(root: &Path, path: &Path) -> bool {
    let (Ok(d), Ok(p)) = (dir(root).canonicalize(), path.canonicalize()) else { return false };
    p.starts_with(d)
}

/// `obsidian://open?path=<caminho absoluto codificado>`.
pub fn obsidian_url(path: &Path) -> String {
    let mut out = String::from("obsidian://open?path=");
    for b in path.to_string_lossy().replace('\\', "/").bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/' | b':') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_points_to_files_and_is_stable() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let proj = root.join("Loja");
        std::fs::create_dir(&proj).unwrap();
        assert_eq!(note(root, &proj), None, "sem a pasta Argus, sem aviso");

        ensure(root).unwrap();
        let a = note(root, &proj).unwrap();
        assert!(a.contains(&prefs_path(root).display().to_string()), "{a}");
        assert!(a.contains(&index_path(root).display().to_string()), "{a}");
        assert!(a.contains(&notes_dir(root).display().to_string()), "{a}");
        assert!(!a.contains('\n') && !a.contains('"'), "vai como argumento: {a}");
        assert!(!a.contains("Loja.md"));

        // O conteúdo das preferências não entra: mudar o arquivo não muda o aviso.
        write_prefs(root, "# Preferências\n\nResponda curto.\n").unwrap();
        assert_eq!(note(root, &proj).unwrap(), a);
        assert!(!a.contains("Responda curto"));

        // A nota do projeto entra quando existe.
        std::fs::write(root.join("Loja.md"), "# Loja\n").unwrap();
        let b = note(root, &proj).unwrap();
        assert!(b.starts_with(&a) && b.contains(&root.join("Loja.md").display().to_string()), "{b}");
        assert_eq!(note(root, &proj).unwrap(), b);
    }

    #[test]
    fn unsafe_paths_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("a&b");
        ensure(&root).unwrap();
        assert_eq!(note(&root, &root.join("p")), None);
    }

    #[test]
    fn ensure_keeps_existing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        assert_eq!(read_prefs(root).unwrap(), None);
        ensure(root).unwrap();
        assert!(read_prefs(root).unwrap().unwrap().contains("Preferências"));
        std::fs::write(index_path(root), "meu índice").unwrap();
        write_prefs(root, "x").unwrap();
        assert_eq!(std::fs::read_to_string(index_path(root)).unwrap(), "meu índice");
        assert_eq!(read_prefs(root).unwrap().unwrap(), "x");
    }

    #[test]
    fn lists_notes_with_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        ensure(root).unwrap();
        std::fs::write(
            notes_dir(root).join("testes-rust.md"),
            "---\ntipo: decisao\nprojetos: [Loja, \"Caixa\"]\natualizado: 2026-11-10\n---\n# Testes no core\n\nVer [[preferencias]].\n",
        )
        .unwrap();
        std::fs::write(notes_dir(root).join("tom.md"), "---\ntipo: preferencia\nprojetos:\n  - '[[Loja]]'\natualizado: 2026-11-18\n---\n\nRespostas curtas.\n").unwrap();
        std::fs::write(notes_dir(root).join("solta.md"), "Sem frontmatter.").unwrap();
        std::fs::write(notes_dir(root).join("x.txt"), "ignorado").unwrap();
        let v = list_notes(root).unwrap();
        let names: Vec<_> = v.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["tom", "testes-rust", "solta"]);
        assert_eq!(v[1].projetos, ["Loja", "Caixa"]);
        assert_eq!(v[1].summary, "Testes no core");
        assert_eq!((v[0].tipo.as_deref(), v[0].projetos.clone(), v[0].summary.as_str()), (Some("preferencia"), vec!["Loja".to_string()], "Respostas curtas."));
        assert_eq!((v[2].tipo.clone(), v[2].summary.as_str()), (None, "Sem frontmatter."));
    }

    #[test]
    fn obsidian_url_encodes_path() {
        assert_eq!(obsidian_url(Path::new(r"D:\Raiz\_Argus\Memoria\tom da conversa.md")), "obsidian://open?path=D:/Raiz/_Argus/Memoria/tom%20da%20conversa.md");
        assert!(obsidian_url(Path::new("/r/Mem\u{f3}ria.md")).ends_with("Mem%C3%B3ria.md"));
    }

    #[test]
    fn inside_only_accepts_memory_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        ensure(root).unwrap();
        assert!(inside(root, &prefs_path(root)));
        std::fs::write(root.join("fora.md"), "").unwrap();
        assert!(!inside(root, &root.join("fora.md")));
        assert!(!inside(root, &dir(root).join("..").join("fora.md")));
    }
}
