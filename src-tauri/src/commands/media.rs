//! Player de música.
//!
//! Dois modos:
//! - **Sistema:** controla a sessão de mídia ativa do Windows (qualquer app que
//!   publique mídia) via `GlobalSystemMediaTransportControlsSessionManager`.
//! - **Local:** toca arquivos de uma pasta com `rodio`, numa thread própria.
//!   Fim de faixa e erros chegam no evento `media-local`.
//!
//! Nenhuma falha gera panic: erros viram mensagens para a interface.
use super::blocking;
use crate::events::MEDIA_LOCAL;
use crate::state::AppCtx;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

// ---------------------------------------------------------------- mídia do sistema

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SystemMedia {
    pub title: String,
    pub artist: String,
    /// Identificador do app que está tocando.
    pub app: String,
    pub playing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum MediaCmd {
    PlayPause,
    Next,
    Previous,
}

/// Teto para a capa do álbum: acima disso a imagem não vale o custo de trafegar em base64.
const MAX_ART_BYTES: u64 = 4 * 1024 * 1024;

#[cfg(windows)]
mod smtc {
    use super::{MediaCmd, SystemMedia, MAX_ART_BYTES};
    use base64::Engine;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Storage::Streams::DataReader;

    fn session() -> Result<Option<Session>, String> {
        let mgr = Manager::RequestAsync()
            .and_then(|op| op.get())
            .map_err(|e| format!("mídia do sistema indisponível: {e}"))?;
        Ok(mgr.GetCurrentSession().ok())
    }

    /// Lê a sessão de mídia atual (bloqueante).
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

    /// Capa do álbum da sessão atual, como data URL. `None` quando o app não publica miniatura.
    ///
    /// Fica fora de `current` de propósito: a interface consulta a mídia a cada 2 s e a capa só
    /// quando a faixa muda, para não trafegar a imagem inteira em toda consulta.
    pub fn art() -> Result<Option<String>, String> {
        let Some(s) = session()? else { return Ok(None) };
        let Ok(props) = s.TryGetMediaPropertiesAsync().and_then(|op| op.get()) else { return Ok(None) };
        let Ok(stream) = props.Thumbnail().and_then(|t| t.OpenReadAsync()).and_then(|op| op.get()) else {
            return Ok(None);
        };
        let size = stream.Size().unwrap_or(0);
        if size == 0 || size > MAX_ART_BYTES {
            return Ok(None);
        }
        let mime = stream.ContentType().map(|h| h.to_string()).unwrap_or_default();
        let mime = if mime.starts_with("image/") { mime } else { "image/jpeg".into() };
        let reader = DataReader::CreateDataReader(&stream).map_err(|e| e.to_string())?;
        let read = reader
            .LoadAsync(size as u32)
            .and_then(|op| op.get())
            .map_err(|e| format!("falha ao ler a capa: {e}"))?;
        let mut bytes = vec![0u8; read as usize];
        reader.ReadBytes(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.is_empty() {
            return Ok(None);
        }
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(Some(format!("data:{mime};base64,{b64}")))
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
    pub fn art() -> Result<Option<String>, String> {
        Ok(None)
    }
    pub fn send(_: MediaCmd) -> Result<(), String> {
        current().map(|_| ())
    }
}

// ---------------------------------------------------------------- arquivos locais

const AUDIO_EXT: &[&str] = &["mp3", "wav", "flac", "ogg", "m4a", "aac"];

/// Lista os arquivos de áudio de uma pasta (não recursivo), ordenados.
pub fn scan_folder(dir: &Path) -> Vec<PathBuf> {
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

/// Payload do evento `media-local`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum LocalEvent {
    Error { message: String },
    /// A faixa terminou (a interface decide a próxima).
    Finished,
}

/// Handle para a thread de áudio. `OutputStream` não é `Send`, por isso tudo vive lá.
pub struct LocalAudio {
    tx: mpsc::Sender<LocalCmd>,
}

impl LocalAudio {
    fn spawn(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<LocalCmd>();
        std::thread::Builder::new()
            .name("argus-audio".into())
            .spawn(move || {
                audio_thread(rx, move |ev| {
                    let _ = app.emit(MEDIA_LOCAL, ev);
                })
            })
            .ok();
        Self { tx }
    }
    fn send(&self, c: LocalCmd) {
        let _ = self.tx.send(c);
    }
}

fn audio_thread(rx: mpsc::Receiver<LocalCmd>, emit: impl Fn(LocalEvent)) {
    use rodio::{Decoder, OutputStream, Sink};
    let (_stream, handle) = match OutputStream::try_default() {
        Ok(v) => v,
        Err(e) => {
            let message = format!("sem saída de áudio: {e}");
            emit(LocalEvent::Error { message: message.clone() });
            // Continua drenando comandos para não travar quem envia.
            while let Ok(cmd) = rx.recv() {
                if matches!(cmd, LocalCmd::Play(_)) {
                    emit(LocalEvent::Error { message: message.clone() });
                }
            }
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
                        emit(LocalEvent::Error { message: format!("não foi possível tocar {name}: {e}") });
                    }
                }
            }
            Ok(LocalCmd::Toggle) => {
                if let Some(s) = &sink {
                    if s.is_paused() {
                        s.play()
                    } else {
                        s.pause()
                    }
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
            emit(LocalEvent::Finished);
        }
    }
}

/// Thread de áudio, criada no primeiro uso.
fn audio<'a>(app: &AppHandle, ctx: &'a AppCtx) -> &'a LocalAudio {
    ctx.audio.get_or_init(|| LocalAudio::spawn(app.clone()))
}

#[derive(Debug, Clone, Serialize)]
pub struct MusicFolder {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
}

// ---------------------------------------------------------------- comandos

/// Mídia que está tocando no sistema (None se nada estiver tocando).
#[tauri::command]
pub async fn media_system_current() -> Result<Option<SystemMedia>, String> {
    blocking(smtc::current).await
}

/// Capa do álbum da mídia do sistema, como data URL (None quando o app não publica a imagem).
#[tauri::command]
pub async fn media_system_art() -> Result<Option<String>, String> {
    blocking(smtc::art).await
}

/// Envia tocar/pausar, próxima ou anterior para a mídia do sistema. Devolve o estado novo.
#[tauri::command]
pub async fn media_system_command(cmd: MediaCmd) -> Result<Option<SystemMedia>, String> {
    blocking(move || smtc::send(cmd)).await?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    blocking(smtc::current).await
}

/// Abre o seletor de pasta de músicas e lista os áudios dela (None se cancelar).
#[tauri::command]
pub async fn media_pick_folder(app: AppHandle) -> Result<Option<MusicFolder>, String> {
    blocking(move || {
        let Some(dir) = app.dialog().file().set_title("Pasta de músicas").blocking_pick_folder().and_then(|f| f.into_path().ok())
        else {
            return Ok(None);
        };
        let files = scan_folder(&dir);
        Ok(Some(MusicFolder { dir, files }))
    })
    .await
}

/// Lista os arquivos de áudio de uma pasta (mp3, wav, flac, ogg, m4a, aac).
#[tauri::command]
pub async fn media_scan_folder(dir: PathBuf) -> Result<Vec<PathBuf>, String> {
    blocking(move || Ok(scan_folder(&dir))).await
}

/// Toca um arquivo local (substitui a faixa atual).
#[tauri::command]
pub async fn media_local_play(app: AppHandle, ctx: State<'_, AppCtx>, path: PathBuf) -> Result<(), String> {
    audio(&app, &ctx).send(LocalCmd::Play(path));
    Ok(())
}

/// Alterna tocar/pausar da faixa local.
#[tauri::command]
pub async fn media_local_toggle(app: AppHandle, ctx: State<'_, AppCtx>) -> Result<(), String> {
    audio(&app, &ctx).send(LocalCmd::Toggle);
    Ok(())
}

/// Para a faixa local.
#[tauri::command]
pub async fn media_local_stop(app: AppHandle, ctx: State<'_, AppCtx>) -> Result<(), String> {
    audio(&app, &ctx).send(LocalCmd::Stop);
    Ok(())
}

/// Volume da faixa local, de 0.0 a 1.0.
#[tauri::command]
pub async fn media_local_volume(app: AppHandle, ctx: State<'_, AppCtx>, volume: f32) -> Result<(), String> {
    audio(&app, &ctx).send(LocalCmd::Volume(volume));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_filters_audio() {
        let d = tempfile::tempdir().unwrap();
        for f in ["b.MP3", "a.flac", "x.txt"] {
            std::fs::write(d.path().join(f), b"").unwrap();
        }
        let v = scan_folder(d.path());
        let names: Vec<_> = v.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["a.flac", "b.MP3"]);
    }

    #[test]
    fn local_event_json_shape() {
        assert_eq!(serde_json::to_string(&LocalEvent::Finished).unwrap(), r#"{"type":"Finished"}"#);
        assert_eq!(
            serde_json::to_string(&LocalEvent::Error { message: "x".into() }).unwrap(),
            r#"{"type":"Error","message":"x"}"#
        );
    }
}
