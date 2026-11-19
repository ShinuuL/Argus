//! Check-list de pendências do projeto: lê o markdown (PROXIMOS_PASSOS.md,
//! PENDENCIAS.md ou TODO.md na raiz) e separa os itens `- [ ]` / `- [x]`
//! pelos títulos em que aparecem.
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Nomes procurados na raiz do projeto, nesta ordem.
pub const CHECKLIST_FILES: [&str; 3] = ["PROXIMOS_PASSOS.md", "PENDENCIAS.md", "TODO.md"];

/// Acima disso o arquivo não é lido (um markdown de pendências é pequeno).
const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChecklistItem {
    /// Texto do item, sem o marcador e sem `**`/`` ` ``.
    pub text: String,
    pub done: bool,
    /// Linha no arquivo (começa em 1).
    pub line: usize,
    /// Nível de recuo (0 = item de primeiro nível).
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChecklistSection {
    /// Título (vazio = itens antes do primeiro título).
    pub title: String,
    /// Nível do título (1 a 6; 0 sem título).
    pub level: usize,
    pub items: Vec<ChecklistItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Checklist {
    pub path: PathBuf,
    /// Nome do arquivo (ex.: PENDENCIAS.md).
    pub name: String,
    /// Só as seções que têm itens.
    pub sections: Vec<ChecklistSection>,
}

/// Tira ênfase simples (`**`, `__`, `` ` ``) e espaços nas pontas.
pub fn clean_text(s: &str) -> String {
    s.replace("**", "").replace("__", "").replace('`', "").trim().to_string()
}

/// Se a linha for um item de check-list, devolve (recuo, feito, texto).
pub fn parse_item(line: &str) -> Option<(usize, bool, String)> {
    let indent_raw = line.len() - line.trim_start().len();
    let indent: usize = line[..indent_raw].chars().map(|c| if c == '\t' { 4 } else { 1 }).sum();
    let rest = line.trim_start();
    let rest = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix("* "))
        .or_else(|| rest.strip_prefix("+ "))?;
    let rest = rest.trim_start();
    let done = match rest.get(..3)? {
        "[ ]" => false,
        "[x]" | "[X]" => true,
        _ => return None,
    };
    let text = clean_text(&rest[3..]);
    if text.is_empty() {
        return None;
    }
    Some((indent / 2, done, text))
}

/// Separa os itens pelos títulos. Ignora o que estiver dentro de blocos de código.
pub fn parse(md: &str) -> Vec<ChecklistSection> {
    let mut out: Vec<ChecklistSection> = Vec::new();
    let mut cur = ChecklistSection { title: String::new(), level: 0, items: Vec::new() };
    let mut fence = false;
    for (i, line) in md.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let hashes = t.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') && line.len() == t.len() {
            if !cur.items.is_empty() {
                out.push(cur);
            }
            cur = ChecklistSection { title: clean_text(t[hashes..].trim_end_matches('#')), level: hashes, items: Vec::new() };
            continue;
        }
        if let Some((depth, done, text)) = parse_item(line) {
            cur.items.push(ChecklistItem { text, done, line: i + 1, depth });
        }
    }
    if !cur.items.is_empty() {
        out.push(cur);
    }
    out
}

/// Primeiro arquivo de pendências que existe na raiz do projeto.
pub fn find_file(project: &Path) -> Option<PathBuf> {
    CHECKLIST_FILES.iter().map(|n| project.join(n)).find(|p| p.is_file())
}

/// Lê e separa o check-list do projeto. `None` se não houver arquivo.
/// Prefere o primeiro arquivo que tenha itens; sem itens em nenhum, devolve o primeiro que existe.
pub fn load(project: &Path) -> Result<Option<Checklist>, String> {
    let mut first: Option<Checklist> = None;
    for name in CHECKLIST_FILES {
        let path = project.join(name);
        if !path.is_file() {
            continue;
        }
        let size = std::fs::metadata(&path).map_err(|e| format!("Erro ao abrir {}: {e}", path.display()))?.len();
        if size > MAX_BYTES {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("Erro ao abrir {}: {e}", path.display()))?;
        let sections = parse(&String::from_utf8_lossy(&bytes));
        let list = Checklist { path, name: name.to_string(), sections };
        if !list.sections.is_empty() {
            return Ok(Some(list));
        }
        first.get_or_insert(list);
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_itens_feitos_e_abertos() {
        assert_eq!(parse_item("- [ ] fazer A"), Some((0, false, "fazer A".into())));
        assert_eq!(parse_item("- [x] feito"), Some((0, true, "feito".into())));
        assert_eq!(parse_item("* [X] **negrito** e `código`"), Some((0, true, "negrito e código".into())));
        assert_eq!(parse_item("    - [ ] filho"), Some((2, false, "filho".into())));
        assert_eq!(parse_item("\t- [ ] tab"), Some((2, false, "tab".into())));
    }

    #[test]
    fn ignora_o_que_nao_e_item() {
        assert_eq!(parse_item("- item comum"), None);
        assert_eq!(parse_item("[ ] sem marcador"), None);
        assert_eq!(parse_item("- [ ]   "), None);
        assert_eq!(parse_item("- [-] outro"), None);
        assert_eq!(parse_item(""), None);
    }

    #[test]
    fn agrupa_por_titulo_e_descarta_secoes_vazias() {
        let md = "# Projeto\n\n- [ ] solto\n\n## Base\ntexto\n- [x] um\n- [ ] dois\n  - [ ] dois.a\n\n## Vazia\nnada\n\n### Fim ##\n- [x] três\n";
        let s = parse(md);
        assert_eq!(s.len(), 3);
        assert_eq!((s[0].title.as_str(), s[0].level, s[0].items.len()), ("Projeto", 1, 1));
        assert_eq!(s[1].title, "Base");
        assert_eq!(s[1].items.iter().map(|i| i.done).collect::<Vec<_>>(), vec![true, false, false]);
        assert_eq!(s[1].items[2].depth, 1);
        assert_eq!(s[1].items[0].line, 7);
        assert_eq!((s[2].title.as_str(), s[2].level), ("Fim", 3));
    }

    #[test]
    fn itens_antes_de_titulo_e_blocos_de_codigo() {
        let md = "- [ ] antes\n```md\n# não é título\n- [ ] não é item\n```\n#sem espaço\n- [x] depois\n";
        let s = parse(md);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].title, "");
        assert_eq!(s[0].items.len(), 2);
        assert_eq!(s[0].items[1].text, "depois");
    }

    #[test]
    fn crlf_nao_entra_no_texto() {
        let s = parse("## A\r\n- [ ] um\r\n");
        assert_eq!(s[0].title, "A");
        assert_eq!(s[0].items[0].text, "um");
    }

    #[test]
    fn carrega_o_primeiro_arquivo_com_itens() {
        let dir = std::env::temp_dir().join(format!("argus-checklist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(load(&dir).unwrap(), None);
        std::fs::write(dir.join("PROXIMOS_PASSOS.md"), "# Sem itens\n").unwrap();
        std::fs::write(dir.join("TODO.md"), "## T\n- [ ] x\n").unwrap();
        let l = load(&dir).unwrap().unwrap();
        assert_eq!(l.name, "TODO.md");
        assert_eq!(l.sections[0].items[0].text, "x");
        std::fs::remove_file(dir.join("TODO.md")).unwrap();
        assert_eq!(load(&dir).unwrap().unwrap().name, "PROXIMOS_PASSOS.md");
        assert!(find_file(&dir).is_some());
        std::fs::remove_dir_all(&dir).ok();
    }
}
