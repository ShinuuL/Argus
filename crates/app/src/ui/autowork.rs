//! Modo auto-work na interface: badge de progresso por sessão (com pausar /
//! retomar / parar) e painel de configuração por projeto (liga/desliga e travas).
use crate::state::use_app;
use dioxus::prelude::*;
use argus_core::agents::orchestrator::TaskId;
use argus_core::autowork::{AutoWork, AutoWorkEvent, AutoWorkSettings, AutoWorkStatus};
use std::path::PathBuf;

pub const CSS: &str = r#"
.aw-badge { display: inline-flex; gap: 6px; align-items: center; flex-wrap: wrap; font-size: 12px; }
.aw-pill { border-radius: 10px; padding: 1px 8px; background: #2a2e37; white-space: nowrap; }
.aw-pill.run { background: #1f3b5a; color: #9fd0ff; }
.aw-pill.done { background: #1d3d2b; color: #5fd38d; }
.aw-pill.stop { background: #45302a; color: #f0a37a; }
.aw-pill.pause { background: #3d3720; color: #e8b04a; }
.aw-badge button { padding: 1px 8px; font-size: 12px; }
.aw-actions { display: inline-flex; gap: 6px; flex: none; white-space: nowrap; }
.aw-badge button { white-space: nowrap; }
.aw-item { max-width: 360px; min-width: 0; flex: 0 1 auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.aw-panel { display: flex; flex-direction: column; gap: 8px; }
.aw-panel input[type=number] { width: 110px; }
"#;

/// Linha de log para um evento de auto-work.
pub fn describe(e: &AutoWorkEvent) -> String {
    let st = match &e.status {
        AutoWorkStatus::Running => "rodando".to_string(),
        AutoWorkStatus::Paused => "pausado".into(),
        AutoWorkStatus::Completed => "concluído".into(),
        AutoWorkStatus::Stopped(r) => format!("parado: {r}"),
    };
    let item = e.current_item.as_deref().map(|i| format!(" · item: {i}")).unwrap_or_default();
    format!("[auto-work] {st} · iteração {}/{} · {}/{} itens{item}", e.iteration, e.max_iterations, e.done, e.total)
}

fn pill(e: &AutoWorkEvent) -> (&'static str, String) {
    match &e.status {
        AutoWorkStatus::Running => ("run", format!("auto {}/{} · it {}/{}", e.done, e.total, e.iteration, e.max_iterations)),
        AutoWorkStatus::Paused => ("pause", format!("auto pausado {}/{}", e.done, e.total)),
        AutoWorkStatus::Completed => ("done", format!("auto concluído {}/{}", e.done, e.total)),
        AutoWorkStatus::Stopped(r) => ("stop", format!("auto parado: {r}")),
    }
}

/// Badge de progresso + controles para uma tarefa (vazio se não houver execução).
#[component]
pub fn AutoWorkBadge(task: TaskId) -> Element {
    let app = use_app();
    let aw = use_context::<AutoWork>();
    let _ = (app.task_rev)();
    let Some(e) = aw.status(task) else {
        return rsx! {};
    };
    let (cls, label) = pill(&e);
    let tip = format!("{}\ntokens: {} · US$ {:.2}", describe(&e), e.tokens, e.cost_usd);
    let (a1, a2, a3, a4) = (aw.clone(), aw.clone(), aw.clone(), aw.clone());
    rsx! {
        span { class: "aw-badge",
            span { class: "aw-pill {cls}", title: "{tip}", "{label}" }
            if let Some(i) = e.current_item.clone() { span { class: "aw-item small muted", title: "{i}", "{i}" } }
            span { class: "aw-actions",
            match e.status {
                AutoWorkStatus::Running => rsx! {
                    button { onclick: move |_| { a1.pause(task); }, "Pausar" }
                    button { onclick: move |_| { a2.stop(task); }, "Parar" }
                },
                AutoWorkStatus::Paused => rsx! {
                    button { onclick: move |_| { a3.resume(task); }, "Retomar" }
                    button { onclick: move |_| { a2.stop(task); }, "Parar" }
                },
                _ => rsx! {
                    button { onclick: move |_| { a4.start_task(task); }, "Reiniciar auto-work" }
                },
            }
            }
        }
    }
}

fn parse_opt<T: std::str::FromStr>(s: &str) -> Option<T> {
    let s = s.trim();
    if s.is_empty() { None } else { s.parse().ok() }
}

/// Liga/desliga e limites do auto-work de um projeto (persistido no banco).
#[component]
pub fn AutoWorkPanel(project: PathBuf) -> Element {
    let aw = use_context::<AutoWork>();
    let rev = use_signal(|| 0u64);
    let _ = rev();
    let s: AutoWorkSettings = aw.settings(&project);
    let save = {
        let aw = aw.clone();
        let project = project.clone();
        move |f: Box<dyn FnOnce(&mut AutoWorkSettings)>| {
            let mut s = aw.settings(&project);
            f(&mut s);
            aw.set_settings(&project, s);
            let mut r = rev;
            *r.write() += 1;
        }
    };
    let (s1, s2, s3, s4, s5) = (save.clone(), save.clone(), save.clone(), save.clone(), save.clone());
    let tokens = s.max_tokens.map(|t| t.to_string()).unwrap_or_default();
    let cost = s.max_cost_usd.map(|c| c.to_string()).unwrap_or_default();
    rsx! {
        div { class: "aw-panel",
            label { class: "row",
                input { r#type: "checkbox", checked: s.enabled,
                    onchange: move |e| { let v = e.checked(); s1(Box::new(move |s| s.enabled = v)); } }
                b { "Auto-work neste projeto" }
            }
            p { class: "small muted",
                "Ao fim de cada turno, varre PENDENCIAS.md, TODO/FIXME e build/testes, e dispara novos turnos até tudo ficar [x]."
            }
            div { class: "row",
                label { class: "small", "Máx. iterações " }
                input { r#type: "number", min: "1", value: "{s.max_iterations}",
                    onchange: move |e| { if let Some(v) = parse_opt::<u32>(&e.value()) { s2(Box::new(move |s| s.max_iterations = v.max(1))); } } }
                label { class: "small", "Teto de tokens " }
                input { r#type: "number", min: "0", placeholder: "sem teto", value: "{tokens}",
                    onchange: move |e| { let v = parse_opt::<u64>(&e.value()); s3(Box::new(move |s| s.max_tokens = v)); } }
                label { class: "small", "Teto US$ " }
                input { r#type: "number", min: "0", step: "0.5", placeholder: "sem teto", value: "{cost}",
                    onchange: move |e| { let v = parse_opt::<f64>(&e.value()); s4(Box::new(move |s| s.max_cost_usd = v)); } }
            }
            label { class: "row small",
                input { r#type: "checkbox", checked: s.run_checks,
                    onchange: move |e| { let v = e.checked(); s5(Box::new(move |s| s.run_checks = v)); } }
                "Rodar build/testes na verificação (timeout {s.check_timeout_secs}s)"
            }
        }
    }
}
