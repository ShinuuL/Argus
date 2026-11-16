/**
 * Funções puras do Mapa de agentes: números de cada subagente, posições dos
 * nós (colunas por profundidade) e as linhas entre eles, e os comandos em
 * segundo plano pedidos pelo agente. Sem React, para poder conferir com node.
 */
import type { AgentEvent } from "@/api";
import { subagentStatus, unwrap, type SubagentNode, type SubagentStatus } from "@/views/session/events";

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
}

const SHELL_TOOLS = new Set(["bash", "powershell"]);

/** `ToolUse` de shell com `run_in_background: true`, na ordem em que foram pedidos. */
export function backgroundCommands(events: AgentEvent[]): BackgroundCommand[] {
  const out: BackgroundCommand[] = [];
  for (const raw of events) {
    const { owner, event } = unwrap(raw);
    if (!("ToolUse" in event)) continue;
    const { name, input } = event.ToolUse;
    if (!SHELL_TOOLS.has(name.toLowerCase()) || input == null || typeof input !== "object") continue;
    const inp = input as Record<string, unknown>;
    if (inp.run_in_background !== true) continue;
    out.push({
      index: out.length,
      owner,
      tool: name,
      command: typeof inp.command === "string" ? inp.command : "",
      description: typeof inp.description === "string" ? inp.description : "",
    });
  }
  return out;
}

/** "N agentes" contando a sessão principal. */
export function agentsLabel(subagents: number): string {
  const n = subagents + 1;
  return n === 1 ? "1 agente" : `${n} agentes`;
}
