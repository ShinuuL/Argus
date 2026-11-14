//! Tela de agentes: login dos provedores, nova tarefa e acompanhamento das tarefas.
//!
//! A IDE apenas executa as CLIs oficiais; o login é feito pela própria CLI
//! (rodando no terminal embutido). Nenhum token é lido ou guardado aqui.
use crate::state::{use_app, use_orchestrator};
use crate::ui::sidebar::dir_name;
use dioxus::prelude::*;
use argus_core::agents::orchestrator::{TaskId, TaskState};
use argus_core::agents::provider::{AgentProvider, LoginStatus};
use argus_core::pty;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Lista de provedores disponível no contexto.
#[derive(Clone)]
pub struct Providers(pub Vec<Arc<dyn AgentProvider>>);

fn to_pty(spec: argus_core::agents::provider::CommandSpec, cwd: PathBuf) -> pty::CommandSpec {
    pty::CommandSpec { program: spec.program, args: spec.args, cwd: spec.cwd.or(Some(cwd)), env: spec.env }
}

fn state_label(s: &TaskState) -> String {
    match s {
        TaskState::Queued => "na fila".into(),
        TaskState::Running => "executando".into(),
        TaskState::Completed => "concluída".into(),
        TaskState::Failed(e) => format!("falhou: {e}"),
        TaskState::Cancelled => "cancelada".into(),
        TaskState::Applied => "aplicada".into(),
        TaskState::Discarded => "descartada".into(),
    }
}

/// Linha de log que é aviso (stderr/estado), exibida discretamente.
fn is_notice_line(l: &str) -> bool {
    l.starts_with("[stderr]") || l.starts_with("[estado]")
}

/// Log da tarefa: texto normal em bloco; blocos consecutivos de avisos colapsados e cinza.
fn task_log_view(log: &[String]) -> Element {
    let mut groups: Vec<(bool, Vec<String>)> = Vec::new();
    for l in log {
        let n = is_notice_line(l);
        match groups.last_mut() {
            Some((k, v)) if *k == n => v.push(l.clone()),
            _ => groups.push((n, vec![l.clone()])),
        }
    }
    if groups.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "log",
            for (i, (notice, lines)) in groups.into_iter().enumerate() {
                if notice {
                    details { key: "{i}", class: "wb-notice",
                        summary { "\u{24D8} {crate::ui::workbench::notice_summary(&lines[0])}"
                            if lines.len() > 1 { " (+{lines.len() - 1})" } }
                        pre { {lines.join("\n")} }
                    }
                } else {
                    pre { key: "{i}", class: "log-text", {lines.join("\n")} }
                }
            }
        }
    }
}

#[component]
pub fn AgentsView() -> Element {
    let providers = use_context::<Providers>();
    let mut app = use_app();
    let orch = use_orchestrator();

    // Status de login de cada provedor (consulta bloqueante fora da UI).
    let mut login = use_signal(HashMap::<String, LoginStatus>::new);
    let refresh_login = {
        let providers = providers.clone();
        move || {
            for p in providers.0.iter().cloned() {
                spawn(async move {
                    let id = p.id().to_string();
                    let st = tokio::task::spawn_blocking(move || {
                        if p.detect().is_none() { LoginStatus::NotInstalled } else { p.login_status() }
                    })
                    .await
                    .unwrap_or(LoginStatus::Unknown);
                    login.write().insert(id, st);
                });
            }
        }
    };
    // Atualiza na abertura e sempre que um terminal termina (ex.: login concluído).
    use_effect({
        let r = refresh_login.clone();
        move || {
            let _ = crate::ui::terminal::TERM_EXITS();
            r()
        }
    });
    let wb = crate::ui::workbench::use_workbench();

    let mut form_project = use_signal(|| None::<PathBuf>);
    let mut form_provider = use_signal(|| providers.0.first().map(|p| p.id().to_string()).unwrap_or_default());
    let mut form_prompt = use_signal(String::new);
    let mut form_error = use_signal(|| None::<String>);
    let mut diffs = use_signal(HashMap::<TaskId, String>::new);
    let mut expanded = use_signal(|| None::<TaskId>);

    let _ = (app.task_rev)();
    let mut tasks = orch.tasks();
    tasks.sort_by(|a, b| b.id.cmp(&a.id));
    let projects = app.projects.read().clone();
    let project_value = form_project()
        .or_else(|| app.selected.read().clone())
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let submit = {
        let orch = orch.clone();
        move |e: FormEvent| {
            e.prevent_default();
            let project = form_project().or_else(|| app.selected.read().clone());
            let prompt = form_prompt().trim().to_string();
            let Some(project) = project else { form_error.set(Some("Escolha um projeto.".into())); return; };
            if prompt.is_empty() { form_error.set(Some("Escreva o pedido.".into())); return; }
            match orch.submit(project, &form_provider(), &prompt) {
                Ok(id) => {
                    form_prompt.set(String::new());
                    form_error.set(None);
                    expanded.set(Some(id));
                }
                Err(e) => form_error.set(Some(e.to_string())),
            }
        }
    };

    rsx! {
        div { class: "pad scroll agents",
            section {
                h3 { "Provedores" }
                button { onclick: move |_| refresh_login(), "Atualizar status" }
                div { class: "providers",
                    for p in providers.0.iter().cloned() {
                        {
                            let id = p.id().to_string();
                            let st = login.read().get(&id).copied();
                            let (label, cls) = match st {
                                None => ("verificando…", "muted"),
                                Some(LoginStatus::NotInstalled) => ("não instalado", "err"),
                                Some(LoginStatus::LoggedOut) => ("sem login", "warn"),
                                Some(LoginStatus::LoggedIn) => ("conectado", "ok"),
                                Some(LoginStatus::Unknown) => ("desconhecido", "muted"),
                            };
                            let can_login = !matches!(st, Some(LoginStatus::NotInstalled));
                            let name = p.display_name().to_string();
                            rsx! {
                                div { key: "{id}", class: "card",
                                    b { "{name}" }
                                    span { class: "{cls}", " {label}" }
                                    if can_login {
                                        button {
                                            onclick: move |_| {
                                                let cwd = app.selected.read().clone()
                                                    .or_else(|| app.root.read().clone())
                                                    .or_else(|| std::env::current_dir().ok())
                                                    .unwrap_or_default();
                                                app.open_terminal(format!("login {}", p.id()), cwd.clone(), Some(to_pty(p.login_command(), cwd)));
                                            },
                                            "Fazer login"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            section {
                h3 { "Nova tarefa" }
                form { class: "task-form", onsubmit: submit,
                    select {
                        value: "{project_value}",
                        onchange: move |e| form_project.set(Some(PathBuf::from(e.value()))),
                        option { value: "", disabled: true, "Projeto…" }
                        for p in projects {
                            option { key: "{p.path.display()}", value: "{p.path.display()}", "{p.name}" }
                        }
                    }
                    select {
                        value: "{form_provider}",
                        onchange: move |e| form_provider.set(e.value()),
                        for p in providers.0.iter() {
                            option { key: "{p.id()}", value: "{p.id()}", "{p.display_name()}" }
                        }
                    }
                    textarea {
                        rows: 4,
                        placeholder: "Descreva o que o agente deve fazer…",
                        value: "{form_prompt}",
                        oninput: move |e| form_prompt.set(e.value()),
                    }
                    button { class: "primary", r#type: "submit", "Enviar" }
                    if let Some(e) = form_error() { span { class: "err", "{e}" } }
                }
            }
            if let Some(p) = form_project().or_else(|| app.selected.read().clone()) {
                section {
                    h3 { "Auto-work · {dir_name(&p)}" }
                    crate::ui::autowork::AutoWorkPanel { key: "{p.display()}", project: p.clone() }
                }
            }
            section {
                h3 { "Tarefas" }
                if tasks.is_empty() { p { class: "muted", "Nenhuma tarefa ainda." } }
                for t in tasks {
                    {
                        let id = t.id;
                        let open = expanded() == Some(id);
                        let running = matches!(t.state, TaskState::Queued | TaskState::Running);
                        let finished = matches!(t.state, TaskState::Completed | TaskState::Failed(_) | TaskState::Cancelled);
                        let has_wt = t.workdir.as_ref().is_some_and(|w| w != &t.project);
                        let log = app.task_log.read().get(&id).cloned().unwrap_or_default();
                        let diff = diffs.read().get(&id).cloned();
                        let (o1, o2, o3, o4) = (orch.clone(), orch.clone(), orch.clone(), orch.clone());
                        rsx! {
                            div { key: "{id}", class: "task",
                                div { class: "task-head", onclick: move |_| expanded.set(if open { None } else { Some(id) }),
                                    b { "#{id} " }
                                    span { "{dir_name(&t.project)} · {t.provider} · " }
                                    span { class: "muted", "{state_label(&t.state)}" }
                                    div { class: "small muted ellipsis", "{t.prompt}" }
                                }
                                crate::ui::autowork::AutoWorkBadge { task: id }
                                crate::ui::servers::SessionServersBadge { task: id }
                                if open {
                                    div { class: "task-actions",
                                        button { onclick: move |_| wb.open_session(id), "Abrir sessão" }
                                    if running {
                                            button { onclick: move |_| { let _ = o1.cancel(id); crate::ui::servers::confirm_stop_session(id); }, "Cancelar" }
                                        }
                                        if finished && has_wt {
                                            button { onclick: move |_| {
                                                    let o = o2.clone();
                                                    spawn(async move {
                                                        let d = o.diff(id).await.unwrap_or_else(|e| format!("erro: {e}"));
                                                        diffs.write().insert(id, if d.is_empty() { "(sem alterações)".into() } else { d });
                                                    });
                                                }, "Ver diff" }
                                            button { class: "primary", onclick: move |_| {
                                                    let o = o3.clone();
                                                    spawn(async move {
                                                        if let Err(e) = o.apply(id).await { app.task_log.write().entry(id).or_default().push(format!("[erro ao aplicar] {e}")); }
                                                        *app.task_rev.write() += 1;
                                                    });
                                                }, "Aplicar" }
                                            button { onclick: move |_| {
                                                    let o = o4.clone();
                                                    crate::ui::servers::confirm_stop_session(id);
                                                    spawn(async move {
                                                        if let Err(e) = o.discard(id).await { app.task_log.write().entry(id).or_default().push(format!("[erro ao descartar] {e}")); }
                                                        *app.task_rev.write() += 1;
                                                    });
                                                }, "Descartar" }
                                        }
                                    }
                                    {task_log_view(&log)}
                                    if let Some(d) = diff { pre { class: "diff", "{d}" } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
