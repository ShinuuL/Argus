//! Comandos de shell liberados ao agente sem o usuário configurar nada.
//!
//! O claude roda com `--permission-mode acceptEdits`: edições de arquivo passam,
//! mas todo comando de shell é negado, a não ser o que for listado em
//! `--allowedTools`. Sem um conjunto padrão, nem `git status` funcionava. Aqui
//! ficam: o conjunto padrão (git somente leitura e inspeção básica), os níveis
//! com liga/desliga ("Git local", "Git remoto" e "Compilar e testar") e a
//! montagem dos argumentos, somando a lista "Comandos permitidos ao agente" do
//! projeto.
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
/// Procurar e ler também são inspeção: sem `grep`/`rg`/`find` o agente não
/// consegue nem procurar um texto no projeto (visto numa tarefa real, com o
/// comando negado no meio do trabalho). Só entram formas que leem; `sed` e
/// `awk` ficam de fora porque escrevem arquivo (`sed -i`).
pub const INSPECT: &[&str] = &[
    "cd:*", "ls:*", "dir:*", "pwd", "cat:*", "type:*", "head:*", "tail:*", "wc:*", "which:*", "where:*", "grep:*", "rg:*",
    // `findstr` (Windows) é busca de texto; o `find` do unix fica de fora, porque
    // apaga e executa (`-delete`, `-exec`).
    "findstr:*", "sort:*", "uniq:*", "cut:*", "tr:*", "echo:*", "basename:*", "dirname:*", "stat:*", "file:*",
    "diff:*", "tree:*", "du:*", "df:*", "env", "date",
];

/// As formas do PowerShell para a mesma inspeção (só na ferramenta `PowerShell`).
pub const INSPECT_POWERSHELL: &[&str] = &[
    "Set-Location:*", "Get-ChildItem:*", "Get-Content:*", "Get-Location", "Get-Command:*", "Test-Path:*", "Select-String:*",
    "Select-Object:*", "Measure-Object:*", "Where-Object:*", "Sort-Object:*", "Resolve-Path:*", "Get-Item:*", "Get-Date",
];

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

/// Nível "Compilar e testar": o que o agente roda para conferir o próprio
/// trabalho (build, testes, tipos, formatação). Nada que instale dependência,
/// publique pacote ou fale com servidor de produção — isso continua de fora.
/// O `pnpm`/`yarn`/`bun` aceitam o nome do script direto (`pnpm build` em vez de
/// `pnpm run build`), e é assim que o agente escreve. Sem estes, o comando de
/// build do projeto continuava negado.
pub const BUILD: &[&str] = &[
    "npm run:*",
    "npm test:*",
    "npm exec:*",
    "pnpm build:*",
    "pnpm dev:*",
    "pnpm test:*",
    "pnpm lint:*",
    "pnpm typecheck:*",
    "pnpm check:*",
    "pnpm format:*",
    "pnpm preview:*",
    "pnpm -r:*",
    "yarn build:*",
    "yarn dev:*",
    "yarn lint:*",
    "yarn typecheck:*",
    "corepack pnpm:*",
    "corepack yarn:*",
    "corepack enable:*",
    "corepack --version",
    "bun run:*",
    "bun test:*",
    "deno task:*",
    "deno test:*",
    // O agente roda script do próprio projeto (teste, gerador, verificação).
    // Não é um poder novo: `npm run` já executa o que estiver no package.json.
    "node:*",
    "tsx:*",
    "vitest:*",
    "jest:*",
    "eslint:*",
    "prettier:*",
    "npx tsc:*",
    "npx vitest:*",
    "npx jest:*",
    "npx eslint:*",
    "npx prettier:*",
    "pnpm run:*",
    "pnpm test:*",
    "yarn run:*",
    "yarn test:*",
    "cargo build:*",
    "cargo check:*",
    "cargo test:*",
    "cargo clippy:*",
    "cargo fmt:*",
    "go build:*",
    "go test:*",
    "go vet:*",
    "dotnet build:*",
    "dotnet test:*",
    "mvn -B test:*",
    "mvn -B package:*",
    "gradle build:*",
    "gradlew build:*",
    "./gradlew build:*",
    "make:*",
    "pytest:*",
    "python -m pytest:*",
    "python -m unittest:*",
    "ruff check:*",
    "tsc:*",
];

/// Nunca entram no padrão (só pela lista do projeto, por decisão do usuário).
pub const NEVER_BY_DEFAULT: &[&str] = &["git push --force", "git reset --hard", "git clean", "shell genérico (qualquer comando)"];

/// Os níveis que o usuário liga e desliga: os dois de git que alteram o
/// repositório e o de compilar e testar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitLevels {
    pub local: bool,
    pub remote: bool,
    /// Build, testes e checagem de tipos (`BUILD`).
    pub build: bool,
}

impl GitLevels {
    /// Padrão sem nenhuma preferência gravada: "Git local" ligado na cópia isolada
    /// (o usuário revisa antes de aplicar) e desligado direto na pasta; "Git
    /// remoto" desligado nos dois; "Compilar e testar" ligado, para o agente
    /// conferir o próprio trabalho sem precisar pedir.
    pub fn defaults(mode: WorkMode) -> Self {
        Self { local: mode == WorkMode::Isolated, remote: false, build: true }
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
    pub build: Vec<String>,
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
        build: v(BUILD),
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
    if levels.build {
        v.extend(BUILD);
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

/// Política do opencode no modo Restrito: edição liberada, shell só nos padrões
/// liberados (os mesmos do claude) e o resto negado. Sem regra "ask": num `run`
/// não há quem aprove (e o serviço em segundo plano aprovava sozinho).
pub fn opencode_restricted_config(project_cmds: &[String], levels: GitLevels) -> String {
    let mut bash = serde_json::Map::new();
    bash.insert("*".into(), "deny".into());
    let mut allow = |p: &str| {
        let p = p.trim();
        if p.is_empty() {
            return;
        }
        match p.strip_suffix(":*") {
            Some(base) => {
                bash.insert(base.to_string(), "allow".into());
                bash.insert(format!("{base} *"), "allow".into());
            }
            None => {
                bash.insert(p.to_string(), "allow".into());
            }
        }
    };
    for p in shell_patterns(levels).into_iter().chain(INSPECT_POWERSHELL.iter().copied()) {
        allow(p);
    }
    for c in project_cmds {
        // `Bash(npm test:*)` e afins: só o que está dentro dos parênteses.
        let c = c.trim();
        let inner = c.find('(').filter(|_| c.ends_with(')')).map(|i| &c[i + 1..c.len() - 1]).unwrap_or(c);
        allow(inner);
    }
    if levels.remote {
        for p in FORCE_PUSH {
            bash.insert(p.trim_end_matches(":*").to_string() + "*", "deny".into());
        }
    }
    serde_json::json!({ "permission": { "edit": "allow", "bash": bash, "external_directory": "deny" } }).to_string()
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

/// Modo de permissão dos agentes. "Auto" (padrão): o agente roda comandos sem
/// pedir e só os nocivos (`crate::harmful`) ficam bloqueados. "Restrito": o
/// comportamento anterior (no claude, só edições e a lista liberada).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionMode {
    #[default]
    Auto,
    Restricted,
}

impl PermissionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Restricted => "restricted",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(Self::Auto),
            "restricted" => Some(Self::Restricted),
            _ => None,
        }
    }
}

/// Modo de acesso de uma conversa (por tarefa, escolhido no composer).
/// - Planejar: o agente só lê e propõe; nada é alterado.
/// - Normal: vale o modo de permissão (Auto/Restrito) e a política de shell.
/// - Bypass: o agente roda sem pedir aprovação e sem a política do Argus. Nunca é
///   o padrão; vale só para a conversa onde foi ligado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessMode {
    Plan,
    #[default]
    Normal,
    Bypass,
}

impl AccessMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Normal => "normal",
            Self::Bypass => "bypass",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "plan" => Some(Self::Plan),
            "normal" => Some(Self::Normal),
            "bypass" => Some(Self::Bypass),
            _ => None,
        }
    }
}

/// Configuração do opencode que libera tudo (modo Bypass da conversa).
pub const OPENCODE_BYPASS_CONFIG: &str = r#"{"permission":"allow"}"#;

fn program_is(spec: &CommandSpec, name: &str) -> bool {
    Path::new(&spec.program).file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(name))
}

fn replace_after(args: &mut [String], flag: &str, value: &str) {
    if let Some(i) = args.iter().position(|a| a == flag) {
        if let Some(v) = args.get_mut(i + 1) {
            *v = value.to_string();
        }
    }
}

/// Arquivo de política do gemini para o modo Auto (conteúdo fixo, regravado a
/// cada turno numa pasta temporária do Argus; nunca no `~/.gemini`).
fn gemini_policy_file() -> Option<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("argus");
    std::fs::create_dir_all(&dir).ok()?;
    let f = dir.join("gemini-auto-policy.toml");
    std::fs::write(&f, crate::harmful::gemini_policy_toml()).ok()?;
    Some(f)
}

/// Acrescenta as permissões a um comando de agente, conforme o modo:
/// - claude: Restrito = `acceptEdits` + `--allowedTools` (padrão + níveis + lista
///   do projeto); Auto = `--permission-mode auto` (classificador da CLI), a mesma
///   lista liberada e os nocivos em `--disallowedTools`;
/// - codex (Auto): rede liberada dentro do sandbox `workspace-write`;
/// - gemini (Auto): `--approval-mode yolo` com a política de nocivos (`--admin-policy`);
/// - opencode (Auto): `OPENCODE_CONFIG_CONTENT` com shell liberado e nocivos negados.
pub fn apply(mut spec: CommandSpec, project: &Path, levels: GitLevels, mode: PermissionMode) -> CommandSpec {
    let auto = mode == PermissionMode::Auto;
    if program_is(&spec, "claude") {
        let cmds = crate::procs::allowed_commands(project);
        spec.args.extend(args(&cmds, levels));
        if auto {
            // O classificador do modo `auto` não atende o Haiku: com ele a CLI
            // cai em "pedir permissão" e, em `-p`, nega até o Edit (visto no
            // claude 2.1.220). Para o Haiku fica `acceptEdits` com o shell
            // liberado e só a lista de nocivos negada.
            let haiku = spec.args.windows(2).any(|w| w[0] == "--model" && w[1].to_lowercase().contains("haiku"));
            if haiku {
                if let Some(i) = spec.args.iter().position(|a| a == "--allowedTools") {
                    spec.args.insert(i + 1, "PowerShell".into());
                    spec.args.insert(i + 1, "Bash".into());
                }
            } else {
                replace_after(&mut spec.args, "--permission-mode", "auto");
            }
            let mut deny = crate::harmful::claude_disallowed();
            // Quem liberou algo de propósito na lista do projeto não é contrariado.
            deny.retain(|d| !cmds.iter().any(|c| d.contains(c.trim_end_matches(":*").trim_end_matches('*'))));
            if !spec.args.iter().any(|a| a == "--disallowedTools") {
                spec.args.push("--disallowedTools".into());
            }
            spec.args.extend(deny);
        }
    } else if auto && program_is(&spec, "codex") {
        let at = spec.args.iter().position(|a| a == "approval_policy=never").map(|i| i + 1).unwrap_or(spec.args.len().min(1));
        spec.args.insert(at, "sandbox_workspace_write.network_access=true".into());
        spec.args.insert(at, "-c".into());
    } else if auto && program_is(&spec, "gemini") {
        if let Some(f) = gemini_policy_file() {
            replace_after(&mut spec.args, "--approval-mode", "yolo");
            spec.args.push("--admin-policy".into());
            spec.args.push(f.to_string_lossy().into_owned());
        }
    } else if program_is(&spec, "opencode") {
        let cfg = if auto {
            crate::harmful::opencode_config_json()
        } else {
            opencode_restricted_config(&crate::procs::allowed_commands(project), levels)
        };
        spec.env.push(("OPENCODE_CONFIG_CONTENT".into(), cfg));
    }
    spec
}

/// Como `apply`, considerando o modo de acesso da conversa: em Planejar e em
/// Bypass os argumentos já vêm do provedor (`TurnOptions::access`) e a política
/// de shell do Argus não entra (Planejar não altera nada; Bypass a desliga).
pub fn apply_for(spec: CommandSpec, project: &Path, levels: GitLevels, mode: PermissionMode, access: AccessMode) -> CommandSpec {
    match access {
        AccessMode::Normal => apply(spec, project, levels, mode),
        AccessMode::Plan | AccessMode::Bypass => spec,
    }
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

    #[test]
    fn access_mode_parse_and_skip_policy() {
        for m in [AccessMode::Plan, AccessMode::Normal, AccessMode::Bypass] {
            assert_eq!(AccessMode::parse(m.as_str()), Some(m));
        }
        assert_eq!(AccessMode::default(), AccessMode::Normal);
        assert_eq!(AccessMode::parse("yolo"), None);
        let spec = CommandSpec::new("claude").arg("-p").arg("--permission-mode").arg("bypassPermissions");
        let lv = GitLevels::defaults(WorkMode::Isolated);
        let out = apply_for(spec.clone(), Path::new("."), lv, PermissionMode::Auto, AccessMode::Bypass);
        assert_eq!(out.args, spec.args, "Bypass não recebe a política do Argus");
        let out = apply_for(spec.clone(), Path::new("."), lv, PermissionMode::Auto, AccessMode::Normal);
        assert!(out.args.iter().any(|a| a == "--allowedTools"));
    }

    fn has(v: &[String], s: &str) -> bool {
        v.iter().any(|x| x == s)
    }

    #[test]
    fn defaults_by_mode() {
        assert_eq!(GitLevels::defaults(WorkMode::Isolated), GitLevels { local: true, remote: false, build: true });
        assert_eq!(GitLevels::defaults(WorkMode::Direct), GitLevels { local: false, remote: false, build: true });
    }

    /// Sem nível nenhum (o padrão de "Direto na pasta"): leitura e inspeção, nada que altere.
    #[test]
    fn read_only_set() {
        let t = allowed_tools(&[], GitLevels { local: false, remote: false, build: true });
        for ok in [
            "Bash(git status:*)",
            "Bash(git log:*)",
            "Bash(git diff:*)",
            "Bash(git branch)",
            "Bash(git remote -v)",
            "Bash(git stash list:*)",
            "Bash(ls:*)",
            "Bash(pwd)",
            // Procurar é inspeção: sem isto o agente não acha texto no projeto.
            "Bash(grep:*)",
            "Bash(rg:*)",
            "PowerShell(git status:*)",
            "PowerShell(Get-ChildItem:*)",
            "PowerShell(Select-String:*)",
        ] {
            assert!(has(&t, ok), "{ok}");
        }
        for no in ["git add", "git commit", "git push", "git pull", "git reset", "git clean", "git branch:*", "git stash:*", "Bash(*)", "Bash(git:*)", "Bash(git *)"] {
            assert!(!t.iter().any(|x| x.contains(no)), "{no} não devia estar liberado: {t:?}");
        }
        assert!(!t.iter().any(|x| x == "Bash" || x == "PowerShell"), "nunca o shell genérico");
        assert!(disallowed_tools(&[], GitLevels { local: false, remote: false, build: true }).is_empty());
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
        let lv = GitLevels { local: false, remote: true, build: true };
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

    /// "Compilar e testar" (ligado por padrão): build, testes e tipos passam;
    /// instalar dependência e publicar, não.
    #[test]
    fn build_level() {
        let on = allowed_tools(&[], GitLevels { local: false, remote: false, build: true });
        for ok in [
            "Bash(npm run:*)",
            "Bash(cargo test:*)",
            "Bash(npx tsc:*)",
            "Bash(pytest:*)",
            "PowerShell(npm run:*)",
            // Atalho do pnpm/yarn (o jeito que o agente escreve) e o node de script.
            "Bash(pnpm build:*)",
            "Bash(pnpm test:*)",
            "Bash(yarn build:*)",
            "Bash(corepack pnpm:*)",
            "Bash(node:*)",
            "PowerShell(pnpm build:*)",
        ] {
            assert!(has(&on, ok), "{ok}");
        }
        for no in ["npm install", "npm publish", "cargo publish", "pip install", "pnpm install", "pnpm add"] {
            assert!(!on.iter().any(|x| x.contains(no)), "{no} não devia entrar: {on:?}");
        }
        let off = allowed_tools(&[], GitLevels { local: false, remote: false, build: false });
        assert!(!off.iter().any(|x| x.contains("npm run") || x.contains("cargo test")));
        // O padrão de qualquer modo de trabalho já vem com o nível ligado.
        assert!(GitLevels::defaults(WorkMode::Direct).build && GitLevels::defaults(WorkMode::Isolated).build);
    }

    /// A lista do projeto soma ao padrão, sem repetir.
    #[test]
    fn project_list_is_added() {
        let cmds = vec!["npm run dev:*".to_string(), " ".to_string(), "Read(//c/dados/**)".to_string(), "git status:*".to_string()];
        let t = allowed_tools(&cmds, GitLevels { local: false, remote: false, build: true });
        assert!(has(&t, "Bash(npm run dev:*)") && has(&t, "Read(//c/dados/**)"));
        assert_eq!(t.iter().filter(|x| *x == "Bash(git status:*)").count(), 1);
        assert!(!args(&cmds, GitLevels { local: false, remote: false, build: true }).contains(&"--disallowedTools".to_string()));
    }

    #[test]
    fn only_claude_gets_the_arguments() {
        let dir = Path::new("projeto-que-nao-existe");
        let lv = GitLevels::defaults(WorkMode::Direct);
        let r = PermissionMode::Restricted;
        let c = apply(CommandSpec::new("claude").arg("-p"), dir, lv, r);
        assert_eq!(c.args[1], "--allowedTools");
        assert!(c.args.contains(&"Bash(git status:*)".to_string()));
        assert_eq!(apply(CommandSpec::new("opencode").arg("run"), dir, lv, r).args, ["run"]);
        // Restrito: shell negado por padrão, liberados os padrões de leitura.
        let env = apply(CommandSpec::new("opencode").arg("run"), dir, lv, r).env;
        let cfg: serde_json::Value = serde_json::from_str(&env[0].1).unwrap();
        assert_eq!(cfg["permission"]["bash"]["*"], "deny");
        assert_eq!(cfg["permission"]["bash"]["git status *"], "allow");
        assert_eq!(cfg["permission"]["edit"], "allow");
        let g = CommandSpec::new("gemini").arg("--approval-mode").arg("auto_edit");
        assert_eq!(apply(g, dir, lv, r).args, ["--approval-mode", "auto_edit"]);
    }

    fn sp(p: &str, a: &[&str]) -> CommandSpec {
        let mut s = CommandSpec::new(p);
        s.args = a.iter().map(|x| x.to_string()).collect();
        s
    }

    #[test]
    fn auto_mode_per_cli() {
        let dir = Path::new("projeto-que-nao-existe");
        let lv = GitLevels::defaults(WorkMode::Direct);
        let a = PermissionMode::Auto;
        let c = apply(CommandSpec::new("claude").arg("-p").arg("--permission-mode").arg("acceptEdits"), dir, lv, a);
        assert_eq!(c.args[2], "auto");
        assert!(!c.args.iter().any(|x| x.contains("bypass") || x.contains("dangerously")));
        let cut = c.args.iter().position(|x| x == "--disallowedTools").unwrap();
        assert!(c.args[cut..].iter().any(|x| x == "Bash(git reset --hard:*)"));
        assert!(c.args[..cut].iter().any(|x| x == "Bash(git status:*)"));
        // Haiku: o classificador do `auto` não o atende; fica acceptEdits + shell liberado.
        let h = apply(CommandSpec::new("claude").arg("-p").arg("--model").arg("haiku").arg("--permission-mode").arg("acceptEdits"), dir, lv, a);
        assert_eq!(h.args[4], "acceptEdits");
        let i = h.args.iter().position(|x| x == "--allowedTools").unwrap();
        assert_eq!(&h.args[i + 1..i + 3], ["Bash", "PowerShell"]);
        assert!(h.args.iter().any(|x| x == "Bash(git reset --hard:*)"));
        let x = apply(sp("codex", &["exec", "--sandbox", "workspace-write", "-c", "approval_policy=never", "-"]), dir, lv, a);
        assert_eq!(x.args, ["exec", "--sandbox", "workspace-write", "-c", "approval_policy=never", "-c", "sandbox_workspace_write.network_access=true", "-"]);
        let g = apply(sp("gemini", &["--approval-mode", "auto_edit"]), dir, lv, a);
        assert_eq!(&g.args[..3], ["--approval-mode", "yolo", "--admin-policy"]);
        assert!(std::fs::read_to_string(&g.args[3]).unwrap().contains("decision = \"deny\""));
        let o = apply(CommandSpec::new("opencode").arg("run"), dir, lv, a);
        assert_eq!(o.env[0].0, "OPENCODE_CONFIG_CONTENT");
    }

    #[test]
    fn pattern_for_a_denied_command() {
        assert_eq!(pattern_for_denied("Bash", " git push origin main ").as_deref(), Some("git push origin main"));
        assert_eq!(pattern_for_denied("PowerShell", "npm test").as_deref(), Some("PowerShell(npm test)"));
        assert_eq!(pattern_for_denied("Read", "x"), None);
        assert_eq!(pattern_for_denied("Bash", "a\nb"), None);
    }
}
