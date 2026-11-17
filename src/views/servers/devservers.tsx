/**
 * Servidores de dev com um clique: seção do painel Servidores, botão discreto na
 * linha do projeto (barra esquerda) e itens do menu do projeto. A detecção, a
 * escolha de porta e o bloqueio por build em andamento ficam no backend
 * (`runs_prepare`); aqui só se abre o terminal e mostra o estado.
 */
import { useEffect, useState, type MouseEvent } from "react";
import { api, baseName, errorMessage, samePath, type Path, type RunInfo } from "@/api";
import type { MenuItem } from "@/shell/ContextMenu";
import { Icon } from "@/shell/icons";
import { useApp } from "@/store";
import {
  buildOf,
  commandsOf,
  ensureRunsSync,
  flash,
  preferredServer,
  serverOf,
  startAllRemembered,
  startCommand,
  useRuns,
} from "@/views/builds/runs";
import "@/views/builds/builds.css";

function stateText(run: RunInfo): string {
  if (run.state === "ready") return `pronto em :${run.port}`;
  return run.port != null ? `iniciando (aguardando :${run.port})` : "rodando";
}

async function stopRun(run: RunInfo): Promise<void> {
  await api.runsStop(run.id);
}

/** Inicia o servidor preferido do projeto ou para o que está no ar. Erros vão para o aviso global. */
async function toggleServer(project: Path): Promise<void> {
  const s = useRuns.getState();
  const running = serverOf(s.runs, project);
  try {
    if (running) {
      await stopRun(running);
      return;
    }
    const cmd = preferredServer(s, project);
    if (!cmd) throw new Error(`Nenhum servidor de dev detectado em ${baseName(project)}.`);
    await startCommand(project, cmd.id);
  } catch (e) {
    useApp.getState().setError(errorMessage(e));
  }
}

/** Botão Iniciar/Parar na linha do projeto (aparece no hover; fica visível com o servidor no ar). */
export function ProjectServerButton({ project }: { project: Path }) {
  useEffect(ensureRunsSync, []);
  const run = useRuns((s) => serverOf(s.runs, project));
  const cmd = useRuns((s) => preferredServer(s, project));
  if (!run && !cmd) return null;
  const label = run
    ? `Parar ${run.label} (${stateText(run)})`
    : `Iniciar servidor de dev: ${cmd?.label}${cmd?.port != null ? ` (porta ${cmd.port})` : ""}`;
  const onClick = (e: MouseEvent) => {
    e.stopPropagation();
    void toggleServer(project);
  };
  return (
    <button className={run ? `dsv-btn ${run.state === "ready" ? "on" : "on wait"}` : "dsv-btn"} title={label} aria-label={label} onClick={onClick}>
      <Icon name={run ? "stop" : "play"} size={12} />
    </button>
  );
}

/** Itens do menu de contexto do projeto (vazio se não houver servidor detectado). */
export function devServerMenuItems(project: Path): MenuItem[] {
  const s = useRuns.getState();
  const run = serverOf(s.runs, project);
  const servers = commandsOf(s.commands, project).filter((c) => c.kind === "servidor");
  if (!run && servers.length === 0) return [];
  const items: MenuItem[] = [];
  if (run) {
    items.push({ label: `Parar servidor (${run.label})`, separator: true, onSelect: () => void toggleServer(project) });
    if (run.url) items.push({ label: "Abrir no navegador", onSelect: () => void api.systemOpenUrl(run.url as string) });
  } else {
    servers.forEach((c, i) =>
      items.push({
        label: `Iniciar ${c.label}`,
        separator: i === 0,
        onSelect: () => void startCommand(project, c.id).catch((e) => useApp.getState().setError(errorMessage(e))),
      }),
    );
  }
  return items;
}

function DevServerRow({ project, onError }: { project: Path; onError(m: string | null): void }) {
  const all = useRuns((s) => s.commands);
  const commands = commandsOf(all, project).filter((c) => c.kind === "servidor");
  const preferred = useRuns((s) => preferredServer(s, project));
  const run = useRuns((s) => serverOf(s.runs, project));
  const build = useRuns((s) => buildOf(s.runs, project));
  const [choice, setChoice] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const chosen = commands.find((c) => c.id === choice) ?? preferred;

  const act = async (fn: () => Promise<void>) => {
    setBusy(true);
    try {
      await fn();
      onError(null);
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="dsv-row">
      <span className="dsv-name ellipsis" title={project}>
        {baseName(project)}
      </span>
      <span className="dsv-state" title={run?.cmdline}>
        {run ? stateText(run) : build ? `build em andamento (${build.label})` : chosen?.port != null ? `porta ${chosen.port}` : ""}
      </span>
      {!run && commands.length > 1 && (
        <select value={chosen?.id ?? ""} aria-label={`Comando de servidor de ${baseName(project)}`} onChange={(e) => setChoice(e.target.value)}>
          {commands.map((c) => (
            <option key={c.id} value={c.id}>
              {c.label}
            </option>
          ))}
        </select>
      )}
      {run?.url && <button onClick={() => void act(() => api.systemOpenUrl(run.url as string))}>Abrir</button>}
      {run ? (
        <button className="srv-danger" disabled={busy} onClick={() => void act(() => stopRun(run))}>
          Parar
        </button>
      ) : (
        <button
          disabled={busy || !chosen || !!build}
          title={build ? "Espere o build do projeto terminar" : chosen ? `${chosen.program} ${chosen.args.join(" ")}` : undefined}
          onClick={() => chosen && void act(() => startCommand(project, chosen.id))}
        >
          {commands.length === 1 ? `Iniciar ${chosen?.label}` : "Iniciar"}
        </button>
      )}
    </div>
  );
}

/** Seção "Servidores de dev" do painel Servidores: um Iniciar/Parar por projeto e "Iniciar todos". */
export function DevServersSection({ project }: { project?: Path | null }) {
  useEffect(ensureRunsSync, []);
  const commands = useRuns((s) => s.commands);
  const remembered = useRuns((s) => s.remembered);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const projects = Object.keys(commands)
    .filter((p) => commands[p].some((c) => c.kind === "servidor"))
    .filter((p) => !project || samePath(p, project))
    .sort((a, b) => baseName(a).localeCompare(baseName(b)));
  if (projects.length === 0) return null;

  const startAll = async () => {
    setStarting(true);
    const errors = await startAllRemembered().catch((e) => [errorMessage(e)]);
    setStarting(false);
    setError(errors.length ? errors.join("\n") : null);
    if (!errors.length) flash("Servidores lembrados iniciados.", "ok");
  };

  return (
    <div className="dsv">
      <div className="dsv-head">
        <b>Servidores de dev</b>
        <button
          disabled={starting || remembered.length === 0}
          title={
            remembered.length === 0
              ? "Inicie um servidor em algum projeto para ele ser lembrado"
              : `Inicia o último servidor usado em: ${remembered.map((r) => baseName(r.project)).join(", ")}`
          }
          onClick={() => void startAll()}
        >
          {starting ? "Iniciando…" : `Iniciar todos (${remembered.length})`}
        </button>
      </div>
      {error && (
        <div className="err small dsv-err" role="alert">
          {error}
        </div>
      )}
      {projects.map((p) => (
        <DevServerRow key={p} project={p} onError={setError} />
      ))}
    </div>
  );
}
