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

/// Multiplicador da escrita de cache de 1 hora sobre o preço da entrada.
pub const CACHE_WRITE_1H: f64 = 2.0;
/// Turno com regravação de cache: escrita acima disto (tokens)...
pub const REWRITE_MIN_TOKENS: u64 = 50_000;
/// ...ou acima desta fração do volume de entrada do turno (com ao menos
/// `REWRITE_RATIO_FLOOR` tokens gravados, para não marcar turnos pequenos).
pub const REWRITE_RATIO: f64 = 0.30;
const REWRITE_RATIO_FLOOR: u64 = 10_000;

/// Preço de um modelo (USD por 1M tokens).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModelPrice {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

impl ModelPrice {
    /// Cache read = 0,1x e cache write = 2x do input. O claude grava o cache com
    /// vida de 1 hora, cobrado a 2x (o de 5 min seria 1,25x): o custo informado
    /// pela CLI bate com 2x, então a estimativa pela tabela usa o mesmo.
    pub const fn standard(input: f64, output: f64) -> Self {
        Self { input, output, cache_read: input * 0.1, cache_write: input * CACHE_WRITE_1H }
    }
    /// Preço com cache lido explícito e sem escrita em cache (OpenAI, Gemini).
    pub const fn cached(input: f64, output: f64, cache_read: f64) -> Self {
        Self { input, output, cache_read, cache_write: 0.0 }
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
        // OpenAI (codex), tabela oficial "Standard, short context", 2026-11-16
        // (developers.openai.com/api/docs/pricing). Sem cobrança de escrita em cache.
        m.insert("gpt-6.1-sol".into(), ModelPrice::cached(2.0, 10.0, 0.10));
        m.insert("gpt-6.1-terra".into(), ModelPrice::cached(1.0, 5.0, 0.05));
        m.insert("gpt-6-astra".into(), ModelPrice::cached(10.0, 50.0, 1.0));
        m.insert("gpt-6-luna".into(), ModelPrice::cached(0.10, 0.50, 0.01));
        m.insert("gpt-5.3-codex".into(), ModelPrice::cached(1.75, 14.0, 0.175));
        // Gemini, tabela oficial do AI Studio (pago, Standard, até 200 mil tokens),
        // 2026-11-16 (aistudio.google.com/docs/pricing). O 3.8 Flash muda em 2027-01-01.
        m.insert("gemini-2.5-pro".into(), ModelPrice::cached(1.25, 10.0, 0.125));
        m.insert("gemini-2.5-flash".into(), ModelPrice::cached(0.30, 2.50, 0.03));
        m.insert("gemini-3.1-pro".into(), ModelPrice::cached(2.0, 12.0, 0.20));
        m.insert("gemini-3.8-flash".into(), ModelPrice::cached(0.75, 3.75, 0.075));
        m.insert("gemini-3.5-flash-lite".into(), ModelPrice::cached(0.30, 2.50, 0.03));
        m.insert("gemini-3.1-flash-lite".into(), ModelPrice::cached(0.25, 1.50, 0.025));
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
        if is_free_model(&m) {
            return ModelPrice { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 };
        }
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

    /// Custo pela tabela separado por tipo de token.
    pub fn split(&self, u: &TokenUsage) -> CostSplit {
        let p = self.price_for(u.model.as_deref());
        let m = |n: u64, price: f64| n as f64 * price / 1_000_000.0;
        CostSplit {
            input: m(u.input_tokens, p.input),
            cache_write: m(u.cache_write, p.cache_write),
            cache_read: m(u.cache_read, p.cache_read),
            output: m(u.output_tokens, p.output),
        }
    }

    /// Custo equivalente em API (USD).
    pub fn cost(&self, u: &TokenUsage) -> f64 {
        match u.cost_usd {
            Some(c) if self.prefer_reported && c > 0.0 => c,
            _ => self.table_cost(u),
        }
    }
}

/// Modelo gratuito (OpenCode `...-free`, OpenRouter `...:free`): custo zero,
/// nunca o fallback da tabela. `m` já em minúsculas.
fn is_free_model(m: &str) -> bool {
    m.ends_with("-free") || m.ends_with(":free") || m.ends_with("/free")
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

/// Custo estimado pela tabela, por tipo de token (USD).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CostSplit {
    pub input: f64,
    pub cache_write: f64,
    pub cache_read: f64,
    pub output: f64,
}

impl CostSplit {
    pub fn total(&self) -> f64 {
        self.input + self.cache_write + self.cache_read + self.output
    }
    fn add(&mut self, o: CostSplit) {
        self.input += o.input;
        self.cache_write += o.cache_write;
        self.cache_read += o.cache_read;
        self.output += o.output;
    }
}

/// Taxa de acerto do cache: `cache_read / (cache_read + cache_write + input)`.
/// 0 sem entrada nenhuma.
pub fn cache_hit_rate(l: &CostLine) -> f64 {
    let den = l.cache_read + l.cache_write + l.input_tokens;
    if den == 0 { 0.0 } else { l.cache_read as f64 / den as f64 }
}

/// Uma conversa no ranking de custo.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaskLine {
    pub task_id: i64,
    pub project: String,
    /// Título (prompt inicial); vazio se a conversa foi excluída. Preenchido por quem chama.
    pub title: String,
    pub cost: f64,
    pub turns: u64,
    pub cost_per_turn: f64,
    /// Maior contexto conhecido (tokens): o maior entre a maior escrita de cache
    /// de um turno (numa regravação, o contexto inteiro) e o último contexto medido.
    pub max_context: u64,
    pub cache_hit_rate: f64,
}

/// Ranking por conversa, do custo maior ao menor. `known_context` = último
/// contexto medido de cada tarefa (o banco guarda só o último).
pub fn by_task(rows: &[UsageRow], p: &Pricing, known_context: &BTreeMap<i64, u64>) -> Vec<TaskLine> {
    let lines = aggregate(rows, p, |r| r.task_id.to_string());
    let mut max_write: BTreeMap<i64, u64> = BTreeMap::new();
    let mut project: BTreeMap<i64, String> = BTreeMap::new();
    for t in turns(rows) {
        let e = max_write.entry(t.task_id).or_default();
        *e = (*e).max(t.cache_write);
    }
    for r in rows {
        project.entry(r.task_id).or_insert_with(|| r.project.clone());
    }
    let mut out: Vec<TaskLine> = lines
        .into_iter()
        .filter_map(|l| {
            let id: i64 = l.key.parse().ok()?;
            Some(TaskLine {
                task_id: id,
                project: project.get(&id).cloned().unwrap_or_default(),
                title: String::new(),
                cost: l.api_cost_usd,
                turns: l.turns,
                cost_per_turn: if l.turns > 0 { l.api_cost_usd / l.turns as f64 } else { 0.0 },
                max_context: max_write.get(&id).copied().unwrap_or(0).max(known_context.get(&id).copied().unwrap_or(0)),
                cache_hit_rate: cache_hit_rate(&l),
            })
        })
        .collect();
    out.sort_by(|a, b| b.cost.total_cmp(&a.cost));
    out
}

/// Um turno (as linhas de todos os modelos dele somadas).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TurnLine {
    pub task_id: i64,
    pub created_at: i64,
    pub input_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// Custo estimado da escrita de cache deste turno (USD, tabela).
    pub write_cost: f64,
}

impl TurnLine {
    /// Volume de entrada do turno (somado em todos os pedidos dele).
    pub fn volume(&self) -> u64 {
        self.input_tokens + self.cache_read + self.cache_write
    }
    /// O turno regravou o contexto (cache expirado ou invalidado)?
    pub fn is_rewrite(&self) -> bool {
        self.cache_write > REWRITE_MIN_TOKENS
            || (self.cache_write >= REWRITE_RATIO_FLOOR && self.cache_write as f64 > REWRITE_RATIO * self.volume() as f64)
    }
}

fn turns_priced(rows: &[UsageRow], p: Option<&Pricing>) -> Vec<TurnLine> {
    let mut m: BTreeMap<(i64, bool, i64), TurnLine> = BTreeMap::new();
    for r in rows {
        let t = m.entry(r.turn_key()).or_insert_with(|| TurnLine { task_id: r.task_id, created_at: r.created_at, ..Default::default() });
        t.created_at = t.created_at.max(r.created_at);
        t.input_tokens += r.usage.input_tokens;
        t.cache_read += r.usage.cache_read;
        t.cache_write += r.usage.cache_write;
        if let Some(p) = p {
            t.write_cost += p.split(&r.usage).cache_write;
        }
    }
    m.into_values().collect()
}

fn turns(rows: &[UsageRow]) -> Vec<TurnLine> {
    turns_priced(rows, None)
}

/// Indicadores de desperdício e de onde o dinheiro vai.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Insights {
    pub cache_hit_rate: f64,
    /// Custo pela tabela por tipo de token (pode diferir do total quando a CLI informa o custo).
    pub split: CostSplit,
    /// Conversas do custo maior ao menor (até 10).
    pub tasks: Vec<TaskLine>,
    /// Turnos com regravação de cache.
    pub rewrite_turns: u64,
    pub rewrite_write_cost: f64,
    /// As regravações mais caras (até 10).
    pub rewrites: Vec<TurnLine>,
}

pub fn insights(rows: &[UsageRow], p: &Pricing, known_context: &BTreeMap<i64, u64>) -> Insights {
    let total = aggregate(rows, p, |_| String::new()).pop().unwrap_or_default();
    let mut split = CostSplit::default();
    for r in rows {
        split.add(p.split(&r.usage));
    }
    let mut rw: Vec<TurnLine> = turns_priced(rows, Some(p)).into_iter().filter(TurnLine::is_rewrite).collect();
    rw.sort_by(|a, b| b.write_cost.total_cmp(&a.write_cost));
    let mut tasks = by_task(rows, p, known_context);
    tasks.truncate(10);
    Insights {
        cache_hit_rate: cache_hit_rate(&total),
        split,
        tasks,
        rewrite_turns: rw.len() as u64,
        rewrite_write_cost: rw.iter().map(|t| t.write_cost).sum::<f64>() + 0.0,
        rewrites: rw.into_iter().take(10).collect(),
    }
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
        assert_eq!(p.price_for(Some("gpt-6.1-sol")).cache_read, 0.10);
        assert_eq!(p.price_for(Some("gemini-3.1-pro-preview")).output, 12.0);
        assert_eq!(p.price_for(Some("gemini-3.1-flash-lite")).input, 0.25);
        assert_eq!(p.price_for(None), p.fallback);
    }

    #[test]
    fn free_models_cost_nothing() {
        let p = Pricing::default();
        for m in ["opencode/deepseek-v4.1-flash-free", "openrouter/qwen/qwen3-coder:FREE"] {
            let u = usage(Some(m), 1_000_000, 1_000_000, 0, 0, None);
            assert_eq!(p.cost(&u), 0.0, "{m}");
        }
        assert_ne!(p.price_for(Some("free-tier-x/gpt")), ModelPrice { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 });
    }

    #[test]
    fn computes_cost() {
        let p = Pricing::default();
        // 1M de input + 1M de output em sonnet = 3 + 15.
        let u = usage(Some("claude-sonnet-4-5"), 1_000_000, 1_000_000, 0, 0, None);
        assert!((p.cost(&u) - 18.0).abs() < 1e-9);
        // Cache: read 0,1x e write 2x (cache de 1 hora).
        let u = usage(Some("haiku"), 0, 0, 1_000_000, 1_000_000, Some(0.0));
        assert!((p.cost(&u) - (0.1 + 2.0)).abs() < 1e-9);
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
        assert_eq!(p.price_for(Some("openai/gpt-6-nova")).output, 8.0);
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

    fn turn_row(task: i64, turn: i64, u: TokenUsage) -> UsageRow {
        UsageRow { task_id: task, project: format!("/p{task}"), provider: "claude".into(), usage: u, created_at: turn, turn: Some(turn) }
    }

    #[test]
    fn split_and_hit_rate() {
        let p = Pricing::default();
        // opus: entrada 5, saída 25, leitura 0,5, escrita 10 (por 1M).
        let s = p.split(&usage(Some("claude-opus-5"), 1_000_000, 1_000_000, 1_000_000, 1_000_000, None));
        assert_eq!((s.input, s.output, s.cache_read, s.cache_write), (5.0, 25.0, 0.5, 10.0));
        assert!((s.total() - 40.5).abs() < 1e-9);
        let l = CostLine { input_tokens: 10, cache_read: 80, cache_write: 10, ..Default::default() };
        assert!((cache_hit_rate(&l) - 0.8).abs() < 1e-9);
        assert_eq!(cache_hit_rate(&CostLine::default()), 0.0);
    }

    #[test]
    fn ranking_and_rewrites() {
        let p = Pricing::default();
        let u = |i, cr, cw| usage(Some("claude-opus-5"), i, 100, cr, cw, None);
        let rows = vec![
            // Tarefa 1: turno normal (escrita pequena) e uma regravação de 300 mil.
            turn_row(1, 10, u(10, 500_000, 5_000)),
            turn_row(1, 20, u(10, 20_000, 300_000)),
            // Tarefa 2: turno pequeno com 40% de escrita (regrava pela fração).
            turn_row(2, 30, u(0, 30_000, 20_000)),
            // Escrita de 5 mil em turno pequeno: abaixo do piso, não conta.
            turn_row(2, 40, u(0, 5_000, 5_000)),
        ];
        let ranked = by_task(&rows, &p, &BTreeMap::from([(2, 70_000)]));
        assert_eq!(ranked.iter().map(|t| t.task_id).collect::<Vec<_>>(), [1, 2]);
        assert_eq!((ranked[0].turns, ranked[0].max_context, ranked[0].project.as_str()), (2, 300_000, "/p1"));
        assert!((ranked[0].cost_per_turn - ranked[0].cost / 2.0).abs() < 1e-12);
        assert_eq!(ranked[1].max_context, 70_000);
        let ins = insights(&rows, &p, &BTreeMap::new());
        assert_eq!(ins.rewrite_turns, 2);
        assert_eq!(ins.rewrites[0].cache_write, 300_000);
        assert!((ins.rewrite_write_cost - (300_000.0 + 20_000.0) * 10.0 / 1e6).abs() < 1e-9);
        assert!((ins.split.total() - rows.iter().map(|r| p.table_cost(&r.usage)).sum::<f64>()).abs() < 1e-9);
        let read: u64 = 555_000;
        let all: u64 = 20 + read + 330_000;
        assert!((ins.cache_hit_rate - read as f64 / all as f64).abs() < 1e-9);
    }

    #[test]
    fn days() {
        assert_eq!(day_string(0), "1970-01-01");
        assert_eq!(day_string(1_794_616_650), "2026-11-14");
        assert_eq!(day_string(951_782_400), "2000-02-29");
    }
}
