//! Comandos do chat (caixa de composição rica da aba Sessão e da aba Chat).
//!
//! A interface chama estes comandos por `src/views/chat/backend.ts`.
//!
//! Nada aqui lê credenciais: só nomes de arquivos do projeto, o cabeçalho dos
//! arquivos de agente (`.claude/agents/*.md`, `.opencode/agents/*.md`) e a
//! pasta de anexos do próprio Argus.
use super::{blocking, err};
use base64::Engine;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

// ------------------------------------------------------------ @arquivo

/// Pastas que nunca entram na busca de menções.
const EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "out",
    "dist",
    ".argus-worktrees",
    ".next",
    ".venv",
    "__pycache__",
];
/// Teto de arquivos listados por projeto (evita travar em árvores enormes).
const MAX_FILES: usize = 30_000;
/// Validade da lista de arquivos em memória.
const FILES_TTL: Duration = Duration::from_secs(20);

type FileCache = Mutex<HashMap<PathBuf, (Instant, Arc<Vec<String>>)>>;

fn file_cache() -> &'static FileCache {
    static CACHE: OnceLock<FileCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Caminhos relativos (com `/`) de todos os arquivos de `root`, sem as pastas excluídas.
pub fn list_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if ft.is_dir() {
                if !EXCLUDED_DIRS.iter().any(|x| name.eq_ignore_ascii_case(x)) {
                    stack.push(entry.path());
                }
            } else if let Ok(rel) = entry.path().strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
                if out.len() >= MAX_FILES {
                    return out;
                }
            }
        }
    }
    out
}

/// Pontua um caminho para a busca: nome que começa igual vale mais que trecho
/// no meio do caminho; por último, letras na ordem ("extts" acha "extension.ts").
pub fn score(rel: &str, query: &str) -> i64 {
    let target = rel.to_lowercase();
    let name = target.rsplit('/').next().unwrap_or(&target);
    let q = query.to_lowercase().replace('\\', "/");
    if q.is_empty() {
        return 1;
    }
    let name_len = name.chars().count() as i64;
    if name == q {
        return 1000;
    }
    if name.starts_with(&q) {
        return 800 - name_len;
    }
    if name.contains(&q) {
        return 600 - name_len;
    }
    if target.contains(&q) {
        return 400 - (target.chars().count() as i64) / 10;
    }
    let mut wanted = q.chars().peekable();
    for c in name.chars() {
        if wanted.peek() == Some(&c) {
            wanted.next();
        }
    }
    if wanted.peek().is_none() {
        return 200 - name_len;
    }
    0
}

/// Os melhores caminhos para a busca, em ordem. Rasos primeiro no empate.
pub fn search(files: &[String], query: &str, limit: usize) -> Vec<String> {
    let mut found: Vec<(i64, usize, &String)> = files
        .iter()
        .filter_map(|rel| {
            let s = score(rel, query);
            (s > 0).then(|| (s, rel.matches('/').count(), rel))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));
    found.into_iter().take(limit).map(|(_, _, rel)| rel.clone()).collect()
}

/// Busca arquivos de `project` (o diretório de trabalho da tarefa) para a menção `@`.
/// Devolve caminhos relativos com `/`. A lista fica 20 s em memória.
#[tauri::command]
pub async fn chat_project_files(project: PathBuf, query: String, limit: Option<usize>) -> Result<Vec<String>, String> {
    blocking(move || {
        if !project.is_dir() {
            return Err("Pasta do projeto não encontrada.".into());
        }
        let limit = limit.unwrap_or(30).clamp(1, 200);
        let cached = {
            let cache = file_cache().lock().map_err(err)?;
            cache.get(&project).filter(|(at, _)| at.elapsed() < FILES_TTL).map(|(_, f)| f.clone())
        };
        let files = match cached {
            Some(f) => f,
            None => {
                let f = Arc::new(list_files(&project));
                let mut cache = file_cache().lock().map_err(err)?;
                cache.retain(|_, (at, _)| at.elapsed() < FILES_TTL);
                cache.insert(project.clone(), (Instant::now(), f.clone()));
                f
            }
        };
        Ok(search(&files, query.trim(), limit))
    })
    .await
}

// ------------------------------------------------------------- @agente

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatAgent {
    pub name: String,
    pub description: String,
    /// "claude" ou "opencode": de qual CLI é a pasta onde o arquivo estava.
    pub provider: String,
    /// "projeto" ou "usuario".
    pub scope: String,
}

/// Lê `name` e `description` do cabeçalho (`--- ... ---`) de um arquivo de agente.
pub fn parse_agent_header(text: &str) -> (Option<String>, Option<String>) {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    let (mut name, mut description) = (None, None);
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim().trim_matches(|c| c == '"' || c == '\'').trim().to_string();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "name" => name = Some(value),
            "description" => description = Some(value),
            _ => {}
        }
    }
    (name, description)
}

fn read_head(path: &Path, max: usize) -> String {
    use std::io::Read;
    let mut buf = vec![0u8; max];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut buf)).unwrap_or(0);
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

fn agents_in(dir: &Path, provider: &str, scope: &str, out: &mut Vec<ChatAgent>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("md")) != Some(true) {
            continue;
        }
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
        let (name, description) = parse_agent_header(&read_head(&path, 4096));
        let name = name.unwrap_or(stem);
        if out.iter().any(|a| a.name == name) {
            continue;
        }
        let mut description = description.unwrap_or_default();
        if description.chars().count() > 200 {
            description = description.chars().take(200).collect::<String>() + "…";
        }
        out.push(ChatAgent { name, description, provider: provider.into(), scope: scope.into() });
    }
}

/// Agentes definidos em arquivos do projeto e da pasta do usuário
/// (`.claude/agents`, `.opencode/agents`). Os do projeto vêm primeiro.
#[tauri::command]
pub async fn chat_agents(project: Option<PathBuf>) -> Result<Vec<ChatAgent>, String> {
    blocking(move || {
        let mut out = Vec::new();
        if let Some(p) = project.filter(|p| p.is_dir()) {
            agents_in(&p.join(".claude").join("agents"), "claude", "projeto", &mut out);
            agents_in(&p.join(".opencode").join("agents"), "opencode", "projeto", &mut out);
            agents_in(&p.join(".opencode").join("agent"), "opencode", "projeto", &mut out);
        }
        if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from) {
            agents_in(&home.join(".claude").join("agents"), "claude", "usuario", &mut out);
            agents_in(&home.join(".config").join("opencode").join("agents"), "opencode", "usuario", &mut out);
            agents_in(&home.join(".config").join("opencode").join("agent"), "opencode", "usuario", &mut out);
        }
        Ok(out)
    })
    .await
}

// --------------------------------------------------------------- anexos

/// Tamanho máximo de um anexo gravado.
pub const MAX_ATTACHMENT: usize = argus_core::attachments::MAX_BYTES;
/// Anexos mais velhos que isto são apagados por `chat_attachments_cleanup`.
const ATTACHMENT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Serialize)]
pub struct SavedAttachment {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttachmentData {
    pub mime: String,
    /// Conteúdo em base64.
    pub data: String,
}

/// `%APPDATA%/argus/anexos` (ou a pasta temporária, se não houver APPDATA).
/// A mesma pasta que o core libera ao agente com `--add-dir`.
pub fn attachments_root() -> PathBuf {
    argus_core::attachments::root()
}

/// Nome de arquivo seguro (a mesma regra do core, usada também pelo celular).
pub fn sanitize_name(name: &str) -> String {
    argus_core::attachments::sanitize_name(name)
}

fn save_attachment_in(
    root: &Path,
    task: Option<u64>,
    draft: Option<&str>,
    name: &str,
    bytes: &[u8],
) -> Result<SavedAttachment, String> {
    let (path, size) = argus_core::attachments::save(root, task, draft, name, bytes)?;
    Ok(SavedAttachment { path, size })
}

/// Grava um anexo (conteúdo em base64) na pasta de anexos da tarefa e devolve o caminho.
/// `task` vazio = conversa ainda não criada: vai para `novo/<draft>`, a pasta só
/// daquela aba (o agente de uma conversa não enxerga anexos de outra).
#[tauri::command]
pub async fn chat_attachment_save(
    task: Option<u64>,
    draft: Option<String>,
    name: String,
    data: String,
) -> Result<SavedAttachment, String> {
    blocking(move || {
        // 4 caracteres de base64 = 3 bytes: recusa antes de decodificar.
        if data.len() / 4 * 3 > MAX_ATTACHMENT + 3 {
            return Err(format!("{name}: anexo acima de {} MB.", MAX_ATTACHMENT / 1024 / 1024));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|_| format!("{name}: conteúdo inválido."))?;
        save_attachment_in(&attachments_root(), task, draft.as_deref(), &name, &bytes)
    })
    .await
}

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn read_attachment_in(root: &Path, path: &Path) -> Result<AttachmentData, String> {
    let denied = || "Só é possível ler imagens da pasta de anexos do Argus.".to_string();
    let root = root.canonicalize().map_err(|_| denied())?;
    let real = path.canonicalize().map_err(|_| "Anexo não encontrado.".to_string())?;
    let mime = mime_for(&real);
    if !real.starts_with(&root) || !mime.starts_with("image/") {
        return Err(denied());
    }
    let meta = std::fs::metadata(&real).map_err(err)?;
    if !meta.is_file() || meta.len() as usize > MAX_ATTACHMENT {
        return Err(denied());
    }
    let bytes = std::fs::read(&real).map_err(err)?;
    Ok(AttachmentData { mime: mime.into(), data: base64::engine::general_purpose::STANDARD.encode(bytes) })
}

/// Lê uma imagem da pasta de anexos (para a miniatura na conversa). Qualquer
/// caminho fora dela, ou que não seja imagem, é recusado.
#[tauri::command]
pub async fn chat_attachment_read(path: PathBuf) -> Result<AttachmentData, String> {
    blocking(move || read_attachment_in(&attachments_root(), &path)).await
}

fn cleanup_in(root: &Path, max_age: Duration) -> u32 {
    let now = SystemTime::now();
    let mut removed = 0;
    let Ok(entries) = std::fs::read_dir(root) else { return 0 };
    for entry in entries.flatten() {
        // não segue atalhos: só pastas de verdade (`<tarefa>`, `novo` e `novo/<rascunho>`)
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            removed += cleanup_dir(&entry.path(), max_age, now, 2);
        }
    }
    removed
}

/// Apaga os arquivos velhos de `dir` e, até `depth` níveis abaixo, os das
/// subpastas; pastas que ficam vazias saem.
fn cleanup_dir(dir: &Path, max_age: Duration, now: SystemTime, depth: u32) -> u32 {
    let mut removed = 0;
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            if depth > 1 {
                removed += cleanup_dir(&path, max_age, now, depth - 1);
            }
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age > max_age);
        if old && kind.is_file() && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    // só sai se ficou vazia
    let _ = std::fs::remove_dir(dir);
    removed
}

/// Apaga anexos com mais de 1 dia. Devolve quantos arquivos saíram.
/// Chame uma vez na abertura do app.
#[tauri::command]
pub async fn chat_attachments_cleanup() -> Result<u32, String> {
    blocking(|| Ok(cleanup_in(&attachments_root(), ATTACHMENT_MAX_AGE))).await
}

// ---------------------------------------------------- comandos de barra

#[derive(Debug, Clone, Default, Serialize)]
pub struct AgentCapabilities {
    /// Comandos de barra da CLI (sem a `/`), incluindo skills e plugins.
    pub slash_commands: Vec<String>,
    /// Nomes dos servidores MCP configurados.
    pub mcp_servers: Vec<String>,
    /// Modelo padrão informado pela CLI.
    pub model: Option<String>,
}

/// Extrai o que interessa da linha `system/init` do stream-json do claude.
pub fn capabilities_from_init(init: &serde_json::Value) -> AgentCapabilities {
    let strings = |key: &str| -> Vec<String> {
        init.get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().or_else(|| x.get("name").and_then(|n| n.as_str())))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    AgentCapabilities {
        slash_commands: strings("slash_commands"),
        mcp_servers: strings("mcp_servers"),
        model: init.get("model").and_then(|m| m.as_str()).map(str::to_string),
    }
}

fn caps_cache() -> &'static Mutex<HashMap<PathBuf, AgentCapabilities>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, AgentCapabilities>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Sobe um `claude -p` que não grava sessão, manda `/context` (respondido
/// localmente pela CLI, sem chamada ao modelo), lê o `init` e mata o processo.
fn probe_claude(dir: &Path) -> Result<AgentCapabilities, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    let exe = argus_core::agents::orchestrator::resolve_program("claude");
    let mut cmd = Command::new(exe);
    cmd.args([
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--no-session-persistence",
    ])
    .current_dir(dir)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("Não foi possível executar o claude: {e}"))?;
    // O stdin fica aberto até o fim: fechá-lo antes do `init` encerraria a CLI.
    let mut stdin = child.stdin.take();
    if let Some(stdin) = stdin.as_mut() {
        let line = serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": [{ "type": "text", "text": "/context" }] }
        });
        let _ = stdin.write_all(format!("{line}\n").as_bytes());
        let _ = stdin.flush();
    }
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("O claude não abriu a saída.".into());
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
            let is = |k: &str, want: &str| v.get(k).and_then(|x| x.as_str()) == Some(want);
            if is("type", "system") && is("subtype", "init") {
                let _ = tx.send(v);
                return;
            }
        }
    });
    let init = rx.recv_timeout(Duration::from_secs(45));
    let _ = child.kill();
    let _ = child.wait();
    drop(stdin);
    let init = init.map_err(|_| "O claude não respondeu a tempo.".to_string())?;
    Ok(capabilities_from_init(&init))
}

/// Comandos de barra, MCPs e modelo que a CLI teria em `project`, sem gastar
/// tokens. Só o claude tem essa consulta; para outros provedores devolve vazio.
/// O resultado fica em memória por projeto; `refresh` força nova consulta.
#[tauri::command]
pub async fn chat_agent_capabilities(
    project: PathBuf,
    provider: Option<String>,
    refresh: Option<bool>,
) -> Result<AgentCapabilities, String> {
    if provider.as_deref().is_some_and(|p| p != "claude") {
        return Ok(AgentCapabilities::default());
    }
    blocking(move || {
        if !project.is_dir() {
            return Err("Pasta do projeto não encontrada.".into());
        }
        if refresh != Some(true) {
            if let Some(c) = caps_cache().lock().map_err(err)?.get(&project) {
                return Ok(c.clone());
            }
        }
        let caps = probe_claude(&project)?;
        caps_cache().lock().map_err(err)?.insert(project, caps.clone());
        Ok(caps)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_ranks_name_matches_first() {
        assert_eq!(score("src/extension.ts", "extension.ts"), 1000);
        assert!(score("src/extension.ts", "ext") > score("src/next/a.ts", "ext"));
        assert!(score("src/a/extension.ts", "src/a") > 0);
        assert!(score("src/extension.ts", "extts") > 0, "letras na ordem");
        assert_eq!(score("src/extension.ts", "zzz"), 0);
        assert_eq!(score("qualquer", ""), 1);
    }

    #[test]
    fn search_prefers_shallow_on_tie() {
        let files = vec!["a/b/main.rs".to_string(), "main.rs".to_string(), "lib.rs".to_string()];
        assert_eq!(search(&files, "main.rs", 10), vec!["main.rs", "a/b/main.rs"]);
        assert_eq!(search(&files, "", 2).len(), 2);
    }

    #[test]
    fn list_files_skips_excluded_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/x")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("src/a.ts"), "").unwrap();
        std::fs::write(root.join("node_modules/x/b.js"), "").unwrap();
        std::fs::write(root.join(".git/HEAD"), "").unwrap();
        std::fs::write(root.join(".env.example"), "").unwrap();
        let mut files = list_files(root);
        files.sort();
        assert_eq!(files, vec![".env.example", "src/a.ts"]);
    }

    #[test]
    fn agent_header_reads_name_and_description() {
        let text = "---\nname: revisor\ndescription: \"Revisa o código\"\ntools: Read\n---\ncorpo\nname: outro";
        assert_eq!(parse_agent_header(text), (Some("revisor".into()), Some("Revisa o código".into())));
        assert_eq!(parse_agent_header("sem cabeçalho\nname: x"), (None, None));
    }

    #[test]
    fn names_are_sanitized() {
        assert_eq!(sanitize_name("..\\..\\Windows\\system32\\a.dll"), "a.dll");
        assert_eq!(sanitize_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_name("print 1 (2).png"), "print 1 (2).png");
        assert_eq!(sanitize_name("a:b*c?.txt"), "a_b_c_.txt");
        assert_eq!(sanitize_name(".."), "anexo");
        let long = sanitize_name(&format!("{}.png", "x".repeat(200)));
        assert_eq!(long.chars().count(), 80);
        assert!(long.ends_with(".png"));
    }

    #[test]
    fn attachments_save_read_and_cleanup() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("anexos");
        let a = save_attachment_in(&root, Some(7), None, "print.png", b"abc").unwrap();
        let b = save_attachment_in(&root, Some(7), None, "print.png", b"abcd").unwrap();
        assert!(a.path.ends_with("7/print.png") || a.path.ends_with("7\\print.png"));
        assert_ne!(a.path, b.path, "não sobrescreve");
        assert_eq!(b.size, 4);
        assert!(save_attachment_in(&root, None, Some("r1"), "g.bin", &vec![0u8; MAX_ATTACHMENT + 1]).is_err());

        let read = read_attachment_in(&root, &a.path).unwrap();
        assert_eq!(read.mime, "image/png");
        assert_eq!(read.data, "YWJj");
        // fora da pasta de anexos, ou não imagem: recusado
        let outside = tmp.path().join("fora.png");
        std::fs::write(&outside, b"x").unwrap();
        assert!(read_attachment_in(&root, &outside).is_err());
        assert!(read_attachment_in(&root, &root.join("7").join("..").join("..").join("fora.png")).is_err());
        let txt = save_attachment_in(&root, Some(7), None, "nota.txt", b"oi").unwrap();
        assert!(read_attachment_in(&root, &txt.path).is_err());

        // conversa nova: cada aba na sua subpasta de rascunho
        let d1 = save_attachment_in(&root, None, Some("aba1"), "p.png", b"x").unwrap();
        let d2 = save_attachment_in(&root, None, Some("aba2"), "p.png", b"y").unwrap();
        assert_eq!(d1.path, root.join("novo").join("aba1").join("p.png"));
        assert_eq!(d2.path, root.join("novo").join("aba2").join("p.png"));
        assert!(read_attachment_in(&root, &d1.path).is_ok());
        assert!(save_attachment_in(&root, None, None, "p.png", b"x").is_err(), "sem tarefa nem rascunho");
        assert!(save_attachment_in(&root, None, Some("../fora"), "p.png", b"x").is_err());

        assert_eq!(cleanup_in(&root, Duration::from_secs(3600)), 0, "arquivos novos ficam");
        assert_eq!(cleanup_in(&root, Duration::ZERO), 5);
        assert!(!root.join("7").exists(), "pasta vazia é removida");
        assert!(!root.join("novo").exists(), "rascunhos vazios e a pasta novo saem");
    }

    #[test]
    fn capabilities_come_from_init() {
        let init = serde_json::json!({
            "type": "system", "subtype": "init", "model": "modelo-x",
            "slash_commands": ["compact", "context"],
            "mcp_servers": [{ "name": "docs", "status": "connected" }]
        });
        let c = capabilities_from_init(&init);
        assert_eq!(c.slash_commands, vec!["compact", "context"]);
        assert_eq!(c.mcp_servers, vec!["docs"]);
        assert_eq!(c.model.as_deref(), Some("modelo-x"));
    }
}
