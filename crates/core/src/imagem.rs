//! Geração de imagem como fallback do agente.
//!
//! O Argus não lê token das CLIs: a chave usada aqui é exclusivamente a do
//! Google AI Studio. Ela vem de `GEMINI_API_KEY` / `GOOGLE_API_KEY` no ambiente
//! ou do campo "Imagens" da interface; nesse caso fica só na memória do
//! processo e some ao reiniciar. Nunca é gravada, exibida ou enviada em
//! argumento de processo: vai pela stdin do `curl`, como header `x-goog-api-key`.
//!
//! O transporte usa o `curl` do sistema para não acrescentar dependência de
//! TLS ao `argus-core` (o `reqwest` do `Cargo.lock` vem do `tauri`, sem
//! backend TLS no grafo). A resposta pode vir tanto do endpoint `interactions`
//! quanto de `generateContent`; a leitura aceita os dois formatos.
//!
//! Atenção honesta: os modelos de imagem do Gemini não estão no free tier
//! (documentação consultada em 2026-11-26). Esta função é um fallback pago,
//! sempre disparado pelo usuário. Os agentes continuam com login das CLIs
//! oficiais e com o que o plano free deles permite.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Ordem de preferência das variáveis de ambiente com a chave.
pub const KEY_VARS: [&str; 2] = ["GEMINI_API_KEY", "GOOGLE_API_KEY"];
/// Modelo atual recomendado pela documentação do Gemini para imagem.
pub const MODELO_PADRAO: &str = "gemini-nano-banana-2.1";
const ENDPOINT_PADRAO: &str = "https://generativelanguage.googleapis.com/v1beta/interactions";
const SOURCE_UI: &str = "tela";

/// Chave colada na interface, válida só enquanto o processo está aberto.
static CHAVE_TELA: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// Estado mostrado na interface. `source` é a origem (nome da variável ou
/// "tela"), nunca o valor da chave.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImagemStatus {
    pub configured: bool,
    pub source: String,
    pub from_ui: bool,
    pub model: String,
    pub dir: String,
    pub hint: String,
}

/// Imagem recém-gerada. `data` é base64 para o chat poder anexá-la sem reler
/// o arquivo; o caminho fica na galeria local.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImagemGerada {
    pub path: PathBuf,
    pub name: String,
    pub bytes: u64,
    pub mime: String,
    pub data: String,
    /// Texto que o modelo devolveu junto (pode ser vazio).
    pub text: String,
}

/// Item da galeria. A imagem em si vem por `read`, com caminho validado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImagemArquivo {
    pub path: PathBuf,
    pub name: String,
    pub bytes: u64,
    pub modified: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImagemDados {
    pub mime: String,
    pub data: String,
}

/// `%APPDATA%/<perfil>/imagens` (ou a pasta temporária, se não houver APPDATA).
pub fn root() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(crate::paths::dir_name())
        .join("imagens")
}

fn ui_key() -> Option<String> {
    CHAVE_TELA
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|g| g.as_ref().cloned())
}

/// Define a chave colada na tela. Vale só até o Argus reiniciar; nada é gravado.
pub fn set_key_from_ui(key: String) -> Result<(), String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Cole uma chave válida do Google AI Studio.".into());
    }
    let slot = CHAVE_TELA.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().map_err(|_| "Não foi possível guardar a chave nesta sessão.".to_string())?;
    *guard = Some(key);
    Ok(())
}

/// Esquece a chave da tela. A variável de ambiente continua valendo, se houver.
pub fn clear_key_from_ui() {
    if let Some(slot) = CHAVE_TELA.get() {
        if let Ok(mut guard) = slot.lock() {
            *guard = None;
        }
    }
}

/// Chave e origem. A da tela tem precedência; nada é logado ou devolvido na UI.
fn key() -> Option<(String, &'static str)> {
    if let Some(k) = ui_key() {
        return Some((k, SOURCE_UI));
    }
    KEY_VARS.iter().find_map(|v| std::env::var(v).ok().map(|k| (k, *v))).filter(|(k, _)| !k.trim().is_empty())
}

fn model() -> String {
    std::env::var("ARGUS_IMAGEM_MODELO").ok().filter(|m| !m.trim().is_empty()).unwrap_or_else(|| MODELO_PADRAO.into())
}

fn endpoint() -> String {
    std::env::var("ARGUS_IMAGEM_ENDPOINT")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| ENDPOINT_PADRAO.into())
}

pub fn status() -> ImagemStatus {
    let (_, source) = key().unzip();
    let configured = source.is_some();
    let from_ui = source == Some(SOURCE_UI);
    let hint = if configured {
        "Modelos de imagem do Gemini são pagos (sem free tier). O comando /imagem só chama a API quando você pede."
            .into()
    } else {
        format!(
            "Cole a chave em Configurações › Imagens (só nesta sessão) ou defina {} no ambiente e reinicie o Argus. Aviso: os modelos de imagem do Gemini são pagos (sem free tier).",
            KEY_VARS[0]
        )
    };
    ImagemStatus {
        configured,
        source: source.unwrap_or_default().into(),
        from_ui,
        model: model(),
        dir: root().to_string_lossy().into_owned(),
        hint,
    }
}

/// Escapa valores para o formato de arquivo de configuração do curl.
fn cfg_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn ext_for_mime(mime: &str) -> &'static str {
    match mime.to_ascii_lowercase().as_str() {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    }
}

/// Procura a primeira imagem na resposta, seja do endpoint `interactions`
/// (`output_image.data`) seja de `generateContent` (`inlineData.data`).
fn find_image(value: &Value) -> Option<(String, String)> {
    match value {
        Value::Object(map) => {
            let mime = map.get("mimeType").or_else(|| map.get("mime_type")).and_then(Value::as_str);
            let data = map.get("data").and_then(Value::as_str);
            if let (Some(mime), Some(data)) = (mime, data) {
                if mime.starts_with("image/") {
                    return Some((mime.to_string(), data.to_string()));
                }
            }
            for (k, v) in map {
                if matches!(k.as_str(), "output_image" | "outputImage" | "inlineData" | "inline_data") {
                    if let Some(found) = find_image(v) {
                        return Some(found);
                    }
                }
            }
            map.values().find_map(find_image)
        }
        Value::Array(items) => items.iter().find_map(find_image),
        _ => None,
    }
}

/// Junta os trechos de texto da resposta (sem incluir a imagem em base64).
fn find_text(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(Value::as_str) {
                out.push(text.to_string());
            }
            for v in map.values() {
                find_text(v, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                find_text(v, out);
            }
        }
        _ => {}
    }
}

fn api_error(value: &Value, raw: &str) -> String {
    let msg = value
        .get("error")
        .and_then(|e| e.get("message"))
        .or_else(|| value.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if msg.is_empty() {
        format!("A API do Gemini não devolveu imagem. Resposta: {}", truncate(raw, 300))
    } else {
        format!("Gemini: {}", truncate(msg, 400))
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// Remove a chave de qualquer mensagem de erro antes de mostrar na interface.
fn redact(s: &str, secret: &str) -> String {
    if secret.is_empty() {
        s.to_string()
    } else {
        s.replace(secret, "***")
    }
}

fn stamp() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()
}

/// Gera uma imagem e salva em `%APPDATA%/<perfil>/imagens`.
pub fn generate(prompt: &str) -> Result<ImagemGerada, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err("Escreva o pedido da imagem. Exemplo: /imagem um gato astronauta".into());
    }
    let (key, key_var) = key().ok_or_else(|| {
        format!("Chave do Google AI Studio não encontrada. Defina {} no ambiente e reinicie o Argus.", KEY_VARS[0])
    })?;
    let model = model();
    let body = serde_json::json!({
        "model": model,
        "input": [{ "type": "text", "text": prompt }]
    });
    let body_path = std::env::temp_dir().join(format!("argus-imagem-{}.json", stamp()));
    std::fs::write(&body_path, body.to_string()).map_err(|e| format!("falha ao preparar pedido: {e}"))?;

    // A chave vai pela stdin do curl, nunca na linha de comando (que aparece
    // em lista de processos). O corpo vai por arquivo temporário: escapar JSON
    // dentro de config do curl é fonte de erro difícil de diagnosticar.
    let config = format!(
        "request = \"POST\"\nurl = \"{}\"\nheader = \"Content-Type: application/json\"\nheader = \"Api-Revision: 2026-05-20\"\nheader = \"x-goog-api-key: {}\"\ndata-binary = \"@{}\"\nmax-time = 120\n",
        cfg_escape(&endpoint()),
        cfg_escape(&key),
        cfg_escape(&body_path.to_string_lossy()),
    );
    let program = if cfg!(windows) { "curl.exe" } else { "curl" };
    let result = (|| -> Result<ImagemGerada, String> {
        let mut child = Command::new(program)
            .args(["--silent", "--show-error", "--config", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("falha ao iniciar curl: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(config.as_bytes()).map_err(|e| format!("falha ao enviar pedido: {e}"))?;
        }
        let out = child.wait_with_output().map_err(|e| format!("falha ao ler resposta: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if !out.status.success() && stdout.trim().is_empty() {
            return Err(redact(&truncate(&stderr, 400), &key));
        }
        let json: Value = serde_json::from_str(&stdout)
            .map_err(|_| format!("Resposta inesperada do Gemini: {}", redact(&truncate(&stdout, 300), &key)))?;
        if json.get("error").is_some() {
            return Err(redact(&api_error(&json, &stdout), &key));
        }
        let (mime, data) = find_image(&json).ok_or_else(|| redact(&api_error(&json, &stdout), &key))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.as_bytes())
            .map_err(|e| format!("imagem inválida na resposta: {e}"))?;
        if bytes.is_empty() {
            return Err("O Gemini devolveu uma imagem vazia.".into());
        }
        let dir = root();
        std::fs::create_dir_all(&dir).map_err(|e| format!("falha ao criar pasta de imagens: {e}"))?;
        let name = format!("gemini-{}-{:x}.{}", chrono::Local::now().format("%Y%m%d-%H%M%S"), stamp() % 1_000_000, ext_for_mime(&mime));
        let path = dir.join(&name);
        std::fs::write(&path, &bytes).map_err(|e| format!("falha ao salvar imagem: {e}"))?;
        let mut texts = Vec::new();
        find_text(&json, &mut texts);
        Ok(ImagemGerada {
            path,
            name,
            bytes: bytes.len() as u64,
            mime,
            data,
            text: truncate(&texts.join(" "), 500),
        })
    })();
    let _ = std::fs::remove_file(&body_path);
    result.map_err(|e| format!("{e} (origem: {key_var})"))
}

/// Galeria local, mais recente primeiro. Só arquivos de imagem reconhecidos.
pub fn list() -> Result<Vec<ImagemArquivo>, String> {
    let dir = root();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified = meta
            .modified()
            .ok()
            .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d/%m %H:%M").to_string())
            .unwrap_or_default();
        out.push(ImagemArquivo { path, name, bytes: meta.len(), modified });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    out.truncate(60);
    Ok(out)
}

/// Lê uma imagem da galeria como base64. Caminho fora da pasta é recusado.
pub fn read(path: &Path) -> Result<ImagemDados, String> {
    let root = root();
    let real = std::fs::canonicalize(path).map_err(|_| "Imagem não encontrada.".to_string())?;
    let base = std::fs::canonicalize(&root).unwrap_or(root);
    if !real.starts_with(&base) {
        return Err("A imagem precisa estar na pasta do Argus.".into());
    }
    let name = real.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "png" => "image/png",
        _ => return Err("Tipo de imagem não reconhecido.".into()),
    };
    let bytes = std::fs::read(&real).map_err(|e| format!("falha ao ler imagem: {e}"))?;
    Ok(ImagemDados { mime: mime.into(), data: base64::engine::general_purpose::STANDARD.encode(bytes) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cfg_escape_keeps_quotes_and_backslashes_inside_config() {
        assert_eq!(cfg_escape(r#"a"b\c"#), r#"a\"b\\c"#);
    }

    #[test]
    fn mime_picks_extension() {
        assert_eq!(ext_for_mime("image/png"), "png");
        assert_eq!(ext_for_mime("image/jpeg"), "jpg");
        assert_eq!(ext_for_mime("image/webp"), "webp");
    }

    #[test]
    fn reads_image_from_interactions_response() {
        let v: Value = serde_json::from_str(r#"{"output_image":{"mime_type":"image/png","data":"YWJj"}}"#).unwrap();
        assert_eq!(find_image(&v), Some(("image/png".into(), "YWJj".into())));
    }

    #[test]
    fn reads_image_from_generate_content_response() {
        let v: Value = serde_json::from_str(
            r#"{"candidates":[{"content":{"parts":[{"text":"pronto"},{"inlineData":{"mimeType":"image/jpeg","data":"eA=="}}]}}]}"#,
        )
        .unwrap();
        assert_eq!(find_image(&v), Some(("image/jpeg".into(), "eA==".into())));
        let mut texts = Vec::new();
        find_text(&v, &mut texts);
        assert_eq!(texts, vec!["pronto"]);
    }

    #[test]
    fn no_image_is_none() {
        let v: Value = serde_json::from_str(r#"{"error":{"message":"sem permissão"}}"#).unwrap();
        assert_eq!(find_image(&v), None);
        assert!(api_error(&v, "").contains("sem permissão"));
    }

    #[test]
    fn redaction_removes_secret_from_error() {
        assert_eq!(redact("falha com AIzaSEGREDO aqui", "AIzaSEGREDO"), "falha com *** aqui");
    }

    #[test]
    fn empty_prompt_is_rejected_before_network() {
        assert!(generate("   ").is_err());
    }

    #[test]
    fn ui_key_is_kept_in_memory_only_until_cleared() {
        clear_key_from_ui();
        assert!(set_key_from_ui("   ".into()).is_err());
        assert!(!status().from_ui);

        assert!(set_key_from_ui("  chave-teste  ".into()).is_ok());
        let s = status();
        assert!(s.configured);
        assert!(s.from_ui);
        assert_eq!(s.source, SOURCE_UI);

        clear_key_from_ui();
        assert!(!status().from_ui);
    }
}
