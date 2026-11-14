//! Painel com o status git de todos os projetos.
use crate::state::use_app;
use dioxus::prelude::*;

#[component]
pub fn Dashboard() -> Element {
    let mut app = use_app();
    let projects = app.projects.read().clone();
    let statuses = app.statuses.read().clone();
    if projects.is_empty() {
        return rsx! { p { class: "muted pad", "Escolha uma pasta raiz com seus projetos." } };
    }
    rsx! {
        div { class: "pad scroll",
            table { class: "grid",
                thead {
                    tr { th { "Projeto" } th { "Branch" } th { "Alterados" } th { "↑ / ↓" } th { "Último commit" } }
                }
                tbody {
                    for p in projects {
                        {
                            let st = statuses.get(&p.path).cloned().flatten();
                            let path = p.path.clone();
                            let cells = match st {
                                Some(s) => {
                                    let commit = s
                                        .last_commit
                                        .as_ref()
                                        .map(|c| format!("{} {} ({})", c.id, c.message.lines().next().unwrap_or(""), c.author))
                                        .unwrap_or_default();
                                    let cls = if s.changed_files.is_empty() { "" } else { "warn" };
                                    rsx! {
                                        td { {s.branch.clone().unwrap_or_else(|| "(detached)".into())} }
                                        td { class: "{cls}", "{s.changed_files.len()}" }
                                        td { "{s.ahead} / {s.behind}" }
                                        td { "{commit}" }
                                    }
                                }
                                None => rsx! { td { colspan: 4, class: "muted", "sem repositório git" } },
                            };
                            rsx! {
                                tr { key: "{p.path.display()}", onclick: move |_| app.selected.set(Some(path.clone())),
                                    td { "{p.name}" }
                                    {cells}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
