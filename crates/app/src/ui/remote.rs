//! Tela "Remoto": liga/desliga o servidor web para acompanhar e responder
//! sessões pelo celular (pareamento por QR code com token revogável).
use crate::state::use_orchestrator;
use dioxus::prelude::*;
use argus_core::remote::{RemoteInfo, RemoteServer};
use std::sync::Arc;

/// Servidor remoto compartilhado (contexto).
#[derive(Clone)]
pub struct Remote(pub Arc<RemoteServer>);

#[component]
pub fn RemotePanel() -> Element {
    let remote = use_context::<Remote>();
    let orch = use_orchestrator();
    let mut info = use_signal(|| remote.0.info().ok());
    let mut error = use_signal(|| None::<String>);
    let mut port = use_signal(|| "8787".to_string());
    let mut busy = use_signal(|| false);

    let start = {
        let remote = remote.clone();
        move |_| {
            let p: u16 = match port().trim().parse() {
                Ok(p) => p,
                Err(_) => { error.set(Some("Porta inválida.".into())); return; }
            };
            let r = remote.0.clone();
            let o = orch.clone();
            busy.set(true);
            spawn(async move {
                match r.start(o, p).await {
                    Ok(i) => { info.set(Some(i)); error.set(None); }
                    Err(e) => error.set(Some(format!("Falha ao iniciar: {e}"))),
                }
                busy.set(false);
            });
        }
    };
    let stop = {
        let r = remote.0.clone();
        move |_| { r.stop(); info.set(None); }
    };
    let regen = {
        let r = remote.0.clone();
        move |_| match r.regenerate_token() {
            Ok(i) => { info.set(Some(i)); error.set(None); }
            Err(e) => error.set(Some(e.to_string())),
        }
    };

    let pick_host = {
        let r = remote.0.clone();
        move |e: Event<FormData>| {
            let v = e.value();
            let h = if v.is_empty() { None } else { Some(v) };
            match r.set_host(h) {
                Ok(i) => { info.set(Some(i)); error.set(None); }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };

    let cur: Option<RemoteInfo> = info();
    rsx! {
        div { class: "pad remote",
            p { class: "small muted",
                "Abre um servidor na rede local para ver as sessões e responder aos agentes pelo celular. "
                "Leia o QR code com a câmera. O tráfego não é criptografado: use só em redes confiáveis "
                "(ou via VPN como Tailscale)."
            }
            if let Some(i) = cur {
                p { class: "ok", "Servidor ligado na porta {i.port}" }
                div { class: "qr", dangerous_inner_html: "{i.qr_svg}" }
                p { class: "small", style: "word-break:break-all", "{i.url}" }
                if i.hosts.len() > 1 {
                    div { class: "row wrap",
                        label { class: "small", "Endereço " }
                        select { onchange: pick_host,
                            for h in i.hosts.iter() {
                                option { value: "{h.ip}", selected: h.ip == i.host, "{h.ip} — {h.iface} ({h.kind.label()})" }
                            }
                        }
                    }
                    p { class: "small muted", "Se o celular não abrir, escolha o IP da mesma rede Wi-Fi do celular. IPs VPN/virtuais só funcionam com o celular na mesma VPN." }
                }
                p { class: "small warn", "Firewall do Windows: libere a porta {i.port} (TCP) para redes privadas, ou aceite o aviso do Firewall. Em rede marcada como Pública o acesso costuma ser bloqueado." }
                div { class: "row",
                    button { onclick: stop, "Desligar" }
                    button { title: "Revoga o acesso de todos os aparelhos pareados", onclick: regen, "Gerar novo token" }
                }
            } else {
                div { class: "row",
                    label { class: "small", "Porta " }
                    input { value: "{port}", size: 6, oninput: move |e| port.set(e.value()) }
                    button { class: "primary", disabled: busy(), onclick: start, if busy() { "Ligando..." } else { "Ligar servidor" } }
                }
                p { class: "small warn", "O Firewall do Windows pode pedir permissão ao ligar: permita em redes privadas. Se o celular não conectar, verifique se a rede Wi-Fi está como Privada." }
            }
            if let Some(e) = error() { p { class: "err", "{e}" } }
        }
    }
}
