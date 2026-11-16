/**
 * Indicador de estado do agente: um ícone pequeno (spinner, check, alerta...)
 * com tooltip em pt-BR, usado na barra lateral, nas abas e no cabeçalho do chat.
 */
import { variantOf, type AgentEvent, type Task, type TaskState } from "@/api";
import { Icon, type IconName } from "@/shell/icons";
import { useApp } from "@/store";
import { unwrap } from "./events";

export type AgentStateKind =
  | "working"
  | "queued"
  | "attention"
  | "failed"
  | "done"
  | "applied"
  | "interrupted"
  | "idle";

interface StateInfo {
  icon: IconName;
  label: string;
}

const INFO: Record<AgentStateKind, StateInfo> = {
  working: { icon: "spinner", label: "Trabalhando" },
  queued: { icon: "circleDashed", label: "Na fila" },
  attention: { icon: "shieldAlert", label: "Precisa de atenção" },
  failed: { icon: "circleAlert", label: "Falhou" },
  done: { icon: "circleCheck", label: "Concluída" },
  applied: { icon: "circleCheck", label: "Aplicada" },
  interrupted: { icon: "circleSlash", label: "Interrompida" },
  idle: { icon: "circleDot", label: "Descartada" },
};

/** Estado visível de uma sessão: o do core, mais o "precisa de atenção" (comando negado). */
export function agentStateOf(state: TaskState, denied: string | null | undefined): AgentStateKind {
  const { kind } = variantOf(state);
  switch (kind) {
    case "Running":
      return "working";
    case "Queued":
      return "queued";
    case "Failed":
      return "failed";
    case "Cancelled":
      return "interrupted";
    case "Discarded":
      return "idle";
    case "Applied":
      return "applied";
    default:
      return denied ? "attention" : "done";
  }
}

/** Rótulo curto do estado (ex.: "Concluída", "Precisa de atenção"). */
export function agentStateLabel(kind: AgentStateKind): string {
  return INFO[kind].label;
}

/** Texto do tooltip: rótulo, mais o motivo quando houver. */
export function agentStateTitle(kind: AgentStateKind, state: TaskState, denied?: string | null): string {
  const base = INFO[kind].label;
  if (kind === "failed") {
    const { data } = variantOf(state);
    return data ? `${base}: ${String(data)}` : base;
  }
  if (kind === "attention") return `${base}: comando negado${denied ? ` (${denied})` : ""}`;
  return base;
}

/** Comando negado no último turno dos eventos (null = nenhum). */
export function lastTurnDenied(events: AgentEvent[]): string | null {
  for (let i = events.length - 1; i >= 0; i--) {
    const { owner, event } = unwrap(events[i]);
    if (owner == null && "User" in event) return null;
    if ("Denied" in event) return event.Denied.command || event.Denied.tool;
  }
  return null;
}

export interface AgentStateProps {
  task: Task;
  size?: number;
  /** Mostra o rótulo ao lado do ícone (cabeçalho do chat). */
  label?: boolean;
}

/** Ícone de estado de uma sessão, com tooltip; o rótulo é opcional. */
export function AgentState({ task, size = 14, label = false }: AgentStateProps) {
  const denied = useApp((s) => s.attention[task.id]);
  const kind = agentStateOf(task.state, denied);
  const title = agentStateTitle(kind, task.state, denied);
  return (
    <span
      className={`agent-state ${kind}${label ? " with-label" : ""}`}
      title={title}
      role="img"
      aria-label={title}
      data-state={kind}
    >
      <Icon name={INFO[kind].icon} size={size} />
      {label && <span className="agent-state-label">{INFO[kind].label}</span>}
    </span>
  );
}
