//! Conexões com serviços externos (GitHub, Google Cloud, Firebase, Vercel).
//!
//! Só pergunta às próprias CLIs se estão instaladas e logadas. Nunca lê
//! arquivos de config/credenciais, nunca devolve tokens: toda linha que
//! menciona "token" é descartada antes de qualquer parsing.
use crate::agents::provider::{which_bin, CommandSpec};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Connection {
    /// "gh", "gcloud", "firebase", "vercel".
    pub id: String,
    pub label: String,
    pub installed: bool,
    pub logged_in: bool,
    /// Conta/usuário logado, quando a CLI informa.
    pub account: Option<String>,
    /// Mensagem curta de erro (timeout, falha ao executar).
    pub error: Option<String>,
    /// Comando de login para rodar no terminal embutido.
    pub login: CommandSpec,
}

pub const SERVICES: [(&str, &str); 4] = [
    ("gh", "GitHub"),
    ("gcloud", "Google Cloud"),
    ("firebase", "Firebase"),
    ("vercel", "Vercel"),
];

/// Remove toda linha que mencione token (qualquer caixa).
pub fn strip_token_lines(text: &str) -> String {
    text.lines()
        .filter(|l| !l.to_ascii_lowercase().contains("token"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn spec(program: &str, args: &[&str], cwd: Option<&Path>) -> CommandSpec {
    CommandSpec {
        program: program.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: cwd.map(Path::to_path_buf),
        env: Vec::new(),
        stdin: None,
    }
}

/// Comando de login (rodado pelo usuário no terminal, nunca por aqui).
pub fn login_command(id: &str) -> Option<CommandSpec> {
    Some(match id {
        "gh" => spec("gh", &["auth", "login"], None),
        "gcloud" => spec("gcloud", &["auth", "login"], None),
        "firebase" => spec("firebase", &["login"], None),
        "vercel" => spec("vercel", &["login"], None),
        _ => return None,
    })
}

/// Saída de um comando, já sem linhas de token.
struct Out {
    ok: bool,
    stdout: String,
    stderr: String,
}

fn read_all<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_string(&mut s);
        }
        s
    })
}

fn run(exe: &Path, args: &[&str], cwd: Option<&Path>) -> Result<Out, String> {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1")
        .env("GH_PROMPT_DISABLED", "1");
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("falha ao executar: {e}"))?;
    let t_out = read_all(child.stdout.take());
    let t_err = read_all(child.stderr.take());
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("tempo esgotado".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(e.to_string()),
        }
    };
    Ok(Out {
        ok: status.success(),
        stdout: strip_token_lines(&t_out.join().unwrap_or_default()),
        stderr: strip_token_lines(&t_err.join().unwrap_or_default()),
    })
}

/// `gh auth status` (stdout ou stderr, conforme a versão).
pub fn parse_gh_status(text: &str) -> Option<String> {
    for line in strip_token_lines(text).lines() {
        let Some(i) = line.find("Logged in to ") else { continue };
        let rest = &line[i..];
        // Novo: "Logged in to github.com account fulano (keyring)"; antigo: "... as fulano (...)".
        for key in [" account ", " as "] {
            if let Some(j) = rest.find(key) {
                let name = rest[j + key.len()..].split_whitespace().next()?;
                return Some(name.to_string());
            }
        }
    }
    None
}

/// `gcloud auth list --filter=status:ACTIVE --format=value(account)`.
pub fn parse_gcloud(text: &str) -> Option<String> {
    strip_token_lines(text)
        .lines()
        .map(str::trim)
        .find(|l| l.contains('@') && !l.contains(' '))
        .map(str::to_string)
}

/// `firebase login:list`.
pub fn parse_firebase(text: &str) -> Option<String> {
    for line in strip_token_lines(text).lines() {
        if let Some(i) = line.find("Logged in as ") {
            let name = line[i + 13..].split_whitespace().next()?;
            return Some(name.to_string());
        }
    }
    None
}

/// `vercel whoami`: o nome sai sozinho no stdout (avisos vão para o stderr).
pub fn parse_vercel(stdout: &str) -> Option<String> {
    strip_token_lines(stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.contains(' ') && !l.starts_with('>'))
        .last()
        .map(str::to_string)
}

fn check_one(id: &str, label: &str) -> Connection {
    let mut c = Connection {
        id: id.into(),
        label: label.into(),
        installed: false,
        logged_in: false,
        account: None,
        error: None,
        login: login_command(id).unwrap_or_else(|| spec(id, &[], None)),
    };
    let Some(exe) = which_bin(id) else { return c };
    c.installed = true;
    let res = match id {
        "gh" => run(&exe, &["auth", "status"], None).map(|o| parse_gh_status(&format!("{}\n{}", o.stdout, o.stderr))),
        "gcloud" => run(&exe, &["auth", "list", "--filter=status:ACTIVE", "--format=value(account)"], None)
            .map(|o| parse_gcloud(&o.stdout)),
        "firebase" => run(&exe, &["login:list", "--non-interactive"], None)
            .map(|o| parse_firebase(&format!("{}\n{}", o.stdout, o.stderr))),
        "vercel" => run(&exe, &["whoami", "--no-color"], None).map(|o| if o.ok { parse_vercel(&o.stdout) } else { None }),
        _ => Ok(None),
    };
    match res {
        Ok(acc) => {
            c.logged_in = acc.is_some();
            c.account = acc;
        }
        Err(e) => c.error = Some(e),
    }
    c
}

/// Estado de todas as conexões (em paralelo; cada uma com timeout curto).
pub fn check_all() -> Vec<Connection> {
    let handles: Vec<_> = SERVICES
        .iter()
        .map(|&(id, label)| std::thread::spawn(move || check_one(id, label)))
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepoInfo {
    pub name_with_owner: String,
    pub url: String,
    pub default_branch: Option<String>,
    pub pull_requests: Vec<PullRequest>,
}

/// Interpreta `gh repo view --json nameWithOwner,url,defaultBranchRef`.
pub fn parse_repo_view(json: &str) -> Option<(String, String, Option<String>)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let name = v.get("nameWithOwner")?.as_str()?.to_string();
    let url = v.get("url")?.as_str()?.to_string();
    let branch = v
        .get("defaultBranchRef")
        .and_then(|b| b.get("name"))
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Some((name, url, branch))
}

/// Cartão do repositório GitHub do projeto (None se não for repo do GitHub).
pub fn repo_info(project: &Path) -> Result<Option<RepoInfo>, String> {
    let exe: PathBuf = which_bin("gh").ok_or("A CLI gh não está instalada.")?;
    let view = run(&exe, &["repo", "view", "--json", "nameWithOwner,url,defaultBranchRef"], Some(project))?;
    if !view.ok {
        return Ok(None);
    }
    let Some((name_with_owner, url, default_branch)) = parse_repo_view(&view.stdout) else {
        return Ok(None);
    };
    let pull_requests = run(&exe, &["pr", "list", "--state", "open", "--json", "number,title", "--limit", "5"], Some(project))
        .ok()
        .filter(|o| o.ok)
        .and_then(|o| serde_json::from_str::<Vec<PullRequest>>(&o.stdout).ok())
        .unwrap_or_default();
    Ok(Some(RepoInfo { name_with_owner, url, default_branch, pull_requests }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gh_new_format_and_token_dropped() {
        let out = "github.com\n  ✓ Logged in to github.com account fulano (keyring)\n  - Active account: true\n  - Token: gho_************\n  - Token scopes: 'repo'";
        assert_eq!(parse_gh_status(out).as_deref(), Some("fulano"));
        assert!(!strip_token_lines(out).contains("gho_"));
    }

    #[test]
    fn gh_old_format_and_logged_out() {
        assert_eq!(parse_gh_status("  ✓ Logged in to github.com as beltrano (keyring)").as_deref(), Some("beltrano"));
        // Linha que cita token é descartada inteira, por segurança.
        assert_eq!(parse_gh_status("  ✓ Logged in to github.com as beltrano (oauth_token)"), None);
        assert_eq!(parse_gh_status("You are not logged into any GitHub hosts. To log in, run: gh auth login"), None);
    }

    #[test]
    fn gcloud() {
        assert_eq!(parse_gcloud("dev@empresa.com\n").as_deref(), Some("dev@empresa.com"));
        assert_eq!(parse_gcloud(""), None);
        assert_eq!(parse_gcloud("WARNING: No credentialed accounts."), None);
    }

    #[test]
    fn firebase() {
        assert_eq!(parse_firebase("Logged in as dev@empresa.com").as_deref(), Some("dev@empresa.com"));
        assert_eq!(parse_firebase("No authorized accounts, run \"firebase login\""), None);
    }

    #[test]
    fn vercel() {
        assert_eq!(parse_vercel("fulano\n").as_deref(), Some("fulano"));
        assert_eq!(parse_vercel(""), None);
    }

    #[test]
    fn repo_view() {
        let j = r#"{"defaultBranchRef":{"name":"main"},"nameWithOwner":"org/app","url":"https://github.com/org/app"}"#;
        assert_eq!(parse_repo_view(j), Some(("org/app".into(), "https://github.com/org/app".into(), Some("main".into()))));
        let prs: Vec<PullRequest> = serde_json::from_str(r#"[{"number":3,"title":"Fix"}]"#).unwrap();
        assert_eq!(prs[0].number, 3);
    }
}
