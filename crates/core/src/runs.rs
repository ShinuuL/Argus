//! Comandos de build e de servidor de desenvolvimento de cada projeto.
//!
//! - `detect`: lê os arquivos do projeto (scripts do `package.json`, `Cargo.toml`,
//!   `tauri.conf.json`, Gradle, `pubspec.yaml`, `go.mod`, `manage.py`, `*.csproj`) e
//!   devolve os comandos com rótulo, tipo (build ou servidor), porta esperada e onde
//!   procurar o artefato gerado.
//! - `match_cmdline`: reconhece, pela linha de comando de um processo, qual comando
//!   detectado ele está rodando (builds iniciados à mão num terminal ou por agentes).
//! - Portas: `port_open` (TCP connect), `port_busy` e `nearest_free`.
//! - `find_artifact`: o arquivo (ou pasta) gerado depois de um instante.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tipo do comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunKind {
    Build,
    Servidor,
}

impl RunKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RunKind::Build => "build",
            RunKind::Servidor => "servidor",
        }
    }
}

/// Como passar outra porta ao comando.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum PortStyle {
    /// Parâmetro, ex.: `--port` (vite, next, astro...).
    Flag(String),
    /// Variável de ambiente, ex.: `PORT` (react-scripts).
    Env(String),
}

/// Onde procurar o artefato: arquivos com uma das extensões em `dir` (até 2 níveis),
/// ou, sem extensões, a própria pasta `dir`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactHint {
    pub dir: String,
    pub exts: Vec<String>,
}

fn files(dir: &str, exts: &[&str]) -> ArtifactHint {
    ArtifactHint { dir: dir.into(), exts: exts.iter().map(|e| e.to_string()).collect() }
}

fn folder(dir: &str) -> ArtifactHint {
    ArtifactHint { dir: dir.into(), exts: Vec::new() }
}

/// Um comando detectado no projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedCommand {
    /// Identificador estável no projeto, ex.: `npm:dev`, `cargo:build-release`.
    pub id: String,
    /// Rótulo para o botão, ex.: `npm run dev`.
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    pub kind: RunKind,
    /// Porta esperada (servidores), se dedutível.
    pub port: Option<u16>,
    /// Como trocar a porta, se o comando aceita.
    pub port_style: Option<PortStyle>,
    /// Arquivo de onde veio (`package.json`, `Cargo.toml`...).
    pub source: String,
    /// Pistas do artefato (builds), relativas ao projeto.
    pub artifacts: Vec<ArtifactHint>,
}

impl DetectedCommand {
    /// Linha de comando para exibir.
    pub fn cmdline(&self) -> String {
        std::iter::once(self.program.as_str()).chain(self.args.iter().map(String::as_str)).collect::<Vec<_>>().join(" ")
    }

    /// Argumentos e variáveis para rodar na porta `port` (None se o comando não aceita porta).
    pub fn with_port(&self, port: u16) -> Option<(Vec<String>, Vec<(String, String)>)> {
        match self.port_style.as_ref()? {
            PortStyle::Flag(flag) => {
                let mut args = self.args.clone();
                // `npm run x -- --port N`: sem o `--`, o npm engole o parâmetro.
                if self.program == "npm" && args.first().is_some_and(|a| a == "run") {
                    args.push("--".into());
                }
                args.push(flag.clone());
                args.push(port.to_string());
                Some((args, Vec::new()))
            }
            PortStyle::Env(var) => Some((self.args.clone(), vec![(var.clone(), port.to_string())])),
        }
    }
}

// ---------------------------------------------------------------- detecção

fn read(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

/// Gerenciador de pacotes JS pelo lockfile.
pub fn package_manager(project: &Path) -> &'static str {
    if project.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if project.join("yarn.lock").exists() {
        "yarn"
    } else if project.join("bun.lockb").exists() || project.join("bun.lock").exists() {
        "bun"
    } else {
        "npm"
    }
}

/// Porta explícita num script: `--port N`, `--port=N`, `-p N`, `PORT=N`.
pub fn explicit_port(script: &str) -> Option<u16> {
    let toks: Vec<&str> = script.split_whitespace().collect();
    for (i, t) in toks.iter().enumerate() {
        let t = t.trim_matches(|c| c == '"' || c == '\'');
        if let Some(v) = t.strip_prefix("--port=").or_else(|| t.strip_prefix("PORT=")) {
            if let Ok(p) = v.parse() {
                return Some(p);
            }
        }
        if matches!(t, "--port" | "-p") {
            if let Some(p) = toks.get(i + 1).and_then(|v| v.parse().ok()) {
                return Some(p);
            }
        }
    }
    None
}

fn has_word(s: &str, w: &str) -> bool {
    s.split(|c: char| c.is_whitespace() || c == '&' || c == ';' || c == '|').any(|x| x == w)
}

/// Porta padrão e como trocá-la, pelo framework do script.
fn framework_port(script: &str) -> Option<(u16, PortStyle)> {
    let flag = |p: u16| Some((p, PortStyle::Flag("--port".into())));
    let s = script.to_ascii_lowercase();
    if s.contains("next dev") || s.contains("next start") {
        flag(3000)
    } else if s.contains("vite preview") {
        flag(4173)
    } else if s.contains("astro dev") || s.contains("astro preview") {
        flag(4321)
    } else if s.contains("nuxt dev") || s.contains("nuxi dev") {
        flag(3000)
    } else if s.contains("ng serve") {
        flag(4200)
    } else if s.contains("react-scripts start") {
        Some((3000, PortStyle::Env("PORT".into())))
    } else if s.contains("webpack serve") || s.contains("webpack-dev-server") {
        flag(8080)
    } else if has_word(&s, "vite") && !s.contains("vite build") {
        flag(5173)
    } else {
        None
    }
}

/// Porta do `devUrl` (ou `devPath`) do `tauri.conf.json`.
fn tauri_dev_port(conf: &Value) -> Option<u16> {
    let url = conf.pointer("/build/devUrl").or_else(|| conf.pointer("/build/devPath"))?.as_str()?;
    let rest = url.split("://").nth(1)?;
    let host = rest.split('/').next()?;
    host.rsplit(':').next()?.parse().ok()
}

/// `tauri.conf.json` do projeto (em `src-tauri/` ou na raiz).
fn tauri_conf(project: &Path) -> Option<Value> {
    ["src-tauri/tauri.conf.json", "tauri.conf.json"]
        .iter()
        .find_map(|p| read(&project.join(p)))
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn tauri_artifacts() -> Vec<ArtifactHint> {
    let mut v = Vec::new();
    for base in ["src-tauri/target/release/bundle", "target/release/bundle"] {
        for (sub, ext) in [("nsis", "exe"), ("msi", "msi"), ("dmg", "dmg"), ("appimage", "AppImage"), ("deb", "deb")] {
            v.push(files(&format!("{base}/{sub}"), &[ext]));
        }
    }
    v
}

fn exe_exts() -> &'static [&'static str] {
    if cfg!(windows) {
        &["exe"]
    } else {
        &[""]
    }
}

fn detect_package_json(project: &Path, out: &mut Vec<DetectedCommand>) -> bool {
    let Some(pkg) = read(&project.join("package.json")).and_then(|s| serde_json::from_str::<Value>(&s).ok()) else {
        return false;
    };
    let Some(scripts) = pkg.get("scripts").and_then(Value::as_object) else { return false };
    let pm = package_manager(project);
    let tauri_port = tauri_conf(project).as_ref().and_then(tauri_dev_port);
    let mut has_tauri = false;
    for (name, value) in scripts {
        let Some(script) = value.as_str() else { continue };
        let low = script.to_ascii_lowercase();
        let is_tauri_build = low.contains("tauri build");
        let is_tauri_dev = low.contains("tauri dev");
        let builder = is_tauri_build || low.contains("electron-builder") || low.contains("electron-forge make");
        let kind = if builder || matches!(name.as_str(), "build" | "dist") || name.starts_with("build:") || name.starts_with("dist:") {
            RunKind::Build
        } else if is_tauri_dev || matches!(name.as_str(), "dev" | "start" | "serve" | "preview") {
            RunKind::Servidor
        } else {
            continue;
        };
        has_tauri |= is_tauri_build || is_tauri_dev;
        let (mut port, mut style) = (None, None);
        if kind == RunKind::Servidor {
            if is_tauri_dev {
                port = tauri_port;
            } else if let Some(p) = explicit_port(script) {
                port = Some(p);
            } else if let Some((p, s)) = framework_port(script) {
                port = Some(p);
                style = Some(s);
            }
        }
        let artifacts = if is_tauri_build {
            tauri_artifacts()
        } else if builder {
            vec![files("dist", &["exe", "msi", "dmg", "AppImage", "deb"]), files("out/make", &["exe", "zip", "msi"])]
        } else if kind == RunKind::Build {
            vec![folder("dist"), folder("build"), folder("out"), folder(".next")]
        } else {
            Vec::new()
        };
        out.push(DetectedCommand {
            id: format!("{pm}:{name}"),
            label: format!("{pm} run {name}"),
            program: pm.into(),
            args: vec!["run".into(), name.clone()],
            kind,
            port,
            port_style: style,
            source: "package.json".into(),
            artifacts,
        });
    }
    has_tauri
}

fn detect_cargo(project: &Path, out: &mut Vec<DetectedCommand>) {
    let Some(toml) = read(&project.join("Cargo.toml")) else { return };
    if !toml.contains("[package]") && !toml.contains("[workspace]") {
        return;
    }
    let cargo = |id: &str, args: &[&str], kind, artifacts| DetectedCommand {
        id: format!("cargo:{id}"),
        label: format!("cargo {}", args.join(" ")),
        program: "cargo".into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        kind,
        port: None,
        port_style: None,
        source: "Cargo.toml".into(),
        artifacts,
    };
    out.push(cargo("build", &["build"], RunKind::Build, vec![files("target/debug", exe_exts())]));
    out.push(cargo("build-release", &["build", "--release"], RunKind::Build, vec![files("target/release", exe_exts())]));
    if toml.contains("[package]") {
        out.push(cargo("run", &["run"], RunKind::Servidor, Vec::new()));
    }
}

fn detect_tauri_without_scripts(project: &Path, out: &mut Vec<DetectedCommand>) {
    let Some(conf) = tauri_conf(project) else { return };
    let mk = |id: &str, args: &[&str], kind, port, artifacts| DetectedCommand {
        id: format!("tauri:{id}"),
        label: format!("cargo tauri {}", args[1..].join(" ")),
        program: "cargo".into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        kind,
        port,
        port_style: None,
        source: "tauri.conf.json".into(),
        artifacts,
    };
    out.push(mk("dev", &["tauri", "dev"], RunKind::Servidor, tauri_dev_port(&conf), Vec::new()));
    out.push(mk("build", &["tauri", "build"], RunKind::Build, None, tauri_artifacts()));
}

fn detect_gradle(project: &Path, out: &mut Vec<DetectedCommand>) {
    let has_build = ["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts"]
        .iter()
        .any(|f| project.join(f).exists());
    if !has_build {
        return;
    }
    let wrapper = if cfg!(windows) { "gradlew.bat" } else { "gradlew" };
    let program = if project.join(wrapper).exists() {
        project.join(wrapper).to_string_lossy().into_owned()
    } else {
        "gradle".to_string()
    };
    let shown = if program == "gradle" { "gradle" } else { "gradlew" };
    let android = project.join("app/build.gradle").exists() || project.join("app/build.gradle.kts").exists();
    let tasks: Vec<(&str, Vec<ArtifactHint>)> = if android {
        vec![
            ("assembleDebug", vec![files("app/build/outputs/apk/debug", &["apk"])]),
            ("assembleRelease", vec![files("app/build/outputs/apk/release", &["apk"])]),
            ("bundleRelease", vec![files("app/build/outputs/bundle/release", &["aab"])]),
        ]
    } else {
        vec![("build", vec![files("build/libs", &["jar", "war"])]), ("assemble", vec![files("build/libs", &["jar", "war"])])]
    };
    for (task, artifacts) in tasks {
        out.push(DetectedCommand {
            id: format!("gradle:{task}"),
            label: format!("{shown} {task}"),
            program: program.clone(),
            args: vec![task.into()],
            kind: RunKind::Build,
            port: None,
            port_style: None,
            source: "Gradle".into(),
            artifacts,
        });
    }
}

fn detect_flutter(project: &Path, out: &mut Vec<DetectedCommand>) {
    let Some(pubspec) = read(&project.join("pubspec.yaml")) else { return };
    if !pubspec.lines().any(|l| l.trim_start().starts_with("flutter:")) {
        return;
    }
    let mk = |id: &str, args: &[&str], kind, artifacts| DetectedCommand {
        id: format!("flutter:{id}"),
        label: format!("flutter {}", args.join(" ")),
        program: "flutter".into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        kind,
        port: None,
        port_style: None,
        source: "pubspec.yaml".into(),
        artifacts,
    };
    out.push(mk("run", &["run"], RunKind::Servidor, Vec::new()));
    out.push(mk("build-apk", &["build", "apk"], RunKind::Build, vec![files("build/app/outputs/flutter-apk", &["apk"])]));
    out.push(mk("build-appbundle", &["build", "appbundle"], RunKind::Build, vec![files("build/app/outputs/bundle/release", &["aab"])]));
    if project.join("windows").is_dir() {
        out.push(mk("build-windows", &["build", "windows"], RunKind::Build, vec![folder("build/windows/x64/runner/Release")]));
    }
    if project.join("web").is_dir() {
        out.push(mk("build-web", &["build", "web"], RunKind::Build, vec![folder("build/web")]));
    }
}

fn detect_misc(project: &Path, out: &mut Vec<DetectedCommand>) {
    let mk = |id: &str, program: &str, args: &[&str], kind, port, source: &str, artifacts| DetectedCommand {
        id: id.into(),
        label: std::iter::once(program).chain(args.iter().copied()).collect::<Vec<_>>().join(" "),
        program: program.into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        kind,
        port,
        port_style: None,
        source: source.into(),
        artifacts,
    };
    if project.join("go.mod").exists() {
        out.push(mk("go:build", "go", &["build", "./..."], RunKind::Build, None, "go.mod", vec![files(".", exe_exts())]));
        out.push(mk("go:run", "go", &["run", "."], RunKind::Servidor, None, "go.mod", Vec::new()));
    }
    if project.join("manage.py").exists() {
        out.push(mk("django:runserver", "python", &["manage.py", "runserver"], RunKind::Servidor, Some(8000), "manage.py", Vec::new()));
    }
    let csproj = std::fs::read_dir(project)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("csproj")));
    if csproj {
        out.push(mk("dotnet:build", "dotnet", &["build"], RunKind::Build, None, ".csproj", vec![folder("bin/Debug")]));
        out.push(mk("dotnet:publish", "dotnet", &["publish", "-c", "Release"], RunKind::Build, None, ".csproj", vec![folder("bin/Release")]));
        out.push(mk("dotnet:run", "dotnet", &["run"], RunKind::Servidor, None, ".csproj", Vec::new()));
    }
}

/// Comandos de build e de servidor do projeto. Servidores primeiro, depois builds;
/// dentro de cada tipo, na ordem em que aparecem nos arquivos.
pub fn detect(project: &Path) -> Vec<DetectedCommand> {
    let mut out = Vec::new();
    let has_tauri_scripts = detect_package_json(project, &mut out);
    detect_cargo(project, &mut out);
    if !has_tauri_scripts {
        detect_tauri_without_scripts(project, &mut out);
    }
    detect_gradle(project, &mut out);
    detect_flutter(project, &mut out);
    detect_misc(project, &mut out);
    out.sort_by_key(|c| c.kind == RunKind::Build);
    out
}

// ---------------------------------------------------------------- reconhecimento

/// Nome de programa sem pasta, aspas nem extensão, em minúsculas
/// (`"C:\..\npm-cli.js"` -> `npm`, `cargo.exe` -> `cargo`).
fn norm_token(t: &str) -> String {
    let t = t.trim_matches(|c| c == '"' || c == '\'').to_ascii_lowercase();
    let base = t.rsplit(['/', '\\']).next().unwrap_or(&t).to_string();
    let base = [".exe", ".cmd", ".bat", ".cjs", ".mjs", ".js", ".ps1"]
        .iter()
        .find_map(|e| base.strip_suffix(e))
        .map(str::to_string)
        .unwrap_or(base);
    match base.as_str() {
        "npm-cli" => "npm".into(),
        "pnpm" | "pnpm-cli" => "pnpm".into(),
        "yarn" | "yarn-cli" => "yarn".into(),
        "gradlew" | "gradle" => "gradle".into(),
        _ => base,
    }
}

fn tokens(cmdline: &str) -> Vec<String> {
    cmdline.split_whitespace().map(norm_token).filter(|t| !t.is_empty()).collect()
}

/// Tokens de um comando detectado, para comparar com processos.
fn command_tokens(c: &DetectedCommand) -> Vec<String> {
    std::iter::once(norm_token(&c.program)).chain(c.args.iter().map(|a| norm_token(a))).collect()
}

/// O comando detectado (o mais específico) que a linha de comando de um processo roda.
/// `npm run build` casa com `node npm-cli.js run build`; `cargo build` casa com
/// `cargo.exe build -p x`, mas `cargo build --release` vence se a linha tiver `--release`.
pub fn match_cmdline<'a>(cmdline: &str, commands: &'a [DetectedCommand]) -> Option<&'a DetectedCommand> {
    let toks = tokens(cmdline);
    commands
        .iter()
        .filter_map(|c| {
            let want = command_tokens(c);
            let hit = toks.windows(want.len()).any(|w| w == want.as_slice());
            hit.then_some((want.len(), c))
        })
        .max_by_key(|(n, _)| *n)
        .map(|(_, c)| c)
}

// ---------------------------------------------------------------- portas

/// A porta responde a uma conexão TCP local (IPv4 ou IPv6)?
pub fn port_open(port: u16) -> bool {
    let timeout = Duration::from_millis(300);
    [SocketAddr::from(([127, 0, 0, 1], port)), SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port))]
        .iter()
        .any(|a| TcpStream::connect_timeout(a, timeout).is_ok())
}

/// A porta está em uso (alguém responde ou não dá para escutar nela)?
pub fn port_busy(port: u16) -> bool {
    port_open(port) || TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).is_err()
}

/// A porta livre mais próxima a partir de `port` (até 50 acima).
pub fn nearest_free(port: u16) -> Option<u16> {
    (port..=port.saturating_add(50)).find(|p| !port_busy(*p))
}

// ---------------------------------------------------------------- artefatos

/// Artefato gerado por um build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub path: PathBuf,
    /// Bytes (soma dos arquivos, se for pasta).
    pub size: u64,
    pub is_dir: bool,
}

fn mtime(p: &Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// (tamanho total, mtime mais recente) de uma pasta, com limite de entradas.
fn dir_stats(dir: &Path) -> (u64, u64) {
    let (mut size, mut newest, mut seen) = (0u64, 0u64, 0usize);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            seen += 1;
            if seen > 20_000 {
                return (size, newest);
            }
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                stack.push(e.path());
            } else {
                size += m.len();
                let t = m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                newest = newest.max(t);
            }
        }
    }
    (size, newest)
}

fn ext_ok(p: &Path, exts: &[String]) -> bool {
    match p.extension() {
        Some(e) => exts.iter().any(|x| e.to_string_lossy().eq_ignore_ascii_case(x)),
        None => exts.iter().any(String::is_empty),
    }
}

/// O artefato mais recente modificado a partir de `since` (segundos Unix), pelas pistas.
pub fn find_artifact(root: &Path, hints: &[ArtifactHint], since: u64) -> Option<Artifact> {
    let mut best: Option<(u64, Artifact)> = None;
    let mut consider = |t: u64, a: Artifact| {
        if t >= since && best.as_ref().is_none_or(|(bt, _)| t > *bt) {
            best = Some((t, a));
        }
    };
    for h in hints {
        let dir = root.join(&h.dir);
        if !dir.is_dir() {
            continue;
        }
        if h.exts.is_empty() {
            let (size, newest) = dir_stats(&dir);
            consider(newest, Artifact { path: dir, size, is_dir: true });
            continue;
        }
        let mut level = vec![dir];
        for _ in 0..2 {
            let mut next = Vec::new();
            for d in &level {
                for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                    let p = e.path();
                    let Ok(m) = e.metadata() else { continue };
                    if m.is_dir() {
                        next.push(p);
                    } else if ext_ok(&p, &h.exts) {
                        consider(mtime(&p), Artifact { path: p, size: m.len(), is_dir: false });
                    }
                }
            }
            level = next;
        }
    }
    best.map(|(_, a)| a)
}

/// Segundos desde a época Unix.
pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Caminho relativo ao projeto, se estiver dentro dele.
pub fn inside(project: &Path, p: &Path) -> bool {
    let a = project.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_ascii_lowercase();
    let b = p.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
    b == a || b.starts_with(&format!("{a}/"))
}

/// Pasta onde o artefato fica (para "Abrir pasta").
pub fn artifact_dir(a: &Artifact) -> PathBuf {
    if a.is_dir {
        a.path.clone()
    } else {
        a.path.parent().map(Path::to_path_buf).unwrap_or_else(|| a.path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    fn by_id<'a>(v: &'a [DetectedCommand], id: &str) -> &'a DetectedCommand {
        v.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("{id} ausente em {:?}", v.iter().map(|c| &c.id).collect::<Vec<_>>()))
    }

    #[test]
    fn package_json_vite_with_pnpm() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "pnpm-lock.yaml", "");
        write(
            d.path(),
            "package.json",
            r#"{"scripts":{"dev":"vite","build":"tsc && vite build","preview":"vite preview","lint":"eslint .","prebuild":"x"}}"#,
        );
        let v = detect(d.path());
        assert_eq!(v.len(), 3);
        let dev = by_id(&v, "pnpm:dev");
        assert_eq!((dev.kind, dev.port, dev.label.as_str()), (RunKind::Servidor, Some(5173), "pnpm run dev"));
        assert_eq!(dev.port_style, Some(PortStyle::Flag("--port".into())));
        assert_eq!(by_id(&v, "pnpm:preview").port, Some(4173));
        let b = by_id(&v, "pnpm:build");
        assert_eq!(b.kind, RunKind::Build);
        assert!(b.artifacts.iter().any(|h| h.dir == "dist" && h.exts.is_empty()));
        // Servidores antes dos builds.
        assert_eq!(v.last().unwrap().kind, RunKind::Build);
        assert_eq!(dev.with_port(5174).unwrap().0, ["run", "dev", "--port", "5174"]);
    }

    #[test]
    fn npm_next_explicit_port_and_tauri() {
        let d = tempfile::tempdir().unwrap();
        write(
            d.path(),
            "package.json",
            r#"{"scripts":{"dev":"next dev","start":"next start -p 4000","app:dev":"tauri dev","app:pacote":"tauri build","cra":"react-scripts start"}}"#,
        );
        write(d.path(), "src-tauri/tauri.conf.json", r#"{"build":{"devUrl":"http://localhost:1420"}}"#);
        let v = detect(d.path());
        let dev = by_id(&v, "npm:dev");
        assert_eq!(dev.port, Some(3000));
        assert_eq!(dev.with_port(3001).unwrap().0, ["run", "dev", "--", "--port", "3001"]);
        let start = by_id(&v, "npm:start");
        assert_eq!((start.port, start.port_style.clone()), (Some(4000), None));
        assert!(start.with_port(1).is_none());
        let td = by_id(&v, "npm:app:dev");
        assert_eq!((td.kind, td.port), (RunKind::Servidor, Some(1420)));
        let tb = by_id(&v, "npm:app:pacote");
        assert_eq!(tb.kind, RunKind::Build);
        assert!(tb.artifacts.iter().any(|h| h.dir.ends_with("bundle/nsis")));
        // Scripts com tauri: não repete o `cargo tauri`.
        assert!(!v.iter().any(|c| c.id.starts_with("tauri:")));
        // "cra" não é um nome de servidor conhecido.
        assert!(!v.iter().any(|c| c.id == "npm:cra"));
    }

    #[test]
    fn cargo_gradle_flutter_and_lockfiles() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "Cargo.toml", "[package]\nname='x'\n");
        write(d.path(), "settings.gradle", "");
        write(d.path(), "app/build.gradle", "");
        write(d.path(), "pubspec.yaml", "name: x\nflutter:\n  uses-material-design: true\n");
        fs::create_dir_all(d.path().join("web")).unwrap();
        let v = detect(d.path());
        assert_eq!(by_id(&v, "cargo:build-release").args, ["build", "--release"]);
        assert_eq!(by_id(&v, "cargo:run").kind, RunKind::Servidor);
        assert_eq!(by_id(&v, "gradle:assembleRelease").label, "gradle assembleRelease");
        assert_eq!(by_id(&v, "flutter:build-apk").kind, RunKind::Build);
        by_id(&v, "flutter:build-web");
        assert!(!v.iter().any(|c| c.id == "flutter:build-windows"));

        for (lock, pm) in [("yarn.lock", "yarn"), ("bun.lock", "bun")] {
            let d = tempfile::tempdir().unwrap();
            write(d.path(), lock, "");
            assert_eq!(package_manager(d.path()), pm);
        }
        assert_eq!(package_manager(tempfile::tempdir().unwrap().path()), "npm");
        assert!(detect(tempfile::tempdir().unwrap().path()).is_empty());
    }

    #[test]
    fn tauri_conf_without_scripts() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "src-tauri/tauri.conf.json", r#"{"build":{"devPath":"http://127.0.0.1:8080/"}}"#);
        let v = detect(d.path());
        assert_eq!(by_id(&v, "tauri:dev").port, Some(8080));
        assert_eq!(by_id(&v, "tauri:build").args, ["tauri", "build"]);
    }

    #[test]
    fn ports_in_scripts() {
        assert_eq!(explicit_port("vite --port 5000"), Some(5000));
        assert_eq!(explicit_port("vite --port=5001 --host"), Some(5001));
        assert_eq!(explicit_port("PORT=8081 node server.js"), Some(8081));
        assert_eq!(explicit_port("vite"), None);
        assert_eq!(framework_port("vite build"), None);
        assert_eq!(framework_port("ng serve").unwrap().0, 4200);
    }

    #[test]
    fn recognizes_process_cmdlines() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "Cargo.toml", "[package]\nname='x'\n");
        write(d.path(), "package.json", r#"{"scripts":{"build":"vite build","dev":"vite"}}"#);
        let v = detect(d.path());
        let id = |s: &str| match_cmdline(s, &v).map(|c| c.id.clone());
        assert_eq!(id(r#""C:\Program Files\nodejs\node.exe" "C:\npm\node_modules\npm\bin\npm-cli.js" run build"#).as_deref(), Some("npm:build"));
        assert_eq!(id(r"C:\Users\x\.cargo\bin\cargo.exe build --release -p argus").as_deref(), Some("cargo:build-release"));
        assert_eq!(id("cargo build -p argus").as_deref(), Some("cargo:build"));
        assert_eq!(id(r#"powershell -Command "cargo build""#).as_deref(), Some("cargo:build"));
        assert_eq!(id("cargo test"), None);
        assert_eq!(id("node vite.js build"), None);
    }

    #[test]
    fn finds_new_artifacts_only() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "target/release/bundle/nsis/App_1.0_x64-setup.exe", "12345");
        write(d.path(), "dist/index.html", "abc");
        write(d.path(), "dist/assets/a.js", "abcdef");
        let hints = tauri_artifacts();
        let a = find_artifact(d.path(), &hints, 0).unwrap();
        assert!(a.path.ends_with("App_1.0_x64-setup.exe"));
        assert_eq!((a.size, a.is_dir), (5, false));
        assert!(artifact_dir(&a).ends_with("nsis"));
        // Nada modificado depois do início: sem artefato.
        assert!(find_artifact(d.path(), &hints, now_secs() + 60).is_none());
        let dist = find_artifact(d.path(), &[folder("dist")], 0).unwrap();
        assert_eq!((dist.size, dist.is_dir), (9, true));
        assert!(find_artifact(d.path(), &[folder("nao-existe")], 0).is_none());
    }

    #[test]
    fn ports_free_and_busy() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        assert!(port_open(p));
        assert!(port_busy(p));
        let free = nearest_free(p).unwrap();
        assert!(free > p);
        drop(l);
        assert!(inside(Path::new(r"C:\a\proj"), Path::new(r"c:\a\proj\src")));
        assert!(!inside(Path::new(r"C:\a\proj"), Path::new(r"C:\a\project2")));
    }
}
