//! Detecção de servidores contra os processos reais da máquina (precisa de um
//! servidor no ar e de `ARGUS_PROJETO` com a pasta dele):
//! `ARGUS_PROJETO=D:\dev\meu-projeto cargo test -p argus-core --test servers_live -- --ignored --nocapture`.
//!
//! Serve para conferir o que só aparece fora dos testes com fonte falsa: se o
//! sistema informa o `cwd` dos processos (a adoção pela pasta depende disso) e
//! se o endereço público do túnel é lido da API local da ferramenta.
use argus_core::procs::{tracker, Owner, ProcSource, SystemSource};
use std::path::PathBuf;

/// O que o sistema conta sobre quem está escutando: sem `cwd` não há adoção pela pasta.
#[test]
#[ignore]
fn what_the_system_reports() {
    let src = SystemSource::default();
    let rows = src.processes();
    for (pid, port) in src.listening() {
        if let Some(r) = rows.iter().find(|r| r.pid == pid) {
            println!(":{port} pid {pid} {} · cwd {:?} · cmd {}", r.name, r.cwd, r.cmdline);
        } else {
            println!(":{port} pid {pid} (fora da tabela de processos)");
        }
    }
}

#[test]
#[ignore]
fn adopts_and_exposes_real_servers() {
    let projeto = PathBuf::from(std::env::var("ARGUS_PROJETO").expect("defina ARGUS_PROJETO com a pasta do projeto"));
    assert!(projeto.is_dir(), "{} não é uma pasta", projeto.display());

    let t = tracker();
    t.track_dir(Owner::Session(999), projeto.clone());
    let servers = t.poll();

    println!("pasta observada: {}", projeto.display());
    println!("servidores encontrados: {}", servers.len());
    for s in &servers {
        println!(
            "  :{} {} (pid {}) · {} · cwd {:?}",
            s.port,
            s.name,
            s.pid,
            s.source.label(),
            s.cwd.as_deref().map(|p| p.display().to_string())
        );
        if let Some(e) = &s.exposure {
            println!("     exposto pelo {} (pid {}) em {}", e.label, e.pid, e.url.clone().unwrap_or_else(|| "(endereço desconhecido)".into()));
        }
    }
    t.forget(Owner::Session(999));
    assert!(!servers.is_empty(), "nenhum servidor detectado na pasta (o teste precisa de um no ar)");
}
