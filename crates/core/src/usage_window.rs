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
