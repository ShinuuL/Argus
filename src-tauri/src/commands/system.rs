//! Sistema: uso de CPU/memória, abrir URL no navegador, confirmação nativa e dados do app.
use super::{blocking, err};
use crate::state::AppCtx;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct SystemUsage {
    /// CPU global em % (0–100).
    pub cpu: f32,
    pub mem_used_mb: u64,
    pub mem_total_mb: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AppInfo {
    pub version: String,
    /// Caminho do banco em uso (None = histórico desligado).
    pub db_path: Option<PathBuf>,
    pub has_db: bool,
    /// Pasta de dados (`%APPDATA%/argus`), se existir.
    pub data_dir: Option<PathBuf>,
}

/// CPU e memória do sistema. A CPU é a média desde a chamada anterior:
/// chame a cada ~2 s (a primeira leitura vem 0).
#[tauri::command]
pub async fn system_usage(ctx: State<'_, AppCtx>) -> Result<SystemUsage, String> {
    let sys = ctx.sys.clone();
    blocking(move || {
        let mut guard = sys.lock().map_err(err)?;
        if guard.is_none() {
            *guard = Some(sysinfo::System::new().map_err(err)?);
        }
        let Some(sys) = guard.as_mut() else { return Ok(SystemUsage::default()) };
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Ok(SystemUsage {
            cpu: sys.global_cpu_usage(),
            mem_used_mb: sys.used_memory() / 1_048_576,
            mem_total_mb: sys.total_memory() / 1_048_576,
        })
    })
    .await
}

/// Só http(s): a URL vai para o shell do sistema.
pub fn is_web_url(url: &str) -> bool {
    let u = url.trim();
    (u.starts_with("http://") || u.starts_with("https://")) && !u.chars().any(|c| c.is_control() || c == '"')
}

/// Abre uma URL http(s) no navegador padrão.
#[tauri::command]
pub async fn system_open_url(url: String) -> Result<(), String> {
    if !is_web_url(&url) {
        return Err("Só é possível abrir endereços http:// ou https://.".into());
    }
    let url = url.trim().to_string();
    blocking(move || {
        #[cfg(windows)]
        let r = {
            use std::os::windows::process::CommandExt;
            // rundll32 recebe a URL como argumento único (sem passar pelo cmd).
            std::process::Command::new("rundll32")
                .args(["url.dll,FileProtocolHandler", &url])
                .creation_flags(0x0800_0000)
                .spawn()
        };
        #[cfg(target_os = "macos")]
        let r = std::process::Command::new("open").arg(&url).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let r = std::process::Command::new("xdg-open").arg(&url).spawn();
        r.map(|_| ()).map_err(|e| format!("Não foi possível abrir o navegador: {e}"))
    })
    .await
}

/// Caixa nativa de confirmação Sim/Não. true = Sim.
#[tauri::command]
pub async fn system_confirm(app: AppHandle, title: String, message: String) -> Result<bool, String> {
    blocking(move || {
        Ok(app
            .dialog()
            .message(message)
            .title(title)
            .buttons(MessageDialogButtons::OkCancelCustom("Sim".into(), "Não".into()))
            .blocking_show())
    })
    .await
}

/// Versão do app e onde ficam os dados.
#[tauri::command]
pub async fn app_info(app: AppHandle, ctx: State<'_, AppCtx>) -> Result<AppInfo, String> {
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        db_path: ctx.db_path.clone(),
        has_db: ctx.orch.db().is_some(),
        data_dir: std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("argus")),
    })
}

#[cfg(test)]
mod tests {
    use super::is_web_url;

    #[test]
    fn only_web_urls_are_opened() {
        assert!(is_web_url("http://localhost:5173"));
        assert!(is_web_url(" https://exemplo.dev/a?b=1&c=2 "));
        assert!(!is_web_url("file:///C:/Windows/system32/calc.exe"));
        assert!(!is_web_url("calc.exe"));
        assert!(!is_web_url("http://a\" & calc"));
        assert!(!is_web_url("https://a\nb"));
    }
}
