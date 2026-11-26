//! Integração com o CLI `caveman` (https://github.com/JuliusBrussee/caveman).
//!
//! O Argus não reimplementa a compressão: chama `caveman compress`, que lê o
//! texto da stdin, escreve o resultado na stdout e o relatório JSON na stderr.
//! O relatório é devolvido junto com o texto para a interface poder mostrar as
//! duas versões e deixar o usuário escolher.
//!
//! Quando `caveman-engine` não está instalado, a CLI faz pass-through e avisa
//! `engine: "missing"`, `ratio: 0`. Aqui isso é preservado: nunca dizemos que
//! houve compressão quando não houve.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::agents::provider::which_bin;

/// Estado da instalação, para a interface explicar o que falta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CavemanStatus {
    pub installed: bool,
    /// Caminho do executável encontrado (None = não instalado).
    pub bin: Option<String>,
    pub hint: String,
}

/// Relatório da CLI (campos conferidos no `compressFallback` e no engine).
/// Campos ausentes no JSON viram o valor padrão: versões diferentes da CLI
/// omitem campos, e um relatório parcial não pode descartar a compressão.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CavemanReport {
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub ratio: f64,
    pub engine: String,
    pub note: String,
}

/// Resultado de uma compressão. `original` e `compressed` viajam juntos: a
/// escolha entre os dois acontece na interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CavemanResult {
    pub original: String,
    pub compressed: String,
    pub report: CavemanReport,
    /// true quando a CLI comprimiu de fato; false no pass-through.
    pub compressed_for_real: bool,
}

/// Verifica se o CLI está disponível. Não instala nada e não lê configuração.
pub fn status() -> CavemanStatus {
    match which_bin("caveman") {
        Some(bin) => CavemanStatus {
            installed: true,
            bin: Some(bin.to_string_lossy().into_owned()),
            hint: String::new(),
        },
        None => CavemanStatus {
            installed: false,
            bin: None,
            hint: "Instale o CLI com `npm install -g @caveman-ai/cli` e rode `caveman setup` para instalar o engine.".into(),
        },
    }
}

/// O npm instala o CLI como shim `.cmd`. `CreateProcess` não roda script no
/// Windows: sem embrulhar em `cmd /c`, `Command::new` falha com "não é um
/// aplicativo Win32 válido" (o mesmo problema resolvido para o `opencode`).
fn shim_command(bin: &PathBuf) -> (String, Vec<String>) {
    let ext = bin.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    if cfg!(windows) && matches!(ext.as_deref(), Some("cmd") | Some("bat")) {
        return ("cmd.exe".to_string(), vec!["/c".into(), bin.to_string_lossy().into_owned(), "compress".into()]);
    }
    (bin.to_string_lossy().into_owned(), vec!["compress".into()])
}

/// Comprime um trecho com o CLI real. `DO_NOT_TRACK=1` desliga a telemetria do
/// caveman: o Argus não manda o texto do usuário para lugar nenhum além da CLI.
pub fn compress(text: &str) -> Result<CavemanResult, String> {
    let bin: PathBuf = which_bin("caveman").ok_or_else(|| "caveman não está instalado; rode `caveman setup`".to_string())?;
    let (program, args) = shim_command(&bin);
    let mut child = Command::new(program)
        .args(args)
        .env("DO_NOT_TRACK", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("falha ao iniciar caveman: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(|e| format!("falha ao enviar texto ao caveman: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("falha ao ler caveman: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { format!("caveman saiu com {}", out.status) } else { err });
    }
    let compressed = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let report = stderr
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<CavemanReport>(l.trim()).ok())
        .unwrap_or_else(|| CavemanReport {
            bytes_in: text.len() as u64,
            bytes_out: compressed.len() as u64,
            ratio: 0.0,
            engine: "desconhecido".into(),
            note: "O CLI não devolveu relatório JSON; não é possível afirmar que houve compressão.".into(),
        });
    let compressed_for_real = report.engine != "missing" && report.ratio > 0.0 && compressed != text;
    Ok(CavemanResult { original: text.to_string(), compressed, report, compressed_for_real })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_missing_engine_is_not_claimed_as_compression() {
        let report: CavemanReport = serde_json::from_str(
            r#"{"bytes_in":100,"bytes_out":100,"ratio":0,"engine":"missing","note":"not installed"}"#,
        )
        .unwrap();
        assert_eq!(report.engine, "missing");
        let result = CavemanResult {
            original: "x".repeat(100),
            compressed: "x".repeat(100),
            report,
            compressed_for_real: false,
        };
        assert!(!result.compressed_for_real, "pass-through nunca é compressão");
    }

    #[test]
    fn partial_report_keeps_defaults() {
        let report: CavemanReport = serde_json::from_str(r#"{"engine":"missing"}"#).unwrap();
        assert_eq!(report.engine, "missing");
        assert_eq!(report.bytes_in, 0);
        assert_eq!(report.bytes_out, 0);
        assert_eq!(report.ratio, 0.0);
        assert!(report.note.is_empty());
    }

    #[test]
    fn npm_shim_runs_through_cmd_only_on_windows() {
        let cmd = PathBuf::from(r"C:\npm\global\caveman.cmd");
        let (program, args) = shim_command(&cmd);
        if cfg!(windows) {
            assert_eq!(program, "cmd.exe");
            assert_eq!(args, ["/c", r"C:\npm\global\caveman.cmd", "compress"]);
        } else {
            assert_eq!(program, cmd.to_string_lossy());
            assert_eq!(args, ["compress"]);
        }
        let exe = PathBuf::from("/usr/local/bin/caveman");
        let (program, args) = shim_command(&exe);
        assert_eq!(program, exe.to_string_lossy());
        assert_eq!(args, ["compress"]);
    }

    #[test]
    fn status_is_honest_when_cli_is_absent() {
        let s = status();
        assert_eq!(s.installed, s.bin.is_some());
        if !s.installed {
            assert!(s.hint.contains("caveman"));
        }
    }
}
