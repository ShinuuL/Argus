//! Servidores vinculados às sessões: badge "N servidores", side-window (painel
//! lateral direito retrátil) e janela separada, mais os "Comandos permitidos ao agente".

use crate::state::use_app;
use dioxus::prelude::*;
use argus_core::agents::orchestrator::TaskId;
use argus_core::procs::{self, Owner, ServerInfo};
use std::time::Duration;

/// Painel aberto? (`Some(filtro)`; filtro None = todos os donos).
pub static PANEL: GlobalSignal<Option<Option<Owner>>> = Signal::global(|| None);

/// Abre a side-window (opcionalmente filtrada por dono).
pub fn open_panel(filter: Option<Owner>) {
    *PANEL.write() = Some(filter);
}

/// Lista de servidores atualizada a cada ~2 s.
pub fn use_servers() -> Signal<Vec<ServerInfo>> {
    let mut list = use_signal(|| procs::tracker().servers());
    use_future(move || async move {
        let mut rev = u64::MAX;
        loop {
            let r = procs::tracker().rev();
            if r != rev {
                rev = r;
                list.set(procs::tracker().servers());
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });
    list
}

fn uptime(started: u64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let s = now.saturating_sub(started);
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}min", s / 60),
        _ => format!("{}h{:02}", s / 3600, (s % 3600) / 60),
    }
}

/// Abre a URL no navegador padrão.
pub fn open_browser(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).creation_flags(0x0800_0000).spawn();
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// Ao cancelar/descartar a sessão: se ela tem servidores, pergunta se derruba.
pub fn confirm_stop_session(task: TaskId) {
    let owner = Owner::Session(task);
    let n = procs::tracker().servers_of(owner).len();
    if n == 0 {
        return;
    }
    let yes = rfd::MessageDialog::new()
        .set_title("Servidores da sessão")
        .set_description(format!("A sessão #{task} tem {n} servidor(es) rodando. Derrubar agora?"))
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();
    if yes == rfd::MessageDialogResult::Yes {
        procs::tracker().stop_all(owner);
    }
}

/// Badge "N servidores" para aba/card da sessão. Some quando não há servidores.
#[component]
pub fn SessionServersBadge(task: TaskId) -> Element {
    let list = use_servers();
    let n = list.read().iter().filter(|s| s.owner == Owner::Session(task)).count();
    if n == 0 {
        return rsx! {};
    }
    let label = if n == 1 { "1 servidor".to_string() } else { format!("{n} servidores") };
    rsx! {
        button {
            class: "srv-badge",
            title: "Ver servidores desta sessão",
            onclick: move |e| {
                e.stop_propagation();
                open_panel(Some(Owner::Session(task)));
            },
            "\u{25CF} {label}"
        }
    }
}

/// Botão da status bar: abre o painel com todos os servidores.
#[component]
pub fn ServersStatusButton() -> Element {
    let list = use_servers();
    let n = list.read().len();
    rsx! {
        button {
            class: "wb-sbtn",
            title: "Servidores das sessões",
            onclick: move |_| if PANEL.peek().is_some() { *PANEL.write() = None } else { open_panel(None) },
            "\u{2B21} {n} servidores"
        }
    }
}

/// Side-window: painel lateral direito retrátil. Renderiza nada se fechado.
#[component]
pub fn ServersPanel() -> Element {
    let Some(filter) = PANEL() else { return rsx! {} };
    rsx! {
        style { {CSS} }
        aside { class: "srv-panel",
            div { class: "srv-head",
                b { "Servidores" }
                if let Some(o) = filter {
                    button { class: "srv-chip", title: "Mostrar todos", onclick: move |_| open_panel(None), "{o.label()} \u{2715}" }
                }
                div { style: "flex:1" }
                button { class: "wb-sbtn", title: "Abrir em janela separada", onclick: move |_| open_window(), "\u{29C9}" }
                button { class: "wb-sbtn", title: "Fechar", onclick: move |_| *PANEL.write() = None, "\u{2715}" }
            }
            ServersList { filter }
            AllowedCommands {}
        }
    }
}

fn open_window() {
    let dom = VirtualDom::new(ServersWindow);
    let cfg = dioxus::desktop::Config::new()
        .with_window(dioxus::desktop::WindowBuilder::new().with_title("Servidores").with_inner_size(dioxus::desktop::LogicalSize::new(460.0, 600.0)));
    let _ = dioxus::desktop::window().new_window(dom, cfg);
    *PANEL.write() = None;
}

/// Raiz da janela separada.
#[allow(non_snake_case)]
fn ServersWindow() -> Element {
    rsx! {
        style { {CSS} }
        style { "body{{margin:0;background:#1e1e1e;color:#ddd;font:13px system-ui,sans-serif}}" }
        div { class: "srv-panel srv-window",
            div { class: "srv-head", b { "Servidores das sessões" } }
            ServersList { filter: None }
        }
    }
}

#[component]
fn ServersList(filter: Option<Owner>) -> Element {
    let mut list = use_servers();
    let mut error = use_signal(|| None::<String>);
    let items: Vec<ServerInfo> = list.read().iter().filter(|s| filter.is_none_or(|f| s.owner == f)).cloned().collect();
    let mut owners: Vec<Owner> = items.iter().map(|s| s.owner).collect();
    owners.dedup();
    owners.sort_by_key(|o| match o {
        Owner::Session(i) => (0, *i),
        Owner::Terminal(i) => (1, *i),
    });
    owners.dedup();

    rsx! {
        if let Some(e) = error() { div { class: "err small", "{e}" } }
        if items.is_empty() {
            p { class: "muted small", "Nenhum servidor em execução vinculado às sessões." }
        }
        for o in owners {
            div { key: "{o.label()}", class: "srv-group",
                div { class: "srv-group-head",
                    b { "{o.label()}" }
                    button { class: "srv-danger", onclick: move |_| {
                            spawn(async move {
                                let _ = tokio::task::spawn_blocking(move || procs::tracker().stop_all(o)).await;
                                list.set(procs::tracker().servers());
                            });
                        }, "Derrubar todos" }
                }
                for s in items.iter().filter(|s| s.owner == o).cloned() {
                    {
                        let (url, pid) = (s.url.clone(), s.pid);
                        let s2 = s.clone();
                        rsx! {
                            div { key: "{s.pid}-{s.port}", class: "srv-item",
                                div { class: "srv-line",
                                    b { ":{s.port}" }
                                    span { class: "muted small", "{s.name} · pid {s.pid} · {uptime(s.started_at)}" }
                                }
                                div { class: "small muted ellipsis", title: "{s.cmdline}", "{s.cmdline}" }
                                div { class: "srv-actions",
                                    button { onclick: move |_| open_browser(&url), "Abrir no navegador" }
                                    button { disabled: s.cmdline.is_empty(), onclick: move |_| {
                                            let s = s2.clone();
                                            spawn(async move {
                                                match tokio::task::spawn_blocking(move || procs::restart(&s)).await {
                                                    Ok(Err(e)) => error.set(Some(e.to_string())),
                                                    _ => error.set(None),
                                                }
                                            });
                                        }, "Reiniciar" }
                                    button { class: "srv-danger", onclick: move |_| {
                                            spawn(async move {
                                                match tokio::task::spawn_blocking(move || procs::tracker().kill(pid)).await {
                                                    Ok(Err(e)) => error.set(Some(e.to_string())),
                                                    _ => error.set(None),
                                                }
                                                list.set(procs::tracker().servers());
                                            });
                                        }, "Derrubar" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Configuração por projeto: "Comandos permitidos ao agente" (vira `--allowedTools`).
#[component]
pub fn AllowedCommands() -> Element {
    let app = use_app();
    let project = (app.selected)().or_else(|| (app.root)());
    let mut text = use_signal(String::new);
    let mut msg = use_signal(|| None::<String>);
    let p_eff = project.clone();
    use_effect(use_reactive!(|p_eff| {
        let v = p_eff.as_deref().map(procs::allowed_commands).unwrap_or_default();
        text.set(v.join("\n"));
        msg.set(None);
    }));
    let Some(project) = project else { return rsx! {} };
    let name = project.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    rsx! {
        div { class: "srv-allowed",
            b { "Comandos permitidos ao agente" }
            div { class: "small muted",
                "Projeto {name}. Um padrão por linha (ex.: npm run dev:*). Com acceptEdits o Bash fica bloqueado; cada linha vira --allowedTools \"Bash(...)\"."
            }
            textarea { rows: "4", value: "{text}", placeholder: "npm run dev:*", oninput: move |e| text.set(e.value()) }
            button { onclick: move |_| {
                    let cmds = text.peek().lines().map(str::to_string).collect();
                    msg.set(Some(match procs::set_allowed_commands(&project, cmds) {
                        Ok(()) => "Salvo. Vale a partir do próximo turno.".into(),
                        Err(e) => format!("erro: {e}"),
                    }));
                }, "Salvar" }
            if let Some(m) = msg() { span { class: "small muted", " {m}" } }
        }
    }
}

pub const CSS: &str = r#"
.srv-panel { width: 340px; flex: none; display: flex; flex-direction: column; gap: 8px; padding: 8px 10px; border-left: 1px solid rgba(128,128,128,.25); overflow-y: auto; min-height: 0; box-sizing: border-box; }
.srv-window { width: 100%; height: 100vh; border: 0; }
.srv-head { display: flex; align-items: center; gap: 6px; }
.srv-chip, .srv-badge { font-size: 11px; padding: 1px 8px; border-radius: 10px; border: 1px solid rgba(80,200,120,.5); background: rgba(80,200,120,.12); color: inherit; cursor: pointer; white-space: nowrap; }
.srv-group { border: 1px solid rgba(128,128,128,.25); border-radius: 6px; padding: 6px; display: flex; flex-direction: column; gap: 6px; }
.srv-group-head { display: flex; justify-content: space-between; align-items: center; gap: 6px; }
.srv-item { display: flex; flex-direction: column; gap: 3px; padding-top: 4px; border-top: 1px dashed rgba(128,128,128,.2); min-width: 0; }
.srv-line { display: flex; gap: 8px; align-items: baseline; flex-wrap: wrap; }
.srv-actions { display: flex; gap: 4px; flex-wrap: wrap; }
.srv-actions button, .srv-group-head button { font-size: 11px; }
.srv-danger { color: #f48771; }
.srv-allowed { display: flex; flex-direction: column; gap: 4px; margin-top: auto; padding-top: 8px; border-top: 1px solid rgba(128,128,128,.25); }
.srv-allowed textarea { width: 100%; box-sizing: border-box; font-family: monospace; font-size: 12px; }
"#;
