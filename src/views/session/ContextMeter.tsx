import type { ContextUsage } from "@/api/types";
import { formatTokens } from "@/api/util";
import { useId } from "react";

/** A partir daqui a barra fica em --warn; acima de CONTEXT_ERR, em --err. */
export const CONTEXT_WARN = 80;
export const CONTEXT_ERR = 95;

const fmt = (n: number) => n.toLocaleString("pt-BR");

/**
 * Ocupação da janela de contexto: barra fina com a porcentagem no cabeçalho do
 * chat. Hover ou foco abre o detalhe do último pedido do agente principal.
 */
export function ContextMeter({ ctx }: { ctx: ContextUsage }) {
  const tipId = useId();
  const total = ctx.input_tokens + ctx.cache_read + ctx.cache_write;
  const pct = ctx.window ? Math.min(100, (total / ctx.window) * 100) : null;
  const level = pct == null ? "" : pct >= CONTEXT_ERR ? "err" : pct >= CONTEXT_WARN ? "warn" : "";
  const label = pct == null ? `${formatTokens(total)} tokens` : `${pct < 1 && pct > 0 ? "<1" : Math.round(pct)}%`;
  const alert = level === "err" ? " (quase cheio)" : level === "warn" ? " (acima de 80%)" : "";
  const text =
    pct == null
      ? `Contexto: ${fmt(total)} tokens; tamanho da janela desconhecido`
      : `Contexto: ${Math.round(pct)}% ocupado${alert}, ${fmt(total)} de ${fmt(ctx.window!)} tokens`;
  return (
    <span
      className={`cv-ctx ${level}`}
      tabIndex={0}
      role={pct == null ? "status" : "meter"}
      aria-label={text}
      aria-valuemin={pct == null ? undefined : 0}
      aria-valuemax={pct == null ? undefined : 100}
      aria-valuenow={pct == null ? undefined : Math.round(pct)}
      aria-valuetext={pct == null ? undefined : text}
      aria-describedby={tipId}
    >
      {pct != null && (
        <span className="cv-ctx-bar" aria-hidden>
          <span className="cv-ctx-fill" style={{ width: `${pct}%` }} />
        </span>
      )}
      <span className="cv-ctx-pct">
        {level && <span aria-hidden>! </span>}
        {label}
      </span>
      <span className="cv-ctx-tip" id={tipId} role="tooltip">
        <strong>Janela de contexto (último pedido)</strong>
        <dl>
          <dt>Entrada</dt>
          <dd>{fmt(ctx.input_tokens)}</dd>
          <dt>Cache lido</dt>
          <dd>{fmt(ctx.cache_read)}</dd>
          <dt>Cache gravado</dt>
          <dd>{fmt(ctx.cache_write)}</dd>
          <dt>Saída do último passo</dt>
          <dd>{fmt(ctx.output_tokens)}</dd>
          <dt>Total em contexto</dt>
          <dd>{fmt(total)}</dd>
          <dt>Tamanho da janela</dt>
          <dd>{ctx.window ? fmt(ctx.window) : "desconhecido"}</dd>
          <dt>Modelo</dt>
          <dd>{ctx.model ?? "—"}</dd>
        </dl>
        {level && <span className={`cv-ctx-alert ${level}`}>{level === "err" ? "Contexto quase cheio: a CLI vai resumir a conversa em breve." : "Contexto acima de 80%."}</span>}
      </span>
    </span>
  );
}
