//! Player de música compacto para a barra de status.
//!
//! Dois modos:
//! - **Sistema:** controla a sessão de mídia ativa do Windows (Spotify, YouTube no
//!   navegador etc.) via `GlobalSystemMediaTransportControlsSessionManager`.
//! - **Local:** toca arquivos de uma pasta escolhida com `rodio`, numa thread própria.
//!
//! Nenhuma falha gera panic: erros viram mensagens na interface.
use dioxus::prelude::*;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

// ---------------------------------------------------------------- mídia do sistema

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemMedia {
    pub title: String,
    pub artist: String,
    pub app: String,
    pub playing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaCmd {
    PlayPause,
    Next,
    Previous,
}

#[cfg(windows)]
mod smtc {
    use super::{MediaCmd, SystemMedia};
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };

    fn session() -> Result<Option<Session>, String> {
        let mgr = Manager::RequestAsync()
            .and_then(|op| op.get())
            .map_err(|e| format!("mídia do sistema indisponível: {e}"))?;
        Ok(mgr.GetCurrentSession().ok())
    }

    /// Lê a sessão de mídia atual (bloqueante; chame fora da thread de UI).
    pub fn current() -> Result<Option<SystemMedia>, String> {
        let Some(s) = session()? else { return Ok(None) };
        let mut out = SystemMedia {
            app: s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default(),
            ..Default::default()
        };
        if let Ok(props) = s.TryGetMediaPropertiesAsync().and_then(|op| op.get()) {
            out.title = props.Title().map(|h| h.to_string()).unwrap_or_default();
            out.artist = props.Artist().map(|h| h.to_string()).unwrap_or_default();
        }
        out.playing = s
            .GetPlaybackInfo()
            .and_then(|i| i.PlaybackStatus())
            .map(|st| st == Status::Playing)
            .unwrap_or(false);
        Ok(Some(out))
    }

    pub fn send(cmd: MediaCmd) -> Result<(), String> {
        let Some(s) = session()? else { return Err("nenhuma mídia tocando".into()) };
        let op = match cmd {
            MediaCmd::PlayPause => s.TryTogglePlayPauseAsync(),
            MediaCmd::Next => s.TrySkipNextAsync(),
            MediaCmd::Previous => s.TrySkipPreviousAsync(),
        };
        op.and_then(|o| o.get()).map(|_| ()).map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
mod smtc {
    use super::{MediaCmd, SystemMedia};
    pub fn current() -> Result<Option<SystemMedia>, String> {
        Err("controle de mídia do sistema só no Windows".into())
    }
    pub fn send(_: MediaCmd) -> Result<(), String> {
        current().map(|_| ())
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- arquivos locais

const AUDIO_EXT: &[&str] = &["mp3", "wav", "flac", "ogg", "m4a", "aac"];

/// Lista os arquivos de áudio de uma pasta (não recursivo), ordenados.
pub fn scan_folder(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| AUDIO_EXT.contains(&e.to_ascii_lowercase().as_str()))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

enum LocalCmd {
    Play(PathBuf),
    Toggle,
    Stop,
    Volume(f32),
}

#[derive(Clone, Debug, PartialEq)]
enum LocalEvent {
    Error(String),
    Finished,
}

/// Handle para a thread de áudio. `OutputStream` não é `Send`, por isso tudo vive lá.
#[derive(Clone)]
struct LocalAudio {
    tx: mpsc::Sender<LocalCmd>,
    events: std::sync::Arc<std::sync::Mutex<mpsc::Receiver<LocalEvent>>>,
}

impl LocalAudio {
    fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<LocalCmd>();
        let (etx, erx) = mpsc::channel::<LocalEvent>();
        std::thread::Builder::new()
            .name("argus-audio".into())
            .spawn(move || audio_thread(rx, etx))
            .ok();
        Self { tx, events: std::sync::Arc::new(std::sync::Mutex::new(erx)) }
    }
    fn send(&self, c: LocalCmd) {
        let _ = self.tx.send(c);
    }
    fn poll(&self) -> Vec<LocalEvent> {
        self.events.lock().map(|r| r.try_iter().collect()).unwrap_or_default()
    }
}

fn audio_thread(rx: mpsc::Receiver<LocalCmd>, etx: mpsc::Sender<LocalEvent>) {
    use rodio::{Decoder, OutputStream, Sink};
    let (_stream, handle) = match OutputStream::try_default() {
        Ok(v) => v,
        Err(e) => {
            let _ = etx.send(LocalEvent::Error(format!("sem saída de áudio: {e}")));
            // Continua drenando comandos para não travar quem envia.
            while rx.recv().is_ok() {}
            return;
        }
    };
    let mut sink: Option<Sink> = None;
    let mut volume = 0.7f32;
    let mut active = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(300)) {
            Ok(LocalCmd::Play(path)) => {
                if let Some(s) = sink.take() {
                    s.stop();
                }
                let res = std::fs::File::open(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|f| Decoder::new(std::io::BufReader::new(f)).map_err(|e| e.to_string()))
                    .and_then(|src| {
                        let s = Sink::try_new(&handle).map_err(|e| e.to_string())?;
                        s.set_volume(volume);
                        s.append(src);
                        Ok(s)
                    });
                match res {
                    Ok(s) => {
                        sink = Some(s);
                        active = true;
                    }
                    Err(e) => {
                        active = false;
                        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        let _ = etx.send(LocalEvent::Error(format!("não foi possível tocar {name}: {e}")));
                    }
                }
            }
            Ok(LocalCmd::Toggle) => {
                if let Some(s) = &sink {
                    if s.is_paused() { s.play() } else { s.pause() }
                }
            }
            Ok(LocalCmd::Stop) => {
                if let Some(s) = sink.take() {
                    s.stop();
                }
                active = false;
            }
            Ok(LocalCmd::Volume(v)) => {
                volume = v.clamp(0.0, 1.0);
                if let Some(s) = &sink {
                    s.set_volume(volume);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if active && sink.as_ref().map(|s| s.empty()).unwrap_or(false) {
            active = false;
            let _ = etx.send(LocalEvent::Finished);
        }
    }
}

// ---------------------------------------------------------------- componente

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    System,
    Local,
}

fn file_label(p: &std::path::Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

#[component]
pub fn MusicPlayer() -> Element {
    let mut mode = use_signal(|| Mode::System);
    let mut sys = use_signal(|| None::<SystemMedia>);
    let mut error = use_signal(|| None::<String>);

    let audio = use_hook(LocalAudio::spawn);
    let mut playlist = use_signal(Vec::<PathBuf>::new);
    let mut current = use_signal(|| None::<usize>);
    let mut paused = use_signal(|| false);
    let mut volume = use_signal(|| 70u32);
    let mut show_list = use_signal(|| false);

    // Atualização periódica da mídia do sistema.
    use_future(move || async move {
        loop {
            if *mode.peek() == Mode::System {
                match blocking(smtc::current).await {
                    Ok(m) => {
                        if *sys.peek() != m {
                            sys.set(m);
                        }
                        if error.peek().is_some() {
                            error.set(None);
                        }
                    }
                    Err(e) => error.set(Some(e)),
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    // Eventos da thread de áudio (fim de faixa avança a playlist).
    let a2 = audio.clone();
    use_future(move || {
        let a = a2.clone();
        async move {
            loop {
                for ev in a.poll() {
                    match ev {
                        LocalEvent::Error(e) => error.set(Some(e)),
                        LocalEvent::Finished => {
                            let len = playlist.peek().len();
                            let next = (*current.peek()).map(|i| i + 1).filter(|i| *i < len);
                            current.set(next);
                            if let Some(i) = next {
                                if let Some(p) = playlist.peek().get(i).cloned() {
                                    a.send(LocalCmd::Play(p));
                                }
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(400)).await;
            }
        }
    });

    let sys_cmd = move |cmd: MediaCmd| {
        spawn(async move {
            match blocking(move || smtc::send(cmd)).await {
                Ok(()) => {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    if let Ok(m) = blocking(smtc::current).await {
                        sys.set(m);
                    }
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let play_index = {
        let audio = audio.clone();
        move |i: usize| {
            if let Some(p) = playlist.peek().get(i).cloned() {
                audio.send(LocalCmd::Play(p));
                current.set(Some(i));
                paused.set(false);
                error.set(None);
            }
        }
    };

    let pick_folder = {
        let play_index = play_index.clone();
        move |_| {
            let mut play_index = play_index.clone();
            spawn(async move {
                let Some(dir) = rfd::AsyncFileDialog::new().set_title("Pasta de músicas").pick_folder().await else {
                    return;
                };
                let path = dir.path().to_path_buf();
                let list = tokio::task::spawn_blocking(move || scan_folder(&path)).await.unwrap_or_default();
                if list.is_empty() {
                    error.set(Some("nenhum arquivo de áudio na pasta".into()));
                    return;
                }
                playlist.set(list);
                play_index(0);
            });
        }
    };

    let body = match *mode.read() {
        Mode::System => {
            let m = sys.read().clone();
            let (title, sub, playing) = match &m {
                Some(m) => (
                    if m.title.is_empty() { "(sem título)".to_string() } else { m.title.clone() },
                    if m.artist.is_empty() { m.app.clone() } else { m.artist.clone() },
                    m.playing,
                ),
                None => ("Nada tocando".to_string(), "abra Spotify, YouTube…".to_string(), false),
            };
            let icon = if playing { "⏸" } else { "▶" };
            rsx! {
                button { class: "mp-btn", title: "Anterior", onclick: move |_| sys_cmd(MediaCmd::Previous), "⏮" }
                button { class: "mp-btn", title: "Tocar/pausar", onclick: move |_| sys_cmd(MediaCmd::PlayPause), "{icon}" }
                button { class: "mp-btn", title: "Próxima", onclick: move |_| sys_cmd(MediaCmd::Next), "⏭" }
                span { class: "mp-info", title: "{title} — {sub}",
                    span { class: "mp-title", "{title}" }
                    span { class: "mp-sub", "{sub}" }
                }
            }
        }
        Mode::Local => {
            let list = playlist.read().clone();
            let cur = *current.read();
            let title = cur.and_then(|i| list.get(i)).map(|p| file_label(p)).unwrap_or_else(|| "Nenhuma faixa".into());
            let icon = if cur.is_some() && !*paused.read() { "⏸" } else { "▶" };
            let len = list.len();
            let (a_toggle, a_vol) = (audio.clone(), audio.clone());
            let (mut prev_i, mut next_i, mut start_i, pick_i) =
                (play_index.clone(), play_index.clone(), play_index.clone(), play_index.clone());
            rsx! {
                button { class: "mp-btn", title: "Escolher pasta", onclick: pick_folder.clone(), "📁" }
                button { class: "mp-btn", title: "Anterior", disabled: len == 0,
                    onclick: move |_| { let i = cur.map(|i| i.saturating_sub(1)).unwrap_or(0); prev_i(i) }, "⏮" }
                button { class: "mp-btn", title: "Tocar/pausar", disabled: len == 0,
                    onclick: move |_| {
                        if cur.is_none() { start_i(0); return; }
                        a_toggle.send(LocalCmd::Toggle);
                        let p = *paused.peek();
                        paused.set(!p);
                    }, "{icon}" }
                button { class: "mp-btn", title: "Próxima", disabled: len == 0,
                    onclick: move |_| { let i = cur.map(|i| (i + 1) % len.max(1)).unwrap_or(0); next_i(i) }, "⏭" }
                span { class: "mp-info", title: "{title}",
                    span { class: "mp-title", "{title}" }
                    span { class: "mp-sub", if len > 0 { "{cur.map(|i| i + 1).unwrap_or(0)}/{len}" } else { "escolha uma pasta" } }
                }
                input { r#type: "range", class: "mp-vol", min: "0", max: "100", value: "{volume}",
                    title: "Volume {volume}%", "aria-label": "Volume",
                    oninput: move |e| {
                        let v = e.value().parse::<u32>().unwrap_or(70).min(100);
                        volume.set(v);
                        a_vol.send(LocalCmd::Volume(v as f32 / 100.0));
                    }
                }
                button { class: "mp-btn", title: "Playlist", disabled: len == 0,
                    onclick: move |_| { let s = *show_list.peek(); show_list.set(!s) }, "☰" }
                if *show_list.read() && len > 0 {
                    ul { class: "mp-list", role: "listbox",
                        for (i, p) in list.iter().enumerate() {
                            {let mut pick_i = pick_i.clone(); rsx! { li {
                                key: "{i}",
                                class: if cur == Some(i) { "active" } else { "" },
                                onclick: move |_| { pick_i(i); show_list.set(false) },
                                {file_label(p)}
                            } }}
                        }
                    }
                }
            }
        }
    };

    let a_stop = audio.clone();
    let is_sys = *mode.read() == Mode::System;
    rsx! {
        style { {PLAYER_CSS} }
        div { class: "music-player",
            button {
                class: "mp-btn mp-mode",
                title: if is_sys { "Mídia do sistema (clique para arquivos locais)" } else { "Arquivos locais (clique para mídia do sistema)" },
                onclick: move |_| {
                    if is_sys {
                        mode.set(Mode::Local);
                    } else {
                        a_stop.send(LocalCmd::Stop);
                        current.set(None);
                        mode.set(Mode::System);
                    }
                    error.set(None);
                },
                if is_sys { "🖥" } else { "🎵" }
            }
            {body}
            if let Some(e) = error.read().clone() {
                span { class: "mp-err", title: "{e}", "⚠" }
            }
        }
    }
}

const PLAYER_CSS: &str = r#"
.music-player{position:relative;display:flex;align-items:center;gap:4px;font-size:12px;min-width:0;max-width:420px}
.music-player .mp-btn{background:none;border:0;color:inherit;cursor:pointer;padding:2px 5px;border-radius:4px;line-height:1}
.music-player .mp-btn:hover:not(:disabled){background:rgba(127,127,127,.2)}
.music-player .mp-btn:disabled{opacity:.4;cursor:default}
.music-player .mp-info{display:flex;flex-direction:column;min-width:0;max-width:200px;line-height:1.15}
.music-player .mp-title,.music-player .mp-sub{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.music-player .mp-sub{opacity:.65;font-size:11px}
.music-player .mp-vol{width:70px}
.music-player .mp-err{color:#e5a50a;cursor:help}
.music-player .mp-list{position:absolute;bottom:100%;right:0;margin:0 0 4px;padding:4px 0;list-style:none;max-height:260px;width:280px;overflow:auto;background:#1e1e1e;color:#ddd;border:1px solid #444;border-radius:6px;z-index:50}
.music-player .mp-list li{padding:3px 10px;cursor:pointer;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.music-player .mp-list li:hover{background:#2a2d2e}
.music-player .mp-list li.active{color:#4fc1ff}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_filters_audio() {
        let d = std::env::temp_dir().join(format!("hp-scan-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        for f in ["b.MP3", "a.flac", "x.txt"] {
            std::fs::write(d.join(f), b"").unwrap();
        }
        let v = scan_folder(&d);
        let names: Vec<_> = v.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["a.flac", "b.MP3"]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
