/**
 * Aviso de conversa cara: aparece acima do compositor quando o contexto passa
 * de ~150 mil tokens ou quando a última atividade foi há mais de 60 minutos
 * (o cache de prompt do claude dura 1 hora; depois disso o próximo turno
 * regrava o contexto inteiro a 2x o preço da entrada). Sugere /compact ou
 * conversa nova. Some enquanto o turno roda e pode ser dispensado.
 *
 * Os limites espelham `AUTOCOMPACT_DEFAULT`, `CACHE_TTL_SECS` e
 * `IDLE_COMPACT_MIN` do orquestrador (crates/core/src/agents/orchestrator.rs).
 */
import { useEffect, useState } from "react";
import { api, formatTokensPt, onOrch, variantOf, type ContextUsage, type TaskId } from "@/api";

export const ADVICE_CONTEXT_TOKENS = 150_000;
export const CACHE_TTL_SECS = 60 * 60;
/** Abaixo disto, regravar o cache depois da pausa é barato: não avisa. */
export const ADVICE_IDLE_MIN_TOKENS = 60_000;

const totalOf = (c?: ContextUsage | null) => (c ? c.input_tokens + c.cache_read + c.cache_write : 0);

export interface ContextAdviceProps {
  taskId: TaskId;
  /** Contexto já conhecido pela tela (opcional; os eventos `Context` atualizam). */
  context?: ContextUsage | null;
  /** Turno em andamento: o aviso some. */
  busy?: boolean;
  /** Envia `/compact` na conversa. Sem ele, o botão não aparece. */
  onCompact?: () => void;
  /** Abre uma conversa nova. Sem ele, o botão não aparece. */
  onNewChat?: () => void;
}

export function ContextAdvice({ taskId, context, busy = false, onCompact, onNewChat }: ContextAdviceProps) {
  const [ctx, setCtx] = useState<ContextUsage | null>(context ?? null);
  /** Momento (ms) da última atividade conhecida da conversa. */
  const [lastAt, setLastAt] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [dismissed, setDismissed] = useState<string | null>(null);

  useEffect(() => setCtx(context ?? null), [context]);

  useEffect(() => {
    let alive = true;
    setLastAt(null);
    setDismissed(null);
    const load = () =>
      api.taskIdleSecs(taskId).then(
        (s) => alive && setLastAt(s == null ? null : Date.now() - s * 1000),
        () => undefined,
      );
    void load();
    if (context === undefined) {
      api.taskGet(taskId).then((t) => alive && setCtx(t?.context ?? null), () => undefined);
    }
    const off = onOrch((ev) => {
      if (!("Agent" in ev) || ev.Agent.task !== taskId) return;
      const { kind, data } = variantOf(ev.Agent.event);
      setLastAt(Date.now());
      if (kind === "Context") setCtx(data as ContextUsage);
    });
    const tick = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => {
      alive = false;
      window.clearInterval(tick);
      void off.then((f) => f()).catch(() => undefined);
    };
    // `context` só na carga inicial; as mudanças dele entram pelo efeito acima.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [taskId]);

  if (busy) return null;
  const used = totalOf(ctx);
  const idleMin = lastAt == null ? 0 : Math.floor((now - lastAt) / 60_000);
  const big = used >= ADVICE_CONTEXT_TOKENS;
  const stale = idleMin * 60 > CACHE_TTL_SECS && used >= ADVICE_IDLE_MIN_TOKENS;
  if (!big && !stale) return null;
  const key = `${big}-${stale}`;
  if (dismissed === key) return null;

  const why = stale
    ? `A última mensagem foi há ${idleMin >= 120 ? `${Math.floor(idleMin / 60)} h` : `${idleMin} min`}: o cache expirou e o próximo turno vai regravar os ${formatTokensPt(used)} tokens da conversa (a 2x o preço da entrada).`
    : `A conversa está com ${formatTokensPt(used)} tokens de contexto: cada passo do agente relê tudo isso.`;

  return (
    <div className="chat-notice warn ctx-advice" role="status">
      <span className="ctx-advice-txt">
        {why} Compacte (/compact) para seguir com um resumo, ou comece uma conversa nova.
      </span>
      {onCompact && (
        <button type="button" onClick={onCompact}>
          Compactar
        </button>
      )}
      {onNewChat && (
        <button type="button" onClick={onNewChat}>
          Conversa nova
        </button>
      )}
      <button type="button" aria-label="Dispensar aviso" title="Dispensar" onClick={() => setDismissed(key)}>
        ×
      </button>
    </div>
  );
}
