//! Lista de tarefas do usuário numa conversa e nível de esforço do turno.
//!
//! A lista é escrita pelo usuário no painel "Agentes". Ela vai ao agente no
//! começo de cada pedido (não no prompt de sistema, para não regravar o cache
//! quando muda) e o agente marca um item concluído escrevendo, numa linha da
//! resposta, `argus-feito: N` (N = número do item, a partir de 1).

use serde::{Deserialize, Serialize};

/// Um item da lista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Todo {
    pub text: String,
    pub done: bool,
}

/// Níveis aceitos pelo `--effort` do claude (2.1.220).
pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

pub fn valid_effort(e: &str) -> bool {
    EFFORTS.contains(&e)
}

/// Chave em `settings` da lista da tarefa.
pub fn todos_key(id: u64) -> String {
    format!("task.todos.{id}")
}

/// Chave em `settings` dos limites da tarefa (teto de custo e modelo reserva).
pub fn limits_key(id: u64) -> String {
    format!("task.limits.{id}")
}

/// Teto de custo por turno (US$) e modelo reserva de uma conversa.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    pub max_budget_usd: Option<f64>,
    pub fallback_model: Option<String>,
}

/// Marca de conversa ramificada que ainda não rodou o primeiro turno próprio.
pub fn fork_key(id: u64) -> String {
    format!("task.fork.{id}")
}

/// Chave em `settings` do esforço da tarefa.
pub fn effort_key(id: u64) -> String {
    format!("task.effort.{id}")
}

/// Limpa a lista: sem itens vazios, texto aparado, no máximo 50 itens de 300 caracteres.
pub fn normalize(list: Vec<Todo>) -> Vec<Todo> {
    list.into_iter()
        .map(|t| Todo { text: t.text.trim().chars().take(300).collect(), done: t.done })
        .filter(|t| !t.text.is_empty())
        .take(50)
        .collect()
}

/// Bloco que vai antes do pedido do usuário. Vazio se a lista estiver vazia.
pub fn prompt_block(list: &[Todo]) -> String {
    if list.is_empty() {
        return String::new();
    }
    let mut s = String::from("[Lista de tarefas do usuário, mantida no Argus]\n");
    for (i, t) in list.iter().enumerate() {
        s.push_str(&format!("{}. [{}] {}\n", i + 1, if t.done { "x" } else { " " }, t.text));
    }
    s.push_str(
        "Ao concluir um item pendente, escreva numa linha própria da resposta `argus-feito: N` \
         (N = número do item). Não marque o que não terminou.",
    );
    s
}

/// Números (a partir de 1) marcados como feitos num texto do agente.
pub fn marks(text: &str) -> Vec<usize> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim().trim_matches('`').trim();
            let rest = l.strip_prefix("argus-feito:")?;
            rest.trim().parse::<usize>().ok().filter(|n| *n > 0)
        })
        .collect()
}

/// Aplica as marcas; devolve se algo mudou.
pub fn apply_marks(list: &mut [Todo], nums: &[usize]) -> bool {
    let mut changed = false;
    for &n in nums {
        if let Some(t) = list.get_mut(n - 1) {
            if !t.done {
                t.done = true;
                changed = true;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(text: &str, done: bool) -> Todo {
        Todo { text: text.into(), done }
    }

    #[test]
    fn block_lists_items_and_rule() {
        assert_eq!(prompt_block(&[]), "");
        let b = prompt_block(&[t("relatório", false), t("testes", true)]);
        assert!(b.contains("1. [ ] relatório\n2. [x] testes\n"));
        assert!(b.contains("argus-feito: N"));
    }

    #[test]
    fn marks_read_lines_only() {
        assert_eq!(marks("feito\nargus-feito: 2\n`argus-feito: 1`\nargus-feito: x\nargus-feito: 0"), [2, 1]);
        assert!(marks("o argus-feito: 3 no meio").is_empty());
    }

    #[test]
    fn apply_ignores_out_of_range() {
        let mut l = vec![t("a", false), t("b", true)];
        assert!(apply_marks(&mut l, &[1, 9]));
        assert!(l[0].done);
        assert!(!apply_marks(&mut l, &[2]));
    }

    #[test]
    fn normalize_trims_and_drops_empty() {
        assert_eq!(normalize(vec![t("  a ", false), t("   ", true)]), vec![t("a", false)]);
        assert!(valid_effort("xhigh") && !valid_effort("ultra"));
    }
}
