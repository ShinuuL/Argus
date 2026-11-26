//! Sessão do opencode no serviço compartilhado.
//!
//! Desde a Fase 2 o `opencode run` não usa mais `--standalone`: a política do
//! Argus vai nas regras da sessão, gravadas pela própria CLI oficial com
//! `opencode api` (`POST /api/session` no primeiro turno, `PATCH` nos seguintes).
//! O Argus não lê nem guarda o token do serviço — a CLI faz a autenticação.
//!
//! Conferido no opencode 2.3.0: um `run` não-interativo aplica as regras da
//! sessão; uma regra `ask` é auto-rejeitada ("cannot ask the user for
//! permission"), por isso as regras do Argus são `allow`/`deny`, sem `ask`.

use super::provider::{run_capture, run_capture_stdout, CommandSpec, which_bin};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// O npm instala a CLI como um shim `.cmd`, que roda pelo cmd.exe: ele corta a
/// linha de comando em ~8 mil caracteres e as regras V2 não cabem (erro "Linha
/// de comando muito longa" no Windows). O `.exe` que o shim chama usa o limite
/// do CreateProcess (~32 mil). Continua sendo a CLI oficial quem autentica;
/// o Argus não lê token nem chama o serviço direto.
fn api_bin() -> Result<PathBuf, String> {
    let bin = which_bin("opencode").ok_or_else(|| "a CLI opencode não está instalada".to_string())?;
    if !bin.extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat")) {
        return Ok(bin);
    }
    let text = std::fs::read_to_string(&bin).unwrap_or_default();
    let dir = bin.parent().unwrap_or(Path::new(""));
    Ok(exe_in_shim(&text, dir).unwrap_or(bin))
}

/// Lê o primeiro `.exe` existente entre aspas no shim (o `%dp0%` é a pasta dele).
fn exe_in_shim(text: &str, dir: &Path) -> Option<PathBuf> {
    let base = dir.to_string_lossy();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let after = rest.get(start + 1..)?;
        let end = after.find('"')?;
        let token = after[..end].replace("%dp0%", &base).replace("%DP0%", &base);
        let candidate = PathBuf::from(&token);
        if candidate.is_file() && candidate.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
            return Some(candidate);
        }
        rest = &after[end + 1..];
    }
    None
}

/// Só o opencode passa por aqui; os outros provedores seguem como antes.
pub fn applies(spec: &CommandSpec) -> bool {
    Path::new(&spec.program)
        .file_stem()
        .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("opencode"))
}

fn api(bin: &Path, method: &str, path: &str, data: Option<&str>) -> Result<Value, String> {
    let mut args: Vec<&str> = vec!["api", method, path];
    if let Some(d) = data {
        args.push("--data");
        args.push(d);
    }
    let (ok, out) = run_capture_stdout(bin, &args).ok_or_else(|| "não foi possível executar a CLI opencode".to_string())?;
    if !ok {
        // A mensagem de erro da CLI pode vir no stderr; segunda chamada só para lê-la.
        let (_, all) = run_capture(bin, &args).unwrap_or((false, out.clone()));
        return Err(short(&all));
    }
    if out.trim().is_empty() {
        return Ok(Value::Null); // PATCH e DELETE não devolvem corpo
    }
    let v: Value = serde_json::from_str(&out).map_err(|_| short(&out))?;
    // A CLI imprime erros como JSON e nem sempre sai com código não-zero.
    if v.get("_tag").and_then(Value::as_str).is_some_and(|t| t.contains("Error")) {
        return Err(short(&out));
    }
    Ok(v)
}

/// Se o serviço ainda não estiver no ar, a CLI o inicia e a chamada repete.
fn api_or_start(bin: &Path, method: &str, path: &str, data: Option<&str>) -> Result<Value, String> {
    match api(bin, method, path, data) {
        Ok(v) => Ok(v),
        Err(e) => {
            let _ = run_capture(bin, &["service", "start"]);
            api(bin, method, path, data).map_err(|_| e)
        }
    }
}

fn short(out: &str) -> String {
    let s = out.trim().replace('\n', " ");
    if s.chars().count() <= 240 {
        s
    } else {
        s.chars().take(240).collect::<String>() + "…"
    }
}

fn json(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn create_body(id: Option<&str>, cwd: &Path, rules: &Value) -> String {
    let id = id.map(|s| format!("\"id\":{},", json(s))).unwrap_or_default();
    let rules = serde_json::to_string(rules).unwrap_or_else(|_| "[]".to_string());
    let dir = json(&cwd.to_string_lossy());
    format!("{{{id}\"location\":{{\"directory\":{dir}}},\"permissions\":{rules}}}")
}

/// Garante uma sessão com as regras atuais e devolve o id para `--session`.
///
/// Sessão nova: cria com as regras. Sessão existente: atualiza as regras com
/// `PATCH`; se ela não existir mais no serviço (reinício, outro computador),
/// recria com o mesmo id para não perder a conversa do banco do Argus.
pub fn ensure_session(session: Option<&str>, cwd: &Path, rules: &Value) -> Result<String, String> {
    let bin = api_bin()?;
    match session {
        Some(id) => {
            let patch = format!("{{\"permissions\":{}}}", serde_json::to_string(rules).unwrap_or_else(|_| "[]".to_string()));
            if api(&bin, "PATCH", &format!("/api/session/{id}"), Some(&patch)).is_ok() {
                return Ok(id.to_string());
            }
            let body = create_body(Some(id), cwd, rules);
            api_or_start(&bin, "POST", "/api/session", Some(&body))?;
            Ok(id.to_string())
        }
        None => {
            let body = create_body(None, cwd, rules);
            let v = api_or_start(&bin, "POST", "/api/session", Some(&body))?;
            v.pointer("/data/id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .ok_or_else(|| "o serviço não devolveu o id da sessão".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_has_location_and_rules() {
        let v = serde_json::json!([{ "action": "shell", "resource": "git status *", "effect": "allow" }]);
        let body = create_body(Some("ses_1"), Path::new("D:\\proj"), &v);
        let j: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(j["id"], "ses_1");
        assert_eq!(j["location"]["directory"], "D:\\proj");
        assert_eq!(j["permissions"][0]["resource"], "git status *");
        let body = create_body(None, Path::new("."), &v);
        let j: Value = serde_json::from_str(&body).unwrap();
        assert!(j.get("id").is_none());
        assert_eq!(j["permissions"][0]["effect"], "allow");
    }

    #[test]
    fn only_opencode_uses_the_service() {
        let mut s = CommandSpec::new("opencode");
        assert!(applies(&s));
        s = CommandSpec::new("C:\\bin\\opencode.EXE");
        assert!(applies(&s));
        assert!(!applies(&CommandSpec::new("claude")));
        assert!(!applies(&CommandSpec::new("opencode-run")));
    }

    #[test]
    fn shim_points_to_the_real_exe() {
        let dir = std::env::temp_dir().join(format!("argus-shim-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let exe = dir.join("opencode.exe");
        std::fs::write(&exe, b"fake").unwrap();
        let shim = "@ECHO off\nSETLOCAL\n\"%dp0%\\opencode.exe\"   %*\n";
        assert_eq!(exe_in_shim(shim, &dir), Some(exe.clone()));
        assert_eq!(exe_in_shim("\"%dp0%\\sem-isso.exe\" %*", &dir), None);
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn error_output_is_short() {
        assert_eq!(short(" erro \n"), "erro");
        assert_eq!(short(&"x".repeat(500)).chars().count(), 241);
    }
}
