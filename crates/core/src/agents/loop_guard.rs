//! Detecção de loop no turno: a mesma chamada de ferramenta (nome + entrada)
//! várias vezes seguidas. Roda no core, então vale com a aba fechada e no remoto.
//! Só conta as ferramentas do agente principal (eventos `Scoped` de subagente
//! não entram nem quebram a sequência).

use super::provider::AgentEvent;

/// Quantas chamadas idênticas seguidas param o turno.
pub const REPEAT_LIMIT: u32 = 3;

#[derive(Debug, Default)]
pub struct RepeatGuard {
    last: Option<(String, serde_json::Value)>,
    times: u32,
}

impl RepeatGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observa um evento; devolve o aviso quando a mesma chamada chega a
    /// `REPEAT_LIMIT` vezes seguidas (e recomeça a contagem).
    pub fn observe(&mut self, ev: &AgentEvent) -> Option<String> {
        let AgentEvent::ToolUse { name, input } = ev else { return None };
        // `serde_json::Value` compara objetos sem depender da ordem das chaves.
        if self.last.as_ref().is_some_and(|(n, i)| n == name && i == input) {
            self.times += 1;
        } else {
            self.last = Some((name.clone(), input.clone()));
            self.times = 1;
        }
        if self.times >= REPEAT_LIMIT {
            self.last = None;
            self.times = 0;
            return Some(message(name));
        }
        None
    }
}

pub fn message(tool: &str) -> String {
    format!("Parei: o agente repetiu {tool} {REPEAT_LIMIT} vezes seguidas")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool(name: &str, input: serde_json::Value) -> AgentEvent {
        AgentEvent::ToolUse { name: name.into(), input }
    }

    #[test]
    fn stops_on_third_identical_call() {
        let mut g = RepeatGuard::new();
        let a = tool("Read", json!({"file": "a.rs", "n": 1}));
        assert_eq!(g.observe(&a), None);
        assert_eq!(g.observe(&AgentEvent::Text("pensando".into())), None, "texto não quebra a sequência");
        assert_eq!(g.observe(&tool("Read", json!({"n": 1, "file": "a.rs"}))), None, "ordem das chaves não importa");
        assert_eq!(g.observe(&a).as_deref(), Some("Parei: o agente repetiu Read 3 vezes seguidas"));
        // Recomeça depois de disparar.
        assert_eq!(g.observe(&a), None);
    }

    #[test]
    fn different_call_resets() {
        let mut g = RepeatGuard::new();
        let a = tool("Bash", json!({"command": "ls"}));
        g.observe(&a);
        g.observe(&a);
        assert_eq!(g.observe(&tool("Bash", json!({"command": "pwd"}))), None);
        assert_eq!(g.observe(&a), None);
        assert_eq!(g.observe(&tool("Grep", json!({"command": "ls"}))), None);
    }

    #[test]
    fn subagent_tools_ignored() {
        let mut g = RepeatGuard::new();
        let a = tool("Read", json!({}));
        let scoped = AgentEvent::Scoped { subagent: "s".into(), event: Box::new(a.clone()) };
        for _ in 0..5 {
            assert_eq!(g.observe(&scoped), None);
        }
        g.observe(&a);
        g.observe(&scoped);
        g.observe(&a);
        assert!(g.observe(&a).is_some());
    }
}
