//! Comandos de shell liberados ao agente sem o usuário configurar nada.
//!
//! O claude roda com `--permission-mode acceptEdits`: edições de arquivo passam,
//! mas todo comando de shell é negado, a não ser o que for listado em
//! `--allowedTools`. Sem um conjunto padrão, nem `git status` funcionava. Aqui
//! ficam: o conjunto padrão (git somente leitura e inspeção básica), os dois
//! níveis de git que alteram o repositório ("Git local" e "Git remoto", cada um
//! com liga/desliga) e a montagem dos argumentos, somando a lista "Comandos
//! permitidos ao agente" do projeto.
//!
//! Sintaxe conferida no claude 2.1.220 (`claude --help`: `--allowedTools
//! "Bash(git *)"`): `Ferramenta(prefixo:*)` libera o comando e os argumentos;
//! `Ferramenta(comando)` libera só o comando exato. No Windows a CLI tem também a
//! ferramenta `PowerShell` (vista na lista `tools` da inicialização do
//! stream-json): os mesmos padrões valem para ela.

use crate::agents::orchestrator::WorkMode;
use crate::agents::provider::CommandSpec;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Git que só lê. `branch` e `remote` entram só nas formas que não criam nem apagam.
pub const GIT_READ: &[&str] = &[
    "git status:*",
    "git diff:*",
    "git log:*",
    "git show:*",
    "git ls-files:*",
    "git rev-parse:*",
    "git blame:*",
    "git grep:*",
    "git branch",
    "git branch --list:*",
    "git branch --show-current",
    "git branch -a",
    "git branch -r",
    "git branch -v",
    "git branch -vv",
    "git remote",
    "git remote -v",
    "git stash list:*",
];

/// Inspeção básica, que não altera nada.
/// `cd` entra porque o agente costuma escrever `cd <pasta>; git status`: sem ele o
/// comando inteiro era negado (visto numa tarefa real). O que vem depois do `cd`
/// continua precisando estar liberado.
pub const INSPECT: &[&str] = &["cd:*", "ls:*", "dir:*", "pwd", "cat:*", "type:*", "head:*", "tail:*", "wc:*", "which:*", "where:*"];

/// As formas do PowerShell para a mesma inspeção (só na ferramenta `PowerShell`).
pub const INSPECT_POWERSHELL: &[&str] = &["Set-Location:*", "Get-ChildItem:*", "Get-Content:*", "Get-Location", "Get-Command:*", "Test-Path:*"];

/// Nível "Git local": altera o repositório, sem falar com servidor nenhum.
pub const GIT_LOCAL: &[&str] = &[
    "git add:*",
    "git commit:*",
    "git restore:*",
    "git checkout:*",
    "git switch:*",
    "git branch:*",
    "git merge:*",
    "git rebase:*",
    "git stash:*",
    "git tag:*",
];

/// Nível "Git remoto": fala com o servidor.
pub const GIT_REMOTE: &[&str] = &["git push:*", "git pull:*", "git fetch:*", "git clone:*"];

/// Com o "Git remoto" ligado, o push forçado continua negado.
pub const FORCE_PUSH: &[&str] = &["git push --force:*", "git push -f:*", "git push --force-with-lease:*", "git push * --force*", "git push * -f"];

/// Nunca entram no padrão (só pela lista do projeto, por decisão do usuário).
pub const NEVER_BY_DEFAULT: &[&str] = &["git push --force", "git reset --hard", "git clean", "shell genérico (qualquer comando)"];

/// Os dois níveis de git que alteram o repositório.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitLevels {
    pub local: bool,
    pub remote: bool,
}

impl GitLevels {
    /// Padrão sem nenhuma preferência gravada: "Git local" ligado na cópia isolada
    /// (o usuário revisa antes de aplicar) e desligado direto na pasta; "Git
    /// remoto" desligado nos dois.
    pub fn defaults(mode: WorkMode) -> Self {
        Self { local: mode == WorkMode::Isolated, remote: false }
    }
}

/// O conjunto padrão, para a interface mostrar o que está liberado.
#[derive(Debug, Clone, Serialize)]
pub struct DefaultSet {
    pub git_read: Vec<String>,
    pub inspect: Vec<String>,
    pub inspect_powershell: Vec<String>,
    pub git_local: Vec<String>,
    pub git_remote: Vec<String>,
    pub never: Vec<String>,
}

pub fn default_set() -> DefaultSet {
    let v = |l: &[&str]| l.iter().map(|s| s.to_string()).collect();
    DefaultSet {
        git_read: v(GIT_READ),
        inspect: v(INSPECT),
        inspect_powershell: v(INSPECT_POWERSHELL),
        git_local: v(GIT_LOCAL),
        git_remote: v(GIT_REMOTE),
        never: v(NEVER_BY_DEFAULT),
    }
}

/// Padrões de shell liberados para os níveis dados (sem a lista do projeto).
fn shell_patterns(levels: GitLevels) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = GIT_READ.iter().chain(INSPECT).copied().collect();
    if levels.local {
        v.extend(GIT_LOCAL);
    }
    if levels.remote {
        v.extend(GIT_REMOTE);
    }
    v
}

/// Entradas de `--allowedTools`: o padrão dos níveis (para `Bash` e `PowerShell`)
/// mais a lista do projeto. Entradas do projeto já no formato `Ferramenta(...)`
/// passam direto; o resto vira `Bash(<cmd>)`.
pub fn allowed_tools(project_cmds: &[String], levels: GitLevels) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if !out.contains(&s) {
            out.push(s);
        }
    };
    for tool in ["Bash", "PowerShell"] {
        for p in shell_patterns(levels) {
            push(format!("{tool}({p})"));
        }
    }
    for p in INSPECT_POWERSHELL {
        push(format!("PowerShell({p})"));
    }
    for c in project_cmds.iter().map(|c| c.trim()).filter(|c| !c.is_empty()) {
        push(if c.ends_with(')') && c.contains('(') { c.to_string() } else { format!("Bash({c})") });
    }
    out
}

/// Entradas de `--disallowedTools`: com o "Git remoto" ligado, o push forçado
/// segue negado, a não ser que a lista do projeto o libere de propósito.
pub fn disallowed_tools(project_cmds: &[String], levels: GitLevels) -> Vec<String> {
    let user_forces = project_cmds.iter().any(|c| c.contains("--force") || c.contains("push -f"));
    if !levels.remote || user_forces {
        return Vec::new();
    }
    ["Bash", "PowerShell"].iter().flat_map(|t| FORCE_PUSH.iter().map(move |p| format!("{t}({p})"))).collect()
}

/// Argumentos de permissão para a linha de comando do claude.
pub fn args(project_cmds: &[String], levels: GitLevels) -> Vec<String> {
    let mut v = vec!["--allowedTools".to_string()];
    v.extend(allowed_tools(project_cmds, levels));
    let deny = disallowed_tools(project_cmds, levels);
    if !deny.is_empty() {
        v.push("--disallowedTools".to_string());
        v.extend(deny);
    }
    v
}

/// Acrescenta as permissões (padrão + níveis + lista do projeto) a um comando do
/// `claude`. Outros programas ficam como estão.
pub fn apply(mut spec: CommandSpec, project: &Path, levels: GitLevels) -> CommandSpec {
    let is_claude = Path::new(&spec.program).file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("claude"));
    if is_claude {
        spec.args.extend(args(&crate::procs::allowed_commands(project), levels));
    }
    spec
}

/// Padrão que libera exatamente o comando negado (para o atalho "Liberar este
/// comando neste projeto"). None quando a ferramenta não é de shell.
pub fn pattern_for_denied(tool: &str, command: &str) -> Option<String> {
    let cmd = command.trim();
    if cmd.is_empty() || cmd.contains('\n') {
        return None;
    }
    match tool {
        "Bash" => Some(cmd.to_string()),
        "PowerShell" => Some(format!("PowerShell({cmd})")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(v: &[String], s: &str) -> bool {
        v.iter().any(|x| x == s)
    }

    #[test]
    fn defaults_by_mode() {
        assert_eq!(GitLevels::defaults(WorkMode::Isolated), GitLevels { local: true, remote: false });
        assert_eq!(GitLevels::defaults(WorkMode::Direct), GitLevels { local: false, remote: false });
    }

    /// Sem nível nenhum (o padrão de "Direto na pasta"): leitura e inspeção, nada que altere.
    #[test]
    fn read_only_set() {
        let t = allowed_tools(&[], GitLevels { local: false, remote: false });
        for ok in ["Bash(git status:*)", "Bash(git log:*)", "Bash(git diff:*)", "Bash(git branch)", "Bash(git remote -v)", "Bash(git stash list:*)", "Bash(ls:*)", "Bash(pwd)", "PowerShell(git status:*)", "PowerShell(Get-ChildItem:*)"] {
            assert!(has(&t, ok), "{ok}");
        }
        for no in ["git add", "git commit", "git push", "git pull", "git reset", "git clean", "git branch:*", "git stash:*", "Bash(*)", "Bash(git:*)", "Bash(git *)"] {
            assert!(!t.iter().any(|x| x.contains(no)), "{no} não devia estar liberado: {t:?}");
        }
        assert!(!t.iter().any(|x| x == "Bash" || x == "PowerShell"), "nunca o shell genérico");
        assert!(disallowed_tools(&[], GitLevels { local: false, remote: false }).is_empty());
    }

    /// "Git local" (padrão da cópia isolada): altera o repositório, sem rede.
    #[test]
    fn local_level() {
        let t = allowed_tools(&[], GitLevels::defaults(WorkMode::Isolated));
        for ok in ["Bash(git add:*)", "Bash(git commit:*)", "Bash(git checkout:*)", "Bash(git stash:*)", "Bash(git tag:*)", "PowerShell(git commit:*)"] {
            assert!(has(&t, ok), "{ok}");
        }
        for no in ["git push", "git pull", "git fetch", "git clone", "git reset", "git clean"] {
            assert!(!t.iter().any(|x| x.contains(no)), "{no}");
        }
    }

    /// "Git remoto": push/pull/fetch/clone, com o push forçado negado.
    #[test]
    fn remote_level_keeps_force_push_denied() {
        let lv = GitLevels { local: false, remote: true };
        let t = allowed_tools(&[], lv);
        assert!(has(&t, "Bash(git push:*)") && has(&t, "Bash(git fetch:*)") && has(&t, "PowerShell(git pull:*)"));
        assert!(!t.iter().any(|x| x.contains("git commit")));
        let d = disallowed_tools(&[], lv);
        assert!(has(&d, "Bash(git push --force:*)") && has(&d, "Bash(git push -f:*)") && has(&d, "PowerShell(git push --force:*)"));
        let a = args(&[], lv);
        assert_eq!(a[0], "--allowedTools");
        let cut = a.iter().position(|x| x == "--disallowedTools").unwrap();
        assert!(a[..cut].iter().any(|x| x == "Bash(git push:*)") && a[cut..].iter().any(|x| x == "Bash(git push --force:*)"));
        // Quem liberou o push forçado na lista do projeto, de propósito, não é contrariado.
        assert!(disallowed_tools(&["git push --force:*".into()], lv).is_empty());
    }

    /// A lista do projeto soma ao padrão, sem repetir.
    #[test]
    fn project_list_is_added() {
        let cmds = vec!["npm run dev:*".to_string(), " ".to_string(), "Read(//c/dados/**)".to_string(), "git status:*".to_string()];
        let t = allowed_tools(&cmds, GitLevels { local: false, remote: false });
        assert!(has(&t, "Bash(npm run dev:*)") && has(&t, "Read(//c/dados/**)"));
        assert_eq!(t.iter().filter(|x| *x == "Bash(git status:*)").count(), 1);
        assert!(!args(&cmds, GitLevels { local: false, remote: false }).contains(&"--disallowedTools".to_string()));
    }

    #[test]
    fn only_claude_gets_the_arguments() {
        let dir = Path::new("projeto-que-nao-existe");
        let lv = GitLevels::defaults(WorkMode::Direct);
        let c = apply(CommandSpec::new("claude").arg("-p"), dir, lv);
        assert_eq!(c.args[1], "--allowedTools");
        assert!(c.args.contains(&"Bash(git status:*)".to_string()));
        assert_eq!(apply(CommandSpec::new("opencode").arg("run"), dir, lv).args, ["run"]);
    }

    #[test]
    fn pattern_for_a_denied_command() {
        assert_eq!(pattern_for_denied("Bash", " git push origin main ").as_deref(), Some("git push origin main"));
        assert_eq!(pattern_for_denied("PowerShell", "npm test").as_deref(), Some("PowerShell(npm test)"));
        assert_eq!(pattern_for_denied("Read", "x"), None);
        assert_eq!(pattern_for_denied("Bash", "a\nb"), None);
    }
}
