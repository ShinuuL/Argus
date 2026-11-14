//! Mascote que reflete o uso: agentes rodando, tokens/custo do dia e CPU.
//!
//! Desenho original em SVG inline (um bichinho redondo "Bolota"), animado com CSS.
//! Animações são desligadas com `prefers-reduced-motion`.
use dioxus::prelude::*;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetMood {
    /// Ocioso.
    Sleeping,
    /// Agentes rodando.
    Working,
    /// Muito uso de tokens ou CPU alta.
    Sweating,
    /// Tarefa concluída recentemente.
    Happy,
    /// Erro recente.
    Worried,
}

impl PetMood {
    pub fn class(self) -> &'static str {
        match self {
            PetMood::Sleeping => "sleeping",
            PetMood::Working => "working",
            PetMood::Sweating => "sweating",
            PetMood::Happy => "happy",
            PetMood::Worried => "worried",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            PetMood::Sleeping => "Zzz… tudo tranquilo",
            PetMood::Working => "Trabalhando!",
            PetMood::Sweating => "Ufa, que carga!",
            PetMood::Happy => "Tarefa concluída!",
            PetMood::Worried => "Algo deu errado…",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UsageSnapshot {
    pub running_agents: usize,
    pub tokens_today: u64,
    pub cost_today: f64,
    /// CPU global em % (0–100).
    pub cpu: f32,
    pub mem_used_mb: u64,
    pub mem_total_mb: u64,
    /// Tarefa concluiu há pouco (o chamador decide a janela).
    pub recent_success: bool,
    /// Houve erro há pouco.
    pub recent_error: bool,
}

/// Limiar de tokens/dia considerado "muito uso".
pub const HEAVY_TOKENS: u64 = 2_000_000;
pub const HEAVY_CPU: f32 = 85.0;

impl UsageSnapshot {
    /// Humor derivado: erro > suor > feliz > trabalhando > dormindo.
    pub fn mood(&self) -> PetMood {
        if self.recent_error {
            PetMood::Worried
        } else if self.cpu >= HEAVY_CPU || (self.running_agents > 0 && self.tokens_today >= HEAVY_TOKENS) {
            PetMood::Sweating
        } else if self.recent_success {
            PetMood::Happy
        } else if self.running_agents > 0 {
            PetMood::Working
        } else {
            PetMood::Sleeping
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SystemUsage {
    pub cpu: f32,
    pub mem_used_mb: u64,
    pub mem_total_mb: u64,
}

/// Coleta CPU/memória com `sysinfo` a cada 2 s, fora da thread de UI.
pub fn use_system_usage() -> Signal<SystemUsage> {
    let mut usage = use_signal(SystemUsage::default);
    use_future(move || async move {
        // sysinfo 0.40: System::new() devolve Result; sem coleta se falhar.
        let Ok(sys) = sysinfo::System::new() else { return };
        let sys = std::sync::Arc::new(std::sync::Mutex::new(sys));
        loop {
            let s = sys.clone();
            let r = tokio::task::spawn_blocking(move || {
                let mut sys = s.lock().ok()?;
                sys.refresh_cpu_usage();
                sys.refresh_memory();
                Some(SystemUsage {
                    cpu: sys.global_cpu_usage(),
                    mem_used_mb: sys.used_memory() / 1_048_576,
                    mem_total_mb: sys.total_memory() / 1_048_576,
                })
            })
            .await;
            if let Ok(Some(u)) = r {
                usage.set(u);
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
    usage
}

pub fn fmt_tokens(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.1}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

/// `usage`: dados de agentes/tokens; CPU e memória são preenchidos por
/// `use_system_usage()` quando `usage.cpu` vier 0. `mood` força um humor.
#[component]
pub fn Pet(usage: UsageSnapshot, #[props(default)] mood: Option<PetMood>) -> Element {
    let sysu = use_system_usage();
    let mut open = use_signal(|| false);
    let mut u = usage;
    if u.cpu == 0.0 {
        let s = *sysu.read();
        u.cpu = s.cpu;
        u.mem_used_mb = s.mem_used_mb;
        u.mem_total_mb = s.mem_total_mb;
    }
    let mood = mood.unwrap_or_else(|| u.mood());
    let cls = mood.class();
    let label = mood.label();
    let tip = format!(
        "{label}\nAgentes rodando: {}\nTokens hoje: {}\nCusto hoje: US$ {:.2}\nCPU: {:.0}%  Mem: {} / {} MB",
        u.running_agents,
        fmt_tokens(u.tokens_today),
        u.cost_today,
        u.cpu,
        u.mem_used_mb,
        u.mem_total_mb
    );
    let cpu_w = u.cpu.clamp(0.0, 100.0);

    rsx! {
        style { {PET_CSS} }
        div {
            class: "pet pet-{cls}",
            title: "{tip}",
            role: "button",
            tabindex: "0",
            "aria-label": "{tip}",
            onclick: move |_| { let o = *open.peek(); open.set(!o) },
            onmouseleave: move |_| open.set(false),
            svg { class: "pet-svg", view_box: "0 0 64 64", width: "22", height: "22", "aria-hidden": "true",
                ellipse { class: "pet-shadow", cx: "32", cy: "60", rx: "16", ry: "3" }
                g { class: "pet-body",
                    // orelhas
                    path { d: "M17 20 L14 7 L26 15 Z", class: "pet-fill" }
                    path { d: "M47 20 L50 7 L38 15 Z", class: "pet-fill" }
                    circle { cx: "32", cy: "34", r: "22", class: "pet-fill" }
                    ellipse { cx: "32", cy: "42", rx: "12", ry: "9", class: "pet-belly" }
                    // bochechas
                    circle { cx: "19", cy: "38", r: "3.2", class: "pet-cheek" }
                    circle { cx: "45", cy: "38", r: "3.2", class: "pet-cheek" }
                    {face(mood)}
                    if mood == PetMood::Sweating {
                        path { class: "pet-drop", d: "M50 16 q3 5 0 7 q-3 -2 0 -7 Z" }
                    }
                    if mood == PetMood::Working {
                        g { class: "pet-gear",
                            circle { cx: "52", cy: "14", r: "4", fill: "none", stroke_width: "2.5", stroke_dasharray: "2 1.6" }
                        }
                    }
                }
                if mood == PetMood::Sleeping {
                    text { class: "pet-z", x: "48", y: "14", "z" }
                }
                if mood == PetMood::Happy {
                    text { class: "pet-spark", x: "6", y: "14", "✦" }
                }
            }
            if *open.read() {
                div { class: "pet-bubble",
                    strong { "{label}" }
                    div { "Agentes: {u.running_agents}" }
                    div { "Tokens hoje: {fmt_tokens(u.tokens_today)}" }
                    div { "Custo hoje: US$ {u.cost_today:.2}" }
                    div { class: "pet-cpu",
                        "CPU {u.cpu:.0}%"
                        span { class: "bar", span { style: "width:{cpu_w}%" } }
                    }
                    div { "Mem: {u.mem_used_mb} / {u.mem_total_mb} MB" }
                }
            }
        }
    }
}

fn face(mood: PetMood) -> Element {
    match mood {
        PetMood::Sleeping => rsx! {
            path { class: "pet-line", d: "M22 31 q4 3 8 0" }
            path { class: "pet-line", d: "M34 31 q4 3 8 0" }
            path { class: "pet-line", d: "M30 41 q2 1 4 0" }
        },
        PetMood::Working => rsx! {
            circle { class: "pet-eye pet-blink", cx: "26", cy: "31", r: "2.6" }
            circle { class: "pet-eye pet-blink", cx: "38", cy: "31", r: "2.6" }
            path { class: "pet-line", d: "M29 41 h6" }
        },
        PetMood::Sweating => rsx! {
            path { class: "pet-line", d: "M22 29 l6 3 l-6 3" }
            path { class: "pet-line", d: "M42 29 l-6 3 l6 3" }
            ellipse { class: "pet-mouth", cx: "32", cy: "42", rx: "3.5", ry: "2.5" }
        },
        PetMood::Happy => rsx! {
            path { class: "pet-line", d: "M22 32 q4 -4 8 0" }
            path { class: "pet-line", d: "M34 32 q4 -4 8 0" }
            path { class: "pet-mouth", d: "M27 39 q5 6 10 0 Z" }
        },
        PetMood::Worried => rsx! {
            path { class: "pet-line", d: "M21 26 l6 -2" }
            path { class: "pet-line", d: "M43 26 l-6 -2" }
            circle { class: "pet-eye", cx: "26", cy: "31", r: "2.4" }
            circle { class: "pet-eye", cx: "38", cy: "31", r: "2.4" }
            path { class: "pet-line", d: "M28 43 q4 -4 8 0" }
        },
    }
}

const PET_CSS: &str = r#"
.pet{position:relative;display:inline-flex;align-items:center;cursor:pointer;--pet:#f2b36b;--belly:#fde3c0;--ink:#3a2a1f;--cheek:#f28b82}
.pet:focus-visible{outline:2px solid #4fc1ff;outline-offset:2px;border-radius:6px}
.pet-svg{overflow:visible;display:block}
.pet-fill{fill:var(--pet)}
.pet-belly{fill:var(--belly)}
.pet-cheek{fill:var(--cheek);opacity:.55}
.pet-eye{fill:var(--ink)}
.pet-line{fill:none;stroke:var(--ink);stroke-width:2.2;stroke-linecap:round;stroke-linejoin:round}
.pet-mouth{fill:var(--ink)}
.pet-shadow{fill:rgba(0,0,0,.25)}
.pet-drop{fill:#7cc7ff}
.pet-gear circle{stroke:#9aa4ad}
.pet-z{font:bold 10px sans-serif;fill:#9aa4ad}
.pet-spark{font-size:10px;fill:#ffd54f}
.pet-body{transform-origin:32px 58px}
.pet-worried{--pet:#c9b6a0}
.pet-sweating{--pet:#f59f6b}

.pet-sleeping .pet-body{animation:pet-breathe 3.2s ease-in-out infinite}
.pet-sleeping .pet-z{animation:pet-float 2.4s ease-in-out infinite}
.pet-working .pet-body{animation:pet-bob .7s ease-in-out infinite}
.pet-working .pet-gear{transform-origin:52px 14px;animation:pet-spin 1.6s linear infinite}
.pet-blink{transform-box:fill-box;transform-origin:center;animation:pet-blink 4s infinite}
.pet-sweating .pet-body{animation:pet-shake .25s linear infinite}
.pet-sweating .pet-drop{animation:pet-drip 1.2s ease-in infinite}
.pet-happy .pet-body{animation:pet-jump .9s cubic-bezier(.3,1.6,.5,1) infinite}
.pet-happy .pet-spark{animation:pet-float 1.2s ease-in-out infinite}
.pet-worried .pet-body{animation:pet-tilt 2s ease-in-out infinite}

@keyframes pet-breathe{50%{transform:scale(1.04,.96)}}
@keyframes pet-bob{50%{transform:translateY(-2px)}}
@keyframes pet-spin{to{transform:rotate(360deg)}}
@keyframes pet-blink{0%,92%,100%{transform:scaleY(1)}95%{transform:scaleY(.1)}}
@keyframes pet-shake{25%{transform:translateX(-1px)}75%{transform:translateX(1px)}}
@keyframes pet-drip{from{transform:translateY(0);opacity:1}to{transform:translateY(8px);opacity:0}}
@keyframes pet-jump{0%,100%{transform:translateY(0)}40%{transform:translateY(-6px)}}
@keyframes pet-float{0%,100%{transform:translateY(0);opacity:.6}50%{transform:translateY(-4px);opacity:1}}
@keyframes pet-tilt{50%{transform:rotate(-6deg)}}

.pet-bubble{position:absolute;bottom:calc(100% + 8px);right:0;min-width:180px;padding:8px 10px;font-size:12px;line-height:1.5;background:#252526;color:#ddd;border:1px solid #444;border-radius:8px;box-shadow:0 4px 14px rgba(0,0,0,.35);z-index:60;white-space:nowrap}
.pet-bubble::after{content:"";position:absolute;top:100%;right:12px;border:6px solid transparent;border-top-color:#444}
.pet-bubble strong{display:block;margin-bottom:2px}
.pet-cpu{display:flex;align-items:center;gap:6px}
.pet-cpu .bar{flex:1;height:5px;background:#3c3c3c;border-radius:3px;overflow:hidden}
.pet-cpu .bar span{display:block;height:100%;background:#4fc1ff}

@media (prefers-reduced-motion: reduce){
  .pet *, .pet *::before, .pet *::after{animation:none !important;transition:none !important}
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mood_priority() {
        let mut u = UsageSnapshot::default();
        assert_eq!(u.mood(), PetMood::Sleeping);
        u.running_agents = 1;
        assert_eq!(u.mood(), PetMood::Working);
        u.recent_success = true;
        assert_eq!(u.mood(), PetMood::Happy);
        u.cpu = 95.0;
        assert_eq!(u.mood(), PetMood::Sweating);
        u.recent_error = true;
        assert_eq!(u.mood(), PetMood::Worried);
    }
    #[test]
    fn tokens_fmt() {
        assert_eq!(fmt_tokens(950), "950");
        assert_eq!(fmt_tokens(12_300), "12.3k");
        assert_eq!(fmt_tokens(2_500_000), "2.5M");
    }
}
