/**
 * Aba "Mapa de agentes" de uma sessão: a sessão principal à esquerda, os
 * subagentes à direita ligados por linhas (subagente de subagente nas colunas
 * seguintes) e, embaixo, as tarefas em segundo plano (processos vivos da
 * sessão e comandos de shell que o agente mandou rodar em segundo plano).
 */
import { useEffect, useMemo, useState } from "react";
import {
  api,
  formatTokensPt,
  isTaskActive,
  totalTokens,
  type OwnedProc,
  type TaskId,
} from "@/api";
import { Icon } from "@/shell/icons";
import { useApp, useWorkbench } from "@/store";
import { useProviders } from "@/views/agents";
import { formatDuration, sessionTitle, subagentStatusLabel, subagentTree, usageTotals } from "@/views/session/events";
import { useTaskEvents } from "@/views/session/useTaskEvents";
import {
  agentsLabel,
  backgroundCommands,
  DEFAULT_LAYOUT,
  layoutGraph,
  mapNodes,
  subagentNumbers,
  type BackgroundCommand,
  type MapNode,
} from "./graph";
import "@/views/session/session.css";
import "./agentmap.css";

const L = DEFAULT_LAYOUT;

function openChat(taskId: TaskId, subagent: string | null) {
  useApp.getState().selectSubagent(taskId, subagent);
  useWorkbench.getState().openSession(taskId);
}

/** Relógio que só anda com `on`. */
function useNow(on: boolean, every: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!on) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), every);
    return () => clearInterval(timer);
  }, [on, every]);
  return now;
}

/** Processos vivos da sessão, relidos a cada 2 s enquanto a aba está visível. */
function useSessionProcs(taskId: TaskId, visible: boolean) {
  const [procs, setProcs] = useState<OwnedProc[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!visible) return;
    let alive = true;
    const load = () =>
      api.ownerProcesses({ kind: "Session", id: taskId }).then(
        (p) => {
          if (!alive) return;
          setProcs(p);
          setError(null);
        },
        (e) => alive && setError(String(e)),
      );
    void load();
    const timer = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [taskId, visible]);
  return { procs, error };
}

export interface AgentMapProps {
  taskId: TaskId;
  /** A aba está visível (relógio e leitura de processos só andam assim). */
  active: boolean;
}

type Selected = { kind: "proc"; pid: number } | { kind: "cmd"; index: number } | null;

export default function AgentMap({ taskId, active }: AgentMapProps) {
  const task = useApp((s) => s.tasks.find((t) => t.id === taskId));
  const providers = useProviders();
  const busy = task ? isTaskActive(task.state) : false;
  const { events, times, loaded, error } = useTaskEvents(taskId, busy);
  const tree = useMemo(() => subagentTree(events, times), [events, times]);
  const numbers = useMemo(() => subagentNumbers(events), [events]);
  const commands = useMemo(() => backgroundCommands(events), [events]);
  const totals = useMemo(() => usageTotals(events, null), [events]);
  const ticking = active && busy && tree.some((n) => !n.done);
  const now = useNow(ticking, 1000);
  const nodes = useMemo(() => mapNodes(tree, numbers, busy, now), [tree, numbers, busy, now]);
  const layout = useMemo(() => layoutGraph(nodes), [nodes]);
  const { procs, error: procError } = useSessionProcs(taskId, active);
  const [selected, setSelected] = useState<Selected>(null);

  if (!task) return <div className="am-empty">Sessão #{taskId} não encontrada (pode ter sido excluída).</div>;
  if (!loaded) return <div className="am-empty">Carregando…</div>;

  const providerName = providers.find((p) => p.id === task.provider)?.display_name ?? task.provider;
  const tokens = totalTokens(totals);
  const nameOf = (owner: string | null) =>
    owner == null ? "sessão principal" : (nodes.find((n) => n.id === owner)?.name ?? "subagente");

  return (
    <div className="am">
      <header className="am-head">
        <Icon name="tree" size={15} />
        <b className="ellipsis" title={task.prompt}>
          Mapa de agentes · {sessionTitle(task.prompt, 60)}
        </b>
        <span className="am-head-sub">{agentsLabel(nodes.length)} · clique em um agente para ver detalhes</span>
      </header>
      {error && (
        <div className="err small" role="alert">
          {error}
        </div>
      )}

      <section className="am-canvas" aria-label="Agentes da sessão">
        <div className="am-graph" style={{ width: layout.width, height: layout.height }}>
          <svg className="am-lines" width={layout.width} height={layout.height} aria-hidden="true">
            {layout.edges.map((e) => {
              const st = nodes.find((n) => n.id === e.to)?.status;
              return <path key={e.to} d={e.d} className={st === "running" ? "run" : undefined} />;
            })}
          </svg>
          <button
            className="am-node am-root"
            style={{ left: layout.root.x, top: layout.root.y, width: L.nodeW, height: L.nodeH }}
            title="Abrir o chat da sessão principal"
            onClick={() => openChat(taskId, null)}
          >
            <span className="am-node-l1">
              <span className={`ses-dot ${busy ? "running" : "ok"}`} aria-hidden="true" />
              <span className="ellipsis am-node-name">{sessionTitle(task.prompt, 40)}</span>
            </span>
            <span className="am-node-l2 ellipsis">
              {providerName} · {task.model ?? "modelo padrão"}
            </span>
            <span className="am-node-l2 ellipsis">
              {totals.turns > 0 ? `${formatTokensPt(tokens)} tokens na sessão` : "sem consumo registrado"}
            </span>
          </button>
          {nodes.map((n) => (
            <SubagentNodeView key={n.id} node={n} left={layout.nodes[n.id].x} top={layout.nodes[n.id].y} onOpen={() => openChat(taskId, n.id)} />
          ))}
        </div>
        {nodes.length === 0 && <p className="am-hint">Esta sessão ainda não criou subagentes.</p>}
      </section>

      <section className="am-bg" aria-label="Tarefas em segundo plano">
        <h3 className="am-bg-title">
          Tarefas em segundo plano
          <span className="muted small"> · atualiza a cada 2 s</span>
        </h3>
        {procError && <p className="err small">{procError}</p>}
        <div className="am-bg-cols">
          <div className="am-list" role="list">
            <div className="am-list-h">Processos vivos da sessão</div>
            {procs === null && <p className="am-hint">Lendo os processos…</p>}
            {procs?.length === 0 && <p className="am-hint">Nenhum processo da sessão em execução.</p>}
            {procs?.map((p) => (
              <button
                key={p.pid}
                role="listitem"
                className={selected?.kind === "proc" && selected.pid === p.pid ? "am-row sel" : "am-row"}
                title={p.cmdline || p.name}
                onClick={() => setSelected({ kind: "proc", pid: p.pid })}
              >
                <span className="ses-dot running" aria-hidden="true" />
                <span className="ellipsis am-row-name">{p.cmdline || p.name}</span>
                {p.mcp && <span className="am-tag">MCP</span>}
                <span className="am-row-meta">{formatDuration(Date.now() - p.started_at * 1000)}</span>
              </button>
            ))}
          </div>
          <div className="am-list" role="list">
            <div className="am-list-h">Comandos em segundo plano pedidos pelo agente</div>
            {commands.length === 0 && <p className="am-hint">Nenhum comando de shell em segundo plano nesta sessão.</p>}
            {commands.map((c) => (
              <button
                key={c.index}
                role="listitem"
                className={selected?.kind === "cmd" && selected.index === c.index ? "am-row sel" : "am-row"}
                title={c.command}
                onClick={() => setSelected({ kind: "cmd", index: c.index })}
              >
                <Icon name="terminal" size={13} />
                <span className="ellipsis am-row-name">{c.description || c.command}</span>
                <span className="am-row-meta">{nameOf(c.owner)}</span>
              </button>
            ))}
          </div>
        </div>
        <Detail selected={selected} procs={procs ?? []} commands={commands} nameOf={nameOf} />
      </section>
    </div>
  );
}

function SubagentNodeView({ node, left, top, onOpen }: { node: MapNode; left: number; top: number; onOpen(): void }) {
  const duration = node.durationMs == null ? null : formatDuration(node.durationMs);
  const tip = [
    node.name,
    `${node.type || "subagente"} · ${subagentStatusLabel(node.status)}`,
    duration ? `Duração: ${duration}${node.durationExact ? "" : " (aproximada, pelos horários dos eventos)"}` : "Duração desconhecida",
    node.tokens != null
      ? `Tokens: ${node.tokens} (informado pela CLI: contexto da última chamada do subagente)`
      : node.status === "running"
        ? "Tokens: informados quando o subagente terminar"
        : "Tokens: a CLI não informou",
    node.toolUses != null ? `Ferramentas usadas: ${node.toolUses}` : "",
    "Enter ou clique: abrir a conversa (somente leitura)",
  ]
    .filter(Boolean)
    .join("\n");
  return (
    <button
      className={`am-node ${node.status}`}
      style={{ left, top, width: L.nodeW, height: L.nodeH }}
      title={tip}
      aria-label={`${node.name}, ${subagentStatusLabel(node.status)}${duration ? `, ${duration}` : ""}`}
      onClick={onOpen}
    >
      <span className="am-node-l1">
        <span className={`ses-dot ${node.status}`} aria-hidden="true" />
        <span className="ellipsis am-node-name">{node.name}</span>
      </span>
      <span className="am-node-l2 ellipsis">{node.type || "subagente"}</span>
      <span className="am-node-l2 am-node-nums">
        <span>
          {duration ?? subagentStatusLabel(node.status)}
          {duration && !node.durationExact && node.status !== "running" ? " ≈" : ""}
        </span>
        <span>{node.tokens != null ? `${formatTokensPt(node.tokens)} tokens` : "— tokens"}</span>
      </span>
    </button>
  );
}

function Detail({
  selected,
  procs,
  commands,
  nameOf,
}: {
  selected: Selected;
  procs: OwnedProc[];
  commands: BackgroundCommand[];
  nameOf(owner: string | null): string;
}) {
  if (!selected) return <p className="am-hint">Clique numa tarefa para ver os detalhes.</p>;
  if (selected.kind === "proc") {
    const p = procs.find((x) => x.pid === selected.pid);
    if (!p) return <p className="am-hint">O processo {selected.pid} terminou.</p>;
    return (
      <div className="am-detail">
        <dl>
          <dt>PID</dt>
          <dd>{p.pid}</dd>
          <dt>Programa</dt>
          <dd>{p.name}</dd>
          <dt>Linha de comando</dt>
          <dd className="mono">{p.cmdline || "(indisponível)"}</dd>
          <dt>Início</dt>
          <dd>
            {new Date(p.started_at * 1000).toLocaleString("pt-BR")} · há {formatDuration(Date.now() - p.started_at * 1000)}
          </dd>
          {p.port != null && (
            <>
              <dt>Porta</dt>
              <dd>{p.port}</dd>
            </>
          )}
        </dl>
        <p className="am-hint">
          A saída deste processo não passa pelo Argus: ele foi iniciado pelo agente, que lê a saída por conta própria. Aqui
          aparecem só os dados do processo.
        </p>
      </div>
    );
  }
  const c = commands.find((x) => x.index === selected.index);
  if (!c) return null;
  return (
    <div className="am-detail">
      <dl>
        <dt>Pedido por</dt>
        <dd>{nameOf(c.owner)}</dd>
        <dt>Ferramenta</dt>
        <dd>{c.tool}</dd>
        {c.description && (
          <>
            <dt>Descrição</dt>
            <dd>{c.description}</dd>
          </>
        )}
      </dl>
      <pre className="am-pre">{c.command}</pre>
      <p className="am-hint">
        A saída de comandos em segundo plano não é gravada nos eventos da sessão (a CLI só a devolve ao agente). Se o
        comando ainda roda, ele aparece em "Processos vivos da sessão".
      </p>
    </div>
  );
}
