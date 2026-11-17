//! Métricas: tokens e custo equivalente em API (USD) por projeto, dia, provedor e modelo.
//!
//! Fonte: tabela `usage` do banco do orquestrador; preços de `Pricing`
//! (padrões + `%APPDATA%/argus/pricing.json`).
use super::{blocking, err};
use crate::state::AppCtx;
use argus_core::db::{now, ProjectUsage, TimeRange, UsageRow};
use argus_core::metrics::{self, CostLine, Pricing};
use chrono::{Local, TimeZone};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DayLine {
    /// AAAA-MM-DD.
    pub day: String,
    pub line: CostLine,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Report {
    pub total: CostLine,
    pub projects: Vec<CostLine>,
    pub providers: Vec<CostLine>,
    pub models: Vec<CostLine>,
    /// Todos os dias do período, inclusive os sem uso.
    pub days: Vec<DayLine>,
    /// false = histórico desligado (sem banco): o uso não é registrado.
    pub has_db: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct TodayUsage {
    pub tokens: u64,
    /// Custo equivalente em API (USD).
    pub cost: f64,
}

fn load_pricing() -> Pricing {
    Pricing::default_path().map(|p| Pricing::load(&p)).unwrap_or_default()
}

/// Monta o relatório. `days` = tamanho do período (0 = tudo, eixo de até 90 dias).
/// Os dias são os do calendário no fuso `tz` (na interface, o do computador).
pub fn build_report<Tz: TimeZone>(rows: &[UsageRow], days: i64, has_db: bool, pricing: &Pricing, t: i64, tz: &Tz) -> Report {
    let mut total = metrics::aggregate(rows, pricing, |_| "total".into()).pop().unwrap_or_default();
    total.key = "Total".into();
    let sort = |mut v: Vec<CostLine>| {
        v.sort_by(|a, b| b.api_cost_usd.total_cmp(&a.api_cost_usd).then(b.total_tokens().cmp(&a.total_tokens())));
        v
    };
    let by_day: HashMap<String, CostLine> =
        metrics::by_day_in(rows, pricing, tz).into_iter().map(|l| (l.key.clone(), l)).collect();
    let span = if days > 0 {
        days
    } else {
        rows.iter()
            .map(|r| r.created_at)
            .min()
            .map(|first| ((metrics::day_in(t, tz) - metrics::day_in(first, tz)).num_days() + 1).clamp(1, 90))
            .unwrap_or(7)
    };
    let day_axis = metrics::last_days_in(t, span.max(1) as u64, tz)
        .into_iter()
        .map(|d| {
            let day = d.to_string();
            let line = by_day.get(&day).cloned().unwrap_or_else(|| CostLine { key: day.clone(), ..Default::default() });
            DayLine { day, line }
        })
        .collect();
    Report {
        total,
        projects: sort(metrics::by_project(rows, pricing)),
        providers: sort(metrics::by_provider(rows, pricing)),
        models: sort(metrics::by_model(rows, pricing)),
        days: day_axis,
        has_db,
    }
}

/// Relatório de uso e custo equivalente dos últimos `days` dias (0 = tudo).
#[tauri::command]
pub async fn metrics_report(ctx: State<'_, AppCtx>, days: i64) -> Result<Report, String> {
    let orch = ctx.orch.clone();
    blocking(move || {
        let t = now();
        // O período começa à meia-noite local do primeiro dia do eixo (dias inteiros).
        let range = match metrics::last_days_in(t, days.max(0) as u64, &Local).first() {
            Some(first) => TimeRange { from: Some(metrics::day_start_in(*first, &Local)), to: None },
            None => TimeRange::all(),
        };
        let pricing = load_pricing();
        Ok(match orch.db() {
            Some(db) => build_report(&db.usage_rows(range).unwrap_or_default(), days, true, &pricing, t, &Local),
            None => build_report(&[], days, false, &pricing, t, &Local),
        })
    })
    .await
}

/// Tokens e custo equivalente de hoje (dia do calendário no fuso do computador).
#[tauri::command]
pub async fn metrics_today(ctx: State<'_, AppCtx>) -> Result<TodayUsage, String> {
    let orch = ctx.orch.clone();
    blocking(move || {
        let Some(db) = orch.db() else { return Ok(TodayUsage::default()) };
        let from = metrics::day_start_in(metrics::day_in(now(), &Local), &Local);
        let rows = db.usage_rows(TimeRange { from: Some(from), to: None }).map_err(err)?;
        let pricing = load_pricing();
        Ok(TodayUsage {
            tokens: rows
                .iter()
                .map(|r| r.usage.input_tokens + r.usage.output_tokens + r.usage.cache_read + r.usage.cache_write)
                .sum(),
            // A soma de uma lista vazia de f64 é -0.0, que a interface mostrava como "-0,00".
            cost: rows.iter().map(|r| pricing.cost(&r.usage)).sum::<f64>() + 0.0,
        })
    })
    .await
}

/// Linhas brutas de uso no intervalo (um registro por turno).
#[tauri::command]
pub async fn metrics_usage_rows(ctx: State<'_, AppCtx>, range: Option<TimeRange>) -> Result<Vec<UsageRow>, String> {
    let orch = ctx.orch.clone();
    blocking(move || match orch.db() {
        Some(db) => db.usage_rows(range.unwrap_or_default()).map_err(err),
        None => Ok(vec![]),
    })
    .await
}

/// Uso agregado por projeto, provedor e modelo no intervalo.
#[tauri::command]
pub async fn metrics_usage_by_project(ctx: State<'_, AppCtx>, range: Option<TimeRange>) -> Result<Vec<ProjectUsage>, String> {
    let orch = ctx.orch.clone();
    blocking(move || match orch.db() {
        Some(db) => db.usage_by_project(range.unwrap_or_default()).map_err(err),
        None => Ok(vec![]),
    })
    .await
}

/// Tabela de preços em uso (padrões mesclados com o `pricing.json`).
#[tauri::command]
pub async fn pricing_get() -> Result<Pricing, String> {
    blocking(|| Ok(load_pricing())).await
}

/// Salva a tabela de preços no `pricing.json`.
#[tauri::command]
pub async fn pricing_set(pricing: Pricing) -> Result<Pricing, String> {
    blocking(move || {
        let path = Pricing::default_path().ok_or("pasta de dados indisponível")?;
        pricing.save(&path).map_err(err)?;
        Ok(Pricing::load(&path))
    })
    .await
}

/// Caminho do `pricing.json` (None se não houver pasta de dados).
#[tauri::command]
pub async fn pricing_path() -> Result<Option<PathBuf>, String> {
    Ok(Pricing::default_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use argus_core::agents::provider::TokenUsage;

    fn row(project: &str, provider: &str, at: i64, input: u64) -> UsageRow {
        UsageRow {
            task_id: 1,
            project: project.into(),
            provider: provider.into(),
            usage: TokenUsage { input_tokens: input, output_tokens: 10, ..Default::default() },
            created_at: at,
            // cada linha de teste é um turno próprio
            turn: Some(at * 1_000 + input as i64),
        }
    }

    #[test]
    fn report_has_continuous_day_axis_and_totals() {
        let t = 20_000 * 86_400 + 3_600;
        let rows = vec![row("a", "claude", t - 86_400, 100), row("b", "opencode", t, 300), row("a", "claude", t, 50)];
        let r = build_report(&rows, 7, true, &Pricing::default(), t, &chrono::Utc);
        assert_eq!(r.days.len(), 7);
        assert_eq!(r.days.last().unwrap().day, metrics::day_string(t));
        assert_eq!(r.days[6].line.turns, 2);
        assert_eq!(r.days[5].line.turns, 1);
        assert_eq!(r.days[0].line.turns, 0);
        assert_eq!(r.total.key, "Total");
        assert_eq!(r.total.turns, 3);
        assert_eq!(r.total.input_tokens, 450);
        assert_eq!(r.projects.len(), 2);
        assert_eq!(r.providers.len(), 2);
        assert!(r.has_db);
    }

    #[test]
    fn report_all_time_spans_from_first_row() {
        let t = 20_000 * 86_400;
        let r = build_report(&[row("a", "claude", t - 3 * 86_400, 1)], 0, true, &Pricing::default(), t, &chrono::Utc);
        assert_eq!(r.days.len(), 4);
        let empty = build_report(&[], 0, false, &Pricing::default(), t, &chrono::Utc);
        assert_eq!(empty.days.len(), 7);
        assert_eq!(empty.total.turns, 0);
        assert!(!empty.has_db);
    }

    /// "Hoje" segue o fuso: um turno às 22h de Brasília fica no dia local, e um
    /// turno com dois modelos conta como um turno no dia e no total.
    #[test]
    fn report_uses_local_days_and_counts_turns_once() {
        let brt = chrono::FixedOffset::west_opt(3 * 3600).unwrap();
        // 2026-11-14 22:30 em Brasília (2026-11-15 01:30 UTC).
        let t = 1_794_706_200;
        let two = |model: &str| UsageRow {
            task_id: 9,
            project: "a".into(),
            provider: "claude".into(),
            usage: TokenUsage { input_tokens: 1, model: Some(model.into()), ..Default::default() },
            created_at: t - 60,
            turn: Some(42),
        };
        let rows = vec![two("claude-opus-4-7"), two("claude-haiku-4-5")];
        let r = build_report(&rows, 7, true, &Pricing::default(), t, &brt);
        assert_eq!(r.days.last().unwrap().day, "2026-11-14");
        assert_eq!(r.days.last().unwrap().line.turns, 1);
        assert_eq!(r.days.first().unwrap().day, "2026-11-08");
        assert_eq!(r.total.turns, 1);
        assert_eq!(r.projects[0].turns, 1);
        assert_eq!(r.models.len(), 2);
        // Em UTC o mesmo instante já cairia no dia seguinte.
        assert_eq!(build_report(&rows, 7, true, &Pricing::default(), t, &chrono::Utc).days.last().unwrap().day, "2026-11-15");
    }
}
