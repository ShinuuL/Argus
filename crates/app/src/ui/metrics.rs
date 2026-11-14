//! Tela de métricas: tokens gastos e custo equivalente em API (USD) por projeto,
//! por provedor e por dia, com filtro de período.
//!
//! Fonte: tabela `usage` do SQLite do orquestrador (`Orchestrator::db()`), preços de
//! `argus_core::metrics::Pricing` (padrões + `%APPDATA%/argus/pricing.json`).
//! Sem banco (orquestrador criado com `Orchestrator::new`), mostra um aviso.

use crate::state::{use_app, use_orchestrator};
use dioxus::prelude::*;
use argus_core::db::{now, TimeRange, UsageRow};
use argus_core::metrics::{self, CostLine, Pricing};
use std::collections::HashMap;
use std::path::Path;

/// Período do filtro (0 = tudo).
const PERIODS: [(i64, &str); 3] = [(7, "7 dias"), (30, "30 dias"), (0, "Tudo")];

#[derive(Clone, PartialEq, Default)]
struct Report {
    total: CostLine,
    projects: Vec<CostLine>,
    providers: Vec<CostLine>,
    models: Vec<CostLine>,
    /// (AAAA-MM-DD, linha) para todos os dias do período, inclusive os sem uso.
    days: Vec<(String, CostLine)>,
    has_db: bool,
}

fn build_report(rows: &[UsageRow], days: i64, has_db: bool) -> Report {
    let pricing = Pricing::default_path().map(|p| Pricing::load(&p)).unwrap_or_default();
    let mut total = metrics::aggregate(rows, &pricing, |_| "total".into())
        .pop()
        .unwrap_or_default();
    total.key = "Total".into();
    let sort = |mut v: Vec<CostLine>| {
        v.sort_by(|a, b| b.api_cost_usd.total_cmp(&a.api_cost_usd).then(b.total_tokens().cmp(&a.total_tokens())));
        v
    };
    let by_day: HashMap<String, CostLine> = metrics::by_day(rows, &pricing)
        .into_iter()
        .map(|l| (l.key.clone(), l))
        .collect();
    // Eixo de dias contínuo. Em "Tudo", do primeiro registro até hoje (máx. 90 dias).
    let span = if days > 0 {
        days
    } else {
        rows.iter()
            .map(|r| r.created_at)
            .min()
            .map(|first| ((now() - first) / 86_400 + 1).clamp(1, 90))
            .unwrap_or(7)
    };
    let t = now();
    let day_axis = (0..span)
        .rev()
        .map(|i| {
            let d = metrics::day_string(t - i * 86_400);
            let line = by_day.get(&d).cloned().unwrap_or_else(|| CostLine { key: d.clone(), ..Default::default() });
            (d, line)
        })
        .collect();
    Report {
        total,
        projects: sort(metrics::by_project(rows, &pricing)),
        providers: sort(metrics::by_provider(rows, &pricing)),
        models: sort(metrics::by_model(rows, &pricing)),
        days: day_axis,
        has_db,
    }
}

pub fn fmt_tokens(n: u64) -> String {
    match n {
        n if n >= 1_000_000_000 => format!("{:.2}B", n as f64 / 1e9),
        n if n >= 1_000_000 => format!("{:.2}M", n as f64 / 1e6),
        n if n >= 10_000 => format!("{:.1}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

pub fn fmt_usd(v: f64) -> String {
    if v > 0.0 && v < 0.01 {
        format!("${v:.4}")
    } else {
        format!("${v:.2}")
    }
}

fn project_label(key: &str) -> String {
    Path::new(key)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| key.to_string())
}

/// Tela de métricas. Recarrega quando chegam eventos do orquestrador.
#[component]
pub fn MetricsView() -> Element {
    let app = use_app();
    let orch = use_orchestrator();
    let mut period = use_signal(|| 7i64);
    let mut report = use_signal(Report::default);
    let mut loading = use_signal(|| true);

    use_effect(move || {
        let days = period();
        let _rev = (app.task_rev)(); // dependência: atualiza com novos turnos
        let orch = orch.clone();
        spawn(async move {
            loading.set(true);
            let r = tokio::task::spawn_blocking(move || {
                let range = if days > 0 { TimeRange::last_days(days) } else { TimeRange::all() };
                match orch.db() {
                    Some(db) => build_report(&db.usage_rows(range).unwrap_or_default(), days, true),
                    None => build_report(&[], days, false),
                }
            })
            .await
            .unwrap_or_default();
            report.set(r);
            loading.set(false);
        });
    });

    let r = report();
    let max_proj = r.projects.iter().map(|l| l.api_cost_usd).fold(0.0, f64::max);
    let max_prov = r.providers.iter().map(|l| l.api_cost_usd).fold(0.0, f64::max);

    rsx! {
        style { {METRICS_CSS} }
        div { class: "metrics scroll pad",
            div { class: "m-head",
                h3 { "Uso e custo" }
                div { class: "m-filter",
                    for (d, label) in PERIODS {
                        button {
                            class: if period() == d { "primary" } else { "" },
                            onclick: move |_| period.set(d),
                            "{label}"
                        }
                    }
                }
            }
            if !r.has_db && !loading() {
                div { class: "warn small", "Histórico desligado: o orquestrador está sem banco de dados, então o uso não é registrado." }
            }
            p { class: "muted small",
                "Custo equivalente em API: o valor que os tokens custariam pela API paga. Com assinatura, não é o valor cobrado. Preços editáveis em pricing.json."
            }
            div { class: "m-kpis",
                Kpi { label: "Custo equivalente", value: fmt_usd(r.total.api_cost_usd) }
                Kpi { label: "Tokens", value: fmt_tokens(r.total.total_tokens()) }
                Kpi { label: "Entrada / saída", value: format!("{} / {}", fmt_tokens(r.total.input_tokens), fmt_tokens(r.total.output_tokens)) }
                Kpi { label: "Cache (leitura / escrita)", value: format!("{} / {}", fmt_tokens(r.total.cache_read), fmt_tokens(r.total.cache_write)) }
                Kpi { label: "Turnos", value: r.total.turns.to_string() }
            }

            section {
                h4 { "Por dia" }
                DayChart { days: r.days.clone() }
            }
            div { class: "m-cols",
                section {
                    h4 { "Por projeto" }
                    BarList { lines: r.projects.clone(), max: max_proj, project: true }
                }
                section {
                    h4 { "Por provedor" }
                    BarList { lines: r.providers.clone(), max: max_prov, project: false }
                    h4 { "Por modelo" }
                    table { class: "grid small",
                        thead { tr { th { "Modelo" } th { "Tokens" } th { "Custo" } } }
                        tbody {
                            for l in r.models.iter() {
                                tr { key: "{l.key}",
                                    td { "{l.key}" }
                                    td { {fmt_tokens(l.total_tokens())} }
                                    td { {fmt_usd(l.api_cost_usd)} }
                                }
                            }
                        }
                    }
                }
            }
            if r.total.turns == 0 && !loading() && r.has_db {
                div { class: "muted", "Nenhum uso registrado no período." }
            }
        }
    }
}

#[component]
fn Kpi(label: String, value: String) -> Element {
    rsx! {
        div { class: "m-kpi",
            div { class: "m-kpi-v", "{value}" }
            div { class: "muted small", "{label}" }
        }
    }
}

/// Barras horizontais (custo), com tokens ao lado.
#[component]
fn BarList(lines: Vec<CostLine>, max: f64, project: bool) -> Element {
    if lines.is_empty() {
        return rsx! { div { class: "muted small", "Sem dados." } };
    }
    rsx! {
        div { class: "m-bars",
            for l in lines {
                {
                    let pct = if max > 0.0 { (l.api_cost_usd / max * 100.0).max(1.0) } else { 1.0 };
                    let label = if project { project_label(&l.key) } else { l.key.clone() };
                    rsx! {
                        div { key: "{l.key}", class: "m-bar-row", title: "{l.key}",
                            div { class: "m-bar-label ellipsis", "{label}" }
                            svg { class: "m-bar", view_box: "0 0 100 10", preserve_aspect_ratio: "none",
                                rect { x: "0", y: "0", width: "100", height: "10", fill: "#22262e" }
                                rect { x: "0", y: "0", width: "{pct}", height: "10", fill: "#3b6ef5" }
                            }
                            div { class: "m-bar-val", "{fmt_usd(l.api_cost_usd)}" }
                            div { class: "m-bar-val muted", "{fmt_tokens(l.total_tokens())}" }
                        }
                    }
                }
            }
        }
    }
}

/// Colunas por dia (custo) + linha de tokens, em SVG inline.
#[component]
fn DayChart(days: Vec<(String, CostLine)>) -> Element {
    if days.is_empty() {
        return rsx! { div { class: "muted small", "Sem dados." } };
    }
    let n = days.len() as f64;
    let (w, h) = (600.0f64, 140.0f64);
    let slot = w / n;
    let bar_w = (slot * 0.7).max(1.0);
    let max_cost = days.iter().map(|(_, l)| l.api_cost_usd).fold(0.0, f64::max);
    let max_tok = days.iter().map(|(_, l)| l.total_tokens()).max().unwrap_or(0) as f64;
    let points = days
        .iter()
        .enumerate()
        .map(|(i, (_, l))| {
            let x = slot * i as f64 + slot / 2.0;
            let y = if max_tok > 0.0 { h - 4.0 - (l.total_tokens() as f64 / max_tok) * (h - 12.0) } else { h - 4.0 };
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let first = days.first().map(|d| d.0.clone()).unwrap_or_default();
    let last = days.last().map(|d| d.0.clone()).unwrap_or_default();
    rsx! {
        div { class: "m-chart",
            svg { view_box: "0 0 {w} {h}", preserve_aspect_ratio: "none", class: "m-day-svg",
                for (i, (day, l)) in days.iter().enumerate() {
                    {
                        let bh = if max_cost > 0.0 { (l.api_cost_usd / max_cost) * (h - 12.0) } else { 0.0 };
                        let x = slot * i as f64 + (slot - bar_w) / 2.0;
                        let y = h - bh;
                        let tip = format!("{day}: {} · {} tokens · {} turnos", fmt_usd(l.api_cost_usd), fmt_tokens(l.total_tokens()), l.turns);
                        rsx! {
                            rect { key: "{day}", x: "{x:.1}", y: "{y:.1}", width: "{bar_w:.1}", height: "{bh.max(0.5):.1}", fill: "#3b6ef5", rx: "1",
                                title { "{tip}" }
                            }
                        }
                    }
                }
                polyline { points: "{points}", fill: "none", stroke: "#e8b04a", stroke_width: "1.5", vector_effect: "non-scaling-stroke" }
            }
            div { class: "m-axis small muted",
                span { "{first}" }
                span { "barras: custo (máx. {fmt_usd(max_cost)}) · linha: tokens" }
                span { "{last}" }
            }
        }
    }
}

const METRICS_CSS: &str = r#"
.metrics h3 { margin: 0; }
.metrics h4 { margin: 18px 0 8px; font-size: 11px; text-transform: uppercase; opacity: .6; }
.m-head { display: flex; justify-content: space-between; align-items: center; gap: 8px; flex-wrap: wrap; }
.m-filter { display: flex; gap: 4px; }
.m-kpis { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 8px; }
.m-kpi { background: #1d2027; border: 1px solid #2a2e37; border-radius: 6px; padding: 10px; }
.m-kpi-v { font-size: 18px; font-weight: 600; font-variant-numeric: tabular-nums; }
.m-chart { background: #1b1e24; border: 1px solid #2a2e37; border-radius: 6px; padding: 8px; }
.m-day-svg { width: 100%; height: 140px; display: block; }
.m-axis { display: flex; justify-content: space-between; margin-top: 4px; gap: 8px; }
.m-cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 16px; }
.m-bars { display: flex; flex-direction: column; gap: 6px; }
.m-bar-row { display: grid; grid-template-columns: minmax(80px, 160px) 1fr 70px 60px; gap: 8px; align-items: center; }
.m-bar { width: 100%; height: 10px; border-radius: 3px; }
.m-bar-val { text-align: right; font-variant-numeric: tabular-nums; }
"#;
