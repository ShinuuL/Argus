import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage, type DeviceList, type MirrorStatus } from "@/api";
import { Icon } from "@/shell/icons";
import { useWorkbench } from "@/store";
import { ApkBar } from "./ApkBar";
import { missing, STATE_LABEL } from "./status";
import "./devices.css";

/**
 * Dispositivos: celulares Android pelo Screen-Mirror, que roda como processo
 * auxiliar do Argus. O painel do próprio Screen-Mirror é embutido num iframe
 * (espelhamento, logcat, arquivos, apps); a faixa de cima mostra o estado e os
 * aparelhos conectados.
 */
export default function DevicesView() {
  const [status, setStatus] = useState<MirrorStatus | null>(null);
  const [list, setList] = useState<DeviceList | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [dirInput, setDirInput] = useState("");
  const [frameKey, setFrameKey] = useState(0);
  const autoStarted = useRef(false);

  const run = useCallback(async (action: () => Promise<MirrorStatus>) => {
    setBusy(true);
    try {
      setStatus(await action());
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
        if (s.dir) setDirInput(s.dir);
        if (!s.url && !missing(s) && !autoStarted.current) {
          autoStarted.current = true;
          void run(api.devicesStart);
        }
      },
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [run]);

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

  const stop = () =>
    void run(async () => {
      await api.devicesStop();
      return api.devicesStatus();
    });

  const saveDir = () => {
    const d = dirInput.trim();
    if (d) void run(() => api.devicesSetDir(d));
  };

  const problem = status ? missing(status) : null;

  return (
    <div className="dv">
      <div className="dv-bar">
        <Icon name="devices" size={18} />
        <b>Dispositivos</b>
        {url ? (
          <span className="dv-chip ok" title={`PID ${status?.pid ?? "?"}`}>
            no ar em {url.replace(/^http:\/\//, "")}
          </span>
        ) : (
          <span className="dv-chip">{busy ? "iniciando…" : "parado"}</span>
        )}
        <div className="dv-devs">
          {list?.devices.map((d) => (
            <span key={d.serial} className={`dv-chip ${d.state === "device" ? "ok" : "warn"}`} title={d.serial}>
              {d.model ?? d.serial}
              {d.connection === "wifi" ? " · Wi-Fi" : ""}
              {d.state !== "device" ? ` · ${STATE_LABEL[d.state] ?? d.state}` : ""}
              {list.mirroring.includes(d.serial) ? " · espelhando" : ""}
            </span>
          ))}
          {list && list.devices.length === 0 && <span className="small muted">Nenhum celular conectado por ADB.</span>}
        </div>
        <span className="dv-spacer" />
        {status?.dir && (
          <label
            className="dv-toggle small"
            title="Claude e Codex recebem as ferramentas do Screen-Mirror (mcp__screen-mirror__*), a não ser que já o tenham configurado. A política do Screen-Mirror continua valendo. OpenCode e Gemini: registre o Screen-Mirror na configuração da própria CLI."
          >
            <input
              type="checkbox"
              checked={status.agentsMcp}
              disabled={busy}
              onChange={(e) => {
                const on = e.target.checked;
                void run(() => api.devicesSetAgents(on));
              }}
            />
            Agentes controlam o celular
          </label>
        )}
        {url ? (
          <>
            <button
              title="Abre o espelhamento como aba, ao lado da conversa"
              onClick={() => useWorkbench.getState().openMirror()}
            >
              <Icon name="phone" size={14} /> Abrir como aba
            </button>
            <button title="Recarregar o painel" onClick={() => setFrameKey((k) => k + 1)}>
              <Icon name="refresh" size={14} />
            </button>
            <button title="Abrir o painel no navegador" onClick={() => void api.systemOpenUrl(url)}>
              <Icon name="globe" size={14} />
            </button>
            <button onClick={stop} disabled={busy}>
              Parar
            </button>
          </>
        ) : (
          <button className="primary" onClick={() => void run(api.devicesStart)} disabled={busy || !status || !!problem}>
            <Icon name="play" size={14} /> Iniciar
          </button>
        )}
      </div>

      {url && <ApkBar devices={list?.devices ?? []} />}
      {error && <div className="dv-error">{error}</div>}

      {url ? (
        <iframe
          key={frameKey}
          className="dv-frame"
          src={url}
          title="Painel do Screen-Mirror"
          allow="autoplay; clipboard-read; clipboard-write; fullscreen"
        />
      ) : (
        <div className="page-col">
          {problem && <p className="dv-problem">{problem}</p>}
          <label className="dv-field">
            <span className="small muted">Pasta do projeto Screen-Mirror</span>
            <div className="dv-row">
              <input
                value={dirInput}
                placeholder="D:\…\Screen-Mirror"
                onChange={(e) => setDirInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && saveDir()}
              />
              <button onClick={saveDir} disabled={busy || !dirInput.trim()}>
                Salvar
              </button>
            </div>
          </label>
          <p className="small muted">
            O Screen-Mirror roda só em 127.0.0.1, numa porta livre escolhida pelo Argus, e é encerrado junto com a janela.
            Também é possível apontar a pasta pela variável <code>ARGUS_SCREEN_MIRROR</code>; sem nada configurado, o
            Argus procura <code>Screen-Mirror</code> dentro da pasta raiz.
          </p>
        </div>
      )}
    </div>
  );
}
