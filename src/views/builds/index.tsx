/**
 * Tela "Builds" (rodapé da barra esquerda): comandos de build e de servidor
 * detectados por projeto, execuções em andamento e o histórico de builds.
 * Também o aviso da status bar (`BuildsStatusButton`).
 */
import { useEffect, useMemo, useState } from "react";
import { api, baseName, errorMessage, samePath, type BuildRecord, type Path, type RunInfo } from "@/api";
import { Icon } from "@/shell/icons";
import { useApp, useWorkbench } from "@/store";
import {
  ensureRunsSync,
  formatBytes,
  formatDuration,
  isActive,
  ORIGIN_LABEL,
  startCommand,
  STATE_LABEL,
  useRuns,
} from "./runs";
import "./builds.css";

export { ensureRunsSync, startAllRemembered, startCommand, useRuns } from "./runs";

function useNow(ms: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(t);
  }, [ms]);
  return now;
}

const FLASH_MS = 20_000;

/** Status bar: builds em andamento ou o resultado do último (some depois de 20 s). Abre a tela Builds. */
export function BuildsStatusButton() {
  useEffect(ensureRunsSync, []);
  // Zustand 5: o seletor devolve a lista do store (estável); o filtro fica fora dele.
  const runs = useRuns((s) => s.runs);
  const running = runs.filter((r) => r.kind === "build" && isActive(r));
  const flash = useRuns((s) => s.flash);
  const now = useNow(5000);
  const open = () => useWorkbench.getState().openPage("builds");
  if (running.length > 0) {
    const label = running.length === 1 ? `Build: ${running[0].label}` : `${running.length} builds em andamento`;
    return (
      <button className="wb-sbtn sb-item sb-running" title={running.map((r) => `${baseName(r.project)}: ${r.label}`).join("\n")} onClick={open}>
        <span className="bld-dot running" aria-hidden="true" />
        {label}
      </button>
    );
  }
  if (!flash || now - flash.at > FLASH_MS) return null;
  return (
    <button className="wb-sbtn sb-item" title={`${flash.text}\n\nClique para abrir Builds`} onClick={open}>
      <span className={`bld-dot ${flash.tone}`} aria-hidden="true" />
      <span className="bld-flash">{flash.text}</span>
    </button>
  );
}

function stateClass(state: string): string {
  if (state === "running") return "running";
  if (state === "ready" || state === "ok") return "ok";
  if (state === "failed") return "err";
  return "info";
}

function ArtifactLine({ path, size, isDir, onError }: { path: string; size: number | null; isDir?: boolean; onError(m: string): void }) {
  return (
    <div className="bld-artifact small">
      <Icon name={isDir ? "folder" : "file"} size={12} />
      <span className="ellipsis" title={path}>
        {baseName(path)}
      </span>
      {size != null && <span className="muted">{formatBytes(size)}</span>}
      <button onClick={() => void api.runsOpenFolder(path).catch((e) => onError(errorMessage(e)))}>Abrir pasta</button>
    </div>
  );
}

function ActiveRun({ run, onError }: { run: RunInfo; onError(m: string): void }) {
  const now = useNow(1000);
  const [busy, setBusy] = useState(false);
  const stop = async () => {
    setBusy(true);
    try {
      await api.runsStop(run.id);
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="bld-run">
      <span className={`bld-dot ${stateClass(run.state)}`} aria-hidden="true" />
      <div className="bld-run-main">
        <div className="bld-run-line">
          <b>{run.label}</b>
          <span className="muted small">
            {baseName(run.project)} · {run.kind === "build" ? "build" : "servidor"} · {STATE_LABEL[run.state] ?? run.state} ·{" "}
            {formatDuration(now - run.started_at)} · {ORIGIN_LABEL[run.origin] ?? run.origin}
            {run.port != null && ` · porta ${run.port}`}
          </span>
        </div>
        {run.origin !== "ide" && (
          <div className="muted small ellipsis" title={run.cmdline}>
            {run.cmdline}
          </div>
        )}
      </div>
      <div className="bld-actions">
        {run.url && <button onClick={() => void api.systemOpenUrl(run.url as string).catch((e) => onError(errorMessage(e)))}>Abrir no navegador</button>}
        <button className="srv-danger" disabled={busy} onClick={() => void stop()}>
          {busy ? "Parando…" : "Parar"}
        </button>
      </div>
    </div>
  );
}

function HistoryRow({ rec, onError }: { rec: BuildRecord; onError(m: string): void }) {
  const dur = rec.ended_at != null ? formatDuration(rec.ended_at - rec.started_at) : "";
  const when = new Date(rec.started_at).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
  return (
    <div className="bld-hist">
      <span className={`bld-dot ${stateClass(rec.state)}`} aria-hidden="true" />
      <div className="bld-run-main">
        <div className="bld-run-line">
          <b>{rec.label}</b>
          <span className="muted small">
            {baseName(rec.project)} · {STATE_LABEL[rec.state] ?? rec.state}
            {rec.exit_code != null && rec.state === "failed" && ` (código ${rec.exit_code})`} · {when}
            {dur && ` · ${dur}`} · {ORIGIN_LABEL[rec.origin] ?? rec.origin}
          </span>
        </div>
        {rec.artifact_path && <ArtifactLine path={rec.artifact_path} size={rec.artifact_size} onError={onError} />}
      </div>
    </div>
  );
}

export default function BuildsView() {
  useEffect(ensureRunsSync, []);
  const projects = useApp((s) => s.projects);
  const selected = useApp((s) => s.selected);
  const runs = useRuns((s) => s.runs);
  const commands = useRuns((s) => s.commands);
  const finishedRev = useRuns((s) => s.finishedRev);
  const [filter, setFilter] = useState<Path | "">(selected ?? "");
  const [history, setHistory] = useState<BuildRecord[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    api.runsHistory(filter || null, 30).then(
      (h) => alive && setHistory(h),
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [filter, finishedRev]);

  const shownProjects = useMemo(
    () => Object.keys(commands).filter((p) => !filter || samePath(p, filter)).sort((a, b) => baseName(a).localeCompare(baseName(b))),
    [commands, filter],
  );
  const active = runs.filter((r) => isActive(r) && (!filter || samePath(r.project, filter)));
  // Encerradas nesta sessão que não vão para o histórico (servidores).
  const finishedServers = runs.filter((r) => r.kind === "servidor" && !isActive(r) && (!filter || samePath(r.project, filter))).slice(-5).reverse();

  const start = async (project: Path, id: string) => {
    setStarting(`${project}|${id}`);
    try {
      await startCommand(project, id);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setStarting(null);
    }
  };

  return (
    <div className="bld">
      <div className="bld-bar">
        <label className="small muted" htmlFor="bld-filter">
          Projeto
        </label>
        <select id="bld-filter" value={filter} onChange={(e) => setFilter(e.target.value)}>
          <option value="">Todos os projetos</option>
          {projects
            .filter((p) => !p.missing)
            .map((p) => (
              <option key={p.path} value={p.path}>
                {p.name || baseName(p.path)}
              </option>
            ))}
        </select>
        <span className="muted small">Cada comando roda num terminal da IDE, no painel inferior.</span>
      </div>
      {error && (
        <div className="err small bld-err" role="alert">
          {error}
        </div>
      )}

      <section>
        <h3 className="bld-h">Em andamento</h3>
        {active.length === 0 && <p className="muted small">Nada rodando.</p>}
        {active.map((r) => (
          <ActiveRun key={r.id} run={r} onError={setError} />
        ))}
      </section>

      <section>
        <h3 className="bld-h">Comandos detectados</h3>
        {shownProjects.length === 0 && <p className="muted small">Nenhum comando de build ou de servidor encontrado.</p>}
        {shownProjects.map((p) => (
          <div key={p} className="bld-proj">
            <b className="bld-proj-name">{baseName(p)}</b>
            <div className="bld-cmds">
              {commands[p].map((c) => {
                const busy = starting === `${p}|${c.id}`;
                const running = runs.some((r) => isActive(r) && samePath(r.project, p) && r.command_id === c.id);
                return (
                  <button
                    key={c.id}
                    className={`bld-cmd ${c.kind}`}
                    disabled={busy || running}
                    title={`${c.program} ${c.args.join(" ")}\nOrigem: ${c.source}${c.port != null ? `\nPorta esperada: ${c.port}` : ""}`}
                    onClick={() => void start(p, c.id)}
                  >
                    <Icon name={c.kind === "build" ? "hammer" : "play"} size={12} />
                    {c.label}
                    {c.port != null && <span className="muted">:{c.port}</span>}
                    {running && <span className="muted">(rodando)</span>}
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </section>

      <section>
        <h3 className="bld-h">Histórico de builds</h3>
        {history.length === 0 && <p className="muted small">Nenhum build registrado ainda.</p>}
        {history.map((h) => (
          <HistoryRow key={h.id} rec={h} onError={setError} />
        ))}
        {finishedServers.length > 0 && (
          <>
            <h3 className="bld-h">Servidores encerrados nesta sessão</h3>
            {finishedServers.map((r) => (
              <div key={r.id} className="bld-hist">
                <span className={`bld-dot ${stateClass(r.state)}`} aria-hidden="true" />
                <span className="small">
                  <b>{r.label}</b> <span className="muted">· {baseName(r.project)} · {STATE_LABEL[r.state] ?? r.state}</span>
                </span>
              </div>
            ))}
          </>
        )}
        <p className="muted small bld-note">
          Builds digitados num terminal da IDE ou rodados por agentes aparecem aqui quando a linha de comando casa com um
          comando detectado (verificação a cada 2 s); deles não há código de saída, então terminam como concluídos se
          surgir um artefato novo e como encerrados se não.
        </p>
      </section>
    </div>
  );
}
