/**
 * Área "Agentes" (tela 06 do mockup): linha do tempo com uma linha por agente
 * (recuo = hierarquia, barra = quando rodou, tokens à direita), detalhe do
 * agente selecionado à direita (pedido de permissão, tarefa, atividade ao
 * vivo, instrução) e rodapé com processos, CPU/GPU e gasto da sessão.
 */
import { useEffect, useMemo, useState } from "react";
import { api, formatTokensPt, isTaskActive, sameOwner, type AgentEvent, type OwnedProc, type Task, type TaskId } from "@/api";
import { useActiveContext } from "@/shell/useActiveContext";
import { useApp, useLayout, useWorkbench } from "@/store";
import { AreaHeader, Btn, Card, Chip, StatusDot, AreaStatus } from "@/ui";
import { mapNodes, prettyCmd, shortDuration, subagentNumbers, type MapNode } from "@/views/agentmap/graph";
import { sessionTitle, subagentStatusLabel, subagentTree, unwrap, type SubagentStatus } from "@/views/session/events";
import { useTaskEvents } from "@/views/session/useTaskEvents";
import { useOwnersUsage } from "@/views/servers/SessionUsage";
import "./agents.css";

export default function AgentsArea() {
  const { taskId } = useActiveContext();
  if (taskId == null) {
    return (
      <div className="ui-area">
        <div className="ui-area-main">
          <AreaHeader title="Agentes" sub="Nenhuma sessão em foco" actions={<BackBtn />} />
          <p className="ui-empty">Abra uma conversa para ver os agentes dela.</p>
        </div>
      </div>
    );
  }
  return <AgentsFor key={taskId} taskId={taskId} />;
}

function BackBtn() {
  return (
    <Btn variant="ghost" onClick={() => useLayout.getState().setArea("chat")}>
      Voltar à conversa
    </Btn>
  );
}

function useNow(on: boolean, every: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!on) return;
    setNow(Date.now());
    const t = setInterval(() => setNow(Date.now()), every);
    return () => clearInterval(t);
  }, [on, every]);
  return now;
}

function useSessionProcs(taskId: TaskId): OwnedProc[] {
  const [procs, setProcs] = useState<OwnedProc[]>([]);
  useEffect(() => {
    let alive = true;
    const load = () => api.ownerProcesses({ kind: "Session", id: taskId }).then((p) => alive && setProcs(p), () => undefined);
    void load();
    const t = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [taskId]);
  return procs;
}

/** Linha da linha do tempo: o principal (id null) ou um subagente. */
interface Row {
  id: string | null;
  depth: number;
  name: string;
  hint: string;
  status: SubagentStatus;
  start: number | null;
  end: number | null;
  tokens: number | null;
  node: MapNode | null;
  prompt: string;
  result: string | null;
}

interface Denied {
  key: string;
  owner: string | null;
  tool: string;
  command: string;
}

function toneOf(s: SubagentStatus): "ok" | "bad" | "warn" | undefined {
  return s === "running" ? "ok" : s === "error" ? "bad" : s === "interrupted" ? "warn" : undefined;
}

function hhmm(ms: number): string {
  return new Date(ms).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
}

/** Atividade (ferramentas e textos) de um dono, em ordem. */
function activityOf(events: AgentEvent[], owner: string | null): { key: number; text: string; mono?: string; ok?: boolean }[] {
  const out: { key: number; text: string; mono?: string; ok?: boolean }[] = [];
  events.forEach((raw, i) => {
    const u = unwrap(raw);
    if (u.owner !== owner) return;
    const e = u.event;
    if ("ToolUse" in e) {
      const inp = e.ToolUse.input != null && typeof e.ToolUse.input === "object" ? (e.ToolUse.input as Record<string, unknown>) : {};
      const arg = [inp.file_path, inp.command, inp.pattern, inp.path, inp.url, inp.description].find((v) => typeof v === "string") as string | undefined;
      out.push({ key: i, text: e.ToolUse.name, mono: arg ? arg.split("\n")[0].slice(0, 120) : undefined });
    } else if ("Denied" in e) {
      out.push({ key: i, text: "Negado", mono: e.Denied.command || e.Denied.tool, ok: false });
    }
  });
  return out.slice(-30);
}

function AgentsFor({ taskId }: { taskId: TaskId }) {
  const task = useApp((s) => s.tasks.find((t) => t.id === taskId));
  const busy = task ? isTaskActive(task.state) : false;
  const { events, times, loaded } = useTaskEvents(taskId, busy);
  const tree = useMemo(() => subagentTree(events, times), [events, times]);
  const numbers = useMemo(() => subagentNumbers(events), [events]);
  const procs = useSessionProcs(taskId);
  const usage = useOwnersUsage().find((u) => sameOwner(u.owner, { kind: "Session", id: taskId }));
  const now = useNow(busy, 1000);
  const nodes = useMemo(() => mapNodes(tree, numbers, busy, now), [tree, numbers, busy, now]);
  const [sel, setSel] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState<Set<string>>(() => new Set());

  const rows = useMemo<Row[]>(() => {
    const starts = tree.map((n) => n.startedAt).filter((v): v is number => v != null);
    const ends = tree.map((n) => n.endedAt).filter((v): v is number => v != null);
    const main: Row = {
      id: null,
      depth: 0,
      name: "Principal",
      hint: task?.model ?? "modelo padrão",
      status: busy ? "running" : "ok",
      start: starts.length ? Math.min(...starts) : null,
      end: busy ? now : ends.length ? Math.max(...ends) : null,
      tokens: task?.context ? task.context.input_tokens + task.context.cache_read + task.context.cache_write : null,
      node: null,
      prompt: task?.prompt ?? "",
      result: null,
    };
    return [
      main,
      ...nodes.map((n, i) => ({
        id: n.id,
        depth: n.column,
        name: n.type || "subagente",
        hint: tree[i].description,
        status: n.status,
        start: tree[i].startedAt,
        end: n.status === "running" ? now : (tree[i].endedAt ?? (tree[i].startedAt != null && n.durationMs != null ? tree[i].startedAt! + n.durationMs : null)),
        tokens: n.tokens,
        node: n,
        prompt: tree[i].prompt,
        result: tree[i].result,
      })),
    ];
  }, [tree, nodes, task, busy, now]);

  const denied = useMemo<Denied[]>(() => {
    const out: Denied[] = [];
    events.forEach((raw, i) => {
      const u = unwrap(raw);
      if ("Denied" in u.event) out.push({ key: `d${i}`, owner: u.owner, tool: u.event.Denied.tool, command: u.event.Denied.command });
    });
    return out.filter((d) => !dismissed.has(d.key));
  }, [events, dismissed]);

  if (!task) {
    return (
      <div className="ui-area">
        <div className="ui-area-main">
          <AreaHeader title="Agentes" actions={<BackBtn />} />
          <p className="ui-empty">Sessão #{taskId} não encontrada.</p>
        </div>
      </div>
    );
  }

  const t0 = rows.reduce<number | null>((m, r) => (r.start == null ? m : m == null ? r.start : Math.min(m, r.start)), null);
  const t1 = Math.max(t0 ?? 0, ...rows.map((r) => r.end ?? 0));
  const span = t0 != null && t1 > t0 ? t1 - t0 : null;
  const running = rows.filter((r) => r.status === "running").length;
  const done = rows.filter((r) => r.status === "ok").length;
  const ownersDenied = new Set(denied.map((d) => d.owner ?? ""));
  const selRow = rows.find((r) => r.id === sel) ?? rows[0];
  const ticks = span != null ? [0, 1 / 3, 2 / 3].map((f) => hhmm(t0! + span * f)) : [];

  const totalOut = events.reduce((s, raw) => {
    const e = unwrap(raw);
    return e.owner == null && "Usage" in e.event ? s + e.event.Usage.output_tokens : s;
  }, 0);
  const ctx = task.context;
  const inCtx = ctx ? ctx.input_tokens + ctx.cache_read + ctx.cache_write : null;
  const cmdCount = new Map<string, number>();
  procs.filter((p) => !p.mcp).forEach((p) => cmdCount.set(p.name, (cmdCount.get(p.name) ?? 0) + 1));

  return (
    <div className="ui-area ag-area">
      <div className="ui-area-main ag-center">
        <AreaHeader
          title="Agentes"
          sub={sessionTitle(task.prompt, 60)}
          actions={
            <>
              <Btn variant="ghost" disabled title="O Argus ainda não consegue pausar agentes da CLI">
                Pausar todos
              </Btn>
              <Btn variant="ghost" className="ag-stop" disabled={!busy} onClick={() => void api.taskCancel(taskId)} title="Cancela o turno da sessão e todos os subagentes">
                Parar todos
              </Btn>
              <BackBtn />
            </>
          }
        >
          {running > 0 && <Chip tone="ok">{running} rodando</Chip>}
          {denied.length > 0 && <Chip tone="warn">{denied.length} aguardando você</Chip>}
        </AreaHeader>

        <div className="ag-row ag-hd">
          <span>Agente</span>
          <span className="ag-ticks">
            {ticks.map((t, i) => (
              <span key={i}>{t}</span>
            ))}
            {span != null && <span>{busy ? "agora" : hhmm(t1)}</span>}
          </span>
          <span className="ag-tk">Tokens</span>
        </div>
        {!loaded && <p className="ui-empty">Carregando…</p>}
        {loaded &&
          rows.map((r) => {
            const waiting = ownersDenied.has(r.id ?? "");
            const left = span != null && r.start != null ? ((r.start - t0!) / span) * 100 : 0;
            const width = span != null && r.start != null && r.end != null ? Math.max(1, ((r.end - r.start) / span) * 100) : r.id == null ? 100 : 0;
            const cls = r.id == null ? "main" : waiting ? "warn" : r.status === "running" ? "run" : r.status === "error" ? "bad" : "";
            return (
              <button
                key={r.id ?? "main"}
                className={`ag-row${selRow === r ? " on" : ""}`}
                onClick={() => setSel(r.id)}
                title={`${r.name}${r.hint ? ` · ${r.hint}` : ""}\n${subagentStatusLabel(r.status)}${r.node?.durationMs != null ? ` · ${shortDuration(r.node.durationMs)}` : ""}`}
              >
                <span className="agn" style={{ paddingLeft: r.depth * 16 }}>
                  <StatusDot tone={waiting ? "warn" : r.id == null ? "info" : toneOf(r.status)} />
                  {r.id == null || r.status === "running" ? <b>{r.name}</b> : <span>{r.name}</span>}
                  {r.hint && <span className="ui-mut ellipsis">{r.hint}</span>}
                </span>
                <span className="lane">
                  {width > 0 && <i className={cls} style={{ left: `${left}%`, width: `${Math.min(width, 100 - left)}%` }} />}
                  {waiting && <em>aguardando permissão</em>}
                </span>
                <span className="ag-tk">{r.tokens != null ? formatTokensPt(r.tokens) : "—"}</span>
              </button>
            );
          })}
        {loaded && rows.length === 1 && <p className="ui-empty ag-hint">Esta sessão ainda não criou subagentes.</p>}

        <div className="ag-foot">
          <Card>
            <div className="ui-mut ui-sm">Processos da sessão</div>
            {cmdCount.size === 0 ? "nenhum" : [...cmdCount].map(([n, c]) => `${n} ×${c}`).join(" · ")}
          </Card>
          <Card>
            <div className="ui-mut ui-sm">CPU / GPU</div>
            {usage && usage.processes > 0 ? `${usage.cpu.toFixed(0)}% / ${usage.gpu != null ? `${usage.gpu.toFixed(0)}%` : "—"}` : "sem processos medidos"}
          </Card>
          <Card>
            <div className="ui-mut ui-sm">Gasto da conversa</div>
            {inCtx == null
              ? "sem dados ainda"
              : `${formatTokensPt(inCtx)} no contexto${ctx?.window ? ` · ${Math.round((inCtx / ctx.window) * 100)}% do limite` : ""}${totalOut ? ` · ${formatTokensPt(totalOut)} gerados` : ""}`}
          </Card>
        </div>
        <AreaStatus>
          {running} rodando · {done} concluídos · {denied.length} aguardando
          <span className="ui-sp" />
          clique numa linha para ver o agente
        </AreaStatus>
      </div>

      <Detail
        task={task}
        row={selRow}
        events={events}
        denied={denied.filter((d) => d.owner === selRow.id)}
        busy={busy}
        onDismiss={(k) => setDismissed(new Set([...dismissed, k]))}
        procs={procs}
      />
    </div>
  );
}

function Detail({
  task,
  row,
  events,
  denied,
  busy,
  onDismiss,
  procs,
}: {
  task: Task;
  row: Row;
  events: AgentEvent[];
  denied: Denied[];
  busy: boolean;
  onDismiss(key: string): void;
  procs: OwnedProc[];
}) {
  const activity = useMemo(() => activityOf(events, row.id), [events, row.id]);
  const [err, setErr] = useState<string | null>(null);
  const d = denied[denied.length - 1];
  const shell = d && (d.tool === "Bash" || d.tool === "PowerShell") && !!d.command && !d.command.includes("\n");
  const allowAlways = async () => {
    if (!d) return;
    try {
      await api.permissionsAllowDenied(task.project, d.tool, d.command);
      onDismiss(d.key);
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  };
  const openChat = () => {
    useApp.getState().selectSubagent(task.id, row.id);
    useWorkbench.getState().openSession(task.id);
  };
  return (
    <aside className="ag-panel" aria-label="Detalhe do agente">
      <div className="ag-ph">
        <div className="ag-pt">
          <StatusDot tone={row.id == null ? "info" : toneOf(row.status)} />
          <b>{row.id == null ? "Principal" : row.hint || row.name}</b>
          <Chip>{row.name === "Principal" ? row.hint : row.name}</Chip>
        </div>
        <div className="ui-mut ui-sm ag-task">Tarefa: {row.prompt ? row.prompt.slice(0, 280) : "(vazia)"}</div>
      </div>

      {d && (
        <div className="ui-card ag-perm">
          <div className="ui-sm ag-perm-t">{row.id == null ? "O agente" : row.hint || row.name} teve {d.tool} negado</div>
          {d.command && <div className="ui-mono ui-sm">{d.command}</div>}
          <div className="ag-perm-b">
            <Btn small disabled title="A CLI não pausa esperando resposta: ela já negou. Libere no projeto e peça de novo na conversa.">
              Permitir
            </Btn>
            <Btn small variant="ghost" disabled={!shell} onClick={() => void allowAlways()} title={shell ? "Adiciona o comando aos permitidos deste projeto" : "Só comandos de shell de uma linha podem ser liberados aqui"}>
              Sempre neste projeto
            </Btn>
            <Btn small variant="ghost" onClick={() => onDismiss(d.key)} title="Mantém negado e esconde o aviso">
              Negar
            </Btn>
          </div>
          {err && <div className="ui-sm ag-err">{err}</div>}
        </div>
      )}

      <div className="ag-act">
        <div className="ui-mut ui-sm">{row.status === "running" ? "Atividade ao vivo" : "Atividade"}</div>
        {activity.length === 0 && <div className="ui-mut ui-sm">Nenhuma ferramenta usada.</div>}
        {activity.map((a) => (
          <div key={a.key} className="ag-tool">
            <span className={a.ok === false ? "ag-err" : undefined}>{a.text}</span> {a.mono && <span className="ui-mono ellipsis">{a.mono}</span>}
          </div>
        ))}
        {row.result != null && (
          <>
            <div className="ui-mut ui-sm ag-sec">Resultado</div>
            <pre className="ag-pre">{row.result || "(vazio)"}</pre>
          </>
        )}
        {row.id == null && procs.length > 0 && (
          <>
            <div className="ui-mut ui-sm ag-sec">Processos</div>
            {procs.map((p) => (
              <div key={p.pid} className="ag-tool ui-mono ellipsis" title={p.cmdline}>
                {p.mcp ? "MCP · " : ""}
                {prettyCmd(p.cmdline) || p.name}
              </div>
            ))}
          </>
        )}
      </div>

      <div className="ag-send">
        <input
          className="ag-input"
          disabled
          placeholder={row.id == null ? "Mande instruções pela conversa…" : "Mandar instrução a este agente…"}
          title={
            row.id == null
              ? "Para o agente principal, escreva na conversa (botão Abrir conversa)."
              : "A CLI não aceita mensagens para um subagente em andamento; só o agente principal recebe instruções."
          }
        />
        <Btn variant="ghost" small onClick={openChat} title="Abrir a conversa deste agente">
          Abrir conversa
        </Btn>
        <Btn
          variant="ghost"
          small
          disabled={row.id != null || !busy}
          onClick={() => void api.taskCancel(task.id)}
          title={row.id != null ? "Não dá para parar só um subagente; use Parar todos" : "Cancela o turno do agente"}
        >
          Parar
        </Btn>
      </div>
    </aside>
  );
}
