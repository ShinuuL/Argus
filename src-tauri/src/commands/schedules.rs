//! Tarefas agendadas (`argus_core::schedules`), notificação do sistema e a opção
//! "Iniciar com o Windows".
use super::{blocking, err};
use crate::state::AppCtx;
use argus_core::schedules::{Schedule, SchedulerSignal};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

/// Evento para a janela: a lista de agendamentos mudou (sem payload).
pub const SCHEDULES_CHANGED: &str = "schedules-changed";

/// Argumento passado pelo registro "Iniciar com o Windows": abre minimizado.
pub const MINIMIZED_ARG: &str = "--minimizado";

/// Liga o laço do agendador: mudanças viram evento e avisos viram notificação.
pub fn start(app: &AppHandle) {
    let handle = app.clone();
    app.state::<AppCtx>().scheduler.start(move |sig| match sig {
        SchedulerSignal::Changed => {
            let _ = handle.emit(SCHEDULES_CHANGED, ());
        }
        SchedulerSignal::Notify { title, body, .. } => notify(&title, &body),
    });
}

#[tauri::command]
pub async fn schedules_list(ctx: State<'_, AppCtx>) -> Result<Vec<Schedule>, String> {
    let s = ctx.scheduler.clone();
    blocking(move || s.list().map_err(err)).await
}

/// Cria (id 0) ou edita um agendamento.
#[tauri::command]
pub async fn schedules_save(ctx: State<'_, AppCtx>, schedule: Schedule) -> Result<Schedule, String> {
    let s = ctx.scheduler.clone();
    blocking(move || s.save(schedule).map_err(err)).await
}

#[tauri::command]
pub async fn schedules_set_enabled(ctx: State<'_, AppCtx>, id: i64, enabled: bool) -> Result<Schedule, String> {
    let s = ctx.scheduler.clone();
    blocking(move || s.set_enabled(id, enabled).map_err(err)).await
}

#[tauri::command]
pub async fn schedules_delete(ctx: State<'_, AppCtx>, id: i64) -> Result<(), String> {
    let s = ctx.scheduler.clone();
    blocking(move || s.delete(id).map_err(err)).await
}

/// Executa agora (cria a tarefa ou mostra o lembrete), sem mudar a próxima execução.
#[tauri::command]
pub async fn schedules_run_now(ctx: State<'_, AppCtx>, id: i64) -> Result<Schedule, String> {
    let s = ctx.scheduler.clone();
    // `submit_with` usa `tokio::spawn`: roda no runtime, não numa thread solta.
    s.run_now(id).map_err(err)
}

// -------------------------------------------------------------- notificação

/// Escapa texto para o XML da notificação.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn toast_xml(title: &str, body: &str) -> String {
    format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        xml_escape(title),
        xml_escape(body)
    )
}

/// Notificação do sistema (Windows). O app instalado usa o próprio identificador;
/// a cópia de desenvolvimento não tem atalho registrado e usa o do PowerShell,
/// como faz o plugin oficial de notificação.
pub fn notify(title: &str, body: &str) {
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Data::Xml::Dom::XmlDocument;
        use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
        let aumid = if is_dev_profile() {
            r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe"
        } else {
            "dev.argus.app"
        };
        let show = || -> windows::core::Result<()> {
            let doc = XmlDocument::new()?;
            doc.LoadXml(&HSTRING::from(toast_xml(title, body)))?;
            let toast = ToastNotification::CreateToastNotification(&doc)?;
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(aumid))?.Show(&toast)
        };
        if let Err(e) = show() {
            eprintln!("notificação: {e}");
        }
    }
    #[cfg(not(windows))]
    eprintln!("{title}: {body}");
}

// ---------------------------------------------------- iniciar com o Windows

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Argus";

fn is_dev_profile() -> bool {
    argus_core::paths::dir_name() != "argus"
}

/// Linha gravada no registro: o executável entre aspas e o argumento de minimizado.
fn run_command(exe: &std::path::Path) -> String {
    format!("\"{}\" {MINIMIZED_ARG}", exe.display())
}

#[derive(Debug, Clone, Serialize)]
pub struct AutostartInfo {
    pub enabled: bool,
    /// false na cópia de desenvolvimento (não se registra).
    pub available: bool,
    /// Por que não dá para ligar (quando `available` é false).
    pub reason: Option<String>,
}

#[cfg(windows)]
fn reg(args: &[&str]) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg").args(args).creation_flags(0x0800_0000).output().map_err(err)
}

fn autostart_info() -> AutostartInfo {
    if is_dev_profile() {
        return AutostartInfo {
            enabled: false,
            available: false,
            reason: Some("A cópia de desenvolvimento não se registra para iniciar com o Windows.".into()),
        };
    }
    #[cfg(windows)]
    {
        let enabled = reg(&["query", RUN_KEY, "/v", RUN_VALUE]).map(|o| o.status.success()).unwrap_or(false);
        AutostartInfo { enabled, available: true, reason: None }
    }
    #[cfg(not(windows))]
    AutostartInfo { enabled: false, available: false, reason: Some("Só no Windows.".into()) }
}

#[tauri::command]
pub async fn autostart_get() -> Result<AutostartInfo, String> {
    blocking(|| Ok(autostart_info())).await
}

/// Liga ou desliga a entrada em HKCU\...\Run (só o usuário atual, sem elevação).
#[tauri::command]
pub async fn autostart_set(enabled: bool) -> Result<AutostartInfo, String> {
    blocking(move || {
        let info = autostart_info();
        if !info.available {
            return Err(info.reason.unwrap_or_default());
        }
        #[cfg(windows)]
        {
            let out = if enabled {
                let exe = std::env::current_exe().map_err(err)?;
                reg(&["add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &run_command(&exe), "/f"])?
            } else if info.enabled {
                reg(&["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])?
            } else {
                return Ok(info);
            };
            if !out.status.success() {
                return Err(format!("Não foi possível alterar o registro: {}", String::from_utf8_lossy(&out.stderr).trim()));
            }
        }
        let _ = enabled;
        Ok(autostart_info())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_command_quotes_the_exe() {
        let p = std::path::Path::new(r"C:\Program Files\Argus\argus.exe");
        assert_eq!(run_command(p), r#""C:\Program Files\Argus\argus.exe" --minimizado"#);
    }

    #[test]
    fn toast_text_is_escaped() {
        let x = toast_xml("a <b>", "c & \"d\"");
        assert!(x.contains("a &lt;b&gt;"));
        assert!(x.contains("c &amp; &quot;d&quot;"));
    }

    #[test]
    fn test_build_never_registers() {
        // Testes rodam com o perfil de dados de desenvolvimento ou sem `ARGUS_PROFILE`;
        // com o perfil dev a opção fica indisponível e nada toca no registro.
        std::env::set_var(argus_core::paths::PROFILE_VAR, "dev");
        let info = autostart_info();
        assert!(!info.available && !info.enabled);
    }
}
