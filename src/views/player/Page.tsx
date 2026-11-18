/**
 * Player em página inteira (Configurações › Extras › Player): capa do álbum,
 * faixa atual em destaque, controles grandes e a playlist sempre visível.
 * O estado vem do mesmo `usePlayer` do widget da status bar.
 */
import { baseName } from "@/api";
import { Btn, Icon, ICON } from "./icons";
import { fileLabel, usePlayer, type Mode } from "./usePlayer";
import "./player.css";

const MODES: { id: Mode; label: string; hint: string }[] = [
  { id: "system", label: "Mídia do sistema", hint: "Controla o que estiver tocando no Windows" },
  { id: "local", label: "Arquivos locais", hint: "Toca os áudios de uma pasta do computador" },
];

/** Capa do álbum, ou a arte vazia com a nota musical. */
function Cover({ src, alt }: { src: string | null; alt: string }) {
  if (src) return <img className="pp-cover" src={src} alt={`Capa de ${alt}`} />;
  return (
    <div className="pp-cover pp-cover-empty" role="img" aria-label="Sem capa">
      <svg viewBox="0 0 24 24" width="64" height="64" aria-hidden="true" fill="currentColor">
        <path d={ICON.note} />
      </svg>
    </div>
  );
}

export default function PlayerPage() {
  const p = usePlayer({ art: true });
  const system = p.mode === "system";
  const len = p.playlist.length;

  const title = system
    ? p.sys
      ? p.sys.title || "(sem título)"
      : "Nada tocando"
    : p.current != null && p.playlist[p.current]
      ? fileLabel(p.playlist[p.current])
      : "Nenhuma faixa";
  const artist = system
    ? p.sys
      ? p.sys.artist || "Artista desconhecido"
      : "Abra um app de música ou vídeo"
    : p.folder
      ? baseName(p.folder)
      : "Escolha uma pasta de músicas";
  const playing = system ? p.sys?.playing === true : p.localPlaying;

  return (
    <div className="player-page">
      <div className="pp-modes" role="radiogroup" aria-label="Fonte da música">
        {MODES.map((m) => (
          <button
            key={m.id}
            role="radio"
            aria-checked={p.mode === m.id}
            title={m.hint}
            className={p.mode === m.id ? "pp-mode on" : "pp-mode"}
            onClick={() => p.mode !== m.id && p.switchMode()}
          >
            {m.label}
          </button>
        ))}
      </div>

      <div className="pp-now">
        <Cover src={system ? p.art : null} alt={title} />
        <div className="pp-meta">
          <span className={playing ? "pp-state on" : "pp-state"}>
            {playing ? "Tocando agora" : system && p.sys ? "Pausado" : "Parado"}
          </span>
          <h3 className="pp-title" title={title}>
            {title}
          </h3>
          <p className="pp-artist" title={artist}>
            {artist}
          </p>
          {system && p.sys?.app && <p className="pp-app">via {p.sys.app}</p>}
          {!system && len > 0 && (
            <p className="pp-app">
              Faixa {p.current == null ? 0 : p.current + 1} de {len}
            </p>
          )}

          <div className="pp-controls">
            <Btn
              label="Anterior"
              icon={ICON.prev}
              size={20}
              className="pp-ctl"
              disabled={!system && len === 0}
              onClick={() => (system ? void p.sysCmd("Previous") : p.prevLocal())}
            />
            <Btn
              label={playing ? "Pausar" : "Tocar"}
              icon={playing ? ICON.pause : ICON.play}
              size={24}
              className="pp-ctl pp-ctl-main"
              disabled={!system && len === 0}
              onClick={() => (system ? void p.sysCmd("PlayPause") : p.toggleLocal())}
            />
            <Btn
              label="Próxima"
              icon={ICON.next}
              size={20}
              className="pp-ctl"
              disabled={!system && len === 0}
              onClick={() => (system ? void p.sysCmd("Next") : p.nextLocal())}
            />
            {!system && (
              <>
                <Btn
                  label="Escolher pasta de músicas"
                  icon={ICON.folder}
                  size={18}
                  className="pp-ctl"
                  onClick={() => void p.pickFolder()}
                />
                <input
                  type="range"
                  className="pp-vol"
                  min={0}
                  max={100}
                  value={p.volume}
                  title={`Volume ${p.volume}%`}
                  aria-label="Volume"
                  onChange={(e) => p.changeVolume(Number(e.target.value))}
                />
                <span className="pp-vol-num">{p.volume}%</span>
              </>
            )}
          </div>
        </div>
      </div>

      {p.error && (
        <p className="pp-err" role="alert">
          <Icon d={ICON.alert} /> {p.error}
        </p>
      )}

      {system ? (
        <p className="ui-mut ui-sm pp-note">
          A capa aparece quando o app que está tocando publica a imagem do álbum no Windows. Spotify, Groove e o
          YouTube no Edge costumam publicar; alguns players não.
        </p>
      ) : (
        <div className="pp-pl">
          <div className="pp-pl-head">
            <strong>Playlist</strong>
            <span className="ui-mut ui-sm">{p.folder ? `${len} faixa${len === 1 ? "" : "s"} · ${p.folder}` : "nenhuma pasta escolhida"}</span>
          </div>
          {len === 0 ? (
            <p className="ui-empty ui-sm">Escolha uma pasta para montar a playlist (mp3, wav, flac, ogg, m4a, aac).</p>
          ) : (
            <ul className="pp-tracks" role="listbox" aria-label="Playlist">
              {p.playlist.map((path, i) => (
                <li
                  key={path}
                  role="option"
                  tabIndex={0}
                  aria-selected={p.current === i}
                  className={p.current === i ? "active" : ""}
                  title={path}
                  onClick={() => p.playIndex(i)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      p.playIndex(i);
                    }
                  }}
                >
                  <span className="pp-num">{p.current === i ? <Icon d={playing ? ICON.pause : ICON.play} size={11} /> : i + 1}</span>
                  <span className="pp-track">{fileLabel(path)}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
