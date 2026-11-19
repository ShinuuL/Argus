//! Argus (Tauri 2): janela principal + comandos que expõem o `argus-core` à interface.
//!
//! A interface (React/TypeScript, em `src/`) chama os comandos de `commands/*` e
//! recebe eventos (ver `events.rs`). Toda a lógica de negócio fica no `argus-core`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod state;

use commands::*;
use tauri::Manager;

fn main() {
    // Aberto por um shell (Git Bash, `npm run tauri dev`), o Argus herda `PWD` e
    // `OLDPWD` apontando para a pasta de onde foi chamado. Os processos filhos
    // (agentes, terminais) herdariam o valor, e há CLI que confia nele em vez do
    // diretório real. Aqui ainda não existe nenhuma outra thread.
    // Build de depuração (cópia de desenvolvimento) usa a pasta de dados `argus-dev`,
    // separada das sessões de verdade do app instalado.
    if cfg!(debug_assertions) && std::env::var_os(argus_core::paths::PROFILE_VAR).is_none() {
        std::env::set_var(argus_core::paths::PROFILE_VAR, "dev");
    }
    std::env::remove_var("PWD");
    std::env::remove_var("OLDPWD");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            // O orquestrador e o auto-work usam `tokio::spawn`: criamos dentro do runtime.
            let ctx = tauri::async_runtime::block_on(async { state::AppCtx::new() });
            // MCP do Screen-Mirror nos agentes (pela variável ou pasta salva; a pasta
            // raiz entra quando for aberta).
            ctx.mirror.sync_agent_mcp(None);
            app.manage(ctx);
            // Publicação do Tailscale que uma execução anterior deixou ligada (crash).
            std::thread::spawn(argus_core::tailscale::cleanup_stale);
            events::start_orchestrator_forwarder(&handle);
            events::start_servers_forwarder(&handle);
            commands::runs::start_watcher(&handle);
            // Tarefas agendadas: a primeira volta recupera o que venceu com o app fechado.
            commands::schedules::start(&handle);
            // Aberto pelo "Iniciar com o Windows": começa minimizado.
            if std::env::args().any(|a| a == commands::schedules::MINIMIZED_ARG) {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            // ARGUS_ROOT: abre a pasta raiz automaticamente (útil para testes).
            if let Some(dir) = std::env::var_os("ARGUS_ROOT").map(std::path::PathBuf::from).filter(|d| d.is_dir()) {
                if let Err(e) = workspace::open_root(&handle, dir) {
                    eprintln!("ARGUS_ROOT: {e}");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // workspace
            workspace::workspace_pick_root,
            workspace::workspace_open_root,
            workspace::workspace_current,
            workspace::workspace_scan,
            workspace::workspace_refresh,
            // monitor
            monitor::monitor_status,
            monitor::monitor_statuses,
            monitor::monitor_watch,
            monitor::monitor_unwatch,
            // fs
            fs::fs_read_file,
            fs::fs_write_file,
            fs::fs_list_dir,
            fs::fs_stat,
            // pty
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            pty::pty_list,
            pty::pty_default_shell,
            // agentes e tarefas
            agents::agents_providers,
            agents::agents_login_status,
            agents::agents_login_command,
            agents::agents_models,
            agents::workmode_get,
            agents::workmode_set,
            agents::isolation_check,
            agents::permissions_get,
            agents::permissions_set,
            agents::permission_mode_set,
            agents::autocompact_get,
            agents::autocompact_set,
            agents::permissions_allow_denied,
            agents::task_set_model,
            agents::task_set_provider,
            agents::tasks_list,
            agents::task_get,
            agents::task_create,
            agents::task_cancel,
            agents::task_reply,
            agents::task_history,
            agents::task_subagents,
            agents::task_subagent_history,
            agents::task_diff,
            agents::task_apply,
            agents::task_discard,
            agents::task_delete,
            agents::task_delete_closed,
            extra_agents::task_subagent_times,
            // chat
            extra_chat::chat_project_files,
            extra_chat::chat_agents,
            extra_chat::chat_attachment_save,
            extra_chat::chat_attachment_read,
            extra_chat::chat_attachments_cleanup,
            extra_chat::chat_agent_capabilities,
            // auto-work
            autowork::autowork_get_settings,
            autowork::autowork_set_settings,
            autowork::autowork_all_settings,
            autowork::autowork_status,
            autowork::autowork_all_status,
            autowork::autowork_start,
            autowork::autowork_pause,
            autowork::autowork_resume,
            autowork::autowork_stop,
            // tarefas agendadas e inicialização
            schedules::schedules_list,
            schedules::schedules_save,
            schedules::schedules_set_enabled,
            schedules::schedules_delete,
            schedules::schedules_run_now,
            schedules::autostart_get,
            schedules::autostart_set,
            // servidores
            servers::servers_list,
            // builds e servidores de dev
            runs::runs_detect,
            runs::runs_detect_all,
            runs::runs_prepare,
            runs::runs_list,
            runs::runs_history,
            runs::runs_stop,
            runs::runs_remembered,
            runs::runs_open_folder,
            servers::servers_of,
            servers::owner_processes,
            servers::owners_usage,
            servers::server_kill,
            servers::servers_stop_all,
            servers::server_restart,
            servers::allowed_commands_get,
            servers::allowed_commands_set,
            // celulares (Screen-Mirror)
            devices::devices_status,
            devices::devices_start,
            devices::devices_stop,
            devices::devices_set_dir,
            devices::devices_set_agents,
            devices::devices_list,
            devices::devices_apks,
            devices::devices_install,
            // agentes e skills em arquivo
            agentdefs::agentdefs_list,
            agentdefs::agentdefs_save,
            agentdefs::agentdefs_delete,
            agentdefs::agentdefs_skills,
            agentdefs::agentdefs_skill_save,
            agentdefs::agentdefs_skill_delete,
            agentdefs::agentdefs_sync_md,
            agentdefs::agentdefs_slug,
            // dependências
            connections::connections_status,
            connections::connections_repo,
            deps::deps_analyze,
            deps::deps_detect_missing_command,
            deps::deps_install_plan,
            // métricas
            metrics::metrics_report,
            metrics::metrics_today,
            metrics::metrics_usage_rows,
            metrics::metrics_usage_by_project,
            metrics::pricing_get,
            metrics::pricing_set,
            metrics::pricing_path,
            // remoto
            remote::remote_info,
            remote::remote_start,
            remote::remote_stop,
            remote::remote_regenerate_token,
            remote::remote_set_host,
            remote::remote_hosts,
            remote::remote_tailscale_status,
            // mídia
            media::media_system_current,
            media::media_system_art,
            media::media_system_command,
            media::media_pick_folder,
            media::media_scan_folder,
            media::media_local_play,
            media::media_local_toggle,
            media::media_local_stop,
            media::media_local_volume,
            // sistema
            system::system_usage,
            system::system_open_url,
            system::system_confirm,
            system::app_info,
        ])
        .build(tauri::generate_context!())
        .expect("falha ao iniciar o Argus")
        .run(|app, event| {
            // Processos auxiliares não podem sobreviver à janela.
            if let tauri::RunEvent::Exit = event {
                if let Some(ctx) = app.try_state::<state::AppCtx>() {
                    ctx.mirror.stop();
                }
            }
        });
}
