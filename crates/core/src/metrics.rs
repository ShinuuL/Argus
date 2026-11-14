//! Métricas de consumo de tokens e custo "equivalente em API".
//!
//! O usuário normalmente usa assinatura (OAuth) nas CLIs; o valor calculado
//! aqui é quanto o mesmo consumo custaria pagando a API por token.
//! Preços em USD por 1 milhão de tokens, editáveis via JSON (`pricing.json`).

use crate::agents::provider::TokenUsage;
use crate::db::UsageRow;
use chrono::{Days, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Preço de um modelo (USD por 1M tokens).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModelPrice {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

impl ModelPrice {
    /// Cache read = 0,1x e cache write = 1,25x do input (cache de 5 min).
    pub const fn standard(input: f64, output: f64) -> Self {
        Self { input, output, cache_read: input * 0.1, cache_write: input * 1.25 }
    }
}

/// Tabela de preços. As chaves são trechos do nome do modelo; vence a chave
/// mais longa contida no nome (ex.: "opus-4-1" antes de "opus").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pricing {
    pub models: BTreeMap<String, ModelPrice>,
    pub fallback: ModelPrice,
    /// Se true, usa o custo informado pela CLI quando ele for > 0.
    #[serde(default = "yes")]
    pub prefer_reported: bool,
}

fn yes() -> bool {
    true
}

impl Default for Pricing {
    fn default() -> Self {
        let mut m = BTreeMap::new();
        m.insert("opus".into(), ModelPrice::standard(5.0, 25.0));
        m.insert("opus-4-1".into(), ModelPrice::standard(15.0, 75.0));
        m.insert("opus-4-20250514".into(), ModelPrice::standard(15.0, 75.0));
        m.insert("sonnet".into(), ModelPrice::standard(3.0, 15.0));
        m.insert("haiku".into(), ModelPrice::standard(1.0, 5.0));
        m.insert("haiku-3-5".into(), ModelPrice::standard(0.8, 4.0));
        m.insert("3-5-haiku".into(), ModelPrice::standard(0.8, 4.0));
        Self { models: m, fallback: ModelPrice::standard(3.0, 15.0), prefer_reported: true }
    }
}

/// Arquivo JSON parcial: só o que o usuário quiser sobrescrever.
#[derive(Debug, Default, Deserialize)]
struct PricingOverride {
    #[serde(default)]
    models: BTreeMap<String, ModelPrice>,
    fallback: Option<ModelPrice>,
    prefer_reported: Option<bool>,
}

impl Pricing {
    /// Caminho padrão: `%APPDATA%/argus/pricing.json` (ou `~/.config/argus/pricing.json`).
    pub fn default_path() -> Option<PathBuf> {
        if let Some(a) = std::env::var_os("APPDATA") {
            return Some(PathBuf::from(a).join(crate::paths::dir_name()).join("pricing.json"));
        }
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config").join(crate::paths::dir_name()).join("pricing.json"))
    }

    /// Padrões mesclados com o JSON em `path` (se existir e for válido).
    pub fn load(path: &Path) -> Self {
        let mut p = Self::default();
        if let Ok(s) = std::fs::read_to_string(path) {
            if let Err(e) = p.merge_json(&s) {
                tracing::warn!("pricing.json inválido ({}): {e}", path.display());
            }
        }
        p
    }

    /// Mescla um JSON `{ "models": {"chave": {input,output,cache_read,cache_write}}, "fallback": {...} }`.
    pub fn merge_json(&mut self, json: &str) -> serde_json::Result<()> {
        let o: PricingOverride = serde_json::from_str(json)?;
        self.models.extend(o.models);
        if let Some(f) = o.fallback {
            self.fallback = f;
        }
        if let Some(b) = o.prefer_reported {
            self.prefer_reported = b;
        }
        Ok(())
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).map_err(std::io::Error::other)?)
    }

    pub fn price_for(&self, model: Option<&str>) -> ModelPrice {
        let Some(m) = model.map(str::to_ascii_lowercase) else { return self.fallback };
        self.models
            .iter()
            .filter(|(k, _)| m.contains(&k.to_ascii_lowercase()))
            .max_by_key(|(k, _)| k.len())
            .map(|(_, p)| *p)
            .unwrap_or(self.fallback)
    }

    /// Custo calculado pela tabela, ignorando o informado pela CLI.
    pub fn table_cost(&self, u: &TokenUsage) -> f64 {
        let p = self.price_for(u.model.as_deref());
        (u.input_tokens as f64 * p.input
            + u.output_tokens as f64 * p.output
            + u.cache_read as f64 * p.cache_read
            + u.cache_write as f64 * p.cache_write)
            / 1_000_000.0
    }

    /// Custo equivalente em API (USD).
    pub fn cost(&self, u: &TokenUsage) -> f64 {
        match u.cost_usd {
            Some(c) if self.prefer_reported && c > 0.0 => c,
            _ => self.table_cost(u),
        }
    }
}

/// Custo com a tabela padrão.
pub fn cost(u: &TokenUsage) -> f64 {
    Pricing::default().cost(u)
}

/// Totais de uma agregação.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CostLine {
    pub key: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub turns: u64,
    pub api_cost_usd: f64,
}

impl CostLine {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read + self.cache_write
    }
    fn add(&mut self, u: &TokenUsage, cost: f64) {
        self.input_tokens += u.input_tokens;
        self.output_tokens += u.output_tokens;
        self.cache_read += u.cache_read;
        self.cache_write += u.cache_write;
        self.api_cost_usd += cost;
    }
}

/// Agrega linhas por uma chave arbitrária (ordenado pela chave).
///
/// `turns` conta turnos, não linhas: um turno que usa dois modelos grava duas
/// linhas de consumo (uma por modelo) e continua sendo um turno só.
pub fn aggregate<F: Fn(&UsageRow) -> String>(rows: &[UsageRow], pricing: &Pricing, key: F) -> Vec<CostLine> {
    let mut m: BTreeMap<String, (CostLine, HashSet<(i64, bool, i64)>)> = BTreeMap::new();
    for r in rows {
        let k = key(r);
        let (line, turns) =
            m.entry(k.clone()).or_insert_with(|| (CostLine { key: k, ..Default::default() }, HashSet::new()));
        line.add(&r.usage, pricing.cost(&r.usage));
        turns.insert(r.turn_key());
    }
    m.into_values()
        .map(|(mut line, turns)| {
            line.turns = turns.len() as u64;
            line
        })
        .collect()
}

pub fn by_project(rows: &[UsageRow], p: &Pricing) -> Vec<CostLine> {
    aggregate(rows, p, |r| r.project.clone())
}
pub fn by_provider(rows: &[UsageRow], p: &Pricing) -> Vec<CostLine> {
    aggregate(rows, p, |r| r.provider.clone())
}
pub fn by_model(rows: &[UsageRow], p: &Pricing) -> Vec<CostLine> {
    aggregate(rows, p, |r| r.usage.model.clone().unwrap_or_else(|| "?".into()))
}
/// Por dia (UTC, "AAAA-MM-DD").
pub fn by_day(rows: &[UsageRow], p: &Pricing) -> Vec<CostLine> {
    aggregate(rows, p, |r| day_string(r.created_at))
}
/// Por dia no fuso `tz` ("AAAA-MM-DD"). A interface usa o fuso do computador
/// (`chrono::Local`): em UTC, o "hoje" de Brasília virava às 21h.
pub fn by_day_in<Tz: TimeZone>(rows: &[UsageRow], p: &Pricing, tz: &Tz) -> Vec<CostLine> {
    aggregate(rows, p, |r| day_in(r.created_at, tz).to_string())
}

/// Data (calendário) do instante `ts` no fuso `tz`.
pub fn day_in<Tz: TimeZone>(ts: i64, tz: &Tz) -> NaiveDate {
    tz.timestamp_opt(ts, 0).earliest().map(|d| d.date_naive()).unwrap_or_default()
}

/// Instante (segundos Unix) em que o dia `day` começa no fuso `tz`. Se a
/// meia-noite não existir (virada de horário de verão), vale a primeira hora válida.
pub fn day_start_in<Tz: TimeZone>(day: NaiveDate, tz: &Tz) -> i64 {
    (0..24)
        .find_map(|h| tz.from_local_datetime(&day.and_hms_opt(h, 0, 0)?).earliest())
        .map(|d| d.timestamp())
        .unwrap_or_else(|| day.and_hms_opt(0, 0, 0).map(|d| d.and_utc().timestamp()).unwrap_or(0))
}

/// Os `n` últimos dias do calendário até o dia de `ts` no fuso `tz`, do mais antigo ao mais novo.
pub fn last_days_in<Tz: TimeZone>(ts: i64, n: u64, tz: &Tz) -> Vec<NaiveDate> {
    let today = day_in(ts, tz);
    (0..n).rev().filter_map(|i| today.checked_sub_days(Days::new(i))).collect()
}

/// Segundos Unix -> "AAAA-MM-DD" (UTC).
pub fn day_string(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    // Algoritmo civil_from_days (H. Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(model: Option<&str>, i: u64, o: u64, cr: u64, cw: u64, c: Option<f64>) -> TokenUsage {
        TokenUsage {
            input_tokens: i,
            output_tokens: o,
            cache_read: cr,
            cache_write: cw,
            model: model.map(Into::into),
            cost_usd: c,
        }
    }

    #[test]
    fn price_lookup() {
        let p = Pricing::default();
        assert_eq!(p.price_for(Some("claude-opus-4-7")).input, 5.0);
        assert_eq!(p.price_for(Some("claude-opus-4-1-20250805")).input, 15.0);
        assert_eq!(p.price_for(Some("claude-sonnet-4-6")).output, 15.0);
        assert_eq!(p.price_for(Some("claude-haiku-4-5-20251001")).input, 1.0);
        assert_eq!(p.price_for(Some("claude-3-5-haiku-20241022")).input, 0.8);
        assert_eq!(p.price_for(Some("openai/gpt-x")), p.fallback);
        assert_eq!(p.price_for(None), p.fallback);
    }

    #[test]
    fn computes_cost() {
        let p = Pricing::default();
        // 1M de input + 1M de output em sonnet = 3 + 15.
        let u = usage(Some("claude-sonnet-4-5"), 1_000_000, 1_000_000, 0, 0, None);
        assert!((p.cost(&u) - 18.0).abs() < 1e-9);
        // Cache: read 0,1x e write 1,25x.
        let u = usage(Some("haiku"), 0, 0, 1_000_000, 1_000_000, Some(0.0));
        assert!((p.cost(&u) - (0.1 + 1.25)).abs() < 1e-9);
        // Custo informado > 0 tem precedência.
        let u = usage(Some("haiku"), 10, 10, 0, 0, Some(0.07));
        assert_eq!(p.cost(&u), 0.07);
        assert!(p.table_cost(&u) < 0.07);
        assert_eq!(cost(&u), 0.07);
    }

    #[test]
    fn json_override() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("pricing.json");
        std::fs::write(
            &f,
            r#"{"models":{"gpt-6":{"input":2,"output":8,"cache_read":0.2,"cache_write":0}},"fallback":{"input":1,"output":1,"cache_read":0,"cache_write":0},"prefer_reported":false}"#,
        )
        .unwrap();
        let p = Pricing::load(&f);
        assert_eq!(p.price_for(Some("openai/gpt-6-luna")).output, 8.0);
        assert_eq!(p.price_for(Some("desconhecido")).input, 1.0);
        assert!(p.price_for(Some("sonnet")).input == 3.0); // padrões preservados
        assert_eq!(p.cost(&usage(Some("x"), 1_000_000, 0, 0, 0, Some(9.0))), 1.0);
        // JSON inválido mantém os padrões.
        std::fs::write(&f, "{lixo").unwrap();
        assert_eq!(Pricing::load(&f), Pricing::default());
        // Ida e volta.
        let g = dir.path().join("sub").join("p.json");
        Pricing::default().save(&g).unwrap();
        assert_eq!(Pricing::load(&g), Pricing::default());
    }

    #[test]
    fn aggregations() {
        let p = Pricing::default();
        let row = |proj: &str, prov: &str, ts, c| UsageRow {
            task_id: 1,
            project: proj.into(),
            provider: prov.into(),
            usage: usage(Some("sonnet"), 1_000_000, 0, 0, 0, c),
            created_at: ts,
            turn: None,
        };
        let rows = vec![row("/a", "claude", 0, None), row("/a", "claude", 86_400, Some(1.0)), row("/b", "opencode", 86_401, None)];
        let bp = by_project(&rows, &p);
        assert_eq!(bp.len(), 2);
        assert_eq!(bp[0].key, "/a");
        assert_eq!(bp[0].turns, 2);
        assert!((bp[0].api_cost_usd - 4.0).abs() < 1e-9);
        assert_eq!(bp[0].total_tokens(), 2_000_000);
        let bd = by_day(&rows, &p);
        assert_eq!(bd.iter().map(|l| l.key.as_str()).collect::<Vec<_>>(), ["1970-01-01", "1970-01-02"]);
        assert_eq!(by_provider(&rows, &p).len(), 2);
        assert_eq!(by_model(&rows, &p)[0].key, "sonnet");
    }

    /// Um turno com dois modelos (principal + subagente) é um turno só no total,
    /// no projeto, no provedor e no dia; cada modelo aparece com 1.
    #[test]
    fn two_models_in_one_turn_count_once() {
        let db = crate::db::Db::open_in_memory().unwrap();
        db.insert_usage_turn(1, "/a", "claude", &usage(Some("claude-opus-4-7"), 1_000_000, 0, 0, 0, None), 500).unwrap();
        db.insert_usage_turn(1, "/a", "claude", &usage(Some("claude-haiku-4-5"), 1_000_000, 0, 0, 0, None), 500).unwrap();
        let rows = db.usage_rows(crate::db::TimeRange::last_days(1)).unwrap();
        let p = Pricing::default();
        let s = by_project(&rows, &p);
        assert_eq!(s.len(), 1);
        assert!((s[0].api_cost_usd - 6.0).abs() < 1e-9);
        assert_eq!(s[0].turns, 1);
        assert_eq!(by_provider(&rows, &p)[0].turns, 1);
        assert_eq!(by_day(&rows, &p)[0].turns, 1);
        assert_eq!(aggregate(&rows, &p, |_| "total".into())[0].turns, 1);
        assert_eq!(by_model(&rows, &p).iter().map(|l| l.turns).collect::<Vec<_>>(), [1, 1]);
        // Outro turno da mesma tarefa, e um turno de outra tarefa com o mesmo número.
        db.insert_usage_turn(1, "/a", "claude", &usage(Some("claude-opus-4-7"), 1, 0, 0, 0, None), 501).unwrap();
        db.insert_usage_turn(2, "/a", "claude", &usage(Some("claude-opus-4-7"), 1, 0, 0, 0, None), 500).unwrap();
        let rows = db.usage_rows(crate::db::TimeRange::last_days(1)).unwrap();
        assert_eq!(by_project(&rows, &p)[0].turns, 3);
    }

    /// O dia segue o fuso pedido: 22h30 de Brasília ainda é o mesmo dia (em UTC já seria o seguinte).
    #[test]
    fn days_follow_the_time_zone() {
        let brt = chrono::FixedOffset::west_opt(3 * 3600).unwrap();
        // 2026-11-14 22:30 em Brasília = 2026-11-15 01:30 UTC.
        let ts = 1_794_706_200;
        assert_eq!(day_string(ts), "2026-11-15");
        assert_eq!(day_in(ts, &brt).to_string(), "2026-11-14");
        assert_eq!(day_in(ts, &chrono::Utc).to_string(), "2026-11-15");
        let start = day_start_in(day_in(ts, &brt), &brt);
        assert_eq!(start, ts - 22 * 3600 - 30 * 60);
        assert_eq!(day_in(start, &brt).to_string(), "2026-11-14");
        assert_eq!(day_in(start - 1, &brt).to_string(), "2026-11-13");
        let days: Vec<String> = last_days_in(ts, 3, &brt).iter().map(ToString::to_string).collect();
        assert_eq!(days, ["2026-11-12", "2026-11-13", "2026-11-14"]);
        let p = Pricing::default();
        let row = |at| UsageRow {
            task_id: 1,
            project: "/a".into(),
            provider: "claude".into(),
            usage: usage(Some("sonnet"), 1, 0, 0, 0, None),
            created_at: at,
            turn: Some(at),
        };
        // 20h e 22h30 de Brasília: mesmo dia local, dias diferentes em UTC.
        let rows = vec![row(ts - 9_000), row(ts)];
        assert_eq!(by_day_in(&rows, &p, &brt).len(), 1);
        assert_eq!(by_day_in(&rows, &p, &brt)[0].turns, 2);
        assert_eq!(by_day(&rows, &p).len(), 2);
    }

    #[test]
    fn days() {
        assert_eq!(day_string(0), "1970-01-01");
        assert_eq!(day_string(1_794_616_650), "2026-11-14");
        assert_eq!(day_string(951_782_400), "2000-02-29");
    }
}
