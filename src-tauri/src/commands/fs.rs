//! Arquivos: ler, gravar e listar diretório (para a árvore do Explorador e o editor).
use super::{blocking, err};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Acima disso o arquivo não é aberto no editor (evita travar a interface).
const MAX_OPEN_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileStat {
    pub exists: bool,
    pub is_dir: bool,
    pub size: u64,
    /// Segundos desde a época Unix (0 se desconhecido).
    pub modified: u64,
}

/// Pastas primeiro, depois por nome. Sem `show_hidden`, oculta `.algo`,
/// `node_modules` e `target` (igual à árvore antiga).
pub fn list_dir(dir: &Path, show_hidden: bool) -> std::io::Result<Vec<DirEntry>> {
    let mut v: Vec<DirEntry> = std::fs::read_dir(dir)?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !show_hidden && (name.starts_with('.') || name == "node_modules" || name == "target") {
                return None;
            }
            let path = e.path();
            let is_dir = path.is_dir();
            Some(DirEntry { name, path, is_dir })
        })
        .collect();
    v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(v)
}

/// Lê um arquivo como texto (UTF-8 com substituição de bytes inválidos).
#[tauri::command]
pub async fn fs_read_file(path: PathBuf) -> Result<String, String> {
    blocking(move || {
        let size = std::fs::metadata(&path).map_err(|e| format!("Erro ao abrir {}: {e}", path.display()))?.len();
        if size > MAX_OPEN_BYTES {
            return Err(format!("{} é grande demais para abrir no editor ({} MB).", path.display(), size / 1_048_576));
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("Erro ao abrir {}: {e}", path.display()))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    })
    .await
}

/// Grava o texto no arquivo (substitui o conteúdo).
#[tauri::command]
pub async fn fs_write_file(path: PathBuf, content: String) -> Result<(), String> {
    blocking(move || std::fs::write(&path, content).map_err(|e| format!("Erro ao salvar {}: {e}", path.display()))).await
}

/// Lista um diretório (pastas primeiro).
#[tauri::command]
pub async fn fs_list_dir(path: PathBuf, show_hidden: Option<bool>) -> Result<Vec<DirEntry>, String> {
    blocking(move || list_dir(&path, show_hidden.unwrap_or(false)).map_err(err)).await
}

/// Existência, tipo, tamanho e data de modificação de um caminho.
#[tauri::command]
pub async fn fs_stat(path: PathBuf) -> Result<FileStat, String> {
    blocking(move || {
        Ok(match std::fs::metadata(&path) {
            Ok(m) => FileStat {
                exists: true,
                is_dir: m.is_dir(),
                size: m.len(),
                modified: m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            },
            Err(_) => FileStat { exists: false, is_dir: false, size: 0, modified: 0 },
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_dir_sorts_and_hides() {
        let d = tempfile::tempdir().unwrap();
        for dir in ["src", ".git", "node_modules", "target"] {
            std::fs::create_dir(d.path().join(dir)).unwrap();
        }
        for f in ["b.txt", "A.txt", ".env"] {
            std::fs::write(d.path().join(f), b"").unwrap();
        }
        let names = |hidden| list_dir(d.path(), hidden).unwrap().into_iter().map(|e| e.name).collect::<Vec<_>>();
        assert_eq!(names(false), ["src", "A.txt", "b.txt"]);
        assert_eq!(names(true), [".git", "node_modules", "src", "target", ".env", "A.txt", "b.txt"]);
        assert!(list_dir(&d.path().join("nao-existe"), false).is_err());
    }
}
