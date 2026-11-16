/**
 * Aba "Agentes" do painel direito: em cima o mapa compacto da sessão (agente
 * principal e subagentes em árvore vertical) e, embaixo, as tarefas em segundo
 * plano (subagentes, comandos de shell em segundo plano e processos vivos),
 * agrupadas por estado. A visão grande continua na aba "Mapa de agentes".
 */
import { useEffect, useMemo, useState } from "react";
import { api, formatTokensPt, isTaskActive, type OwnedProc, type Task, type TaskId } from "@/api";
import { Icon } from "@/shell/icons";
import { useApp, useLayout, useWorkbench } from "@/store";
import { AgentState, AgentStateIcon, subagentStateKind } from "@/views/session/AgentState";
import { sessionTitle, subagentStatusLabel, subagentTree, type SubagentNode } from "@/views/session/events";
import { useTaskEvents } from "@/views/session/useTaskEvents";
import {
  agentsLabel,
  backgroundCommands,
  commandGroup,
  commandProcess,
  mapNodes,
  prettyCmd,
  shortDuration,
  subagentGroup,
  subagentNumbers,
  type BackgroundCommand,
  type MapNode,
  type TaskGroup,
} from "./graph";
import "@/views/session/session.css";
import "./agentmap.css";

const PAGE = 5;

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

/** Processos vivos da sessão, relidos a cada 2 s. */
function useSessionProcs(taskId: TaskId) {
  const [procs, setProcs] = useState<OwnedProc[]>([]);
  useEffect(() => {
    let alive = true;
    const load = () =>
      api.ownerProcesses({ kind: "Session", id: taskId }).then(
        (p) => alive && setProcs(p),
        () => undefined,
      );
    void load();
    const timer = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [taskId]);
  return procs;
}

/** Item da lista de tarefas em segundo plano. */
type Item =
  | { key: string; kind: "subagent"; group: TaskGroup; title: string; durationMs: number | null; node: MapNode; info: SubagentNode }
  | { key: string; kind: "command"; group: TaskGroup; title: string; durationMs: number | null; cmd: BackgroundCommand; proc: OwnedProc | null }
  | { key: string; kind: "proc"; group: TaskGroup; title: string; durationMs: number | null; proc: OwnedProc };

const GROUPS: { id: TaskGroup; label: string }[] = [
  { id: "running", label: "Rodando" },
  { id: "done", label: "Concluído" },
  { id: "failed", label: "Falhou" },
];

export function AgentsPanel({ taskId }: { taskId: TaskId | null }) {
  if (taskId == null) return <p className="rp-empty">Abra uma sessão para ver o mapa de agentes dela.</p>;
  return <AgentsPanelFor key={taskId} taskId={taskId} />;
}

function AgentsPanelFor({ taskId }: { taskId: TaskId }) {
  const task = useApp((s) => s.tasks.find((t) => t.id === taskId));
  const busy = task ? isTaskActive(task.state) : false;
  const { events, times, loaded } = useTaskEvents(taskId, busy);
  const tree = useMemo(() => subagentTree(events, times), [events, times]);
  const numbers = useMemo(() => subagentNumbers(events), [events]);
  const commands = useMemo(() => backgroundCommands(events), [events]);
  const procs = useSessionProcs(taskId);
  const now = useNow(busy || procs.length > 0, 1000);
  const nodes = useMemo(() => mapNodes(tree, numbers, busy, now), [tree, numbers, busy, now]);
  const [open, setOpen] = useState<string | null>(null);
  const [folded, setFolded] = useState<Partial<Record<TaskGroup | "mcp", boolean>>>({});
  const [more, setMore] = useState<Partial<Record<TaskGroup, boolean>>>({});
  const [cleared, setCleared] = useState<Set<string>>(() => new Set());

  const items = useMemo(() => {
    const out: Item[] = [];
    const used = new Set<number>();
    nodes.forEach((n, i) => {
      out.push({ key: `s:${n.id}`, kind: "subagent", group: subagentGroup(n.status), title: n.name, durationMs: n.durationMs, node: n, info: tree[i] });
    });
    for (const c of commands) {
      const proc = commandProcess(c.command, procs.filter((p) => !p.mcp));
      if (proc) used.add(proc.pid);
      out.push({
        key: `c:${c.index}`,
        kind: "command",
        group: commandGroup(c, proc != null, busy),
        title: c.command || c.description,
        durationMs: proc ? now - proc.started_at * 1000 : null,
        cmd: c,
        proc,
      });
    }
    // Processos vivos que não correspondem a nenhum comando listado (abertos de outro jeito).
    for (const p of procs) {
      if (p.mcp || used.has(p.pid)) continue;
      out.push({ key: `p:${p.pid}`, kind: "proc", group: "running", title: prettyCmd(p.cmdline) || p.name, durationMs: now - p.started_at * 1000, proc: p });
    }
    return out;
  }, [nodes, tree, commands, procs, busy, now]);
  const mcps = procs.filter((p) => p.mcp);

  if (!task) return <p className="rp-empty">Sessão #{taskId} não encontrada.</p>;
  if (!loaded) return <p className="rp-empty">Carregando…</p>;

  const visible = items.filter((it) => !(it.group !== "running" && cleared.has(it.key)));
  const doneCount = visible.filter((it) => it.group !== "running").length;

  return (
    <div className="ap">
      <section className="ap-map" aria-label="Mapa de agentes">
        <header className="ap-head">
          <div className="ap-head-t">
            <h3>Mapa de agentes</h3>
            <span className="ap-sub">{agentsLabel(nodes.length)} · clique num agente para ver detalhes</span>
          </div>
          <button
            className="ap-ibtn"
            title="Abrir o mapa grande numa aba do editor"
            aria-label="Abrir o mapa grande"
            onClick={() => useWorkbench.getState().openAgentMap(taskId)}
          >
            <Icon name="expand" size={14} />
          </button>
        </header>
        <RootCard task={task} onOpen={() => openChat(taskId, null)} />
        {nodes.length > 0 && (
          <ul className="ap-tree" role="list">
            {nodes.map((n) => (
              <li key={n.id} className="ap-branch" style={{ marginLeft: (n.column - 1) * 16 }}>
                <SubagentCard node={n} onOpen={() => openChat(taskId, n.id)} />
              </li>
            ))}
          </ul>
        )}
        {nodes.length === 0 && <p className="ap-hint">Esta sessão ainda não criou subagentes.</p>}
      </section>

      <section className="ap-bg" aria-label="Tarefas em segundo plano">
        <header className="ap-head">
          <div className="ap-head-t">
            <h3>Tarefas em segundo plano</h3>
          </div>
          <button
            className="ap-ibtn"
            title="Limpar concluídas e com falha"
            aria-label="Limpar concluídas"
            disabled={doneCount === 0}
            onClick={() => setCleared(new Set([...cleared, ...items.filter((it) => it.group !== "running").map((it) => it.key)]))}
          >
            <Icon name="trash" size={14} />
          </button>
          <button
            className="ap-ibtn"
            title="Abrir como aba grande (Mapa de agentes)"
            aria-label="Abrir como aba grande"
            onClick={() => useWorkbench.getState().openAgentMap(taskId)}
          >
            <Icon name="expand" size={14} />
          </button>
          <button className="ap-ibtn" title="Fechar o painel" aria-label="Fechar o painel" onClick={() => useLayout.getState().toggleRight()}>
            <Icon name="close" size={14} />
          </button>
        </header>

        {visible.length === 0 && mcps.length === 0 && <p className="ap-hint">Nenhuma tarefa em segundo plano nesta sessão.</p>}

        {GROUPS.map((g) => {
          const list = visible.filter((it) => it.group === g.id);
          if (list.length === 0) return null;
          const isFolded = folded[g.id] ?? false;
          const shown = more[g.id] ? list : list.slice(0, PAGE);
          return (
            <div key={g.id} className="ap-group">
              <button className="ap-gh" aria-expanded={!isFolded} onClick={() => setFolded({ ...folded, [g.id]: !isFolded })}>
                <Icon name={isFolded ? "chevronRight" : "chevronDown"} size={13} />
                <span className={`ap-gl ${g.id}`}>{g.label}</span>
                <span className="ap-gc">{list.length}</span>
              </button>
              {!isFolded && (
                <ul className="ap-items" role="list">
                  {shown.map((it) => (
                    <TaskItem key={it.key} item={it} open={open === it.key} onToggle={() => setOpen(open === it.key ? null : it.key)} nodes={nodes} />
                  ))}
                  {list.length > shown.length && (
                    <li>
                      <button className="ap-more" onClick={() => setMore({ ...more, [g.id]: true })}>
                        Mostrar mais ({list.length - shown.length})
                      </button>
                    </li>
                  )}
                </ul>
              )}
            </div>
          );
        })}

        {mcps.length > 0 && (
          <div className="ap-group">
            <button className="ap-gh" aria-expanded={!folded.mcp} onClick={() => setFolded({ ...folded, mcp: !folded.mcp })}>
              <Icon name={folded.mcp ? "chevronRight" : "chevronDown"} size={13} />
              <span className="ap-gl">Servidores MCP do agente</span>
              <span className="ap-gc">{mcps.length}</span>
            </button>
            {!folded.mcp && (
              <ul className="ap-items" role="list">
                {mcps.map((p) => (
                  <TaskItem
                    key={p.pid}
                    item={{ key: `m:${p.pid}`, kind: "proc", group: "running", title: p.cmdline || p.name, durationMs: now - p.started_at * 1000, proc: p }}
                    open={open === `m:${p.pid}`}
                    onToggle={() => setOpen(open === `m:${p.pid}` ? null : `m:${p.pid}`)}
                    nodes={nodes}
                    badge="MCP"
                  />
                ))}
              </ul>
            )}
          </div>
        )}
      </section>
    </div>
  );
}

function RootCard({ task, onOpen }: { task: Task; onOpen(): void }) {
  const ctx = task.context;
  const inCtx = ctx ? ctx.input_tokens + ctx.cache_read + ctx.cache_write : null;
  return (
    <button className="ap-card ap-root" title="Abrir o chat da sessão principal" onClick={onOpen}>
      <span className="ap-l1">
        <AgentState task={task} size={13} />
        <b className="ellipsis">{sessionTitle(task.prompt, 60)}</b>
      </span>
      <span className="ap-l2 ellipsis">
        {task.model ?? "modelo padrão"}
        {inCtx != null ? ` · ${formatTokensPt(inCtx)} tokens no contexto` : ""}
      </span>
    </button>
  );
}

function SubagentCard({ node, onOpen }: { node: MapNode; onOpen(): void }) {
  const dur = node.durationMs == null ? null : shortDuration(node.durationMs);
  const label = subagentStatusLabel(node.status);
  return (
    <button
      className={`ap-card ap-sub-card ${node.status}`}
      title={`${node.name}\n${node.type || "subagente"} · ${label}\nClique: abrir a conversa (somente leitura)`}
      onClick={onOpen}
    >
      <span className="ap-l1">
        <AgentStateIcon kind={subagentStateKind(node.status)} title={label} />
        <span className="ellipsis ap-name">{node.name}</span>
      </span>
      <span className="ap-l2 ap-nums">
        {dur ?? label}
        {node.tokens != null ? ` · ${formatTokensPt(node.tokens)} tokens` : ""}
      </span>
    </button>
  );
}

function TaskItem({ item, open, onToggle, nodes, badge }: { item: Item; open: boolean; onToggle(): void; nodes: MapNode[]; badge?: string }) {
  const failed = item.group === "failed";
  const nameOf = (owner: string | null) => (owner == null ? "sessão principal" : (nodes.find((n) => n.id === owner)?.name ?? "subagente"));
  return (
    <li className={open ? "ap-item open" : "ap-item"}>
      <button className="ap-ir" aria-expanded={open} onClick={onToggle} title={item.title}>
        <Icon name={open ? "chevronDown" : "chevronRight"} size={13} />
        {item.kind === "subagent" && <Icon name="bot" size={13} className="ap-ik" />}
        {item.kind !== "subagent" && <Icon name="terminal" size={13} className="ap-ik" />}
        <span className={item.kind === "subagent" ? "ap-it ellipsis" : "ap-it mono ellipsis"}>{item.title}</span>
        {badge && <span className="ap-badge">{badge}</span>}
        <span className={failed ? "ap-id err" : "ap-id"}>{failed ? "Falhou" : item.durationMs != null ? shortDuration(item.durationMs) : ""}</span>
      </button>
      {open && (
        <div className="ap-detail">
          {item.kind === "subagent" && (
            <>
              <p className="ap-dk">{item.node.type || "subagente"} · {subagentStatusLabel(item.node.status)}</p>
              <p className="ap-dk">Prompt</p>
              <pre className="ap-pre">{item.info.prompt || "(vazio)"}</pre>
              {item.info.result != null && (
                <>
                  <p className="ap-dk">Resultado</p>
                  <pre className="ap-pre">{item.info.result || "(vazio)"}</pre>
                </>
              )}
            </>
          )}
          {item.kind === "command" && (
            <>
              <p className="ap-dk">
                {item.cmd.tool} · pedido por {nameOf(item.cmd.owner)}
                {item.proc ? ` · PID ${item.proc.pid}` : ""}
              </p>
              {item.cmd.description && <p className="ap-dk">{item.cmd.description}</p>}
              <pre className="ap-pre">{item.cmd.command}</pre>
              {item.cmd.result && (
                <>
                  <p className="ap-dk">Resposta da ferramenta</p>
                  <pre className="ap-pre">{item.cmd.result.output || "(vazia)"}</pre>
                </>
              )}
              <p className="ap-hint">A saída contínua do comando vai só para o agente; o Argus não a grava.</p>
            </>
          )}
          {item.kind === "proc" && (
            <>
              <p className="ap-dk">
                {item.proc.name} · PID {item.proc.pid}
                {item.proc.port != null ? ` · porta ${item.proc.port}` : ""}
              </p>
              <pre className="ap-pre">{item.proc.cmdline || "(linha de comando indisponível)"}</pre>
              <p className="ap-hint">Processo aberto pelo agente; a saída dele não passa pelo Argus.</p>
            </>
          )}
        </div>
      )}
    </li>
  );
}
