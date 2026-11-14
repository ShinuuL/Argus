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
            projects.push(Project { name, path, kinds });
        }
    }
    projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(projects)
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
}
