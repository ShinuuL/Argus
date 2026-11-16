//! Varredura da pasta raiz e detecção de projetos.
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ProjectKind {
    Rust,
    Node,
    Python,
    Go,
    Java,
    Git,
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub kinds: Vec<ProjectKind>,
    /// A pasta não existe mais no disco, mas o projeto ainda tem sessões.
    #[serde(default)]
    pub missing: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("pasta raiz inválida: {0}")]
    InvalidRoot(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

const MARKERS: &[(&str, ProjectKind)] = &[
    ("Cargo.toml", ProjectKind::Rust),
    ("package.json", ProjectKind::Node),
    ("pyproject.toml", ProjectKind::Python),
    ("requirements.txt", ProjectKind::Python),
    ("go.mod", ProjectKind::Go),
    ("pom.xml", ProjectKind::Java),
    ("build.gradle", ProjectKind::Java),
    (".git", ProjectKind::Git),
];

/// Detecta os tipos de projeto de uma pasta (vazio = não é projeto).
pub fn detect_kinds(dir: &Path) -> Vec<ProjectKind> {
    let mut kinds = Vec::new();
    for (file, kind) in MARKERS {
        if dir.join(file).exists() && !kinds.contains(kind) {
            kinds.push(kind.clone());
        }
    }
    kinds
}

/// Lista os projetos que são filhos diretos da pasta raiz, ordenados por nome.
pub fn scan(root: &Path) -> Result<Vec<Project>, WorkspaceError> {
    if !root.is_dir() {
        return Err(WorkspaceError::InvalidRoot(root.to_path_buf()));
    }
    let mut projects = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !path.is_dir() || name.starts_with('.') || name == "node_modules" {
            continue;
        }
        let kinds = detect_kinds(&path);
        if !kinds.is_empty() {
            projects.push(Project { name, path, kinds, missing: false });
        }
    }
    projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(projects)
}

/// Forma comparável de um caminho (barras iguais; no Windows, sem caixa).
fn key(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

/// Junta aos projetos varridos os de `with_sessions` (projetos que têm sessões)
/// que são filhos diretos de `root` e não apareceram na varredura: a pasta foi
/// apagada (ou deixou de ser projeto). Eles continuam na lista, marcados como
/// `missing` quando a pasta não existe, para as sessões não sumirem junto.
pub fn with_session_projects(root: &Path, mut projects: Vec<Project>, with_sessions: impl IntoIterator<Item = PathBuf>) -> Vec<Project> {
    let root_key = key(root);
    let mut seen: std::collections::HashSet<String> = projects.iter().map(|p| key(&p.path)).collect();
    for path in with_sessions {
        let k = key(&path);
        let child_of_root = path.parent().map(|p| key(p) == root_key).unwrap_or(false);
        if !child_of_root || !seen.insert(k) {
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let missing = !path.is_dir();
        let kinds = if missing { Vec::new() } else { detect_kinds(&path) };
        projects.push(Project { name, path, kinds, missing });
    }
    projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    projects
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scans_projects() {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        fs::create_dir_all(r.join("api/.git")).unwrap();
        fs::write(r.join("api/Cargo.toml"), "").unwrap();
        fs::create_dir(r.join("web")).unwrap();
        fs::write(r.join("web/package.json"), "{}").unwrap();
        fs::create_dir(r.join("vazio")).unwrap();
        fs::create_dir(r.join(".oculto")).unwrap();
        fs::write(r.join(".oculto/Cargo.toml"), "").unwrap();

        let p = scan(r).unwrap();
        assert_eq!(p.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["api", "web"]);
        assert_eq!(p[0].kinds, vec![ProjectKind::Rust, ProjectKind::Git]);
    }

    #[test]
    fn invalid_root() {
        assert!(scan(Path::new("Z:/nao/existe")).is_err());
    }

    #[test]
    fn keeps_projects_with_sessions_when_folder_is_gone() {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        fs::create_dir(r.join("web")).unwrap();
        fs::write(r.join("web/package.json"), "{}").unwrap();
        let scanned = scan(r).unwrap();
        let p = with_session_projects(
            r,
            scanned,
            [r.join("web"), r.join("apagado"), Path::new("Z:/fora/da/raiz").to_path_buf()],
        );
        let names: Vec<(&str, bool)> = p.iter().map(|p| (p.name.as_str(), p.missing)).collect();
        assert_eq!(names, [("apagado", true), ("web", false)]);
    }
}
