//! Resumo da conversa entregue ao agente que assume uma tarefa depois de uma
//! troca de provedor. As CLIs não retomam a sessão uma da outra: o Argus monta o
//! contexto a partir do histórico gravado (pedidos do usuário e respostas em
//! texto do agente principal, mais a lista de arquivos alterados). A saída bruta
//! das ferramentas não entra.

use super::provider::AgentEvent;

/// Tamanho máximo (em caracteres) da parte de conversa do resumo.
pub const MAX_CHARS: usize = 24_000;

/// Corta o meio de `text` quando passa de `max` caracteres, avisando do corte.
/// Fica o começo (um terço) e o fim (dois terços): o fim é o que mais importa
/// para continuar a conversa.
pub fn cut_middle(text: &str, max: usize) -> String {
    let total = text.chars().count();
    if total <= max {
        return text.to_string();
    }
    let head = max / 3;
    let tail = max - head;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(total - tail).collect();
    format!(
        "{start}\n\n[... trecho do meio cortado pelo Argus: {} caracteres omitidos ...]\n\n{end}",
        total - head - tail
    )
}

/// Eventos que interessam ao provedor `to`: se ele já atendeu esta conversa e a
/// sessão dele será retomada (`resumed`), só o que aconteceu depois que ele saiu.
fn relevant<'a>(events: &'a [AgentEvent], to: &str, resumed: bool) -> &'a [AgentEvent] {
    if !resumed {
        return events;
    }
    let left = events.iter().rposition(|e| matches!(e, AgentEvent::ProviderSwitch { from, .. } if from == to));
    match left {
        Some(i) => &events[i..],
        None => events,
    }
}

/// Monta o bloco de contexto que vai na frente do próximo pedido. Devolve vazio
/// quando não há o que contar (histórico vazio).
///
/// - `to`: id do provedor que assume; `from_name`: nome de exibição de quem atendia;
/// - `resumed`: a sessão antiga de `to` será retomada (ele só precisa do que perdeu);
/// - `changed_files`: arquivos alterados na pasta de trabalho até aqui.
pub fn build_context(
    events: &[AgentEvent],
    to: &str,
    from_name: &str,
    resumed: bool,
    changed_files: &[String],
    max_chars: usize,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    for ev in relevant(events, to, resumed) {
        match ev {
            // Só o agente principal: eventos de subagente (`Scoped`) ficam de fora.
            AgentEvent::User(t) if !t.trim().is_empty() => lines.push(format!("Usuário: {}", t.trim())),
            AgentEvent::Text(t) if !t.trim().is_empty() => lines.push(format!("Agente: {}", t.trim())),
            _ => {}
        }
    }
    // Nada foi dito (ou, voltando a um provedor, nada aconteceu enquanto ele esteve fora).
    if lines.is_empty() && (resumed || changed_files.is_empty()) {
        return String::new();
    }
    let intro = if resumed {
        format!(
            "[Contexto montado pelo Argus. Enquanto você esteve fora desta conversa, outro agente ({from_name}) a atendeu, na mesma pasta de trabalho. Abaixo, o que aconteceu nesse intervalo. Não responda a este bloco: responda ao pedido que vem depois dele.]"
        )
    } else {
        format!(
            "[Contexto montado pelo Argus. Esta conversa vinha sendo atendida por outro agente ({from_name}); você continua a partir daqui, na mesma pasta de trabalho. Abaixo, o resumo da conversa até aqui: pedidos do usuário e respostas do agente, sem a saída das ferramentas. Não responda a este bloco: responda ao pedido que vem depois dele.]"
        )
    };
    let mut out = intro;
    if !lines.is_empty() {
        out.push_str("\n\n");
        out.push_str(&cut_middle(&lines.join("\n\n"), max_chars));
    }
    if !changed_files.is_empty() {
        const MAX_FILES: usize = 60;
        out.push_str("\n\nArquivos alterados até aqui:\n");
        for f in changed_files.iter().take(MAX_FILES) {
            out.push_str(&format!("- {f}\n"));
        }
        if changed_files.len() > MAX_FILES {
            out.push_str(&format!("- (e mais {} arquivos)\n", changed_files.len() - MAX_FILES));
        }
    }
    out.push_str("\n\n[Fim do contexto. Pedido atual do usuário:]");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sw(from: &str, to: &str) -> AgentEvent {
        AgentEvent::ProviderSwitch { from: from.into(), to: to.into() }
    }

    #[test]
    fn context_has_requests_answers_and_files_without_tool_output() {
        let events = vec![
            AgentEvent::User("crie o módulo de login".into()),
            AgentEvent::Session("s1".into()),
            AgentEvent::ToolUse { name: "Write".into(), input: json!({"file_path": "login.rs", "content": "SEGREDO_DA_FERRAMENTA"}) },
            AgentEvent::Text("interno do subagente".into()).scoped(Some("toolu_1")),
            AgentEvent::Text("Criei o módulo em login.rs.".into()),
            AgentEvent::Raw("aviso do stderr".into()),
            AgentEvent::Result { ok: true, summary: "fim".into(), cost: None },
            sw("claude", "opencode"),
        ];
        let c = build_context(&events, "opencode", "Claude Code", false, &["login.rs".into()], MAX_CHARS);
        assert!(c.contains("Usuário: crie o módulo de login"));
        assert!(c.contains("Agente: Criei o módulo em login.rs."));
        assert!(c.contains("Arquivos alterados até aqui:\n- login.rs"));
        assert!(c.contains("outro agente (Claude Code)"));
        assert!(c.ends_with("[Fim do contexto. Pedido atual do usuário:]"));
        for absent in ["SEGREDO_DA_FERRAMENTA", "interno do subagente", "aviso do stderr", "s1"] {
            assert!(!c.contains(absent), "{absent} não devia entrar: {c}");
        }
        assert_eq!(build_context(&[], "opencode", "Claude Code", false, &[], MAX_CHARS), "");
    }

    #[test]
    fn long_history_loses_the_middle_with_a_notice() {
        let mut events = vec![AgentEvent::User("PRIMEIRO pedido".into())];
        for i in 0..400 {
            events.push(AgentEvent::Text(format!("resposta intermediária número {i} com algum texto para ocupar espaço")));
        }
        events.push(AgentEvent::User("ÚLTIMO pedido".into()));
        let c = build_context(&events, "opencode", "Claude Code", false, &[], 2_000);
        assert!(c.contains("PRIMEIRO pedido") && c.contains("ÚLTIMO pedido"));
        assert!(c.contains("trecho do meio cortado pelo Argus"));
        assert!(c.chars().count() < 2_000 + 700, "{}", c.chars().count());
        // Texto curto passa inteiro; o corte respeita caracteres acentuados.
        assert_eq!(cut_middle("ação", 10), "ação");
        assert!(cut_middle(&"çã".repeat(50), 30).contains("70 caracteres omitidos"));
    }

    /// Voltando a um provedor que já atendeu a conversa (sessão retomada), ele só
    /// recebe o que aconteceu depois que saiu.
    #[test]
    fn returning_provider_gets_only_what_it_missed() {
        let events = vec![
            AgentEvent::User("pedido antigo".into()),
            AgentEvent::Text("resposta antiga do claude".into()),
            sw("claude", "opencode"),
            AgentEvent::User("pedido ao opencode".into()),
            AgentEvent::Text("resposta do opencode".into()),
            sw("opencode", "claude"),
        ];
        let c = build_context(&events, "claude", "OpenCode", true, &[], MAX_CHARS);
        assert!(c.contains("pedido ao opencode") && c.contains("resposta do opencode"));
        assert!(!c.contains("pedido antigo") && !c.contains("resposta antiga"));
        assert!(c.contains("Enquanto você esteve fora"));
        // Sem sessão para retomar, recebe tudo.
        let c = build_context(&events, "claude", "OpenCode", false, &[], MAX_CHARS);
        assert!(c.contains("pedido antigo") && c.contains("pedido ao opencode"));
    }
}
