//! Agentes e skills em arquivo, no formato que as CLIs já leem.
//!
//! Nada aqui é proprietário: um agente é um `.md` com frontmatter YAML em
//! `.claude/agents/<nome>.md`, uma skill é `.claude/skills/<nome>/SKILL.md`, e
//! o contrato do projeto é o `AGENTS.md` da raiz. O mesmo arquivo vale para
//! Claude, Codex, Gemini e quem mais leia esses caminhos — por isso o Argus
//! grava o formato de todo mundo em vez de um formato só dele.
//!
//! Dois escopos: `Global` (`~/.claude/...`, vale em qualquer pasta) e
//! `Project` (`<projeto>/.claude/...`, entra no git). Quando o mesmo nome
//! existe nos dois, o do projeto é o que a CLI usa; o global fica marcado como
//! `shadowed`.
//!
//! Skills também são lidas de `~/.agents/skills` e `~/.config/opencode/skills`
//! (além de `~/.claude/skills`): são os caminhos que as CLIs da máquina já
//! usam. O Argus apenas lê e rotula a origem; não copia nem regrava esses
//! arquivos.
//!
//! O parse do frontmatter é deliberadamente pequeno: escalares e listas
//! simples, que é o que esses arquivos usam. Campos que o Argus não conhece
//! são preservados na regravação, para não apagar o que outra ferramenta
//! escreveu.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Onde o arquivo mora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// `~/.claude/...`: vale em qualquer pasta.
    Global,
    /// `<projeto>/.claude/...`: versionado com o projeto.
    Project,
}

/// Marcadores do trecho do `AGENTS.md` que o Argus mantém. O que estiver fora
/// deles é do usuário e nunca é tocado.
pub const BLOCK_START: &str = "<!-- argus:inicio fluxo -->";
pub const BLOCK_END: &str = "<!-- argus:fim fluxo -->";

/// Um agente: o frontmatter mais o prompt (corpo do arquivo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDef {
    /// Slug; é também o nome do arquivo.
    pub name: String,
    /// Frase que diz à CLI quando acionar este agente. É o campo que mais pesa.
    pub description: String,
    /// `opus`, `sonnet`, `haiku`, `inherit`... None = o que a CLI decidir.
    #[serde(default)]
    pub model: Option<String>,
    /// Ferramentas permitidas; vazio = todas.
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub color: Option<String>,
    /// Prompt do agente (tudo depois do frontmatter).
    #[serde(default)]
    pub body: String,
    pub scope: Scope,
    /// Caminho do arquivo (vazio em um agente ainda não gravado).
    #[serde(default)]
    pub path: String,
    /// Existe um agente de mesmo nome no projeto, que tem prioridade sobre este.
    #[serde(default)]
    pub shadowed: bool,
    /// Campos do frontmatter que o Argus não conhece, preservados na gravação.
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
    /// O Argus não regrava este arquivo: YAML que ele não reproduz, ou `name:`
    /// diferente do nome do arquivo (gravar criaria outro arquivo).
    #[serde(default)]
    pub read_only: bool,
}

/// Uma skill: pasta com `SKILL.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDef {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub body: String,
    pub scope: Scope,
    /// De onde veio o arquivo: `Projeto`, `Claude`, `Agentes` ou `OpenCode`.
    /// É só rótulo para a interface; a gravação continua indo para `.claude`.
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub shadowed: bool,
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

// ------------------------------------------------------------------ caminhos

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

/// Raiz `.claude` do escopo. `project` só é usado no escopo do projeto.
pub fn claude_dir(scope: Scope, project: Option<&Path>) -> Result<PathBuf> {
    match scope {
        Scope::Global => Ok(home().context("Não foi possível descobrir a pasta do usuário.")?.join(".claude")),
        Scope::Project => Ok(project.context("Escopo do projeto exige um projeto aberto.")?.join(".claude")),
    }
}

pub fn agents_dir(scope: Scope, project: Option<&Path>) -> Result<PathBuf> {
    Ok(claude_dir(scope, project)?.join("agents"))
}

pub fn skills_dir(scope: Scope, project: Option<&Path>) -> Result<PathBuf> {
    Ok(claude_dir(scope, project)?.join("skills"))
}

/// Pastas globais de skills já instaladas na máquina. A ordem é a prioridade
/// quando dois arquivos têm o mesmo nome: Claude, Agentes, OpenCode (config
/// nova e a antiga). Nada é copiado: o Argus só lê o que a CLI também leria.
fn global_skill_dirs() -> Vec<(&'static str, PathBuf)> {
    let Some(home) = home() else { return Vec::new() };
    vec![
        ("Claude", home.join(".claude").join("skills")),
        ("Agentes", home.join(".agents").join("skills")),
        ("OpenCode", home.join(".config").join("opencode").join("skills")),
        ("OpenCode", home.join(".opencode").join("skills")),
    ]
}

/// Nome de arquivo seguro: minúsculas, números e hífen. Barra a travessia de
/// pasta (`..`, `/`) antes de qualquer caminho ser montado.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && !name.starts_with('-')
}

fn check_name(name: &str) -> Result<()> {
    if !valid_name(name) {
        bail!("Nome inválido: use só letras minúsculas, números e hífen (ex.: revisor-fiscal).");
    }
    Ok(())
}

/// Vira um slug utilizável a partir de um título digitado pelo usuário.
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'ã' | 'â' | 'ä' => 'a',
            'é' | 'ê' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'õ' | 'ô' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(64).collect()
}

// -------------------------------------------------------------- frontmatter

/// Frontmatter lido: pares chave/valor na ordem do arquivo, mais o corpo.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Front {
    pub fields: BTreeMap<String, String>,
    pub body: String,
    /// O frontmatter usa YAML que este leitor não reproduz (bloco `|`/`>`, mapa
    /// aninhado, lista em campo desconhecido): regravar perderia conteúdo.
    pub complex: bool,
}

/// Tira as aspas de um valor YAML simples.
fn unquote(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2 && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\''))) {
        return v[1..v.len() - 1].replace("\\\"", "\"").replace("''", "'");
    }
    v.to_string()
}

/// Lê o frontmatter YAML (`---` ... `---`) e devolve os campos e o corpo.
/// Arquivo sem frontmatter devolve campos vazios e o texto inteiro no corpo.
///
/// Aceita `chave: valor`, listas inline (`[a, b]`) e listas em linhas `- item`
/// (que viram `a, b`), que é o que esses arquivos usam na prática.
pub fn parse_front(text: &str) -> Front {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    let first = lines.next();
    if first.map(str::trim) != Some("---") {
        return Front { fields: BTreeMap::new(), body: text.trim_start_matches(['\r', '\n']).to_string(), complex: false };
    }
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut key: Option<String> = None;
    let mut items: Vec<String> = Vec::new();
    let mut rest = String::new();
    let mut closed = false;
    let mut complex = false;

    for line in lines {
        if !closed {
            if line.trim() == "---" {
                closed = true;
                if let Some(k) = key.take() {
                    fields.insert(k, items.join(", "));
                    items.clear();
                }
                continue;
            }
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let indented = line.starts_with([' ', '\t']);
            if let Some(item) = t.strip_prefix("- ") {
                // Item de uma lista aberta pela chave anterior. Lista só é
                // reproduzida em `tools`; em outro campo voltaria como texto.
                if key.is_some() {
                    complex |= key.as_deref() != Some("tools");
                    items.push(unquote(item));
                    continue;
                }
            }
            // Linha indentada que não é item de lista: mapa aninhado.
            if indented {
                complex = true;
                continue;
            }
            if let Some((k, v)) = t.split_once(':') {
                // Fecha a lista da chave anterior antes de começar outra.
                if let Some(prev) = key.take() {
                    fields.insert(prev, items.join(", "));
                    items.clear();
                }
                let k = k.trim().to_lowercase();
                let v = v.trim();
                if matches!(v, "|" | "|-" | "|+" | ">" | ">-" | ">+") {
                    complex = true;
                }
                if v.is_empty() {
                    key = Some(k);
                } else if let Some(inner) = v.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
                    let list: Vec<String> = inner.split(',').map(unquote).filter(|s| !s.is_empty()).collect();
                    fields.insert(k, list.join(", "));
                } else {
                    fields.insert(k, unquote(v));
                }
            }
            continue;
        }
        rest.push_str(line);
        rest.push('\n');
    }
    // Frontmatter sem fechamento: o arquivo está quebrado, trata como corpo.
    if !closed {
        return Front { fields: BTreeMap::new(), body: text.to_string(), complex: false };
    }
    if let Some(k) = key.take() {
        fields.insert(k, items.join(", "));
    }
    Front { fields, body: rest.trim_start_matches(['\r', '\n']).trim_end().to_string(), complex }
}

/// Valor YAML: entre aspas quando tiver algo que mudaria o sentido da linha.
fn yaml_value(v: &str) -> String {
    let needs = v.is_empty()
        || v.starts_with(['[', '{', '&', '*', '!', '|', '>', '%', '@', '`', '\'', '"', '-', '?', ' '])
        || v.ends_with(' ')
        || v.contains(": ")
        || v.contains('\n')
        || v.contains('#');
    if needs {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " "))
    } else {
        v.to_string()
    }
}

/// Monta o arquivo: frontmatter na ordem dada mais o corpo.
fn render(fields: &[(&str, String)], extra: &BTreeMap<String, String>, body: &str) -> String {
    let mut out = String::from("---\n");
    for (k, v) in fields {
        if v.is_empty() {
            continue;
        }
        out.push_str(&format!("{k}: {}\n", yaml_value(v)));
    }
    for (k, v) in extra {
        if fields.iter().any(|(fk, _)| fk == k) || v.is_empty() {
            continue;
        }
        out.push_str(&format!("{k}: {}\n", yaml_value(v)));
    }
    out.push_str("---\n\n");
    out.push_str(body.trim_end());
    out.push('\n');
    out
}

// ----------------------------------------------------------------- leitura

fn front_to_agent(front: Front, path: &Path, scope: Scope, fallback_name: &str) -> AgentDef {
    let complex = front.complex;
    let mut fields = front.fields;
    let take = |f: &mut BTreeMap<String, String>, k: &str| f.remove(k).filter(|s| !s.trim().is_empty());
    let name = take(&mut fields, "name").unwrap_or_else(|| fallback_name.to_string());
    let description = take(&mut fields, "description").unwrap_or_default();
    let model = take(&mut fields, "model");
    let color = take(&mut fields, "color");
    let tools = take(&mut fields, "tools")
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    let read_only = complex || name != fallback_name;
    AgentDef {
        name,
        description,
        model,
        tools,
        color,
        body: front.body,
        scope,
        path: path.to_string_lossy().to_string(),
        shadowed: false,
        read_only,
        extra: fields,
    }
}

fn read_agents_in(dir: &Path, scope: Scope) -> Vec<AgentDef> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()).map(|x| x.eq_ignore_ascii_case("md")) != Some(true) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let stem = path.file_stem().and_then(|x| x.to_str()).unwrap_or_default().to_string();
        out.push(front_to_agent(parse_front(&text), &path, scope, &stem));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Agentes dos dois escopos. O do projeto tem prioridade: o global de mesmo
/// nome volta marcado como `shadowed`.
pub fn list_agents(project: Option<&Path>) -> Vec<AgentDef> {
    let mut global = agents_dir(Scope::Global, None).map(|d| read_agents_in(&d, Scope::Global)).unwrap_or_default();
    let local = project
        .and_then(|p| agents_dir(Scope::Project, Some(p)).ok())
        .map(|d| read_agents_in(&d, Scope::Project))
        .unwrap_or_default();
    for g in &mut global {
        g.shadowed = local.iter().any(|l| l.name == g.name);
    }
    let mut out = local;
    out.extend(global);
    out.sort_by(|a, b| a.name.cmp(&b.name).then(b.scope.cmp_key().cmp(&a.scope.cmp_key())));
    out
}

impl Scope {
    fn cmp_key(self) -> u8 {
        match self {
            Scope::Project => 1,
            Scope::Global => 0,
        }
    }
}

fn read_skills_in(dir: &Path, scope: Scope, source: &str) -> Vec<SkillDef> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let path = e.path().join("SKILL.md");
        if !path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let stem = e.path().file_name().and_then(|x| x.to_str()).unwrap_or_default().to_string();
        let mut front = parse_front(&text);
        let name = front.fields.remove("name").filter(|s| !s.trim().is_empty()).unwrap_or(stem);
        let description = front.fields.remove("description").unwrap_or_default();
        out.push(SkillDef {
            name,
            description,
            body: front.body,
            scope,
            source: source.to_string(),
            path: path.to_string_lossy().to_string(),
            shadowed: false,
            extra: front.fields,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Junta skills de várias raízes. A primeira raiz que traz um nome vence; as
/// seguintes voltam marcadas como `shadowed` para a interface não duplicar o
/// menu `/`. `sort_by` é estável, então a ordem das raízes decide o empate.
pub fn merge_skills(roots: Vec<(PathBuf, Scope, String)>) -> Vec<SkillDef> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (dir, scope, source) in roots {
        for mut skill in read_skills_in(&dir, scope, &source) {
            if !seen.insert(skill.name.clone()) {
                skill.shadowed = true;
            }
            out.push(skill);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Skills do projeto e das CLIs instaladas na máquina. O do projeto tem
/// prioridade; depois vem Claude, Agentes e OpenCode, nessa ordem.
pub fn list_skills(project: Option<&Path>) -> Vec<SkillDef> {
    let mut roots: Vec<(PathBuf, Scope, String)> = Vec::new();
    if let Some(p) = project {
        if let Ok(dir) = skills_dir(Scope::Project, Some(p)) {
            roots.push((dir, Scope::Project, "Projeto".to_string()));
        }
    }
    for (source, dir) in global_skill_dirs() {
        roots.push((dir, Scope::Global, source.to_string()));
    }
    merge_skills(roots)
}

// ----------------------------------------------------------------- escrita

/// Grava sem deixar arquivo pela metade: escreve ao lado e renomeia por cima.
fn write_atomic(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("Não foi possível criar {}", dir.display()))?;
    }
    let tmp = path.with_extension("argus-tmp");
    std::fs::write(&tmp, text).with_context(|| format!("Não foi possível escrever {}", tmp.display()))?;
    // `rename` substitui o destino também no Windows (MoveFileExW com
    // REPLACE_EXISTING): apagar antes perderia o arquivo se o rename falhasse.
    std::fs::rename(&tmp, path).with_context(|| format!("Não foi possível gravar {}", path.display()))?;
    Ok(())
}

/// Grava o agente e devolve o caminho do arquivo. O corpo é o prompt; os
/// campos desconhecidos que vieram na leitura voltam para o arquivo.
pub fn save_agent(def: &AgentDef, project: Option<&Path>) -> Result<PathBuf> {
    if def.read_only {
        bail!("Este agente usa YAML que o Argus não regrava sem perder conteúdo (ou o nome difere do arquivo). Edite o arquivo direto: {}", def.path);
    }
    check_name(&def.name)?;
    if def.description.trim().is_empty() {
        bail!("A descrição é obrigatória: é ela que diz à CLI quando acionar o agente.");
    }
    let path = agents_dir(def.scope, project)?.join(format!("{}.md", def.name));
    let text = render(
        &[
            ("name", def.name.clone()),
            ("description", def.description.trim().to_string()),
            ("model", def.model.clone().unwrap_or_default()),
            ("tools", def.tools.join(", ")),
            ("color", def.color.clone().unwrap_or_default()),
        ],
        &def.extra,
        &def.body,
    );
    write_atomic(&path, &text)?;
    Ok(path)
}

/// Grava a skill em `<skills>/<nome>/SKILL.md`.
pub fn save_skill(def: &SkillDef, project: Option<&Path>) -> Result<PathBuf> {
    check_name(&def.name)?;
    if def.description.trim().is_empty() {
        bail!("A descrição é obrigatória: é ela que diz à CLI quando usar a skill.");
    }
    let path = skills_dir(def.scope, project)?.join(&def.name).join("SKILL.md");
    let text = render(
        &[("name", def.name.clone()), ("description", def.description.trim().to_string())],
        &def.extra,
        &def.body,
    );
    write_atomic(&path, &text)?;
    Ok(path)
}

pub fn delete_agent(name: &str, scope: Scope, project: Option<&Path>) -> Result<()> {
    check_name(name)?;
    let path = agents_dir(scope, project)?.join(format!("{name}.md"));
    if !path.exists() {
        bail!("Arquivo {} não encontrado: nada foi apagado.", path.display());
    }
    std::fs::remove_file(&path).with_context(|| format!("Não foi possível apagar {}", path.display()))?;
    Ok(())
}

pub fn delete_skill(name: &str, scope: Scope, project: Option<&Path>) -> Result<()> {
    check_name(name)?;
    let dir = skills_dir(scope, project)?.join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("Não foi possível apagar {}", dir.display()))?;
    }
    Ok(())
}

// --------------------------------------------------------------- AGENTS.md

/// Troca (ou acrescenta) o trecho entre os marcadores, preservando tudo que o
/// usuário escreveu fora deles. `body` vazio remove o trecho.
pub fn replace_block(existing: &str, body: &str) -> String {
    let block = if body.trim().is_empty() {
        String::new()
    } else {
        format!("{BLOCK_START}\n{}\n{BLOCK_END}\n", body.trim())
    };
    let (start, end) = (existing.find(BLOCK_START), existing.find(BLOCK_END));
    if let (Some(s), Some(e)) = (start, end) {
        if e > s {
            let before = &existing[..s];
            let after = &existing[e + BLOCK_END.len()..];
            let after = after.strip_prefix('\n').unwrap_or(after);
            let joined = format!("{before}{block}{after}");
            return joined.trim_end().to_string() + "\n";
        }
    }
    if block.is_empty() {
        return existing.to_string();
    }
    if existing.trim().is_empty() {
        return block;
    }
    format!("{}\n\n{block}", existing.trim_end())
}

/// Escreve o trecho do Argus no `AGENTS.md` do projeto.
pub fn sync_agents_md(project: &Path, body: &str) -> Result<PathBuf> {
    let path = project.join("AGENTS.md");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let next = replace_block(&existing, body);
    if next != existing {
        write_atomic(&path, &next)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_it_cannot_rewrite_is_flagged() {
        assert!(parse_front("---
name: a
description: |
  linha
---
x").complex);
        assert!(parse_front("---
name: a
hooks:
  pre: x
---
x").complex);
        assert!(parse_front("---
name: a
allow:
 - a
---
x").complex);
        assert!(!parse_front("---
name: a
tools:
 - Read
 - Grep
---
x").complex);
        let f = parse_front("---
name: outro
description: d
---
x");
        assert!(front_to_agent(f, Path::new("a.md"), Scope::Global, "a").read_only);
    }

    #[test]
    fn nomes() {
        assert!(valid_name("revisor-fiscal"));
        assert!(valid_name("agente_1"));
        assert!(!valid_name(""));
        assert!(!valid_name("../escapa"));
        assert!(!valid_name("Maiuscula"));
        assert!(!valid_name("-comeca-com-hifen"));
        assert_eq!(slugify("Revisor Fiscal (NF-e)"), "revisor-fiscal-nf-e");
        assert_eq!(slugify("  Backup Firebird  "), "backup-firebird");
    }

    #[test]
    fn le_frontmatter() {
        let f = parse_front("---\nname: revisor\ndescription: Revisa NF-e\nmodel: sonnet\ntools: Read, Grep\n---\n\nCorpo do prompt.\n");
        assert_eq!(f.fields.get("name").unwrap(), "revisor");
        assert_eq!(f.fields.get("description").unwrap(), "Revisa NF-e");
        assert_eq!(f.fields.get("tools").unwrap(), "Read, Grep");
        assert_eq!(f.body, "Corpo do prompt.");
    }

    #[test]
    fn le_lista_em_linhas_e_inline() {
        let f = parse_front("---\nname: x\ntools:\n  - Read\n  - Bash\nallow: [a, b]\n---\ncorpo\n");
        assert_eq!(f.fields.get("tools").unwrap(), "Read, Bash");
        assert_eq!(f.fields.get("allow").unwrap(), "a, b");
        assert_eq!(f.body, "corpo");
    }

    #[test]
    fn sem_frontmatter_vira_corpo() {
        let f = parse_front("# Só markdown\n\ntexto");
        assert!(f.fields.is_empty());
        assert_eq!(f.body, "# Só markdown\n\ntexto");
        // Frontmatter aberto e não fechado não pode comer o arquivo.
        let f = parse_front("---\nname: x\nsem fechamento");
        assert!(f.fields.is_empty());
    }

    #[test]
    fn ida_e_volta_preserva_campos_desconhecidos() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        let original = "---\nname: revisor\ndescription: Revisa NF-e\nmodel: sonnet\ntools: Read, Grep\ncampo-de-outra-cli: valor\n---\n\nPrompt.\n";
        let path = project.join(".claude/agents/revisor.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, original).unwrap();

        let list = read_agents_in(&project.join(".claude/agents"), Scope::Project);
        assert_eq!(list.len(), 1);
        let def = &list[0];
        assert_eq!(def.description, "Revisa NF-e");
        assert_eq!(def.model.as_deref(), Some("sonnet"));
        assert_eq!(def.tools, vec!["Read", "Grep"]);
        assert_eq!(def.extra.get("campo-de-outra-cli").unwrap(), "valor");

        save_agent(def, Some(project)).unwrap();
        let again = read_agents_in(&project.join(".claude/agents"), Scope::Project);
        assert_eq!(again[0].extra.get("campo-de-outra-cli").unwrap(), "valor");
        assert_eq!(again[0].body, "Prompt.");
        assert_eq!(again[0].tools, vec!["Read", "Grep"]);
    }

    #[test]
    fn descricao_vazia_nao_grava() {
        let dir = tempfile::tempdir().unwrap();
        let def = AgentDef {
            name: "x".into(),
            description: "  ".into(),
            model: None,
            tools: vec![],
            color: None,
            body: "corpo".into(),
            scope: Scope::Project,
            path: String::new(),
            shadowed: false,
            read_only: false,
            extra: BTreeMap::new(),
        };
        assert!(save_agent(&def, Some(dir.path())).is_err());
    }

    #[test]
    fn nome_com_travessia_nao_grava() {
        let dir = tempfile::tempdir().unwrap();
        let def = AgentDef {
            name: "../fora".into(),
            description: "x".into(),
            model: None,
            tools: vec![],
            color: None,
            body: String::new(),
            scope: Scope::Project,
            path: String::new(),
            shadowed: false,
            read_only: false,
            extra: BTreeMap::new(),
        };
        assert!(save_agent(&def, Some(dir.path())).is_err());
        assert!(delete_agent("../fora", Scope::Project, Some(dir.path())).is_err());
    }

    #[test]
    fn skill_vai_para_a_pasta_certa() {
        let dir = tempfile::tempdir().unwrap();
        let def = SkillDef {
            name: "backup-firebird".into(),
            description: "Backup do banco".into(),
            body: "## Quando usar".into(),
            scope: Scope::Project,
            source: "Projeto".into(),
            path: String::new(),
            shadowed: false,
            extra: BTreeMap::new(),
        };
        let path = save_skill(&def, Some(dir.path())).unwrap();
        assert!(path.ends_with("SKILL.md"));
        let list = read_skills_in(&dir.path().join(".claude/skills"), Scope::Project, "Projeto");
        assert_eq!(list[0].name, "backup-firebird");
        assert_eq!(list[0].description, "Backup do banco");

        delete_skill("backup-firebird", Scope::Project, Some(dir.path())).unwrap();
        assert!(read_skills_in(&dir.path().join(".claude/skills"), Scope::Project, "Projeto").is_empty());
    }

    fn escreve_skill(dir: &Path, name: &str, description: &str) {
        let path = dir.join(name).join("SKILL.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("---\nname: {name}\ndescription: {description}\n---\nCorpo.\n")).unwrap();
    }

    #[test]
    fn skills_de_varias_raizes_nao_duplicam_o_menu() {
        let dir = tempfile::tempdir().unwrap();
        let claude = dir.path().join("claude");
        let agents = dir.path().join("agents");
        escreve_skill(&claude, "comum", "Do Claude");
        escreve_skill(&claude, "so-claude", "Só no Claude");
        escreve_skill(&agents, "comum", "Dos Agentes");
        escreve_skill(&agents, "so-agentes", "Só nos Agentes");

        let list = merge_skills(vec![
            (claude, Scope::Global, "Claude".to_string()),
            (agents, Scope::Global, "Agentes".to_string()),
        ]);
        assert_eq!(list.len(), 4);
        let comum: Vec<_> = list.iter().filter(|s| s.name == "comum").collect();
        assert_eq!(comum.len(), 2);
        assert_eq!(comum[0].source, "Claude");
        assert!(!comum[0].shadowed);
        assert_eq!(comum[1].source, "Agentes");
        assert!(comum[1].shadowed);
        assert_eq!(list.iter().find(|s| s.name == "so-claude").unwrap().source, "Claude");
        assert_eq!(list.iter().find(|s| s.name == "so-agentes").unwrap().source, "Agentes");
    }

    #[test]
    fn bloco_do_agents_md_preserva_o_resto() {
        let meu = "# Projeto\n\nRegras minhas.\n";
        let um = replace_block(meu, "Fluxo A");
        assert!(um.contains("Regras minhas."));
        assert!(um.contains("Fluxo A"));

        let dois = replace_block(&um, "Fluxo B");
        assert!(dois.contains("Regras minhas."));
        assert!(dois.contains("Fluxo B"));
        assert!(!dois.contains("Fluxo A"));
        assert_eq!(dois.matches(BLOCK_START).count(), 1);

        // Trecho removido: o texto do usuário continua lá.
        let zero = replace_block(&dois, "");
        assert!(zero.contains("Regras minhas."));
        assert!(!zero.contains(BLOCK_START));
    }

    #[test]
    fn valor_com_dois_pontos_volta_inteiro() {
        let text = render(&[("description", "Use quando: a nota falhar".into())], &BTreeMap::new(), "corpo");
        let back = parse_front(&text);
        assert_eq!(back.fields.get("description").unwrap(), "Use quando: a nota falhar");
    }
}
