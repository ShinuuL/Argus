//! Utilitários compartilhados pelo editor Monaco do workbench (assets, linguagem, ids DOM).

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

/// Gera um id DOM único para cada instância de componente.
pub(crate) fn unique_dom_id(prefix: &str) -> String {
    format!("{prefix}-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

/// URL base (servida pelo protocolo do Dioxus desktop) da pasta `assets`.
///
/// App empacotado: `/assets` (pasta ao lado do executável). Em `cargo run`:
/// caminho absoluto de `crates/app/assets`, que o resolvedor do Dioxus aceita
/// (`/D:/...` vira `D:/...` ao ser unido à raiz de assets).
pub fn assets_base_url() -> String {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("assets").join("monaco")))
        .is_some_and(|p| p.exists());
    if bundled {
        return "/assets".into();
    }
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets").replace('\\', "/");
    let enc = dir
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('#', "%23");
    if enc.starts_with('/') {
        enc
    } else {
        format!("/{enc}")
    }
}

/// Linguagem do Monaco a partir da extensão.
pub fn language_for(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "mts" | "cts" | "tsx" => "typescript",
        "json" => "json",
        "toml" | "ini" => "ini",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" => "cpp",
        "cs" => "csharp",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "md" => "markdown",
        "yml" | "yaml" => "yaml",
        "xml" => "xml",
        "sql" => "sql",
        "sh" | "bash" => "shell",
        "ps1" | "psm1" => "powershell",
        _ => match path.file_name().and_then(|n| n.to_str()) {
            Some("Dockerfile") => "dockerfile",
            _ => "plaintext",
        },
    }
}

