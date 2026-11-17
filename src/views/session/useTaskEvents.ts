/** Eventos de uma tarefa: histórico gravado + stream ao vivo, sem duplicar nem perder a emenda. */
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { api, errorMessage, onTaskEvent, type AgentEvent, type TaskId } from "@/api";
import { unwrap, type SubagentTimes } from "./events";

/** Resposta de `task_subagent_times` (comando extra; pode ainda não estar registrado). */
interface BackendTime {
  id: string;
  started_at: number;
  ended_at: number | null;
}

/**
 * Junta o histórico com os eventos que chegaram ao vivo enquanto ele era lido:
 * o começo de `live` que repete o fim de `history` é descartado.
 */
export function mergeHistory(history: AgentEvent[], live: AgentEvent[]): AgentEvent[] {
  if (live.length === 0) return history;
  if (history.length === 0) return live;
  const liveJson = live.map((e) => JSON.stringify(e));
  const max = Math.min(history.length, live.length);
  for (let k = max; k > 0; k--) {
    let same = true;
    for (let i = 0; i < k && same; i++) {
      same = JSON.stringify(history[history.length - k + i]) === liveJson[i];
    }
    if (same) return [...history, ...live.slice(k)];
  }
  return [...history, ...live];
}

export interface TaskEvents {
  events: AgentEvent[];
  /** Início e fim (ms) dos subagentes: vistos ao vivo ou lidos do histórico. */
  times: SubagentTimes;
  /** false até a primeira leitura do histórico terminar. */
  loaded: boolean;
  error: string | null;
}

/**
 * `active` = turno em andamento. Quando ele termina, o histórico é relido para
 * a tela ficar igual ao que foi gravado.
 */
export function useTaskEvents(taskId: TaskId | null, active: boolean): TaskEvents {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [times, setTimes] = useState<SubagentTimes>({});
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Relê o histórico (definido pelo efeito abaixo). */
  const resync = useRef<() => void>(() => undefined);

  useEffect(() => {
    let alive = true;
    /** Durante uma leitura do histórico, o que chega ao vivo espera aqui. */
    let pending: AgentEvent[] | null = [];
    setEvents([]);
    setTimes({});
    setLoaded(taskId == null);
    setError(null);
    // Conversa ainda sem tarefa: nada a ler nem a ouvir.
    if (taskId == null) return;

    const stamp = (raw: AgentEvent) => {
      const { event } = unwrap(raw);
      const now = Date.now();
      if ("SubagentStart" in event) {
        const id = event.SubagentStart.id;
        setTimes((t) => ({ ...t, [id]: { startedAt: now, endedAt: null } }));
      } else if ("SubagentEnd" in event) {
        const id = event.SubagentEnd.id;
        setTimes((t) => ({ ...t, [id]: { startedAt: t[id]?.startedAt ?? null, endedAt: now } }));
      }
    };

    const loadTimes = () =>
      invoke<BackendTime[]>("task_subagent_times", { id: taskId }).then(
        (rows) => {
          if (!alive) return;
          setTimes((t) => {
            const next = { ...t };
            for (const r of rows) {
              const cur = next[r.id];
              next[r.id] = {
                startedAt: cur?.startedAt ?? r.started_at * 1000,
                endedAt: cur?.endedAt ?? (r.ended_at == null ? null : r.ended_at * 1000),
              };
            }
            return next;
          });
        },
        // Comando ainda não registrado: ficam só os horários vistos ao vivo.
        () => undefined,
      );

    const sync = (first: boolean) => {
      pending ??= [];
      api.taskHistory(taskId).then(
        (history) => {
          if (!alive) return;
          const live = pending ?? [];
          pending = null;
          // Sem banco o histórico vem vazio: na releitura, mantém o que a tela já tem.
          setEvents((cur) => (history.length === 0 && !first ? [...cur, ...live] : mergeHistory(history, live)));
          setLoaded(true);
          void loadTimes();
        },
        (e) => {
          if (!alive) return;
          const live = pending ?? [];
          pending = null;
          setEvents((cur) => [...cur, ...live]);
          setLoaded(true);
          setError(errorMessage(e));
        },
      );
    };
    resync.current = () => sync(false);

    const off = onTaskEvent(taskId, (event) => {
      if (!alive) return;
      stamp(event);
      if (pending) pending.push(event);
      else setEvents((cur) => [...cur, event]);
    });
    // Lê o histórico só depois de já estar ouvindo, para não perder o que sai no meio.
    void off.then(() => alive && sync(true));

    return () => {
      alive = false;
      void off.then((f) => f());
    };
  }, [taskId]);

  const wasActive = useRef(active);
  useEffect(() => {
    if (wasActive.current && !active) {
      // Dá tempo de os últimos eventos do turno serem gravados.
      const timer = setTimeout(() => resync.current(), 400);
      wasActive.current = active;
      return () => clearTimeout(timer);
    }
    wasActive.current = active;
  }, [active]);

  return { events, times, loaded, error };
}
