/**
 * Servidores vinculados às sessões e aos terminais: selo "N servidores",
 * painel lateral direito retrátil e os "Comandos permitidos ao agente".
 */
import { useEffect, useMemo, useState, type MouseEvent } from "react";
import {
  api,
  baseName,
  errorMessage,
  formatUptime,
  ownerLabel,
  sameOwner,
  type Owner,
  type Path,
  type ServerInfo,
  type TaskId,
} from "@/api";
import { useApp, useLayout, useWorkbench } from "@/store";
import { Icon } from "@/shell/icons";
import "./servers.css";

export { askStopSession, stopSessionServers, type StopAction, type StopChoice } from "./stopSession";

export interface SessionServersBadgeProps {
  taskId: TaskId;
}

function countLabel(n: number): string {
  return n === 1 ? "1 servidor" : `${n} servidores`;
}

/** Selo "N servidores" de uma sessão; abre o painel filtrado. Vazio se não houver servidores. */
export function SessionServersBadge({ taskId }: SessionServersBadgeProps) {
  const count = useApp((s) => s.servers.filter((x) => x.owner.kind === "Session" && x.owner.id === taskId).length);
  const openPanel = useLayout((s) => s.openServersPanel);
  if (count === 0) return null;
  const open = (e: MouseEvent) => {
    e.stopPropagation();
    openPanel({ kind: "Session", id: taskId });
  };
  return (
    <button className="srv-badge" title="Ver servidores desta sessão" onClick={open}>
      ● {countLabel(count)}
    </button>
  );
}

/** Botão da status bar: mostra quantos servidores estão rodando e abre/fecha o painel. */
export function ServersStatusButton() {
  const count = useApp((s) => s.servers.length);
  const open = useLayout((s) => s.rightOpen && s.rightTab === "servers");
  const toggle = useLayout((s) => s.toggleServersPanel);
  return (
    <button className="wb-sbtn sb-item" title="Servidores das sessões e dos terminais" aria-pressed={open} onClick={toggle}>
      <Icon name="server" size={13} /> {countLabel(count)}
    </button>
  );
}

/** Relógio que força nova renderização a cada `ms` (tempo no ar). */
function useNow(ms: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(timer);
  }, [ms]);
  return now;
}

function ownerOrder(o: Owner): number {
  return (o.kind === "Session" ? 0 : 1e9) + o.id;
}

function ServerItem({ server, onError }: { server: ServerInfo; onError(msg: string | null): void }) {
  const now = useNow(1000);
  const [busy, setBusy] = useState<"restart" | "kill" | null>(null);

  const run = async (kind: "restart" | "kill", action: () => Promise<unknown>) => {
    setBusy(kind);
    try {
      await action();
      onError(null);
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const openBrowser = () => api.systemOpenUrl(server.url).then(() => onError(null), (e) => onError(errorMessage(e)));
  const restart = () => run("restart", () => api.serverRestart(server));
  const kill = () =>
    run("kill", async () => {
      useApp.setState({ servers: await api.serverKill(server.pid) });
    });

  return (
    <div className="srv-item">
      <div className="srv-line">
        <b title={server.url}>:{server.port}</b>
        <span className="muted small">
          {server.name} · pid {server.pid} · {formatUptime(server.started_at, now)}
        </span>
      </div>
      <div className="small muted ellipsis" title={server.cmdline}>
        {server.cmdline || "(linha de comando indisponível)"}
      </div>
      <div className="srv-actions">
        <button onClick={openBrowser}>Abrir no navegador</button>
        <button disabled={!server.cmdline || busy !== null} onClick={restart}>
          {busy === "restart" ? "Reiniciando…" : "Reiniciar"}
        </button>
        <button className="srv-danger" disabled={busy !== null} onClick={kill}>
          {busy === "kill" ? "Derrubando…" : "Derrubar"}
        </button>
      </div>
    </div>
  );
}

function ServersList({ filter }: { filter: Owner | null }) {
  const servers = useApp((s) => s.servers);
  const [error, setError] = useState<string | null>(null);
  const [stopping, setStopping] = useState<Owner | null>(null);

  const items = useMemo(() => servers.filter((s) => !filter || sameOwner(s.owner, filter)), [servers, filter]);
  const owners = useMemo(() => {
    const list: Owner[] = [];
    for (const s of items) if (!list.some((o) => sameOwner(o, s.owner))) list.push(s.owner);
    return list.sort((a, b) => ownerOrder(a) - ownerOrder(b));
  }, [items]);

  const stopAll = async (owner: Owner) => {
    setStopping(owner);
    try {
      await api.serversStopAll(owner);
      useApp.setState({ servers: await api.serversList() });
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setStopping(null);
    }
  };

  return (
    <>
      {error && (
        <div className="err small srv-err" role="alert">
          {error}
        </div>
      )}
      {items.length === 0 && (
        <p className="muted small srv-empty">
          {filter
            ? `Nenhum servidor em execução em ${ownerLabel(filter)}.`
            : "Nenhum servidor em execução vinculado às sessões."}
        </p>
      )}
      {owners.map((owner) => {
        const busy = stopping !== null && sameOwner(stopping, owner);
        return (
          <div key={`${owner.kind}-${owner.id}`} className="srv-group">
            <div className="srv-group-head">
              <b>{ownerLabel(owner)}</b>
              <button className="srv-danger" disabled={busy} onClick={() => void stopAll(owner)}>
                {busy ? "Derrubando…" : "Derrubar todos"}
              </button>
            </div>
            {items
              .filter((s) => sameOwner(s.owner, owner))
              .map((s) => (
                <ServerItem key={`${s.pid}-${s.port}`} server={s} onError={setError} />
              ))}
          </div>
        );
      })}
    </>
  );
}

/** "Comandos permitidos ao agente" de um projeto (cada linha vira `--allowedTools "Bash(...)"`). */
export function AllowedCommands({ project }: { project: Path }) {
  const [text, setText] = useState("");
  const [saved, setSaved] = useState("");
  const [msg, setMsg] = useState<{ text: string; error: boolean } | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let alive = true;
    setMsg(null);
    api.allowedCommandsGet(project).then(
      (cmds) => {
        if (!alive) return;
        setText(cmds.join("\n"));
        setSaved(cmds.join("\n"));
      },
      (e) => alive && setMsg({ text: errorMessage(e), error: true }),
    );
    return () => {
      alive = false;
    };
  }, [project]);

  const save = async () => {
    setSaving(true);
    try {
      const lines = text.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
      const stored = (await api.allowedCommandsSet(project, lines)).join("\n");
      setText(stored);
      setSaved(stored);
      setMsg({ text: "Salvo. Vale a partir do próximo turno.", error: false });
    } catch (e) {
      setMsg({ text: errorMessage(e), error: true });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="srv-allowed">
      <b>Comandos permitidos ao agente</b>
      <div className="small muted">
        Projeto {baseName(project)}. Soma ao conjunto padrão (git de leitura e inspeção). Um padrão por linha (ex.: npm run
        dev:*); cada linha vira --allowedTools "Bash(...)". Para a ferramenta PowerShell, escreva PowerShell(comando).
      </div>
      <textarea
        rows={4}
        value={text}
        placeholder="npm run dev:*"
        spellCheck={false}
        aria-label="Comandos permitidos ao agente"
        onChange={(e) => {
          setText(e.target.value);
          setMsg(null);
        }}
      />
      <div className="row">
        <button disabled={saving || text === saved} onClick={() => void save()}>
          {saving ? "Salvando…" : "Salvar"}
        </button>
        {msg && <span className={msg.error ? "small err" : "small muted"}>{msg.text}</span>}
      </div>
    </div>
  );
}

/**
 * Aba "Servidores" do painel direito: servidores por dono (abrir no navegador,
 * reiniciar, derrubar). Os comandos permitidos ao agente ficam em Configurações.
 */
export default function ServersView() {
  const filter = useLayout((s) => s.serversFilter);
  const openPanel = useLayout((s) => s.openServersPanel);

  return (
    <div className="srv-panel" aria-label="Servidores">
      {filter && (
        <div className="srv-head">
          <button className="srv-chip" title="Mostrar os servidores de todas as sessões" onClick={() => openPanel(null)}>
            {ownerLabel(filter)} ✕
          </button>
        </div>
      )}
      <ServersList filter={filter} />
      <button className="rp-link" onClick={() => useWorkbench.getState().openPage("settings")}>
        <Icon name="shield" size={13} /> Comandos permitidos ao agente
      </button>
    </div>
  );
}
