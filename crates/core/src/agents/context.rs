//! Ocupação da janela de contexto do modelo.
//!
//! O que está "em contexto" é a entrada do ÚLTIMO pedido que o agente principal
//! fez ao modelo: `input + cache lido + cache gravado` daquela chamada (não a
//! soma do turno, nem os pedidos dos subagentes, que têm janela própria).

use serde::{Deserialize, Serialize};

/// Tokens do último pedido do agente principal ao modelo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextUsage {
    /// Entrada sem cache.
    pub input_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// Saída daquele pedido (não ocupa a janela até o próximo pedido).
    pub output_tokens: u64,
    /// Modelo que atendeu o pedido (como a CLI informou).
    pub model: Option<String>,
    /// Tamanho da janela, em tokens. None = desconhecido (mostra só os tokens).
    pub window: Option<u64>,
}

impl ContextUsage {
    /// Tokens ocupando a janela: entrada + cache lido + cache gravado.
    pub fn total(&self) -> u64 {
        self.input_tokens + self.cache_read + self.cache_write
    }
}

/// Tamanho da janela de contexto de um modelo, pelo nome. Modelos com o sufixo
/// `[1m]` (ou `-1m`) têm 1.000.000 tokens; os do claude (nomes `claude-*` e os
/// apelidos `opus`, `sonnet`, `haiku`, `default`), 200.000. Desconhecido = None.
pub fn window_for_model(model: &str) -> Option<u64> {
    let m = model.trim().to_ascii_lowercase();
    if m.is_empty() {
        return None;
    }
    if m.ends_with("[1m]") || m.ends_with("-1m") {
        return Some(1_000_000);
    }
    // opencode: "provedor/modelo".
    let name = m.rsplit('/').next().unwrap_or(&m);
    let claude = name.starts_with("claude")
        || name.contains("anthropic")
        || ["opus", "sonnet", "haiku", "default", "opusplan"].iter().any(|a| name == *a || name.starts_with(&format!("{a}-")));
    if claude {
        return Some(200_000);
    }
    None
}

/// Completa a janela que a CLI não informou. `task_model` é o modelo pedido à
/// CLI (pode trazer `[1m]`, que o nome devolvido pela API não traz).
pub fn resolve_window(ctx: &mut ContextUsage, task_model: Option<&str>) {
    if ctx.window.is_some() {
        return;
    }
    let task = task_model.and_then(window_for_model);
    let seen = ctx.model.as_deref().and_then(window_for_model);
    ctx.window = match (task, seen) {
        // O pedido com `[1m]` vale sobre o nome sem sufixo que a API devolve.
        (Some(t), Some(s)) => Some(t.max(s)),
        (t, s) => s.or(t),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_by_model() {
        assert_eq!(window_for_model("claude-sonnet-4-5[1m]"), Some(1_000_000));
        assert_eq!(window_for_model("sonnet[1m]"), Some(1_000_000));
        assert_eq!(window_for_model("claude-haiku-4-5-20251001"), Some(200_000));
        assert_eq!(window_for_model("opus"), Some(200_000));
        assert_eq!(window_for_model("anthropic/claude-sonnet-4-5"), Some(200_000));
        assert_eq!(window_for_model("openai/gpt-x"), None);
        assert_eq!(window_for_model(""), None);
    }

    #[test]
    fn resolve_prefers_cli_then_task_suffix() {
        let mut c = ContextUsage { model: Some("claude-sonnet-4-5".into()), window: Some(123), ..Default::default() };
        resolve_window(&mut c, Some("sonnet[1m]"));
        assert_eq!(c.window, Some(123));
        c.window = None;
        resolve_window(&mut c, Some("sonnet[1m]"));
        assert_eq!(c.window, Some(1_000_000));
        c.window = None;
        resolve_window(&mut c, None);
        assert_eq!(c.window, Some(200_000));
        let mut o = ContextUsage { model: Some("openai/x".into()), ..Default::default() };
        resolve_window(&mut o, Some("openai/x"));
        assert_eq!(o.window, None);
    }
}
