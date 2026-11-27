//! Janelas de 5 horas do uso.
//!
//! Duas fontes, na mesma lista: o plugin `usage-window` do OpenCode, que já
//! calcula o consumo do que passa por ele, e o histórico do próprio Argus, que
//! cobre o resto — o claude antes de tudo, que o plugin nunca viu. A janela do
//! plugin manda quando a mesma chave existe nos dois lados: lá existe amostra
//! de rate limit, aqui não.
//!
//! O limite manual segue a fonte: chave do plugin vai para o arquivo dele,
//! chave calculada pelo Argus vai para `settings` no banco do Argus.
//! Leitura e escrita de disco vão para `spawn_blocking`.

use super::blocking;
use crate::state::AppCtx;
use argus_core::db::TimeRange;
use argus_core::usage_window::{self, ManualLimit, UsageWindowInfo};
use std::collections::{BTreeMap, BTreeSet};
use tauri::State;

/// Prefixo dos limites manuais das janelas que o Argus calcula.
const PREFIXO: &str = "usage_window/";

/// Quanto histórico ler para achar o começo da janela viva. Uma janela só
/// nasce da primeira requisição depois que a anterior vence, então não basta
/// ler as últimas 5 horas: é o encadeamento que diz onde a atual começou.
const HISTORICO_DIAS: i64 = 7;

fn limite_salvo(db: &argus_core::db::Db) -> BTreeMap<String, ManualLimit> {
    db.settings_by_prefix(PREFIXO)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(k, v)| {
            let j: serde_json::Value = serde_json::from_str(&v).ok()?;
            let n = |c: &str| j.get(c).and_then(serde_json::Value::as_u64).filter(|x| *x > 0);
            Some((k.trim_start_matches(PREFIXO).to_string(), ManualLimit { tokens: n("tokens"), requests: n("requests") }))
        })
        .collect()
}

/// Janelas ativas das duas fontes, já ordenadas pelo percentual.
#[tauri::command]
pub async fn usage_windows(ctx: State<'_, AppCtx>) -> Result<Vec<UsageWindowInfo>, String> {
    let orch = ctx.orch.clone();
    blocking(move || {
        let mut out = usage_window::list_windows()?;
        let Some(db) = orch.db() else { return Ok(out) };
        let janela = usage_window::window_ms();
        let segundos = argus_core::db::now();
        let agora = (segundos.max(0) as u64).saturating_mul(1000);
        let desde = segundos - HISTORICO_DIAS * 86_400;
        let rows = db.usage_rows(TimeRange { from: Some(desde), to: None }).unwrap_or_default();
        let do_plugin: BTreeSet<&str> = out.iter().map(|w| w.key.as_str()).collect();
        let calculadas: Vec<UsageWindowInfo> = usage_window::list_from_rows(&rows, &limite_salvo(&db), agora, janela)
            .into_iter()
            .filter(|w| !do_plugin.contains(w.key.as_str()))
            .collect();
        out.extend(calculadas);
        out.sort_by(|a, b| b.percent.unwrap_or(0).cmp(&a.percent.unwrap_or(0)).then_with(|| b.tokens.cmp(&a.tokens)));
        Ok(out)
    })
    .await
}

/// A chave existe no estado do plugin? Só essas podem ser gravadas lá.
fn e_do_plugin(key: &str) -> bool {
    usage_window::read_state()
        .ok()
        .and_then(|s| s.get("buckets").and_then(|b| b.as_object()).map(|b| b.contains_key(key)))
        .unwrap_or(false)
}

/// Define limite manual de tokens e/ou requisições para uma janela.
#[tauri::command]
pub async fn usage_window_set_limit(
    ctx: State<'_, AppCtx>,
    key: String,
    tokens: Option<u64>,
    requests: Option<u64>,
) -> Result<(), String> {
    let orch = ctx.orch.clone();
    blocking(move || {
        if e_do_plugin(&key) {
            return usage_window::set_limit(&key, tokens, requests);
        }
        let db = orch.db().ok_or("O banco do Argus não está disponível.")?;
        let limpar = tokens.unwrap_or(0) == 0 && requests.unwrap_or(0) == 0;
        let valor = (!limpar).then(|| {
            serde_json::json!({ "tokens": tokens.filter(|t| *t > 0), "requests": requests.filter(|r| *r > 0) }).to_string()
        });
        db.set_setting(&format!("{PREFIXO}{key}"), valor.as_deref())
            .map_err(|e| format!("Não foi possível salvar o limite: {e}"))
    })
    .await
}

/// Apaga amostras e limite manual de uma janela. Nas janelas calculadas pelo
/// Argus não há amostra: some só o limite manual.
#[tauri::command]
pub async fn usage_window_reset_samples(ctx: State<'_, AppCtx>, key: String) -> Result<(), String> {
    let orch = ctx.orch.clone();
    blocking(move || {
        if e_do_plugin(&key) {
            return usage_window::reset_samples(&key);
        }
        let db = orch.db().ok_or("O banco do Argus não está disponível.")?;
        db.set_setting(&format!("{PREFIXO}{key}"), None)
            .map_err(|e| format!("Não foi possível limpar o limite: {e}"))
    })
    .await
}
