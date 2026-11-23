/** Eventos de uma tarefa: histórico gravado + stream ao vivo, sem duplicar nem perder a emenda. */
import { useEffect, useRef, useState, type MutableRefObject } from "react";
import { invoke } from "@tauri-apps/api/core";
import { api, errorMessage, onOrchLagged, onTaskEvent, type AgentEvent, type SubagentProgress, type TaskId } from "@/api";
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
  /** Texto do agente ainda sendo gerado (some quando chega o bloco completo). */
  draft: string;
  /** Raciocínio ainda sendo gerado (some quando chega o bloco completo ou o texto). */
  thinking: string;
  /** Último andamento de cada subagente (por id), visto ao vivo. */
  progress: Record<string, SubagentProgress>;
  /**
   * Quando chegou o último sinal de vida do agente (ms), contando os deltas de
   * texto. Serve para dizer há quanto tempo a CLI está muda. null = nada ainda.
   *
   * É uma ref de propósito: os deltas chegam a cada token e gravar isso em
   * estado redesenharia a conversa inteira só para mexer num relógio. Quem
   * mostra o tempo já tem o seu próprio tique de 1 s e lê a ref nele.
   */
  lastEventAt: MutableRefObject<number | null>;
  /**
   * Motivo da compactação em andamento ("size" ou "idle"), ou null. Enquanto
   * vale, o silêncio da CLI está explicado: ela está resumindo a conversa.
   */
  compacting: string | null;
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
  const [draft, setDraft] = useState("");
  const [thinking, setThinking] = useState("");
  const [progress, setProgress] = useState<Record<string, SubagentProgress>>({});
  const [compacting, setCompacting] = useState<string | null>(null);
  /** Relê o histórico (definido pelo efeito abaixo). */
  const resync = useRef<() => void>(() => undefined);
  const lastEventAt = useRef<number | null>(null);

  useEffect(() => {
    let alive = true;
    /** Durante uma leitura do histórico, o que chega ao vivo espera aqui. */
    let pending: AgentEvent[] | null = [];
    setEvents([]);
    setDraft("");
    setThinking("");
    setProgress({});
    setCompacting(null);
    setTimes({});
    setLoaded(taskId == null);
    setError(null);
    // Conversa ainda sem tarefa: nada a ler nem a ouvir.
    if (taskId == null) return;

    lastEventAt.current = null;

    // Os deltas chegam token a token: cada um viraria um redesenho da conversa
    // inteira. Eles se juntam aqui e saem num `setState` por quadro.
    let textBuf = "";
    let thinkBuf = "";
    let thinkingOpen = false;
    let frame = 0;
    const flush = () => {
      frame = 0;
      if (textBuf) {
        const t = textBuf;
        textBuf = "";
        setDraft((d) => d + t);
      }
      if (thinkBuf) {
        const t = thinkBuf;
        thinkBuf = "";
        setThinking((d) => d + t);
      }
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(flush);
    };
    /** Bloco fechado: o que ainda não saiu não deve reaparecer depois. */
    const dropBuffers = () => {
      textBuf = "";
      thinkBuf = "";
      thinkingOpen = false;
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
    };

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
      lastEventAt.current = Date.now();
      // Texto parcial: acumula no rascunho, fora da lista de eventos.
      if ("SubagentProgress" in event) {
        const p = event.SubagentProgress;
        setProgress((cur) => ({ ...cur, [p.id]: p }));
        return;
      }
      // Começo e fim da compactação: só a pílula de estado usa, e o evento não
      // entra na conversa (o `Compacted` é que vira linha no histórico).
      if ("Compacting" in event) {
        setCompacting(event.Compacting.reason);
        return;
      }
      // Fim do resumo. O `Result` é a rede de segurança: se a CLI não imprimir o
      // `compact_boundary`, o fim do turno de compactação encerra o aviso.
      if ("Compacted" in event || "Result" in event) setCompacting(null);
      // Limite do plano: vive no popover de uso, não na conversa. Na lista de
      // eventos ele aparecia como a linha solta "(evento RateLimit)".
      if ("RateLimit" in event) return;
      if ("ThinkingDelta" in event) {
        thinkingOpen = true;
        thinkBuf += event.ThinkingDelta;
        schedule();
        return;
      }
      if ("TextDelta" in event) {
        // O raciocínio some quando começa o texto: uma vez só, não a cada token.
        if (thinkingOpen || thinkBuf) {
          thinkBuf = "";
          thinkingOpen = false;
          setThinking("");
        }
        textBuf += event.TextDelta;
        schedule();
        return;
      }
      // Qualquer evento do agente principal fecha o bloco que estava sendo gerado.
      if (!("Scoped" in event)) {
        dropBuffers();
        setDraft("");
        setThinking("");
      }
      stamp(event);
      if (pending) pending.push(event);
      else setEvents((cur) => [...cur, event]);
    });
    // Lê o histórico só depois de já estar ouvindo, para não perder o que sai no meio.
    void off.then(() => alive && sync(true));
    // Eventos perdidos na fila: relê o histórico gravado.
    const offLag = onOrchLagged(() => alive && sync(false));

    return () => {
      alive = false;
      dropBuffers();
      void off.then((f) => f());
      void offLag.then((f) => f());
    };
  }, [taskId]);

  const wasActive = useRef(active);
  useEffect(() => {
    if (wasActive.current && !active) {
      setDraft("");
      setThinking("");
      setCompacting(null);
      // Dá tempo de os últimos eventos do turno serem gravados.
      const timer = setTimeout(() => resync.current(), 400);
      wasActive.current = active;
      return () => clearTimeout(timer);
    }
    wasActive.current = active;
  }, [active]);

  return { events, times, loaded, error, draft, thinking, progress, lastEventAt, compacting };
}
