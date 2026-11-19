/**
 * Aba "Celular": o painel do Screen-Mirror ao lado da conversa, para acompanhar
 * o aparelho enquanto o agente trabalha. A área Dispositivos continua sendo o
 * lugar de gerenciar aparelhos, APKs e a pasta do Screen-Mirror; aqui a faixa
 * é enxuta e só mostra o essencial.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage, type DeviceList, type MirrorStatus } from "@/api";
import { Icon } from "@/shell/icons";
import { useLayout } from "@/store";
import { missing, STATE_LABEL } from "@/views/devices/status";
// Reaproveita as fichas de aparelho, o quadro de erro e o iframe da área Dispositivos.
import "@/views/devices/devices.css";
import "./mirror.css";

export default function MirrorTab() {
  const [status, setStatus] = useState<MirrorStatus | null>(null);
  const [list, setList] = useState<DeviceList | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [frameKey, setFrameKey] = useState(0);
  const autoStarted = useRef(false);

  const start = useCallback(async () => {
    setBusy(true);
    try {
      setStatus(await api.devicesStart());
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
      setStatus(await api.devicesStatus().catch(() => null));
    } finally {
      setBusy(false);
    }
  }, []);

  // Ao abrir a aba: lê o estado e, se tudo estiver pronto, inicia uma vez sozinho.
  useEffect(() => {
    let alive = true;
    api.devicesStatus().then(
      (s) => {
        if (!alive) return;
        setStatus(s);
        if (!s.url && !missing(s) && !autoStarted.current) {
          autoStarted.current = true;
          void start();
        }
      },
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [start]);

  // Aparelhos: atualiza a cada 3 s enquanto o servidor estiver no ar.
  const url = status?.url ?? null;
  useEffect(() => {
    if (!url) {
      setList(null);
      return;
    }
    let alive = true;
    const tick = () =>
      api.devicesList().then(
        (l) => alive && setList(l),
        () => alive && api.devicesStatus().then((s) => alive && setStatus(s), () => {}),
      );
    void tick();
    const id = window.setInterval(tick, 3000);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, [url]);

  const problem = status ? missing(status) : null;
  const devices = list?.devices ?? [];

  return (
    <div className="mr">
      <div className="mr-bar">
        <Icon name="phone" size={16} />
        {devices.length > 0 ? (
          devices.map((d) => (
            <span key={d.serial} className={`dv-chip ${d.state === "device" ? "ok" : "warn"}`} title={d.serial}>
              {d.model ?? d.serial}
              {d.connection === "wifi" ? " · Wi-Fi" : ""}
              {d.state !== "device" ? ` · ${STATE_LABEL[d.state] ?? d.state}` : ""}
            </span>
          ))
        ) : (
          <span className="small muted">{url ? "Nenhum celular conectado por ADB." : busy ? "iniciando…" : "parado"}</span>
        )}
        <span className="mr-spacer" />
        {url && (
          <button title="Recarregar o painel" onClick={() => setFrameKey((k) => k + 1)}>
            <Icon name="refresh" size={14} />
          </button>
        )}
        <button
          title="Abrir a área Dispositivos (APKs, pasta do Screen-Mirror, logcat e arquivos)"
          onClick={() => useLayout.getState().setArea("devices")}
        >
          <Icon name="devices" size={14} /> Dispositivos
        </button>
      </div>

      {error && <div className="dv-error">{error}</div>}

      {url ? (
        <iframe
          key={frameKey}
          className="dv-frame"
          src={url}
          title="Espelhamento do celular"
          allow="autoplay; clipboard-read; clipboard-write; fullscreen"
        />
      ) : (
        <div className="mr-empty">
          <Icon name="phone" size={28} />
          {problem ? (
            <>
              <p className="dv-problem">{problem}</p>
              <button onClick={() => useLayout.getState().setArea("devices")}>Resolver em Dispositivos</button>
            </>
          ) : (
            <>
              <p className="small muted">O Screen-Mirror está parado.</p>
              <button className="primary" onClick={() => void start()} disabled={busy || !status}>
                <Icon name="play" size={14} /> Iniciar
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
