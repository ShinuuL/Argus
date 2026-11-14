//! Barra lateral: projetos (com status git) e navegador de arquivos do projeto selecionado.
use crate::state::use_app;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

fn list_dir(dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut v: Vec<(PathBuf, bool)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    !(n.starts_with('.') || n == "node_modules" || n == "target")
                })
                .map(|e| (e.path(), e.path().is_dir()))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

pub fn dir_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

#[component]
pub fn Sidebar(on_pick: EventHandler<()>) -> Element {
    let mut app = use_app();
    let projects = app.projects.read().clone();
    let statuses = app.statuses.read().clone();
    let selected = app.selected.read().clone();
    let root = app.root.read().clone();

    rsx! {
        aside { class: "sidebar",
            button { class: "primary", onclick: move |_| on_pick.call(()), "Abrir pasta raiz" }
            if let Some(r) = root {
                p { class: "muted small", "{r.display()}" }
            }
            h4 { "Projetos" }
            ul { class: "projects",
                for p in projects {
                    {
                        let st = statuses.get(&p.path).cloned().flatten();
                        let path = p.path.clone();
                        let cls = if selected.as_ref() == Some(&p.path) { "sel" } else { "" };
                        let info = match &st {
                            Some(s) => {
                                let mut t = s.branch.clone().unwrap_or_else(|| "(detached)".into());
                                if !s.changed_files.is_empty() { t += &format!("  ● {}", s.changed_files.len()); }
                                if s.ahead > 0 { t += &format!("  ↑{}", s.ahead); }
                                if s.behind > 0 { t += &format!("  ↓{}", s.behind); }
                                t
                            }
                            None => format!("{:?}", p.kinds),
                        };
                        rsx! {
                            li { key: "{p.path.display()}", class: "{cls}",
                                onclick: move |_| app.selected.set(Some(path.clone())),
                                span { class: "name", "{p.name}" }
                                span { class: "small muted", "{info}" }
                            }
                        }
                    }
                }
            }
            if let Some(sel) = selected {
                h4 { "Arquivos" }
                div { class: "tree",
                    DirNode { key: "{sel.display()}", path: sel.clone(), depth: 0, open: true }
                }
                button {
                    onclick: move |_| {
                        let s = app.selected.read().clone();
                        if let Some(s) = s {
                            app.open_terminal(dir_name(&s), s, None);
                        }
                    },
                    "Novo terminal aqui"
                }
            }
        }
    }
}

#[component]
fn DirNode(path: PathBuf, depth: usize, open: bool) -> Element {
    let mut expanded = use_signal(|| open);
    let mut app = use_app();
    let children = if expanded() { list_dir(&path) } else { vec![] };
    let pad = format!("padding-left:{}px", depth * 12);
    let arrow = if expanded() { "▾ " } else { "▸ " };
    rsx! {
        if depth > 0 {
            div { class: "node dir", style: "{pad}", onclick: move |_| expanded.toggle(),
                "{arrow}{dir_name(&path)}"
            }
        }
        for (child, is_dir) in children {
            if is_dir {
                DirNode { key: "{child.display()}", path: child.clone(), depth: depth + 1, open: false }
            } else {
                {
                    let c = child.clone();
                    let pad = format!("padding-left:{}px", (depth + 1) * 12 + 10);
                    rsx! {
                        div { key: "{child.display()}", class: "node", style: "{pad}",
                            onclick: move |_| {
                                app.open_file.set(Some(c.clone()));
                            },
                            "{dir_name(&child)}"
                        }
                    }
                }
            }
        }
    }
}
