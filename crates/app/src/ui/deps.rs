//! Painel de dependências do projeto e modal de confirmação de instalação.
//!
//! Regra: NADA é instalado sem o usuário clicar em "Executar" no `InstallPrompt`,
//! que sempre mostra o comando exato. A execução acontece numa aba do terminal
//! embutido, onde o usuário acompanha (e pode interromper) a instalação.
//!
//! Integração:
//! - `DepsPanel { project }` em qualquer aba/painel.
//! - `InstallPrompt {}` uma vez na raiz do app (renderiza o modal global).
//! - `on_terminal_output(&texto)` na saída do terminal para sugerir instalação.

use crate::state::use_app;
use dioxus::prelude::*;
use argus_core::deps::{self, DepIssue, DepKind, InstallPlan};
use argus_core::pty;
use std::collections::HashSet;
use std::path::PathBuf;

/// Instalação aguardando confirmação explícita.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingInstall {
    pub title: String,
    pub reason: String,
    pub plan: InstallPlan,
}

/// Modal pendente (único, global).
pub static PENDING: GlobalSignal<Option<PendingInstall>> = Signal::global(|| None);
/// Comandos já sugeridos a partir do terminal (evita repetir a sugestão).
static SUGGESTED: GlobalSignal<HashSet<String>> = Signal::global(HashSet::new);

/// Pede confirmação para um plano (abre o modal). Não executa nada.
pub fn request_install(title: impl Into<String>, reason: impl Into<String>, plan: InstallPlan) {
    *PENDING.write() = Some(PendingInstall { title: title.into(), reason: reason.into(), plan });
}

/// Hook para a saída do terminal: se reconhecer "comando não encontrado" de uma
/// ferramenta conhecida, abre o modal de confirmação (uma vez por comando).
/// Devolve o nome do comando detectado.
pub fn on_terminal_output(text: &str) -> Option<String> {
    let cmd = deps::detect_missing_command(text)?;
    let plan = deps::install_plan_for(&cmd)?;
    let key = cmd.to_ascii_lowercase();
    if SUGGESTED.peek().contains(&key) || PENDING.peek().is_some() {
        return Some(cmd);
    }
    SUGGESTED.write().insert(key);
    request_install(
        format!("Instalar {cmd}?"),
        format!("O terminal informou que `{cmd}` não foi encontrado. Deseja instalá-lo?"),
        plan,
    );
    Some(cmd)
}

fn to_pty(spec: &argus_core::agents::provider::CommandSpec) -> pty::CommandSpec {
    pty::CommandSpec {
        program: spec.program.clone(),
        args: spec.args.clone(),
        cwd: spec.cwd.clone(),
        env: spec.env.clone(),
    }
}

fn kind_label(k: DepKind) -> &'static str {
    match k {
        DepKind::MissingToolchain => "Ferramenta ausente",
        DepKind::ProjectDepsMissing => "Dependências não instaladas",
        DepKind::ProjectDepsOutdated => "Dependências desatualizadas",
    }
}

#[component]
pub fn DepsPanel(project: PathBuf) -> Element {
    let mut rev = use_signal(|| 0u64);
    let proj = project.clone();
    let issues = use_resource(use_reactive!(|proj| {
        let _ = rev();
        async move {
            tokio::task::spawn_blocking(move || deps::analyze(&proj)).await.unwrap_or_default()
        }
    }));

    let body = match &*issues.read() {
        None => rsx! { p { class: "muted", "Verificando dependências..." } },
        Some(list) if list.is_empty() => rsx! { p { class: "muted", "Nenhuma dependência pendente." } },
        Some(list) => {
            let list: Vec<DepIssue> = list.clone();
            rsx! {
                div { class: "deps-list",
                    for (i, issue) in list.into_iter().enumerate() {
                        {
                            let cmd = issue.fix.display();
                            let it = issue.clone();
                            rsx! {
                                div { key: "{i}", class: "dep-item",
                                    span { class: "warn small", {kind_label(issue.kind)} }
                                    div { class: "dep-head",
                                        b { class: "ellipsis", "{issue.tool}" }
                                        button {
                                            title: "Mostra o comando e pede confirmação antes de executar",
                                            onclick: move |_| request_install(
                                                format!("Instalar {}?", it.tool),
                                                it.description.clone(),
                                                it.fix.clone(),
                                            ),
                                            "Instalar..."
                                        }
                                    }
                                    div { class: "small", "{issue.description}" }
                                    code { class: "dep-cmd small", title: "{cmd}", "{cmd}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        div { class: "pad deps-panel",
            div { class: "row deps-top",
                h3 { "Dependências" }
                button { onclick: move |_| rev += 1, "Verificar novamente" }
            }
            {body}
        }
    }
}

/// Modal global de confirmação. Renderize uma vez na raiz do app.
#[component]
pub fn InstallPrompt() -> Element {
    let app = use_app();
    let Some(p) = PENDING.read().clone() else {
        return rsx! {};
    };
    let cmdline = p.plan.display();
    let cwd = p
        .plan
        .command
        .cwd
        .clone()
        .or_else(|| app.selected.peek().clone())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let cwd_txt = cwd.display().to_string();
    let plan = p.plan.clone();
    let title = p.title.clone();

    rsx! {
        div {
            class: "modal-backdrop",
            style: "position:fixed;inset:0;background:rgba(0,0,0,.55);display:flex;align-items:center;justify-content:center;z-index:1000;",
            div {
                class: "modal",
                style: "background:#252526;color:#ddd;border:1px solid #444;border-radius:6px;padding:16px 20px;max-width:640px;width:90%;",
                h3 { "{p.title}" }
                p { "{p.reason}" }
                p { "O comando abaixo será executado em uma nova aba do terminal:" }
                pre {
                    style: "background:#1e1e1e;padding:8px;border-radius:4px;white-space:pre-wrap;word-break:break-all;user-select:text;",
                    "{cmdline}"
                }
                p { class: "muted", "Pasta: {cwd_txt}" }
                if p.plan.needs_admin {
                    p { class: "warn", "Este comando requer privilégios de administrador." }
                }
                div { style: "display:flex;gap:8px;justify-content:flex-end;margin-top:12px;",
                    button { onclick: move |_| *PENDING.write() = None, "Cancelar" }
                    button {
                        class: "primary",
                        onclick: move |_| {
                            app.open_terminal(title.clone(), cwd.clone(), Some(to_pty(&plan.command)));
                            *PENDING.write() = None;
                        },
                        "Executar"
                    }
                }
            }
        }
    }
}
