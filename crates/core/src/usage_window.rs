//! Janelas de uso de 5 horas calculadas pelo plugin `usage-window` do OpenCode.
//!
//! O plugin grava contadores por `provider/modelo` em
//! `~/.local/share/opencode-usage-window/state.json`. Aqui o Argus só lê esses
//! números e escreve de volta os ajustes manuais: nenhum token de login,
//! credencial ou conteúdo de conversa entra nesse arquivo.
//!
//! O cálculo do percentual === o do plugin: `tokens = input + output +
//! reasoning` (`cacheRead` fica de fora), limite manual ou mediana das amostras
//! de rate limit, e o maior entre tokens e requisições manda.

use serde::Serialize;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

const DEFAULT_WINDOW_HOURS: u64 = 5;

/// Uma janela ativa do estado do plugin, já com o percentual calculado.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UsageWindowInfo {
    /// Chave exata do plugin, ex.: `opencode/big-pickle`. Usada nos ajustes.
    pub key: String,
    pub provider: String,
    pub model: String,
    /// Mesma regra de modelo gratuito do restante do Argus.
    pub is_free: bool,
    /// Início da janela, em ms desde 1970.
    pub started_at_ms: u64,
    /// Fim da janela, em ms desde 1970.
    pub resets_at_ms: u64,
    pub tokens: u64,
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub requests: u64,
    /// Limite de tokens (manual ou mediana); `None` = ainda sem calibração.
    pub limit_tokens: Option<u64>,
    pub limit_requests: Option<u64>,
    /// maior entre tokens/limite e requisições/limite; `None` = sem calibração.
    pub percent: Option<u64>,
    /// `manual`, `samples` ou `none`.
    pub calibration: String,
    pub samples: usize,
    /// Quando o plugin detectou um rate limit nesta janela, em ms desde 1970.
    pub limit_hit_at_ms: Option<u64>,
}

/// Duração da janela em ms. A variável do plugin é respeitada; valor inválido
/// ou zero cai nos 5 horas padrão.
pub fn window_ms() -> u64 {
    let hours = std::env::var("USAGE_WINDOW_HOURS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|h| *h > 0)
        .unwrap_or(DEFAULT_WINDOW_HOURS);
    hours.saturating_mul(3_600_000)
}

/// Caminho do estado: `USAGE_WINDOW_FILE` quando definido, senão o padrão do
/// plugin dentro da pasta home do usuário.
pub fn state_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("USAGE_WINDOW_FILE") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("opencode-usage-window")
            .join("state.json"),
    )
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn number(v: Option<&Value>) -> u64 {
    match v {
        Some(Value::Number(n)) => n
            .as_u64()
            .or_else(|| n.as_f64().map(|f| if f.is_finite() && f > 0.0 { f as u64 } else { 0 }))
            .unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse::<u64>().unwrap_or(0),
        _ => 0,
    }
}

fn positive(v: Option<&Value>) -> Option<u64> {
    let n = number(v);
    (n > 0).then_some(n)
}

fn median(xs: &[u64]) -> Option<u64> {
    if xs.is_empty() {
        return None;
    }
    let mut sorted = xs.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    Some(if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        sorted[mid - 1].saturating_add(sorted[mid]) / 2
    })
}

/// Converte o estado do plugin em janelas ativas. Função pura: o mesmo estado
/// e o mesmo instante sempre devolvem a mesma lista.
pub fn list_from_state(state: &Value, now: u64, window: u64) -> Vec<UsageWindowInfo> {
    let Some(buckets) = state.get("buckets").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut out = Vec::new();

    for (key, bucket) in buckets {
        let Some(w) = bucket.get("window").and_then(Value::as_object) else {
            continue;
        };
        let start = number(w.get("start"));
        if start == 0 || now >= start.saturating_add(window) {
            continue; // janela vencida: o plugin vai criar outra na próxima requisição
        }

        let samples = bucket.get("samples").and_then(Value::as_array).cloned().unwrap_or_default();
        let sample_tokens: Vec<u64> = samples.iter().map(|s| number(s.get("tokens"))).collect();
        let sample_requests: Vec<u64> = samples.iter().map(|s| number(s.get("requests"))).collect();
        let manual = bucket.get("manualLimit");
        let manual_tokens = positive(manual.and_then(|m| m.get("tokens")));
        let manual_requests = positive(manual.and_then(|m| m.get("requests")));
        let limit_tokens = manual_tokens.or_else(|| median(&sample_tokens));
        let limit_requests = manual_requests.or_else(|| median(&sample_requests));

        let used_tokens = number(w.get("tokens"));
        let used_requests = number(w.get("requests"));
        let has_estimate = limit_tokens.is_some() || limit_requests.is_some();
        let percent = if has_estimate {
            let by_tokens = limit_tokens.map(|l| used_tokens as f64 / l as f64).unwrap_or(0.0);
            let by_requests = limit_requests.map(|l| used_requests as f64 / l as f64).unwrap_or(0.0);
            let ratio = by_tokens.max(by_requests) * 100.0;
            Some((ratio.round() as u64).min(100_000))
        } else {
            None
        };
        let calibration = if manual_tokens.is_some() || manual_requests.is_some() {
            "manual"
        } else if limit_tokens.is_some() || limit_requests.is_some() {
            "samples"
        } else {
            "none"
        };

        let (provider, model) = key
            .split_once('/')
            .map_or((String::new(), key.clone()), |(p, m)| (p.to_string(), m.to_string()));
        let lower = model.to_lowercase();

        out.push(UsageWindowInfo {
            key: key.clone(),
            provider,
            model,
            is_free: crate::metrics::is_free_model(&lower),
            started_at_ms: start,
            resets_at_ms: start.saturating_add(window),
            tokens: used_tokens,
            input: number(w.get("input")),
            output: number(w.get("output")),
            reasoning: number(w.get("reasoning")),
            cache_read: number(w.get("cacheRead")),
            requests: used_requests,
            limit_tokens,
            limit_requests,
            percent,
            calibration: calibration.to_string(),
            samples: samples.len(),
            limit_hit_at_ms: positive(w.get("limitHitAt")),
        });
    }

    out.sort_by(|a, b| {
        b.percent
            .unwrap_or(0)
            .cmp(&a.percent.unwrap_or(0))
            .then_with(|| b.tokens.cmp(&a.tokens))
    });
    out
}

/// Limite manual de uma janela que o Argus calcula sozinho.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ManualLimit {
    pub tokens: Option<u64>,
    pub requests: Option<u64>,
}

/// Chave de uma linha de uso no formato do plugin (`provedor/modelo`).
/// Linha sem modelo não vira janela: sem saber o modelo, somar o consumo de
/// dois modelos diferentes na mesma barra seria pior que não mostrar nada.
pub fn row_key(provider: &str, model: Option<&str>) -> Option<String> {
    let m = model.map(str::trim).filter(|m| !m.is_empty())?;
    Some(format!("{provider}/{m}"))
}

/// Janelas de 5 horas calculadas do histórico do próprio Argus.
///
/// O plugin `usage-window` só acompanha o que passa pelo OpenCode, então a
/// janela do claude nunca aparecia. Aqui a conta sai das linhas de uso que o
/// Argus já grava, pela mesma regra do plugin: a janela abre na primeira
/// requisição depois que a anterior vence, `tokens = entrada + saída` (o cache
/// lido fica de fora) e uma requisição é um turno, não uma linha — um turno de
/// vários passos grava várias linhas.
///
/// `rows` precisa vir em ordem de tempo e cobrir mais que uma janela: é o
/// encadeamento desde a primeira requisição que diz onde a janela atual
/// começou. Só a janela viva de cada chave é devolvida.
///
/// Sem amostra de rate limit — o Argus não tem de onde tirar isso para o claude
/// — a calibração é só o limite manual, e sem ele o percentual fica em branco.
pub fn list_from_rows(
    rows: &[crate::db::UsageRow],
    manual: &std::collections::BTreeMap<String, ManualLimit>,
    now: u64,
    window: u64,
) -> Vec<UsageWindowInfo> {
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Acc {
        start: u64,
        input: u64,
        output: u64,
        cache_read: u64,
        turnos: std::collections::BTreeSet<(i64, bool, i64)>,
    }

    let mut por_chave: BTreeMap<String, Acc> = BTreeMap::new();
    for r in rows {
        let Some(key) = row_key(&r.provider, r.usage.model.as_deref()) else { continue };
        // `created_at` é gravado em segundos; a janela trabalha em ms.
        let at = (r.created_at.max(0) as u64).saturating_mul(1000);
        let acc = por_chave.entry(key).or_default();
        if acc.start == 0 || at >= acc.start.saturating_add(window) {
            *acc = Acc { start: at, ..Default::default() };
        }
        acc.input += r.usage.input_tokens;
        acc.output += r.usage.output_tokens;
        acc.cache_read += r.usage.cache_read;
        acc.turnos.insert(r.turn_key());
    }

    let mut out: Vec<UsageWindowInfo> = por_chave
        .into_iter()
        .filter(|(_, a)| a.start > 0 && now < a.start.saturating_add(window))
        .map(|(key, a)| {
            let lim = manual.get(&key).copied().unwrap_or_default();
            let tokens = a.input + a.output;
            let requests = a.turnos.len() as u64;
            let percent = (lim.tokens.is_some() || lim.requests.is_some()).then(|| {
                let by_tokens = lim.tokens.map(|l| tokens as f64 / l as f64).unwrap_or(0.0);
                let by_requests = lim.requests.map(|l| requests as f64 / l as f64).unwrap_or(0.0);
                ((by_tokens.max(by_requests) * 100.0).round() as u64).min(100_000)
            });
            let (provider, model) = key
                .split_once('/')
                .map_or((String::new(), key.clone()), |(p, m)| (p.to_string(), m.to_string()));
            UsageWindowInfo {
                is_free: crate::metrics::is_free_model(&model.to_lowercase()),
                key,
                provider,
                model,
                started_at_ms: a.start,
                resets_at_ms: a.start.saturating_add(window),
                tokens,
                input: a.input,
                output: a.output,
                // O Argus não separa raciocínio da saída: ele já entra em `output`.
                reasoning: 0,
                cache_read: a.cache_read,
                requests,
                limit_tokens: lim.tokens,
                limit_requests: lim.requests,
                percent,
                calibration: if percent.is_some() { "manual".into() } else { "none".into() },
                // Amostra de rate limit é coisa do plugin; aqui nunca existe.
                samples: 0,
                limit_hit_at_ms: None,
            }
        })
        .collect();
    out.sort_by(|a, b| b.percent.unwrap_or(0).cmp(&a.percent.unwrap_or(0)).then_with(|| b.tokens.cmp(&a.tokens)));
    out
}

/// Lê o estado do plugin. Arquivo ausente significa "sem janelas", não erro:
/// o plugin pode ainda não ter registrado nenhuma requisição.
pub fn read_state() -> Result<Value, String> {
    let Some(path) = state_path() else {
        return Ok(json!({ "version": 1, "buckets": {} }));
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| format!("Estado do usage-window inválido: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({ "version": 1, "buckets": {} })),
        Err(e) => Err(format!("Não foi possível ler o estado do usage-window: {e}")),
    }
}

/// Escreve o arquivo inteiro via temporário + rename, no mesmo esquema do
/// plugin. Preserva campos que o Argus não conhece (`seen`, `diagnostics`).
pub fn save_state(path: &Path, state: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Não foi possível criar a pasta do estado: {e}"))?;
    }
    let mut tmp_os = path.as_os_str().to_os_string();
    tmp_os.push(".tmp");
    let tmp = PathBuf::from(tmp_os);
    if let Err(e) = std::fs::write(&tmp, text) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Não foi possível salvar o estado do usage-window: {e}"));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Não foi possível salvar o estado do usage-window: {e}"));
    }
    Ok(())
}

fn bucket_mut<'a>(state: &'a mut Value, key: &str) -> Result<&'a mut Value, String> {
    if key.trim().is_empty() {
        return Err("Chave da janela não informada.".into());
    }
    let buckets = state
        .get_mut("buckets")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "Estado do plugin sem a lista de janelas.".to_string())?;
    buckets
        .get_mut(key)
        .ok_or_else(|| format!("Janela “{key}” não encontrada no estado do plugin."))
}

/// Aplica `set_limit` no estado em memória. Sem valores (ou zeros) remove o
/// limite manual, igual à ferramenta do plugin.
pub fn apply_set_limit(state: &mut Value, key: &str, tokens: Option<u64>, requests: Option<u64>) -> Result<(), String> {
    let bucket = bucket_mut(state, key)?;
    let t = tokens.filter(|n| *n > 0);
    let r = requests.filter(|n| *n > 0);
    let obj = bucket
        .as_object_mut()
        .ok_or_else(|| "Janela com formato inesperado no estado do plugin.".to_string())?;
    if t.is_none() && r.is_none() {
        obj.remove("manualLimit");
    } else {
        let mut manual = Map::new();
        if let Some(t) = t {
            manual.insert("tokens".into(), json!(t));
        }
        if let Some(r) = r {
            manual.insert("requests".into(), json!(r));
        }
        obj.insert("manualLimit".into(), Value::Object(manual));
    }
    Ok(())
}

/// Aplica `reset_samples`: apaga as amostras e o limite manual, igual à
/// ferramenta do plugin.
pub fn apply_reset_samples(state: &mut Value, key: &str) -> Result<(), String> {
    let bucket = bucket_mut(state, key)?;
    let obj = bucket
        .as_object_mut()
        .ok_or_else(|| "Janela com formato inesperado no estado do plugin.".to_string())?;
    obj.insert("samples".into(), json!([]));
    obj.remove("manualLimit");
    Ok(())
}

/// Define limite manual para uma janela e salva no arquivo do plugin.
pub fn set_limit(key: &str, tokens: Option<u64>, requests: Option<u64>) -> Result<(), String> {
    let Some(path) = state_path() else {
        return Err("Não foi possível localizar a pasta de estado do plugin usage-window.".into());
    };
    let mut state = read_state()?;
    apply_set_limit(&mut state, key, tokens, requests)?;
    save_state(&path, &state)
}

/// Apaga amostras e limite manual de uma janela.
pub fn reset_samples(key: &str) -> Result<(), String> {
    let Some(path) = state_path() else {
        return Err("Não foi possível localizar a pasta de estado do plugin usage-window.".into());
    };
    let mut state = read_state()?;
    apply_reset_samples(&mut state, key)?;
    save_state(&path, &state)
}

/// Lista as janelas ativas agora.
pub fn list_windows() -> Result<Vec<UsageWindowInfo>, String> {
    Ok(list_from_state(&read_state()?, now_ms(), window_ms()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Linha de uso do histórico do Argus (`created_at` em segundos).
    fn linha(provider: &str, model: Option<&str>, at_ms: u64, turno: i64, inp: u64, out: u64) -> crate::db::UsageRow {
        crate::db::UsageRow {
            task_id: 1,
            project: "p".into(),
            provider: provider.into(),
            usage: crate::agents::provider::TokenUsage {
                model: model.map(str::to_string),
                input_tokens: inp,
                output_tokens: out,
                cache_read: 9_000,
                cache_write: 0,
                cost_usd: None,
            },
            created_at: (at_ms / 1000) as i64,
            turn: Some(turno),
        }
    }

    const JANELA: u64 = 5 * 3_600_000;

    #[test]
    fn claude_ganha_janela_do_historico_do_argus() {
        let agora = 100 * JANELA;
        let rows = vec![
            linha("claude", Some("claude-opus-5"), agora - 3_600_000, 1, 10, 20),
            linha("claude", Some("claude-opus-5"), agora - 3_600_000, 1, 5, 5),
            linha("claude", Some("claude-opus-5"), agora - 600_000, 2, 1, 4),
        ];
        let w = list_from_rows(&rows, &Default::default(), agora, JANELA);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].key, "claude/claude-opus-5");
        assert_eq!(w[0].provider, "claude");
        // entrada + saída; o cache lido fica de fora, como no plugin.
        assert_eq!(w[0].tokens, 45);
        assert_eq!(w[0].cache_read, 27_000);
        // Três linhas, dois turnos: requisição é turno, não linha.
        assert_eq!(w[0].requests, 2);
        assert_eq!(w[0].started_at_ms, agora - 3_600_000);
        assert_eq!(w[0].resets_at_ms, agora - 3_600_000 + JANELA);
        // Sem limite não há como estimar percentual, e nada é chutado.
        assert_eq!(w[0].percent, None);
        assert_eq!(w[0].calibration, "none");
    }

    #[test]
    fn janela_vencida_nao_aparece_e_a_nova_zera() {
        let agora = 100 * JANELA;
        let rows = vec![
            // Janela anterior, já vencida.
            linha("claude", Some("m"), agora - 2 * JANELA, 1, 1_000, 1_000),
            // Primeira requisição depois do vencimento: abre outra.
            linha("claude", Some("m"), agora - 60_000, 2, 7, 3),
        ];
        let w = list_from_rows(&rows, &Default::default(), agora, JANELA);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].tokens, 10, "o consumo da janela vencida nao pode vazar");
        assert_eq!(w[0].started_at_ms, agora - 60_000);

        // Só o que venceu: nenhuma janela viva.
        let so_velha = vec![linha("claude", Some("m"), agora - 2 * JANELA, 1, 1, 1)];
        assert!(list_from_rows(&so_velha, &Default::default(), agora, JANELA).is_empty());
    }

    #[test]
    fn limite_manual_da_o_percentual() {
        let agora = 100 * JANELA;
        let rows = vec![linha("claude", Some("m"), agora - 1000, 1, 100, 150)];
        let mut manual = std::collections::BTreeMap::new();
        manual.insert("claude/m".to_string(), ManualLimit { tokens: Some(1_000), requests: None });
        let w = list_from_rows(&rows, &manual, agora, JANELA);
        assert_eq!(w[0].percent, Some(25));
        assert_eq!(w[0].calibration, "manual");

        // O maior entre tokens e requisições manda, igual ao plugin.
        manual.insert("claude/m".to_string(), ManualLimit { tokens: Some(1_000), requests: Some(2) });
        let w = list_from_rows(&rows, &manual, agora, JANELA);
        assert_eq!(w[0].percent, Some(50));
    }

    #[test]
    fn modelos_diferentes_sao_janelas_diferentes() {
        let agora = 100 * JANELA;
        let rows = vec![
            linha("claude", Some("opus"), agora - 1000, 1, 10, 0),
            linha("claude", Some("haiku"), agora - 1000, 2, 20, 0),
            // Linha sem modelo não vira janela: somar modelos distintos seria pior.
            linha("claude", None, agora - 1000, 3, 999, 0),
        ];
        let w = list_from_rows(&rows, &Default::default(), agora, JANELA);
        let chaves: Vec<&str> = w.iter().map(|x| x.key.as_str()).collect();
        assert_eq!(chaves, vec!["claude/haiku", "claude/opus"], "ordenadas pelo consumo");
    }

    fn state(now: u64, window: u64) -> Value {
        json!({
            "version": 1,
            "seen": { "evt_1": now },
            "diagnostics": { "runtime": "v2" },
            "buckets": {
                "opencode/mimo-v2.6-flash-free": {
                    "samples": [
                        { "tokens": 100, "requests": 2, "elapsedMs": 1000, "at": now },
                        { "tokens": 300, "requests": 4, "elapsedMs": 2000, "at": now }
                    ],
                    "window": {
                        "start": now - 3_600_000,
                        "tokens": 250,
                        "input": 200,
                        "output": 30,
                        "reasoning": 20,
                        "cacheRead": 9999,
                        "requests": 3,
                        "cost": 0,
                        "warned": []
                    }
                },
                "opencode/big-pickle": {
                    "samples": [],
                    "window": { "start": now - 60_000, "tokens": 12, "requests": 1, "warned": [] }
                },
                "opencode/vencido-free": {
                    "samples": [],
                    "window": { "start": now - window - 1, "tokens": 5, "requests": 1, "warned": [] }
                }
            }
        })
    }

    #[test]
    fn so_janelas_ativas_e_com_a_maior_estimativa_mandando() {
        let now = 1_800_000_000_000;
        let window = 5 * 3_600_000;
        let rows = list_from_state(&state(now, window), now, window);
        assert_eq!(rows.len(), 2, "a janela vencida não entra");

        let free = &rows[0];
        assert_eq!(free.key, "opencode/mimo-v2.6-flash-free");
        assert!(free.is_free);
        // Mediana de 100/300 = 200 tokens e 2/4 = 3 requisições.
        assert_eq!(free.limit_tokens, Some(200));
        assert_eq!(free.limit_requests, Some(3));
        assert_eq!(free.calibration, "samples");
        assert_eq!(free.samples, 2);
        // 250/200 = 125% manda sobre 3/3 = 100%.
        assert_eq!(free.percent, Some(125));
        assert_eq!(free.resets_at_ms, free.started_at_ms + window);
        assert_eq!(free.cache_read, 9999, "cacheRead é registrado, mas não entra em tokens");
        assert_eq!(free.tokens, 250);

        let pickle = &rows[1];
        assert_eq!(pickle.model, "big-pickle");
        assert!(!pickle.is_free);
        assert_eq!(pickle.percent, None, "sem amostra e sem limite manual não há percentual");
        assert_eq!(pickle.calibration, "none");
    }

    #[test]
    fn limite_manual_substitui_a_mediana_e_marca_a_calibracao() {
        let now = 1_800_000_000_000;
        let window = 5 * 3_600_000;
        let mut s = state(now, window);
        apply_set_limit(
            &mut s,
            "opencode/mimo-v2.6-flash-free",
            Some(1_000_000),
            None,
        )
        .unwrap();
        let rows = list_from_state(&s, now, window);
        let free = rows.iter().find(|r| r.key == "opencode/mimo-v2.6-flash-free").unwrap();
        assert_eq!(free.limit_tokens, Some(1_000_000));
        assert_eq!(free.limit_requests, Some(3), "o que não foi passado segue da mediana");
        assert_eq!(free.calibration, "manual");
        assert_eq!(free.percent, Some(100), "3/3 requisições mandam sobre 250/1 mi");
    }

    #[test]
    fn reset_apaga_amostras_e_limite_sem_tocar_em_outros_campos() {
        let now = 1_800_000_000_000;
        let window = 5 * 3_600_000;
        let mut s = state(now, window);
        apply_set_limit(&mut s, "opencode/big-pickle", Some(500), Some(10)).unwrap();
        apply_reset_samples(&mut s, "opencode/big-pickle").unwrap();
        let b = &s["buckets"]["opencode/big-pickle"];
        assert_eq!(b["samples"].as_array().map(Vec::len), Some(0));
        assert!(b.get("manualLimit").is_none());
        assert_eq!(s["diagnostics"]["runtime"], "v2");
        assert!(s["seen"].get("evt_1").is_some());
        // A outra janela continua intacta.
        assert_eq!(s["buckets"]["opencode/mimo-v2.6-flash-free"]["samples"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn chave_desconhecida_e_erro_sem_gravar_nada() {
        let mut s = state(1_800_000_000_000, 5 * 3_600_000);
        assert!(apply_set_limit(&mut s, "opencode/inexistente", Some(1), None).is_err());
        assert!(apply_reset_samples(&mut s, "").is_err());
        assert_eq!(s["buckets"].as_object().map(Map::len), Some(3));
    }

    #[test]
    fn escrita_e_relitura_preservam_o_estado() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("aninhada").join("state.json");
        let original = state(1_800_000_000_000, 5 * 3_600_000);
        save_state(&path, &original).unwrap();
        let lido: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(lido, original);
        assert!(!path.with_extension("json.tmp").exists());

        // O arquivo do plugin já existe: o rename precisa substituí-lo, senão
        // nenhum ajuste manual funciona depois da primeira escrita.
        let mut alterado = original.clone();
        apply_set_limit(&mut alterado, "opencode/big-pickle", Some(42), None).unwrap();
        save_state(&path, &alterado).unwrap();
        let lido: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(lido["buckets"]["opencode/big-pickle"]["manualLimit"]["tokens"], 42);
        assert!(!path.with_extension("json.tmp").exists());
    }
}
