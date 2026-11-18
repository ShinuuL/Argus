/**
 * Player de música compacto (status bar, à esquerda). A lógica dos dois modos
 * (mídia do sistema e arquivos locais) mora em `usePlayer`; aqui fica só o
 * desenho que cabe na barra de 24px. A página inteira está em `./Page`.
 */
import { useEffect, useRef, useState, type ReactNode } from "react";
import { Btn, Icon, ICON } from "./icons";
import { fileLabel, usePlayer } from "./usePlayer";
import "./player.css";

function Info({ title, sub }: { title: string; sub: string }) {
  return (
    <span className="mp-info" title={sub ? `${title} — ${sub}` : title}>
      <span className="mp-title">{title}</span>
      {sub && <span className="mp-sub">{sub}</span>}
    </span>
  );
}

export default function PlayerView() {
  const p = usePlayer();
  const [showList, setShowList] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);

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

  const switchMode = () => {
    setShowList(false);
    p.switchMode();
  };

  let body: ReactNode;
  if (p.mode === "system") {
    const title = p.sys ? p.sys.title || "(sem título)" : "Nada tocando";
    const sub = p.sys ? p.sys.artist || p.sys.app : "abra um app de música ou vídeo";
    body = (
      <>
        <Btn label="Anterior" icon={ICON.prev} onClick={() => void p.sysCmd("Previous")} />
        <Btn
          label={p.sys?.playing ? "Pausar" : "Tocar"}
          icon={p.sys?.playing ? ICON.pause : ICON.play}
          onClick={() => void p.sysCmd("PlayPause")}
        />
        <Btn label="Próxima" icon={ICON.next} onClick={() => void p.sysCmd("Next")} />
        <Info title={title} sub={sub} />
      </>
    );
  } else {
    const len = p.playlist.length;
    const title = p.current != null && p.playlist[p.current] ? fileLabel(p.playlist[p.current]) : "Nenhuma faixa";
    body = (
      <>
        <Btn label="Escolher pasta de músicas" icon={ICON.folder} onClick={() => void p.pickFolder()} />
        <Btn label="Anterior" icon={ICON.prev} disabled={len === 0} onClick={p.prevLocal} />
        <Btn
          label={p.localPlaying ? "Pausar" : "Tocar"}
          icon={p.localPlaying ? ICON.pause : ICON.play}
          disabled={len === 0}
          onClick={p.toggleLocal}
        />
        <Btn label="Próxima" icon={ICON.next} disabled={len === 0} onClick={p.nextLocal} />
        <Info title={title} sub={len > 0 ? `${p.current == null ? 0 : p.current + 1}/${len}` : "escolha uma pasta"} />
        <input
          type="range"
          className="mp-vol"
          min={0}
          max={100}
          value={p.volume}
          title={`Volume ${p.volume}%`}
          aria-label="Volume"
          onChange={(e) => p.changeVolume(Number(e.target.value))}
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
            {p.playlist.map((path, i) => (
              <li
                key={path}
                role="option"
                tabIndex={0}
                aria-selected={p.current === i}
                className={p.current === i ? "active" : ""}
                title={path}
                onClick={() => {
                  p.playIndex(i);
                  setShowList(false);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    p.playIndex(i);
                    setShowList(false);
                  }
                }}
              >
                {fileLabel(path)}
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
          p.mode === "system"
            ? "Mídia do sistema (clique para arquivos locais)"
            : "Arquivos locais (clique para mídia do sistema)"
        }
        icon={p.mode === "system" ? ICON.system : ICON.note}
        onClick={switchMode}
      />
      {body}
      {p.error && (
        <span className="mp-err" title={p.error} role="img" aria-label={`Aviso: ${p.error}`}>
          <Icon d={ICON.alert} />
        </span>
      )}
    </div>
  );
}
