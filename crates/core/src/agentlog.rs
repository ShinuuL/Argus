//! Log bruto de cada turno, em arquivo, fora da conversa.
//!
//! O `opencode run` imprime os eventos do stream no stdout e, com
//! `--print-logs`, os logs da CLI no stderr. Parte dos defeitos em aberto do
//! provedor depende de ver essas linhas cruas (o `step_finish` que falta no
//! último passo, o uso que não entra, o daemon que prende o worktree). O Argus
//! grava as duas saídas em `%APPDATA%/argus/logs/<provedor>-<tarefa>-<turno>.log`,
//! sempre, sem ligar nada na interface.
//!
//! As linhas de log da CLI (`timestamp=... level=... message=...`) não viram
//! evento `Stderr` nem texto no chat: o destino delas é o arquivo. O resto do
//! stderr (erro real da CLI) continua indo para a conversa como antes.
//!
//! Nenhum valor de credencial é gravado: `redact` mascara o que vem depois de
//! chaves como `Authorization`, `token=` ou `password=`.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// `%APPDATA%/argus/logs` (ou a pasta temporária, se não houver APPDATA).
/// O nome da pasta de dados respeita `ARGUS_PROFILE`, como os anexos.
pub fn root() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(crate::paths::dir_name())
        .join("logs")
}

/// Só o que é seguro para nome de arquivo. O provedor vem da tarefa, então
/// barras ou `..` não podem sair da pasta de logs.
pub fn safe_name(s: &str) -> String {
    let out: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let out = out.trim_matches('-').to_ascii_lowercase();
    if out.is_empty() { "agente".into() } else { out }
}

/// Caminho do log de um turno.
pub fn path_for(provider: &str, task: u64, turn: i64) -> PathBuf {
    root().join(format!("{}-{}-{}.log", safe_name(provider), task, turn))
}

/// Linha de log da própria CLI, no formato visto no `opencode` 2.3.0:
/// `timestamp=2026-... level=DEBUG run=... message="..."`.
pub fn is_cli_log(line: &str) -> bool {
    let s = line.trim_start();
    (s.starts_with("timestamp=") || s.starts_with("time=")) && s.contains("level=")
}

const SECRET_KEYS: &[&str] = &[
    "authorization:",
    "x-api-key:",
    "api-key:",
    "apikey:",
    "api_key:",
    "apikey=",
    "api_key=",
    "api-key=",
    "access_token=",
    "refresh_token=",
    "session_token=",
    "token=",
    "password=",
    "passwd=",
    "secret=",
    "client_secret=",
];

fn skip_value(line: &str, mut i: usize) -> usize {
    while i < line.len() {
        let c = line.as_bytes()[i] as char;
        if c.is_ascii_whitespace() || matches!(c, '"' | '\'' | ',' | '}' | ']') {
            break;
        }
        i += 1;
    }
    i
}

/// Copia o valor já mascarado, devolvendo a posição seguinte. Trata valor entre
/// aspas (`"sk-..."`), o par `Bearer <token>` e o valor solto até um delimitador.
fn mask_value(line: &str, mut i: usize, out: &mut String) -> usize {
    while i < line.len() && (line.as_bytes()[i] as char).is_ascii_whitespace() {
        out.push(line.as_bytes()[i] as char);
        i += 1;
    }
    let quoted = line.as_bytes().get(i).copied().map(|b| b as char).filter(|c| matches!(c, '"' | '\''));
    if let Some(q) = quoted {
        out.push(q);
        out.push_str("<redacted>");
        out.push(q);
        i += 1;
        while i < line.len() && line.as_bytes()[i] as char != q {
            i += 1;
        }
        return usize::from(i < line.len()) + i;
    }
    if line[i..].to_ascii_lowercase().starts_with("bearer ") {
        out.push_str("<redacted>");
        return skip_value(line, i + 7);
    }
    out.push_str("<redacted>");
    skip_value(line, i)
}

/// Mascara valores de credenciais sem depender de regex. A busca é feita numa
/// cópia em minúsculas (ASCII mantém o mesmo tamanho em bytes), a cópia de
/// saída preserva o texto original.
pub fn redact(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let mut out = String::with_capacity(line.len() + 32);
    let mut i = 0;
    while i < line.len() {
        let rest = &lower[i..];
        if let Some(key) = SECRET_KEYS.iter().find(|k| rest.starts_with(**k)) {
            let n = key.len();
            out.push_str(&line[i..i + n]);
            i = mask_value(line, i + n, &mut out);
            continue;
        }
        if rest.starts_with("bearer ") {
            out.push_str(&line[i..i + 7]);
            out.push_str("<redacted>");
            i = skip_value(line, i + 7);
            continue;
        }
        let ch = line[i..].chars().next().unwrap_or('?');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Escritor compartilhado pelas leituras de stdout e stderr de um turno.
/// Um arquivo só, na ordem em que as linhas chegam. Falha de disco nunca
/// derruba o turno: log é diagnóstico, não parte da conversa.
#[derive(Clone)]
pub struct TurnLog {
    file: Arc<Mutex<Option<BufWriter<File>>>>,
    path: PathBuf,
}

impl TurnLog {
    pub fn new(provider: &str, task: u64, turn: i64) -> Self {
        Self::with_path(path_for(provider, task, turn))
    }

    /// Mesmo escritor, com caminho escolhido (usado nos testes).
    pub fn with_path(path: PathBuf) -> Self {
        let file = std::fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))
            .ok()
            .and_then(|_| OpenOptions::new().create(true).append(true).open(&path).ok())
            .map(BufWriter::new);
        Self { file: Arc::new(Mutex::new(file)), path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Grava uma linha com rótulo de origem (`meta`, `cmd`, `out`, `err`).
    /// Faz flush a cada linha para o conteúdo sobreviver a cancelamento ou
    /// encerramento abruptos — justamente os casos que o log ajuda a explicar.
    pub fn line(&self, stream: &str, text: &str) {
        let mut guard = self.file.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(f) = guard.as_mut() {
            let _ = writeln!(f, "{} {} {}", now(), stream, redact(text));
            let _ = f.flush();
        }
    }

    /// Grava uma medição de latência (`prepare_ms`, `mcp_ms`, `spawn_ms`,
    /// `first_event_ms`, `result_ms`) na mesma ordem das linhas do turno.
    pub fn latency(&self, phase: &str, ms: u128) {
        self.line("lat", &format!("{phase}_ms={ms}"));
    }
}

fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Relógio de um turno. `Instant` não é serializável: os tempos viram linhas
/// `lat` no arquivo do turno, nunca entram na conversa nem no banco.
#[derive(Debug, Clone, Copy)]
pub struct TurnTimer {
    start: Instant,
}

impl TurnTimer {
    pub fn new() -> Self {
        Self { start: Instant::now() }
    }

    /// Milissegundos desde o começo do turno.
    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }
}

impl Default for TurnTimer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_name_keeps_only_path_safe_characters() {
        assert_eq!(safe_name("OpenCode"), "opencode");
        assert_eq!(safe_name("../claude/.."), "claude");
        assert_eq!(safe_name("  "), "agente");
    }

    #[test]
    fn cli_log_line_is_recognized() {
        let real = r#"timestamp=2026-11-21T15:47:28.804Z level=DEBUG run=63763b15 message="spawning process" role=cli"#;
        assert!(is_cli_log(real));
        assert!(!is_cli_log(r#"{"type":"text","text":"oi"}"#));
        assert!(!is_cli_log("Error: não achei o arquivo"));
    }

    #[test]
    fn redact_hides_credentials_but_keeps_the_rest() {
        assert_eq!(redact("Bearer abc123 terminou"), "Bearer <redacted> terminou");
        assert_eq!(redact("Authorization: Bearer abc123"), "Authorization: <redacted>");
        assert_eq!(redact("token=abc123 cwd=/tmp"), "token=<redacted> cwd=/tmp");
        assert_eq!(redact(r#"api_key: "sk-xyz""#), r#"api_key: "<redacted>""#);
        assert_eq!(redact("mensagem sem segredo"), "mensagem sem segredo");
    }

    #[test]
    fn writes_stdout_and_stderr_to_the_same_file() {
        let dir = std::env::temp_dir().join(format!("argus-log-{}", std::process::id()));
        let path = dir.join("turno.log");
        let _ = std::fs::remove_file(&path);
        let log = TurnLog::with_path(path.clone());
        log.line("out", r#"{"type":"text","text":"oi"}"#);
        log.line("err", "timestamp=2026-11-21T15:47:28.804Z level=DEBUG message=\"spawning process\"");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains(r#"out {"type":"text","text":"oi"}"#), "{content}");
        assert!(content.contains("err timestamp=2026-11-21T15:47:28.804Z level=DEBUG"), "{content}");
        assert_eq!(content.lines().count(), 2);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn latency_lines_record_phase_and_milliseconds() {
        let dir = std::env::temp_dir().join(format!("argus-lat-{}", std::process::id()));
        let path = dir.join("turno.log");
        let _ = std::fs::remove_file(&path);
        let log = TurnLog::with_path(path.clone());
        log.latency("mcp", 3);
        log.latency("spawn", 17);
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("lat mcp_ms=3"), "{content}");
        assert!(content.contains("lat spawn_ms=17"), "{content}");
        assert_eq!(content.lines().count(), 2);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn timer_starts_near_zero() {
        assert!(TurnTimer::new().elapsed_ms() < 1_000);
    }

    #[test]
    fn file_name_uses_provider_task_and_turn() {
        let p = path_for("OpenCode", 42, 1732200000123);
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "opencode-42-1732200000123.log");
        assert_eq!(p.parent().unwrap().file_name().unwrap(), "logs");
    }
}
