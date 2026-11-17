/**
 * Funções puras do Mapa de agentes: números de cada subagente, posições dos
 * nós (colunas por profundidade) e as linhas entre eles, e os comandos em
 * segundo plano pedidos pelo agente. Sem React, para poder conferir com node.
 */
import type { AgentEvent } from "@/api";
import { SPAWN_TOOLS, subagentStatus, unwrap, type SubagentNode, type SubagentStatus } from "@/views/session/events";

/** Números que a CLI informou no fim de um subagente (`SubagentStats`). */
export interface SubagentNumbers {
  duration_ms: number | null;
  tokens: number | null;
  tool_uses: number | null;
}

export function subagentNumbers(events: AgentEvent[]): Record<string, SubagentNumbers> {
  const out: Record<string, SubagentNumbers> = {};
  for (const raw of events) {
    const { event } = unwrap(raw);
    if ("SubagentStats" in event) {
      const s = event.SubagentStats;
      out[s.id] = { duration_ms: s.duration_ms ?? null, tokens: s.tokens ?? null, tool_uses: s.tool_uses ?? null };
    }
  }
  return out;
}

/** Nó de subagente pronto para desenhar. */
export interface MapNode {
  id: string;
  parent: string | null;
  /** Coluna (1 = chamado pela sessão principal). */
  column: number;
  name: string;
  type: string;
  status: SubagentStatus;
  /** ms; null = desconhecida. */
  durationMs: number | null;
  /** true = informada pela CLI; false = calculada pelos horários dos eventos. */
  durationExact: boolean;
  tokens: number | null;
  toolUses: number | null;
}

/**
 * Junta a árvore (já em ordem pai → filhos) com os números da CLI. A duração
 * vem do `SubagentStats` quando existe; senão é o tempo entre o início e o fim
 * vistos (ou até `now`, enquanto roda).
 */
export function mapNodes(
  tree: SubagentNode[],
  numbers: Record<string, SubagentNumbers>,
  taskActive: boolean,
  now: number,
): MapNode[] {
  return tree.map((n) => {
    const status = subagentStatus(n, taskActive);
    const num = numbers[n.id];
    let durationMs: number | null = null;
    let durationExact = false;
    if (num?.duration_ms != null) {
      durationMs = num.duration_ms;
      durationExact = true;
    } else if (n.startedAt != null) {
      if (status === "running") durationMs = Math.max(0, now - n.startedAt);
      else if (n.endedAt != null) durationMs = Math.max(0, n.endedAt - n.startedAt);
    }
    return {
      id: n.id,
      parent: n.parent,
      column: n.depth + 1,
      name: n.description || n.subagent_type || n.id,
      type: n.subagent_type,
      status,
      durationMs,
      durationExact,
      tokens: num?.tokens ?? null,
      toolUses: num?.tool_uses ?? null,
    };
  });
}

export interface LayoutOptions {
  nodeW: number;
  nodeH: number;
  /** Espaço horizontal entre colunas. */
  gapX: number;
  /** Espaço vertical entre linhas. */
  gapY: number;
  pad: number;
}

export const DEFAULT_LAYOUT: LayoutOptions = { nodeW: 220, nodeH: 64, gapX: 64, gapY: 12, pad: 16 };

export interface Box {
  x: number;
  y: number;
}

export interface Edge {
  from: string | null;
  to: string;
  /** Caminho SVG (curva da borda direita do pai à borda esquerda do filho). */
  d: string;
}

export interface GraphLayout {
  /** Nó da sessão principal. */
  root: Box;
  nodes: Record<string, Box>;
  edges: Edge[];
  width: number;
  height: number;
}

/**
 * Cada subagente ocupa uma linha, na ordem da árvore (pai antes dos filhos),
 * e a coluna da sua profundidade. A sessão principal fica na coluna 0, no
 * meio da altura. Pai desconhecido liga na sessão principal.
 */
export function layoutGraph(nodes: Pick<MapNode, "id" | "parent" | "column">[], o: LayoutOptions = DEFAULT_LAYOUT): GraphLayout {
  const pos: Record<string, Box> = {};
  const rowY = (i: number) => o.pad + i * (o.nodeH + o.gapY);
  const colX = (c: number) => o.pad + c * (o.nodeW + o.gapX);
  nodes.forEach((n, i) => {
    pos[n.id] = { x: colX(n.column), y: rowY(i) };
  });
  const rows = Math.max(1, nodes.length);
  const contentH = rows * o.nodeH + (rows - 1) * o.gapY;
  const root: Box = { x: colX(0), y: o.pad + (contentH - o.nodeH) / 2 };
  const edges: Edge[] = nodes.map((n) => {
    const parentBox = (n.parent != null && pos[n.parent]) || root;
    const from = n.parent != null && pos[n.parent] ? n.parent : null;
    const child = pos[n.id];
    const x1 = parentBox.x + o.nodeW;
    const y1 = parentBox.y + o.nodeH / 2;
    const x2 = child.x;
    const y2 = child.y + o.nodeH / 2;
    const mx = (x1 + x2) / 2;
    return { from, to: n.id, d: `M${x1} ${y1} C${mx} ${y1} ${mx} ${y2} ${x2} ${y2}` };
  });
  const maxCol = nodes.reduce((m, n) => Math.max(m, n.column), 0);
  return {
    root,
    nodes: pos,
    edges,
    width: colX(maxCol) + o.nodeW + o.pad,
    height: o.pad * 2 + contentH,
  };
}

/** Comando de shell que o agente pediu para rodar em segundo plano. */
export interface BackgroundCommand {
  /** Ordem do pedido na sessão (para chave estável). */
  index: number;
  /** Quem pediu: null = sessão principal; senão o id do subagente. */
  owner: string | null;
  tool: string;
  command: string;
  description: string;
  /** Resposta da ferramenta ao pedido (null = ainda sem resposta). */
  result: { ok: boolean; output: string } | null;
}

const SHELL_TOOLS = new Set(["bash", "powershell"]);

/**
 * `ToolUse` de shell com `run_in_background: true`, na ordem em que foram
 * pedidos, com a resposta da ferramenta (o aviso de que o comando foi para o
 * segundo plano, ou o erro). As saídas vêm na ordem dos `ToolUse` comuns do
 * mesmo dono, como em `chatEntries`.
 */
export function backgroundCommands(events: AgentEvent[]): BackgroundCommand[] {
  const out: BackgroundCommand[] = [];
  const pending = new Map<string, (BackgroundCommand | null)[]>();
  const queue = (owner: string | null) => {
    const k = owner ?? "";
    let q = pending.get(k);
    if (!q) pending.set(k, (q = []));
    return q;
  };
  for (const raw of events) {
    const { owner, event } = unwrap(raw);
    if ("User" in event && owner == null) pending.clear();
    if ("ToolResult" in event) {
      const cmd = queue(owner).shift();
      if (cmd) cmd.result = { ok: event.ToolResult.ok, output: event.ToolResult.output };
      continue;
    }
    if (!("ToolUse" in event)) continue;
    const { name, input } = event.ToolUse;
    if (SPAWN_TOOLS.has(name.toLowerCase())) continue;
    const inp = input != null && typeof input === "object" ? (input as Record<string, unknown>) : null;
    if (!SHELL_TOOLS.has(name.toLowerCase()) || inp?.run_in_background !== true) {
      queue(owner).push(null);
      continue;
    }
    const cmd: BackgroundCommand = {
      index: out.length,
      owner,
      tool: name,
      command: typeof inp.command === "string" ? inp.command : "",
      description: typeof inp.description === "string" ? inp.description : "",
      result: null,
    };
    queue(owner).push(cmd);
    out.push(cmd);
  }
  return out;
}

/** Texto sem aspas e com espaços simples (para achar o comando na linha de um processo). */
function squash(s: string): string {
  return s.replace(/["'`]/g, "").replace(/\s+/g, " ").trim().toLowerCase();
}

/** Processo vivo da sessão que roda `command` (pelo começo da linha de comando), se houver. */
export function commandProcess<P extends { cmdline: string }>(command: string, procs: P[]): P | null {
  const key = squash(command).slice(0, 60);
  if (key.length < 3) return null;
  return procs.find((p) => squash(p.cmdline).includes(key)) ?? null;
}

export type TaskGroup = "running" | "done" | "failed";

/** Estado de um comando em segundo plano: falhou ao iniciar, ainda roda (processo vivo) ou acabou. */
export function commandGroup(c: BackgroundCommand, alive: boolean, taskActive: boolean): TaskGroup {
  if (c.result && !c.result.ok) return "failed";
  if (alive || (c.result == null && taskActive)) return "running";
  return "done";
}

/** Grupo de um subagente na lista de tarefas. */
export function subagentGroup(status: SubagentStatus): TaskGroup {
  return status === "running" ? "running" : status === "error" ? "failed" : "done";
}

/** "16 s", "1 min 02 s", "1 h 05 min" (estilo do painel de tarefas). */
export function shortDuration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s} s`;
  if (s < 3600) return `${Math.floor(s / 60)} min ${String(s % 60).padStart(2, "0")} s`;
  return `${Math.floor(s / 3600)} h ${String(Math.floor((s % 3600) / 60)).padStart(2, "0")} min`;
}

/** "N agentes" contando a sessão principal. */
export function agentsLabel(subagents: number): string {
  const n = subagents + 1;
  return n === 1 ? "1 agente" : `${n} agentes`;
}

/** Linha de comando legível: o shell do claude embrulha o comando em `eval '...'`. */
export function prettyCmd(cmdline: string): string {
  const m = /eval '((?:[^']|'\'')*)'/.exec(cmdline);
  return m ? m[1].replace(/'\''/g, "'") : cmdline;
}
