/**
 * Player de música compacto (status bar, à esquerda). Dois modos:
 * - Sistema: controla a mídia que estiver tocando no Windows (qualquer app que publique mídia).
 * - Local: toca os arquivos de áudio de uma pasta escolhida, com volume e playlist.
 *
 * Nenhuma falha derruba a interface: erros viram o aviso ao lado dos controles.
 */
import { useEffect, useRef, useState, type ReactNode } from "react";
import { api, baseName, errorMessage, onMediaLocal, type MediaCmd, type Path, type SystemMedia } from "@/api";
import "./player.css";

type Mode = "system" | "local";

const SYSTEM_POLL_MS = 2000;

/** Ícones (viewBox 0 0 24 24, preenchidos com a cor do texto). */
const ICON = {
  prev: "M6 5h2v14H6zM20 5v14L9 12z",
  next: "M16 5h2v14h-2zM4 5v14l11-7z",
  play: "M7 4v16l13-8z",
  pause: "M6 5h4v14H6zM14 5h4v14h-4z",
  folder: "M3 5h7l2 2h9v12H3z",
  list: "M4 6h16v2H4zM4 11h16v2H4zM4 16h16v2H4z",
  system: "M3 4h18v12H3zM9 18h6v2H9z",
  note: "M9 4h10v3h-7v10.5a3.5 3.5 0 11-3-3.46z",
  alert: "M12 3l10 18H2zM11 10v5h2v-5zM11 17v2h2v-2z",
} as const;

function Icon({ d }: { d: string }) {
  return (
    <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true" fill="currentColor" fillRule="evenodd">
      <path d={d} />
    </svg>
  );
}

function Btn(props: { label: string; icon: string; onClick: () => void; disabled?: boolean; pressed?: boolean }) {
  return (
    <button
      className="mp-btn"
      title={props.label}
      aria-label={props.label}
      aria-pressed={props.pressed}
      disabled={props.disabled}
      onClick={props.onClick}
    >
      <Icon d={props.icon} />
    </button>
  );
}

/** Nome da faixa: o arquivo sem a extensão. */
function fileLabel(path: Path): string {
  const name = baseName(path);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

function Info({ title, sub }: { title: string; sub: string }) {
  return (
    <span className="mp-info" title={sub ? `${title} — ${sub}` : title}>
      <span className="mp-title">{title}</span>
      {sub && <span className="mp-sub">{sub}</span>}
    </span>
  );
}

export default function PlayerView() {
  const [mode, setMode] = useState<Mode>("system");
  const [sys, setSys] = useState<SystemMedia | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [playlist, setPlaylist] = useState<Path[]>([]);
  const [current, setCurrent] = useState<number | null>(null);
  const [paused, setPaused] = useState(false);
  const [volume, setVolume] = useState(70);
  const [showList, setShowList] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  // O ouvinte de fim de faixa é registrado uma vez; lê o estado atual por aqui.
  const live = useRef({ playlist, current });
  live.current = { playlist, current };

  const fail = (e: unknown) => setError(errorMessage(e));

  // Mídia do sistema: consulta periódica enquanto o modo estiver ativo e a janela visível.
  useEffect(() => {
    if (mode !== "system") return;
    let alive = true;
    const poll = () => {
      if (document.hidden) return;
      api.mediaSystemCurrent().then(
        (m) => {
          if (!alive) return;
          setSys(m);
          setError(null);
        },
        (e) => alive && fail(e),
      );
    };
    poll();
    const timer = setInterval(poll, SYSTEM_POLL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [mode]);

  const playIndex = (i: number, list: Path[] = live.current.playlist) => {
    const path = list[i];
    if (path == null) return;
    setCurrent(i);
    setPaused(false);
    setError(null);
    api.mediaLocalPlay(path).catch(fail);
  };

  // Fim de faixa avança a playlist; erros do áudio aparecem no aviso.
  useEffect(() => {
    const off = onMediaLocal((ev) => {
      if (ev.type === "Error") {
        setError(ev.message);
        return;
      }
      const { playlist: list, current: cur } = live.current;
      const next = cur == null ? null : cur + 1;
      if (next != null && next < list.length) playIndex(next, list);
      else setCurrent(null);
    });
    return () => void off.then((f) => f());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Playlist aberta: fecha com Esc ou clique fora.
  useEffect(() => {
    if (!showList) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setShowList(false);
    const onDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setShowList(false);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousedown", onDown);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", onDown);
    };
  }, [showList]);

  const sysCmd = (cmd: MediaCmd) =>
    api.mediaSystemCommand(cmd).then((m) => {
      setSys(m);
      setError(null);
    }, fail);

  const pickFolder = async () => {
    try {
      const folder = await api.mediaPickFolder();
      if (!folder) return;
      if (folder.files.length === 0) {
        setError("Nenhum arquivo de áudio na pasta.");
        return;
      }
      setPlaylist(folder.files);
      playIndex(0, folder.files);
    } catch (e) {
      fail(e);
    }
  };

  const toggleLocal = () => {
    if (current == null) {
      playIndex(0);
      return;
    }
    setPaused((p) => !p);
    api.mediaLocalToggle().catch(fail);
  };

  const changeVolume = (v: number) => {
    const vol = Math.min(100, Math.max(0, Math.round(v)));
    setVolume(vol);
    api.mediaLocalVolume(vol / 100).catch(fail);
  };

  const switchMode = () => {
    if (mode === "system") {
      setMode("local");
    } else {
      api.mediaLocalStop().catch(fail);
      setCurrent(null);
      setShowList(false);
      setMode("system");
    }
    setError(null);
  };

  let body: ReactNode;
  if (mode === "system") {
    const title = sys ? sys.title || "(sem título)" : "Nada tocando";
    const sub = sys ? sys.artist || sys.app : "abra um app de música ou vídeo";
    body = (
      <>
        <Btn label="Anterior" icon={ICON.prev} onClick={() => void sysCmd("Previous")} />
        <Btn
          label={sys?.playing ? "Pausar" : "Tocar"}
          icon={sys?.playing ? ICON.pause : ICON.play}
          onClick={() => void sysCmd("PlayPause")}
        />
        <Btn label="Próxima" icon={ICON.next} onClick={() => void sysCmd("Next")} />
        <Info title={title} sub={sub} />
      </>
    );
  } else {
    const len = playlist.length;
    const playing = current != null && !paused;
    const title = current != null && playlist[current] ? fileLabel(playlist[current]) : "Nenhuma faixa";
    body = (
      <>
        <Btn label="Escolher pasta de músicas" icon={ICON.folder} onClick={() => void pickFolder()} />
        <Btn
          label="Anterior"
          icon={ICON.prev}
          disabled={len === 0}
          onClick={() => playIndex(current == null ? 0 : Math.max(0, current - 1))}
        />
        <Btn
          label={playing ? "Pausar" : "Tocar"}
          icon={playing ? ICON.pause : ICON.play}
          disabled={len === 0}
          onClick={toggleLocal}
        />
        <Btn
          label="Próxima"
          icon={ICON.next}
          disabled={len === 0}
          onClick={() => playIndex(current == null ? 0 : (current + 1) % len)}
        />
        <Info title={title} sub={len > 0 ? `${current == null ? 0 : current + 1}/${len}` : "escolha uma pasta"} />
        <input
          type="range"
          className="mp-vol"
          min={0}
          max={100}
          value={volume}
          title={`Volume ${volume}%`}
          aria-label="Volume"
          onChange={(e) => changeVolume(Number(e.target.value))}
        />
        <Btn
          label="Playlist"
          icon={ICON.list}
          disabled={len === 0}
          pressed={showList}
          onClick={() => setShowList((s) => !s)}
        />
        {showList && len > 0 && (
          <ul className="mp-list" role="listbox" aria-label="Playlist">
            {playlist.map((p, i) => (
              <li
                key={p}
                role="option"
                tabIndex={0}
                aria-selected={current === i}
                className={current === i ? "active" : ""}
                title={p}
                onClick={() => {
                  playIndex(i);
                  setShowList(false);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    playIndex(i);
                    setShowList(false);
                  }
                }}
              >
                {fileLabel(p)}
              </li>
            ))}
          </ul>
        )}
      </>
    );
  }

  return (
    <div className="music-player" ref={rootRef}>
      <Btn
        label={
          mode === "system"
            ? "Mídia do sistema (clique para arquivos locais)"
            : "Arquivos locais (clique para mídia do sistema)"
        }
        icon={mode === "system" ? ICON.system : ICON.note}
        onClick={switchMode}
      />
      {body}
      {error && (
        <span className="mp-err" title={error} role="img" aria-label={`Aviso: ${error}`}>
          <Icon d={ICON.alert} />
        </span>
      )}
    </div>
  );
}
