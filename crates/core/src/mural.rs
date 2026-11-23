//! Mural de arquivos: quem está mexendo em quê, agora.
//!
//! Duas tarefas podem rodar na mesma pasta ao mesmo tempo (modo direto), e uma
//! tarefa sozinha espalha subagentes que editam em paralelo. Até aqui ninguém
//! avisava ninguém: dois agentes abriam o mesmo arquivo, o segundo salvava por
//! cima do primeiro e o conflito só aparecia no diff, depois de pronto.
//!
//! O mural resolve isso sem pedir nada à CLI. O orquestrador já vê todo
//! `ToolUse` que passa; daqui a gente reconhece quais ferramentas tocam em
//! arquivo ([`tool_path`]) e anota. O recado volta para os agentes de duas
//! formas:
//!
//! - no começo do turno seguinte, como o aviso de [`Mural::aviso`] colado antes
//!   do pedido (é isso que faz um agente "saber" do outro);
//! - na tela, pelo evento `Mural` do orquestrador.
//!
//! Anotação é lembrete, não tranca: ninguém é impedido de escrever. Travar de
//! verdade exigiria a CLI cooperar, e um agente bloqueado em silêncio é pior do
//! que dois agentes avisados.

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

/// Quanto tempo uma anotação vale sem ser renovada. Agente que abriu um arquivo
/// e seguiu a vida não deve segurar o nome dele para sempre.
pub const VALIDADE_SECS: u64 = 15 * 60;

/// Quantos arquivos o aviso cita antes de resumir o resto em "e mais N".
const AVISO_MAX: usize = 8;

/// Uma anotação: um agente tocou num arquivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    pub task: i64,
    /// Subagente que mexeu, quando foi um (`None` = o agente principal).
    pub agent: Option<String>,
    /// Caminho como a ferramenta escreveu (é o que vai para a tela).
    pub path: String,
    /// Escreveu (`true`) ou só leu.
    pub write: bool,
    /// Ferramenta que deu origem à anotação (`Edit`, `Write`, `Read`…).
    pub tool: String,
    /// Segundos desde 1970.
    pub at: u64,
}

impl Claim {
    /// Nome de quem mexeu, para uma linha de texto.
    pub fn quem(&self) -> String {
        match &self.agent {
            Some(a) => format!("tarefa {} / {a}", self.task),
            None => format!("tarefa {}", self.task),
        }
    }
}

/// Chave de comparação: no Windows `src\a.rs` e `SRC/A.RS` são o mesmo arquivo.
fn chave(path: &str) -> String {
    let p = path.replace('\\', "/");
    if cfg!(windows) { p.to_lowercase() } else { p }
}

/// Arquivo que uma ferramenta tocou, e se foi para escrever.
///
/// A lista cobre as ferramentas das quatro CLIs (claude, codex, gemini,
/// opencode). Ferramenta desconhecida não vira anotação: errar para menos aqui
/// custa um aviso a menos; errar para mais enche o mural de ruído.
pub fn tool_path(tool: &str, input: &serde_json::Value) -> Option<(String, bool)> {
    let t = tool.rsplit(['.', '/', '_']).next().unwrap_or(tool).to_lowercase();
    let write = match t.as_str() {
        "edit" | "write" | "multiedit" | "notebookedit" | "create" | "patch" | "replace" => true,
        "read" | "view" | "notebookread" | "cat" => false,
        _ => return None,
    };
    let path = ["file_path", "filePath", "path", "notebook_path", "target_file", "filename", "absolute_path"]
        .iter()
        .find_map(|k| input.get(k).and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|p| !p.is_empty())?;
    Some((path.to_string(), write))
}

/// O quadro em si. Vive no processo, não no banco: o que importa é quem está
/// mexendo *agora*, e um Argus reiniciado não tem agente nenhum rodando.
#[derive(Default)]
pub struct Mural {
    itens: Mutex<Vec<Claim>>,
}

/// Mural do processo.
pub fn global() -> &'static Mural {
    static M: OnceLock<Mural> = OnceLock::new();
    M.get_or_init(Mural::default)
}

/// Agora, em segundos desde 1970.
pub fn agora() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Mural {
    /// Anota um `ToolUse`. Devolve a anotação e com quem ela bate de frente —
    /// vazio quando ninguém mais está no arquivo. `None` se a ferramenta não
    /// mexe em arquivo.
    pub fn note(&self, task: i64, agent: Option<&str>, tool: &str, input: &serde_json::Value, now: u64) -> Option<(Claim, Vec<Claim>)> {
        let (path, write) = tool_path(tool, input)?;
        let novo = Claim { task, agent: agent.map(str::to_string), path, write, tool: tool.to_string(), at: now };
        let mut itens = self.itens.lock().unwrap();
        itens.retain(|c| now.saturating_sub(c.at) < VALIDADE_SECS);
        let k = chave(&novo.path);
        // Conflito é entre tarefas *diferentes* com pelo menos uma escrita:
        // dois subagentes da mesma tarefa já compartilham a conversa do pai, e
        // duas leituras não atrapalham ninguém.
        let conflitos: Vec<Claim> = itens.iter().filter(|c| c.task != task && (c.write || write) && chave(&c.path) == k).cloned().collect();
        // Mesma dupla no mesmo arquivo só renova a hora: um agente edita o mesmo
        // arquivo dez vezes seguidas e isso são dez linhas iguais na tela.
        match itens.iter_mut().find(|c| c.task == task && c.agent.as_deref() == agent && chave(&c.path) == k) {
            Some(c) => {
                c.at = now;
                c.write |= write;
                c.tool = novo.tool.clone();
            }
            None => itens.push(novo.clone()),
        }
        Some((novo, conflitos))
    }

    /// Tudo que ainda vale, do mais recente para o mais antigo.
    pub fn list(&self, now: u64) -> Vec<Claim> {
        let mut itens = self.itens.lock().unwrap();
        itens.retain(|c| now.saturating_sub(c.at) < VALIDADE_SECS);
        let mut out = itens.clone();
        out.sort_by(|a, b| b.at.cmp(&a.at));
        out
    }

    /// O que as *outras* tarefas estão mexendo (só escrita: é o que atrapalha).
    pub fn others(&self, task: i64, now: u64) -> Vec<Claim> {
        self.list(now).into_iter().filter(|c| c.task != task && c.write).collect()
    }

    /// Esquece a tarefa. Chamado quando ela termina: agente parado não segura arquivo.
    pub fn clear_task(&self, task: i64) {
        self.itens.lock().unwrap().retain(|c| c.task != task);
    }

    /// Aviso para colar antes do pedido do turno, ou `None` se o campo está livre.
    ///
    /// É assim que um agente fica sabendo do outro: o texto entra no prompt como
    /// contexto, em português, no mesmo tom do resto dos avisos do Argus.
    pub fn aviso(&self, task: i64, now: u64) -> Option<String> {
        let outros = self.others(task, now);
        if outros.is_empty() {
            return None;
        }
        let mut linhas: Vec<String> = Vec::new();
        for c in outros.iter().take(AVISO_MAX) {
            linhas.push(format!("- {} ({})", c.path, c.quem()));
        }
        if outros.len() > AVISO_MAX {
            linhas.push(format!("- e mais {} arquivo(s)", outros.len() - AVISO_MAX));
        }
        Some(format!(
            "[Argus] Outros agentes estão editando estes arquivos agora:\n{}\n\nSe precisar mexer num deles, leia o arquivo de novo imediatamente antes de escrever, e prefira avisar no texto da resposta a sobrescrever trabalho alheio.",
            linhas.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reconhece_so_ferramenta_de_arquivo() {
        assert_eq!(tool_path("Edit", &json!({"file_path": "a.rs"})), Some(("a.rs".into(), true)));
        assert_eq!(tool_path("Read", &json!({"path": "b.rs"})), Some(("b.rs".into(), false)));
        // Nome com prefixo de servidor MCP (`mcp__x__write`) cai no último pedaço.
        assert_eq!(tool_path("mcp__fs__write", &json!({"filePath": "c.rs"})), Some(("c.rs".into(), true)));
        // Sem caminho, ou ferramenta que não é de arquivo: nada.
        assert_eq!(tool_path("Edit", &json!({"velho": "x"})), None);
        assert_eq!(tool_path("Bash", &json!({"file_path": "a.rs"})), None);
        assert_eq!(tool_path("Write", &json!({"file_path": "  "})), None);
    }

    #[test]
    fn aponta_conflito_entre_tarefas_e_poupa_a_mesma() {
        let m = Mural::default();
        let t = 1_000;
        let (_, c) = m.note(1, None, "Edit", &json!({"file_path": "src/a.rs"}), t).unwrap();
        assert!(c.is_empty(), "primeiro a chegar não conflita com ninguém");

        // Mesma tarefa, outro subagente: não é conflito.
        let (_, c) = m.note(1, Some("sub-1"), "Edit", &json!({"file_path": "src/a.rs"}), t).unwrap();
        assert!(c.is_empty());

        // Outra tarefa só lendo o que a 1 escreve: conflita (ela vai ler metade).
        let (_, c) = m.note(2, None, "Read", &json!({"file_path": "src/a.rs"}), t).unwrap();
        assert_eq!(c.len(), 2, "as duas anotações da tarefa 1: {c:?}");

        // Duas leituras não atrapalham.
        let (_, c) = m.note(3, None, "Read", &json!({"file_path": "só-leitura.md"}), t).unwrap();
        assert!(c.is_empty());
        let (_, c) = m.note(4, None, "Read", &json!({"file_path": "só-leitura.md"}), t).unwrap();
        assert!(c.is_empty());
    }

    #[test]
    fn repetir_no_mesmo_arquivo_nao_duplica_a_linha() {
        let m = Mural::default();
        m.note(1, None, "Read", &json!({"file_path": "a.rs"}), 10).unwrap();
        m.note(1, None, "Edit", &json!({"file_path": "a.rs"}), 20).unwrap();
        let l = m.list(20);
        assert_eq!(l.len(), 1);
        // A leitura virou escrita e a hora é a da última vez.
        assert!(l[0].write && l[0].at == 20 && l[0].tool == "Edit");
    }

    #[test]
    fn esquece_o_que_venceu_e_a_tarefa_que_acabou() {
        let m = Mural::default();
        m.note(1, None, "Edit", &json!({"file_path": "a.rs"}), 100).unwrap();
        m.note(2, None, "Edit", &json!({"file_path": "b.rs"}), 100).unwrap();
        assert_eq!(m.list(100 + VALIDADE_SECS - 1).len(), 2);
        m.clear_task(2);
        assert_eq!(m.list(100).len(), 1);
        assert!(m.list(100 + VALIDADE_SECS).is_empty(), "passou da validade");
    }

    #[test]
    fn aviso_cita_os_outros_e_cala_sobre_a_propria_tarefa() {
        let m = Mural::default();
        let t = 500;
        m.note(1, None, "Edit", &json!({"file_path": "src/a.rs"}), t).unwrap();
        m.note(1, Some("revisor"), "Edit", &json!({"file_path": "src/b.rs"}), t).unwrap();
        m.note(2, None, "Edit", &json!({"file_path": "src/c.rs"}), t).unwrap();

        assert!(m.aviso(1, t).is_some_and(|a| a.contains("src/c.rs") && !a.contains("src/a.rs")));
        let a = m.aviso(2, t).unwrap();
        assert!(a.contains("src/a.rs") && a.contains("tarefa 1 / revisor"));
        // Tarefa sozinha no campo não recebe aviso nenhum.
        m.clear_task(1);
        assert_eq!(m.aviso(2, t), None);
    }

    #[test]
    fn aviso_resume_quando_a_lista_e_longa() {
        let m = Mural::default();
        for i in 0..AVISO_MAX + 3 {
            m.note(9, None, "Edit", &json!({ "file_path": format!("f{i}.rs") }), 1).unwrap();
        }
        let a = m.aviso(1, 1).unwrap();
        assert!(a.contains("e mais 3 arquivo(s)"), "{a}");
        assert!(!a.contains("f10.rs"));
    }
}
