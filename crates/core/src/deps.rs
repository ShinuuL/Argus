//! Detecção de dependências ausentes (toolchains e dependências do projeto)
//! e sugestão de instalação. Nada aqui executa instalações: apenas monta o
//! `InstallPlan`, que a interface só roda após confirmação explícita do usuário.

use crate::agents::provider::{strip_ansi, which_bin, CommandSpec};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepKind {
    /// Ferramenta (node, cargo, python...) não encontrada no PATH.
    MissingToolchain,
    /// Dependências do projeto nunca instaladas (node_modules, .venv...).
    ProjectDepsMissing,
    /// Dependências instaladas, mas mais antigas que o lockfile.
    ProjectDepsOutdated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub command: CommandSpec,
    pub needs_admin: bool,
}

impl InstallPlan {
    /// Linha de comando legível, para exibir ao usuário antes de confirmar.
    pub fn display(&self) -> String {
        let mut s = self.command.program.clone();
        for a in &self.command.args {
            if a.contains(' ') || a.is_empty() {
                s.push_str(&format!(" \"{a}\""));
            } else {
                s.push(' ');
                s.push_str(a);
            }
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepIssue {
    pub kind: DepKind,
    /// Nome curto da ferramenta/ecossistema (ex.: "node", "pnpm", "python").
    pub tool: String,
    pub description: String,
    pub fix: InstallPlan,
}

/// Tabela comando -> (id winget, pacote npm global). Um dos dois é usado.
const INSTALL_TABLE: &[(&str, Option<&str>, Option<&str>)] = &[
    ("node", Some("OpenJS.NodeJS.LTS"), None),
    ("npm", Some("OpenJS.NodeJS.LTS"), None),
    ("npx", Some("OpenJS.NodeJS.LTS"), None),
    ("pnpm", None, Some("pnpm")),
    ("yarn", None, Some("yarn")),
    ("tsc", None, Some("typescript")),
    ("cargo", Some("Rustlang.Rustup"), None),
    ("rustc", Some("Rustlang.Rustup"), None),
    ("rustup", Some("Rustlang.Rustup"), None),
    ("python", Some("Python.Python.3.12"), None),
    ("python3", Some("Python.Python.3.12"), None),
    ("pip", Some("Python.Python.3.12"), None),
    ("py", Some("Python.Python.3.12"), None),
    ("uv", Some("astral-sh.uv"), None),
    ("go", Some("GoLang.Go"), None),
    ("java", Some("EclipseAdoptium.Temurin.21.JDK"), None),
    ("javac", Some("EclipseAdoptium.Temurin.21.JDK"), None),
    ("mvn", Some("Apache.Maven"), None),
    ("gradle", Some("Gradle.Gradle"), None),
    ("git", Some("Git.Git"), None),
    ("docker", Some("Docker.DockerDesktop"), None),
    ("gh", Some("GitHub.cli"), None),
    ("claude", None, Some("@anthropic-ai/claude-code")),
    ("opencode", None, Some("opencode-ai")),
    ("codex", None, Some("@openai/codex")),
    ("gemini", None, Some("@google/gemini-cli")),
];

fn winget_plan(id: &str) -> InstallPlan {
    InstallPlan {
        command: CommandSpec::new("winget")
            .arg("install")
            .arg("--id")
            .arg(id)
            .arg("-e")
            .arg("--accept-source-agreements")
            .arg("--accept-package-agreements"),
        // O winget solicita elevação (UAC) sozinho quando o instalador exige.
        needs_admin: false,
    }
}

fn npm_global_plan(pkg: &str) -> InstallPlan {
    InstallPlan { command: CommandSpec::new("npm").arg("install").arg("-g").arg(pkg), needs_admin: false }
}

/// Plano de instalação sugerido para um comando ausente, se conhecido.
pub fn install_plan_for(command: &str) -> Option<InstallPlan> {
    let c = command.to_ascii_lowercase();
    let c = c.trim_end_matches(".exe").trim_end_matches(".cmd");
    INSTALL_TABLE.iter().find(|(n, _, _)| *n == c).map(|(_, w, n)| match (w, n) {
        (_, Some(pkg)) => npm_global_plan(pkg),
        (Some(id), None) => winget_plan(id),
        (None, None) => unreachable!(),
    })
}

/// Reconhece mensagens de "comando não encontrado" (PowerShell, cmd, bash/zsh,
/// em inglês e português) e devolve o nome do comando.
pub fn detect_missing_command(output_text: &str) -> Option<String> {
    let text = strip_ansi(output_text);
    for line in text.lines().rev() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let lower = l.to_lowercase();
        let quoted_markers = [
            "is not recognized as the name of a cmdlet",
            "is not recognized as an internal or external command",
            "não é reconhecido como",
            "nao e reconhecido como",
        ];
        if quoted_markers.iter().any(|m| lower.contains(m)) {
            if let Some(name) = first_quoted(l) {
                return clean_name(&name);
            }
        }
        // zsh: "zsh: command not found: foo"
        if let Some(idx) = lower.find("command not found: ") {
            return clean_name(&l[idx + "command not found: ".len()..]);
        }
        // bash: "bash: foo: command not found" / "foo: command not found"
        if lower.ends_with(": command not found") || lower.ends_with(": comando não encontrado") {
            let head = &l[..l.rfind(':').unwrap()];
            let name = head.rsplit(':').next().unwrap_or(head);
            return clean_name(name);
        }
    }
    None
}

fn first_quoted(s: &str) -> Option<String> {
    for q in ['\'', '"', '‘', '“'] {
        if let Some(a) = s.find(q) {
            let rest = &s[a + q.len_utf8()..];
            let close = match q {
                '‘' => '’',
                '“' => '”',
                c => c,
            };
            if let Some(b) = rest.find(close) {
                return Some(rest[..b].to_string());
            }
        }
    }
    None
}

fn clean_name(s: &str) -> Option<String> {
    let n = s.trim().trim_matches(|c| c == '\'' || c == '"' || c == '.').trim();
    let n = n.split_whitespace().next()?;
    // Caminhos (./script) não são ferramentas instaláveis.
    let n = n.rsplit(['\\', '/']).next().unwrap_or(n);
    if n.is_empty() {
        None
    } else {
        Some(n.to_string())
    }
}

/// Analisa um projeto usando o PATH real.
pub fn analyze(project: &Path) -> Vec<DepIssue> {
    analyze_with(project, &|bin| which_bin(bin).is_some())
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn missing_tool(tool: &str, why: &str) -> Option<DepIssue> {
    install_plan_for(tool).map(|fix| DepIssue {
        kind: DepKind::MissingToolchain,
        tool: tool.to_string(),
        description: format!("`{tool}` não encontrado no PATH ({why})."),
        fix,
    })
}

fn shell_plan(line: &str, cwd: &Path) -> InstallPlan {
    let cmd = if cfg!(windows) {
        CommandSpec::new("cmd").arg("/C").arg(line)
    } else {
        CommandSpec::new("sh").arg("-c").arg(line)
    };
    InstallPlan { command: cmd.cwd(cwd), needs_admin: false }
}

/// Igual a `analyze`, com o teste de existência de binário injetável (testes).
pub fn analyze_with(project: &Path, has_bin: &dyn Fn(&str) -> bool) -> Vec<DepIssue> {
    let mut out = Vec::new();
    let f = |n: &str| project.join(n);
    let exists = |n: &str| f(n).exists();
    let need = |tool: &str, why: &str, out: &mut Vec<DepIssue>| {
        if !has_bin(tool) {
            if let Some(i) = missing_tool(tool, why) {
                if !out.iter().any(|o: &DepIssue| o.fix == i.fix) {
                    out.push(i);
                }
            }
            false
        } else {
            true
        }
    };

    if exists(".git") {
        need("git", "repositório git", &mut out);
    }

    // Node
    if exists("package.json") {
        let (pm, lock) = if exists("pnpm-lock.yaml") {
            ("pnpm", Some("pnpm-lock.yaml"))
        } else if exists("yarn.lock") {
            ("yarn", Some("yarn.lock"))
        } else if exists("package-lock.json") {
            ("npm", Some("package-lock.json"))
        } else {
            ("npm", None)
        };
        let node_ok = need("node", "package.json", &mut out);
        let pm_ok = need(pm, lock.unwrap_or("package.json"), &mut out);
        if node_ok && pm_ok {
            let install = match (pm, lock) {
                ("npm", Some(_)) => "ci",
                _ => "install",
            };
            let plan = InstallPlan { command: CommandSpec::new(pm).arg(install).cwd(project), needs_admin: false };
            let nm = f("node_modules");
            if !nm.exists() {
                out.push(DepIssue {
                    kind: DepKind::ProjectDepsMissing,
                    tool: pm.into(),
                    description: "node_modules ausente: dependências do projeto não instaladas.".into(),
                    fix: plan,
                });
            } else if let Some(l) = lock {
                // npm/yarn/pnpm gravam um marcador dentro de node_modules a cada instalação.
                let marker = [".package-lock.json", ".yarn-integrity", ".modules.yaml"]
                    .iter()
                    .filter_map(|m| mtime(&nm.join(m)))
                    .max()
                    .or_else(|| mtime(&nm));
                if let (Some(lt), Some(mt)) = (mtime(&f(l)), marker) {
                    if lt > mt {
                        out.push(DepIssue {
                            kind: DepKind::ProjectDepsOutdated,
                            tool: pm.into(),
                            description: format!("{l} é mais recente que node_modules: reinstale as dependências."),
                            fix: plan,
                        });
                    }
                }
            }
        }
    }

    // Rust
    if exists("Cargo.toml") && need("cargo", "Cargo.toml", &mut out) && exists("Cargo.lock") && !exists("target") {
        out.push(DepIssue {
            kind: DepKind::ProjectDepsMissing,
            tool: "cargo".into(),
            description: "Crates ainda não baixados (sem pasta target).".into(),
            fix: InstallPlan { command: CommandSpec::new("cargo").arg("fetch").cwd(project), needs_admin: false },
        });
    }

    // Python
    let has_req = exists("requirements.txt");
    let has_pyproj = exists("pyproject.toml");
    if has_req || has_pyproj || exists("uv.lock") {
        let use_uv = exists("uv.lock");
        let ok = if use_uv {
            need("uv", "uv.lock", &mut out)
        } else {
            let py = if cfg!(windows) { "python" } else { "python3" };
            need(py, if has_req { "requirements.txt" } else { "pyproject.toml" }, &mut out)
        };
        let venv = [".venv", "venv"].iter().any(|v| exists(v));
        if ok && !venv {
            let fix = if use_uv {
                InstallPlan { command: CommandSpec::new("uv").arg("sync").cwd(project), needs_admin: false }
            } else {
                let (py, pip) = if cfg!(windows) {
                    ("python", r".venv\Scripts\pip")
                } else {
                    ("python3", ".venv/bin/pip")
                };
                let what = if has_req { "-r requirements.txt" } else { "-e ." };
                shell_plan(&format!("{py} -m venv .venv && {pip} install {what}"), project)
            };
            out.push(DepIssue {
                kind: DepKind::ProjectDepsMissing,
                tool: if use_uv { "uv" } else { "python" }.into(),
                description: "Ambiente virtual (.venv) ausente: dependências Python não instaladas.".into(),
                fix,
            });
        }
    }

    // Go
    if exists("go.mod") && need("go", "go.mod", &mut out) && exists("go.sum") && !go_modcache_populated() {
        out.push(DepIssue {
            kind: DepKind::ProjectDepsMissing,
            tool: "go".into(),
            description: "Cache de módulos Go vazio: execute go mod download.".into(),
            fix: InstallPlan { command: CommandSpec::new("go").arg("mod").arg("download").cwd(project), needs_admin: false },
        });
    }

    // Java
    if exists("pom.xml") {
        need("java", "pom.xml", &mut out);
        if !exists("mvnw") && !exists("mvnw.cmd") {
            need("mvn", "pom.xml sem mvnw", &mut out);
        }
    }
    if exists("build.gradle") || exists("build.gradle.kts") {
        need("java", "build.gradle", &mut out);
        if !exists("gradlew") && !exists("gradlew.bat") {
            need("gradle", "build.gradle sem gradlew", &mut out);
        }
    }

    out
}

fn go_modcache_populated() -> bool {
    let dir: Option<PathBuf> = std::env::var_os("GOMODCACHE").map(PathBuf::from).or_else(|| {
        std::env::var_os("GOPATH")
            .map(PathBuf::from)
            .or_else(|| dirs_home().map(|h| h.join("go")))
            .map(|g| g.join("pkg").join("mod"))
    });
    dir.and_then(|d| std::fs::read_dir(d).ok()).map(|mut r| r.next().is_some()).unwrap_or(false)
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("argus-deps-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn detects_powershell_en_pt_cmd_bash_zsh() {
        let ps = "pnpm : The term 'pnpm' is not recognized as the name of a cmdlet, function, script file";
        assert_eq!(detect_missing_command(ps).as_deref(), Some("pnpm"));
        let pt = "uv : O termo 'uv' não é reconhecido como nome de cmdlet, função, arquivo de script";
        assert_eq!(detect_missing_command(pt).as_deref(), Some("uv"));
        let cmd = "'go' is not recognized as an internal or external command,\r\noperable program or batch file.";
        assert_eq!(detect_missing_command(cmd).as_deref(), Some("go"));
        let cmdpt = "'yarn' não é reconhecido como um comando interno\r\nou externo, um programa operável ou um arquivo em lotes.";
        assert_eq!(detect_missing_command(cmdpt).as_deref(), Some("yarn"));
        assert_eq!(detect_missing_command("bash: cargo: command not found").as_deref(), Some("cargo"));
        assert_eq!(detect_missing_command("zsh: command not found: mvn").as_deref(), Some("mvn"));
        assert_eq!(detect_missing_command("\x1b[31mbash: gh: command not found\x1b[0m\n$ ").as_deref(), Some("gh"));
        assert_eq!(detect_missing_command("all good\nbuild ok"), None);
    }

    #[test]
    fn table_maps_to_winget_or_npm() {
        let p = install_plan_for("pnpm").unwrap();
        assert_eq!(p.display(), "npm install -g pnpm");
        let p = install_plan_for("node.exe").unwrap();
        assert!(p.display().starts_with("winget install --id OpenJS.NodeJS.LTS -e"));
        assert!(install_plan_for("definitely-unknown-tool").is_none());
    }

    #[test]
    fn node_missing_toolchain_and_modules() {
        let d = tmp("node");
        std::fs::write(d.join("package.json"), "{}").unwrap();
        std::fs::write(d.join("pnpm-lock.yaml"), "").unwrap();
        let none = analyze_with(&d, &|_| false);
        assert!(none.iter().any(|i| i.kind == DepKind::MissingToolchain && i.tool == "node"));
        assert!(none.iter().any(|i| i.tool == "pnpm" && i.fix.display() == "npm install -g pnpm"));
        assert!(!none.iter().any(|i| i.kind == DepKind::ProjectDepsMissing));

        let all = analyze_with(&d, &|_| true);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].kind, DepKind::ProjectDepsMissing);
        assert_eq!(all[0].fix.display(), "pnpm install");
        assert_eq!(all[0].fix.command.cwd.as_deref(), Some(d.as_path()));

        // node_modules mais antigo que o lockfile -> desatualizado
        std::fs::create_dir(d.join("node_modules")).unwrap();
        let m = d.join("node_modules").join(".modules.yaml");
        std::fs::write(&m, "").unwrap();
        let old = SystemTime::now() - std::time::Duration::from_secs(3600);
        std::fs::File::options().write(true).open(&m).unwrap().set_modified(old).unwrap();
        let r = analyze_with(&d, &|_| true);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, DepKind::ProjectDepsOutdated);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn npm_lock_uses_ci_and_python_venv() {
        let d = tmp("mix");
        std::fs::write(d.join("package.json"), "{}").unwrap();
        std::fs::write(d.join("package-lock.json"), "{}").unwrap();
        std::fs::write(d.join("requirements.txt"), "requests").unwrap();
        let r = analyze_with(&d, &|_| true);
        assert!(r.iter().any(|i| i.fix.display() == "npm ci"));
        let py = r.iter().find(|i| i.tool == "python").unwrap();
        assert!(py.fix.display().contains("-m venv .venv"));
        assert!(py.fix.display().contains("requirements.txt"));
        std::fs::create_dir(d.join(".venv")).unwrap();
        let r = analyze_with(&d, &|_| true);
        assert!(!r.iter().any(|i| i.tool == "python"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn cargo_java_git_dedup() {
        let d = tmp("rs");
        std::fs::create_dir(d.join(".git")).unwrap();
        std::fs::write(d.join("Cargo.toml"), "").unwrap();
        std::fs::write(d.join("Cargo.lock"), "").unwrap();
        std::fs::write(d.join("pom.xml"), "").unwrap();
        std::fs::write(d.join("build.gradle"), "").unwrap();
        let r = analyze_with(&d, &|_| false);
        let tools: Vec<_> = r.iter().map(|i| i.tool.as_str()).collect();
        assert!(tools.contains(&"git") && tools.contains(&"cargo") && tools.contains(&"mvn") && tools.contains(&"gradle"));
        assert_eq!(tools.iter().filter(|t| **t == "java").count(), 1);
        let r = analyze_with(&d, &|_| true);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].fix.display(), "cargo fetch");
        let _ = std::fs::remove_dir_all(&d);
    }
}
