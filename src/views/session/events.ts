/**
 * Funções puras sobre a lista de eventos de uma tarefa: separar o chat
 * principal do chat de cada subagente, montar a árvore e somar o consumo.
 */
import { variantOf, type AgentEvent, type Pricing, type SubagentInfo, type TokenUsage } from "@/api";
import { splitPrompt } from "@/views/chat/attachments";

/**
 * Aviso da CLI que cita a organização da conta ("Warning: claude.ai MCP servers
 * blocked ..."): nunca é mostrado.
 */
export function isHiddenNotice(line: string): boolean {
  let text = line.trim();
  if (text.startsWith("[stderr]")) text = text.slice("[stderr]".length).trim();
  return /^warning:\s*claude\.ai mcp servers blocked/i.test(text);
}

/** Título de uma sessão: a primeira linha do primeiro pedido, sem o bloco de anexos. */
export function sessionTitle(prompt: string, max = 80): string {
  const text = splitPrompt(prompt).text.trim() || prompt.trim();
  const first = (text.split(/\r?\n/).find((l) => l.trim()) ?? "").replace(/\s+/g, " ").trim();
  if (!first) return "(sem texto)";
  const chars = Array.from(first);
  return chars.length > max ? `${chars.slice(0, max).join("")}…` : first;
}

/** Item exibido num chat. */
export type ChatEntry =
  | { type: "user"; text: string }
  | { type: "text"; text: string }
  | { type: "tool"; name: string; input: unknown }
  | { type: "result"; ok: boolean; summary: string }
  | { type: "usage"; usage: TokenUsage }
  | { type: "notices"; lines: string[] }
  | { type: "subagent"; id: string }
  /** Linha divisória "Agente trocado: X → Y" (ids de provedor). */
  | { type: "switch"; from: string; to: string }
  /** Ferramenta negada por falta de permissão. */
  | { type: "denied"; tool: string; command: string }
  | { type: "unknown"; kind: string };

/** Ferramentas que criam subagente: o `SubagentStart` já vira o cartão. */
const SPAWN_TOOLS = new Set(["agent", "task"]);

/** Dono do evento (null = agente principal) e o evento sem o invólucro `Scoped`. */
export function unwrap(event: AgentEvent): { owner: string | null; event: AgentEvent } {
  let owner: string | null = null;
  let cur = event;
  // O invólucro mais interno é o dono direto do evento.
  while ("Scoped" in cur) {
    owner = cur.Scoped.subagent;
    cur = cur.Scoped.event;
  }
  return { owner, event: cur };
}

function pushNotice(out: ChatEntry[], line: string): void {
  const text = line
    .split(/\r?\n/)
    .filter((l) => !isHiddenNotice(l))
    .join("\n")
    .trim();
  if (!text) return;
  const last = out[out.length - 1];
  if (last?.type === "notices") last.lines.push(text);
  else out.push({ type: "notices", lines: [text] });
}

/**
 * Itens do chat de `owner` (null = agente principal). Subagentes criados por
 * `owner` viram um cartão; o que acontece dentro deles fica fora.
 */
export function chatEntries(events: AgentEvent[], owner: string | null): ChatEntry[] {
  const out: ChatEntry[] = [];
  /** Posições dos `ToolUse` de criação ainda sem cartão; o `SubagentStart` os substitui. */
  const spawnTools: number[] = [];
  for (const raw of events) {
    const { owner: evOwner, event } = unwrap(raw);
    const { kind } = variantOf(event);
    if ("SubagentStart" in event) {
      if (event.SubagentStart.parent !== owner) continue;
      const card: ChatEntry = { type: "subagent", id: event.SubagentStart.id };
      const at = spawnTools.shift();
      if (at != null) out[at] = card;
      else out.push(card);
      continue;
    }
    if ("SubagentEnd" in event) continue;
    if (evOwner !== owner) continue;
    if ("Denied" in event) {
      out.push({ type: "denied", ...event.Denied });
      continue;
    }
    if ("ProviderSwitch" in event) {
      const last = out[out.length - 1];
      // Trocas seguidas sem nenhuma mensagem no meio viram uma divisória só.
      if (last?.type === "switch") {
        if (last.from === event.ProviderSwitch.to) out.pop();
        else last.to = event.ProviderSwitch.to;
      } else out.push({ type: "switch", ...event.ProviderSwitch });
      continue;
    }
    if ("User" in event) out.push({ type: "user", text: event.User });
    else if ("Text" in event) {
      if (event.Text.trim()) out.push({ type: "text", text: event.Text });
    } else if ("ToolUse" in event) {
      if (SPAWN_TOOLS.has(event.ToolUse.name.toLowerCase())) spawnTools.push(out.length);
      out.push({ type: "tool", name: event.ToolUse.name, input: event.ToolUse.input });
    } else if ("Result" in event) out.push({ type: "result", ok: event.Result.ok, summary: event.Result.summary });
    else if ("Usage" in event) out.push({ type: "usage", usage: event.Usage });
    else if ("Raw" in event) pushNotice(out, event.Raw);
    else if ("Session" in event) continue;
    else out.push({ type: "unknown", kind });
  }
  return out;
}

/** Subagente com os horários que a tela conhece (ms). */
export interface SubagentNode extends SubagentInfo {
  startedAt: number | null;
  endedAt: number | null;
  depth: number;
}

/** Início e fim (ms) de cada subagente, por id; null = desconhecido. */
export type SubagentTimes = Record<string, { startedAt: number | null; endedAt: number | null }>;

/**
 * Subagentes da tarefa em ordem de árvore (pai antes dos filhos), com a
 * profundidade de cada um. Mesma regra de `provider::subagents_of` do core.
 */
export function subagentTree(events: AgentEvent[], times: SubagentTimes = {}): SubagentNode[] {
  const flat: SubagentNode[] = [];
  const byId = new Map<string, SubagentNode>();
  for (const raw of events) {
    const { event } = unwrap(raw);
    if ("SubagentStart" in event) {
      const s = event.SubagentStart;
      const t = times[s.id];
      const node: SubagentNode = {
        ...s,
        done: false,
        ok: null,
        result: null,
        startedAt: t?.startedAt ?? null,
        endedAt: null,
        depth: 0,
      };
      byId.set(s.id, node);
      flat.push(node);
    } else if ("SubagentEnd" in event) {
      const node = byId.get(event.SubagentEnd.id);
      if (node) {
        node.done = true;
        node.ok = event.SubagentEnd.ok;
        node.result = event.SubagentEnd.result;
        node.endedAt = times[node.id]?.endedAt ?? null;
      }
    }
  }
  const out: SubagentNode[] = [];
  const placed = new Set<SubagentNode>();
  const visit = (parent: string | null, depth: number) => {
    for (const node of flat) {
      if (node.parent !== parent || placed.has(node)) continue;
      node.depth = depth;
      placed.add(node);
      out.push(node);
      visit(node.id, depth + 1);
    }
  };
  visit(null, 0);
  // Pai desconhecido (histórico cortado): mostra na raiz em vez de sumir.
  for (const node of flat) if (!placed.has(node)) out.push(node);
  return out;
}

export type SubagentStatus = "running" | "ok" | "error" | "interrupted";

/** `taskActive` = o turno ainda roda; subagente sem fim com a tarefa parada foi interrompido. */
export function subagentStatus(node: Pick<SubagentInfo, "done" | "ok">, taskActive: boolean): SubagentStatus {
  if (node.done) return node.ok === false ? "error" : "ok";
  return taskActive ? "running" : "interrupted";
}

export function subagentStatusLabel(status: SubagentStatus): string {
  switch (status) {
    case "running":
      return "rodando";
    case "ok":
      return "concluído";
    case "error":
      return "falhou";
    case "interrupted":
      return "interrompido";
  }
}

/** 8s, 2min 05s, 1h 03min. */
export function formatDuration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}min ${String(s % 60).padStart(2, "0")}s`;
  return `${Math.floor(s / 3600)}h ${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}min`;
}

/** Duração do subagente; null quando a tela não conhece o início (ou o fim de um já terminado). */
export function subagentDuration(node: SubagentNode, status: SubagentStatus, nowMs: number): string | null {
  if (node.startedAt == null) return null;
  if (status === "running") return formatDuration(nowMs - node.startedAt);
  if (node.endedAt == null) return null;
  return formatDuration(node.endedAt - node.startedAt);
}

/** Resumo de um aviso (stderr/sistema): primeira linha, até 90 caracteres. */
export function noticeSummary(raw: string): string {
  const lines = raw.trim().split(/\r?\n/);
  let first = (lines[0] ?? "").trim();
  if (first.startsWith("[stderr]")) first = first.slice("[stderr]".length).trim();
  const chars = Array.from(first);
  const cut = chars.slice(0, 90).join("");
  return chars.length > 90 || lines.length > 1 ? `${cut}…` : cut;
}

/** Mesma regra de `Pricing::price_for` do core: a chave mais longa contida no nome do modelo. */
function priceFor(pricing: Pricing, model: string | null) {
  if (!model) return pricing.fallback;
  const m = model.toLowerCase();
  let best: string | null = null;
  for (const key of Object.keys(pricing.models)) {
    if (m.includes(key.toLowerCase()) && (best == null || key.length > best.length)) best = key;
  }
  return best ? pricing.models[best] : pricing.fallback;
}

/** Custo equivalente em API (USD) de um uso; mesma regra de `Pricing::cost` do core. */
export function usageCost(usage: TokenUsage, pricing: Pricing): number {
  if (pricing.prefer_reported && usage.cost_usd != null && usage.cost_usd > 0) return usage.cost_usd;
  const p = priceFor(pricing, usage.model);
  return (
    (usage.input_tokens * p.input +
      usage.output_tokens * p.output +
      usage.cache_read * p.cache_read +
      usage.cache_write * p.cache_write) /
    1e6
  );
}

export interface UsageTotals {
  input_tokens: number;
  output_tokens: number;
  cache_read: number;
  cache_write: number;
  turns: number;
  /** Custo equivalente em API (USD); null sem tabela de preços. */
  cost: number | null;
}

/** Soma dos `Usage` do agente principal (o do claude já inclui os subagentes). */
export function usageTotals(events: AgentEvent[], pricing: Pricing | null): UsageTotals {
  const t: UsageTotals = { input_tokens: 0, output_tokens: 0, cache_read: 0, cache_write: 0, turns: 0, cost: pricing ? 0 : null };
  for (const event of events) {
    if (!("Usage" in event)) continue;
    const u = event.Usage;
    t.input_tokens += u.input_tokens;
    t.output_tokens += u.output_tokens;
    t.cache_read += u.cache_read;
    t.cache_write += u.cache_write;
    t.turns += 1;
    if (pricing && t.cost != null) t.cost += usageCost(u, pricing);
  }
  return t;
}

/** Um turno da conversa: a fala do usuário (se houver) e o que o agente fez até a próxima. */
export interface Turn {
  user: string | null;
  items: ChatEntry[];
}

export function splitTurns(entries: ChatEntry[]): Turn[] {
  const turns: Turn[] = [];
  for (const entry of entries) {
    if (entry.type === "user") turns.push({ user: entry.text, items: [] });
    // A divisória de troca de agente fica entre os turnos, fora do agrupamento de passos.
    else if (entry.type === "switch") turns.push({ user: null, items: [entry] }, { user: null, items: [] });
    else {
      if (turns.length === 0) turns.push({ user: null, items: [] });
      turns[turns.length - 1].items.push(entry);
    }
  }
  return turns.filter((t) => t.user != null || t.items.length > 0);
}

/**
 * Num turno terminado, o que veio antes da resposta final (ferramentas, textos
 * intermediários, subagentes) vira um grupo "N passos" recolhido. Devolve o
 * índice da resposta final em `items`, ou -1 quando não há o que agrupar.
 */
export function stepsCut(items: ChatEntry[]): number {
  let last = -1;
  for (let i = items.length - 1; i >= 0; i--) {
    if (items[i].type === "text") {
      last = i;
      break;
    }
  }
  if (last <= 0) return -1;
  const before = items.slice(0, last);
  return before.some((e) => e.type === "tool" || e.type === "subagent") ? last : -1;
}

export function stepsLabel(items: ChatEntry[]): string {
  const steps = items.filter((e) => e.type === "tool" || e.type === "text" || e.type === "subagent").length;
  const subs = items.filter((e) => e.type === "subagent").length;
  const label = steps === 1 ? "1 passo" : `${steps} passos`;
  return subs > 0 ? `${label} · ${subs === 1 ? "1 subagente" : `${subs} subagentes`}` : label;
}
