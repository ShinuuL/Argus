/**
 * Funções puras do painel Pendências: a lista TodoWrite do agente no turno
 * atual e os itens do markdown de pendências que o agente acabou de editar.
 */
import type { AgentEvent } from "@/api";
import { unwrap } from "./events";

export type TodoStatus = "pending" | "in_progress" | "completed";

export interface AgentTodo {
  content: string;
  /** Texto no gerúndio ("Corrigindo…"), quando a CLI manda. */
  activeForm: string;
  status: TodoStatus;
}

/** Eventos do agente principal desde a última mensagem do usuário (o turno atual ou o último). */
function lastTurn(events: AgentEvent[]): AgentEvent[] {
  let start = 0;
  for (let i = events.length - 1; i >= 0; i--) {
    const { owner, event } = unwrap(events[i]);
    if (owner == null && "User" in event) {
      start = i + 1;
      break;
    }
  }
  return events.slice(start);
}

function asRecord(v: unknown): Record<string, unknown> | null {
  return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
}

/** Última lista TodoWrite do agente principal no turno; null se ele não usou. */
export function agentTodos(events: AgentEvent[]): AgentTodo[] | null {
  const turn = lastTurn(events);
  for (let i = turn.length - 1; i >= 0; i--) {
    const { owner, event } = unwrap(turn[i]);
    if (owner != null || !("ToolUse" in event) || event.ToolUse.name !== "TodoWrite") continue;
    const list = asRecord(event.ToolUse.input)?.todos;
    if (!Array.isArray(list)) continue;
    return list
      .map(asRecord)
      .filter((t): t is Record<string, unknown> => t != null && typeof t.content === "string")
      .map((t) => ({
        content: String(t.content),
        activeForm: typeof t.activeForm === "string" ? t.activeForm : "",
        status: t.status === "in_progress" || t.status === "completed" ? t.status : "pending",
      }));
  }
  return null;
}

/** Normaliza o texto de um item para comparar (sem marcador, ênfase e espaços repetidos). */
export function itemKey(text: string): string {
  return text
    .replace(/^\s*[-*+]\s+\[[ xX]\]\s*/, "")
    .replace(/\*\*|__|`/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .toLowerCase();
}

const ITEM_LINE = /^\s*[-*+]\s+\[[ xX]\]\s+\S/;

/** Linhas de item que aparecem em `next` e não estavam iguais em `prev`. */
export function changedItems(prev: string, next: string): string[] {
  const before = new Set(prev.split(/\r?\n/).filter((l) => ITEM_LINE.test(l)).map((l) => l.trim()));
  return next
    .split(/\r?\n/)
    .filter((l) => ITEM_LINE.test(l) && !before.has(l.trim()))
    .map(itemKey);
}

/** Nome do arquivo de um caminho (Windows ou Unix), em minúsculas. */
function fileName(p: string): string {
  return (p.split(/[\\/]/).pop() ?? "").toLowerCase();
}

export interface ChecklistTouch {
  /** Itens (normalizados por `itemKey`) que o agente mudou na última edição. */
  keys: string[];
  /** true quando o arquivo foi reescrito inteiro (Write) e não dá para saber o item. */
  whole: boolean;
}

/**
 * Última edição (Edit, MultiEdit ou Write) do agente no arquivo de pendências
 * durante o turno. Compara só o nome (`name`): a sessão pode estar num
 * worktree isolado, com outro caminho para o mesmo arquivo.
 */
export function checklistTouch(events: AgentEvent[], name: string): ChecklistTouch | null {
  const turn = lastTurn(events);
  for (let i = turn.length - 1; i >= 0; i--) {
    const { owner, event } = unwrap(turn[i]);
    if (owner != null || !("ToolUse" in event)) continue;
    const input = asRecord(event.ToolUse.input);
    const file = input?.file_path ?? input?.path;
    if (typeof file !== "string" || fileName(file) !== name.toLowerCase()) continue;
    const tool = event.ToolUse.name;
    if (tool === "Write") return { keys: [], whole: true };
    const edits = tool === "MultiEdit" && Array.isArray(input?.edits) ? input.edits.map(asRecord) : [input];
    const keys: string[] = [];
    for (const e of edits) {
      if (e && typeof e.old_string === "string" && typeof e.new_string === "string") keys.push(...changedItems(e.old_string, e.new_string));
    }
    return { keys, whole: false };
  }
  return null;
}
