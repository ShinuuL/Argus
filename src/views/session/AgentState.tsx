/**
 * Indicador de estado do agente: um ícone pequeno (spinner, check, alerta...)
 * com tooltip em pt-BR, usado na barra lateral, nas abas e no cabeçalho do chat.
 */
import { useEffect, useState, type MutableRefObject } from "react";
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

/** Estado visível de um subagente (mesmos ícones das sessões). */
export function subagentStateKind(status: "running" | "ok" | "error" | "interrupted"): AgentStateKind {
  return status === "running" ? "working" : status === "ok" ? "done" : status === "error" ? "failed" : "interrupted";
}

/** Só o ícone de um estado, sem precisar de uma `Task` (subagentes, comandos). */
export function AgentStateIcon({ kind, size = 13, title }: { kind: AgentStateKind; size?: number; title?: string }) {
  const t = title ?? INFO[kind].label;
  return (
    <span className={`agent-state ${kind}`} title={t} role="img" aria-label={t} data-state={kind}>
      <Icon name={INFO[kind].icon} size={size} />
    </span>
  );
}

/** A partir daqui o silêncio da CLI deixa de ser espera normal e vira aviso. */
const STALL_MS = 60_000;

export interface StalledPillProps {
  /** Último sinal de vida do agente (ref: os deltas chegam rápido demais para virar estado). */
  lastEventAt: MutableRefObject<number | null>;
  /** Turno em andamento; fora dele não há o que avisar. */
  active: boolean;
  /** Interrompe o turno. Ausente enquanto já está sendo interrompido. */
  onStop?(): void;
  /** Compactação em andamento ("size" ou "idle"), que explica o silêncio. */
  compacting?: string | null;
}

/** Por que o Argus resolveu compactar, em uma linha. */
const COMPACT_WHY: Record<string, string> = {
  size: "A conversa passou do limite de contexto: resumir agora deixa o turno mais rápido e mais barato.",
  idle: "A conversa ficou parada além da vida do cache (1 h): sem resumir, o turno pagaria o histórico inteiro de novo.",
};

/**
 * Quanto tempo faz que a CLI não dá sinal de vida, com o atalho para desistir.
 * Fica invisível enquanto o silêncio for curto: o aviso só aparece quando a
 * espera deixa de ser normal.
 *
 * Durante uma compactação o silêncio é esperado e pode durar minutos, então a
 * pílula aparece de imediato dizendo o que está acontecendo, em vez de só
 * reclamar da demora depois de um minuto.
 */
export function StalledPill({ lastEventAt, active, onStop, compacting }: StalledPillProps) {
  const [quiet, setQuiet] = useState(0);

  useEffect(() => {
    if (!active) {
      setQuiet(0);
      return;
    }
    // Sem evento neste turno ainda, o silêncio conta do começo dele, não do
    // último evento do turno anterior.
    const since = Date.now();
    const read = () => setQuiet(Date.now() - Math.max(lastEventAt.current ?? 0, since));
    read();
    const timer = setInterval(read, 1000);
    return () => clearInterval(timer);
  }, [active, lastEventAt]);

  if (!active || (!compacting && quiet < STALL_MS)) return null;
  const mins = Math.floor(quiet / 60_000);
  const label = compacting
    ? `Compactando a conversa${mins >= 1 ? ` · ${mins} min` : ""}`
    : mins < 1
      ? "sem resposta há menos de 1 min"
      : `${mins} min sem resposta`;
  const title = compacting ? (COMPACT_WHY[compacting] ?? "Resumindo a conversa antes de responder.") : undefined;
  return (
    <span className={`agent-stalled${compacting ? " compacting" : ""}`} role="status" title={title}>
      <Icon name={compacting ? "collapse" : "clock"} size={12} />
      <span className="agent-stalled-label">{label}</span>
      {onStop && (
        <button type="button" className="agent-stalled-stop" onClick={onStop}>
          Interromper
        </button>
      )}
    </span>
  );
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
