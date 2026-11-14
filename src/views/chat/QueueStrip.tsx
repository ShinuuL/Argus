import type { QueueItem } from "./store";

export interface QueueStripProps {
  items: QueueItem[];
  /** Motivo da pausa (turno falhou ou foi interrompido); null = fila andando. */
  paused: string | null;
  onEdit: (item: QueueItem) => void;
  onRemove: (item: QueueItem) => void;
  /** Manda a próxima agora (só aparece com a fila pausada). */
  onResume: () => void;
}

/** Faixa "N na fila · sai quando o agente terminar", com editar e tirar por item. */
export function QueueStrip({ items, paused, onEdit, onRemove, onResume }: QueueStripProps) {
  if (!items.length) return null;
  return (
    <div className={`chat-queue${paused ? " paused" : ""}`}>
      <span className="chat-queue-label">
        {items.length === 1 ? "1 na fila" : `${items.length} na fila`}
        <small>{paused ? ` · fila pausada: ${paused}` : " · sai quando o agente terminar"}</small>
      </span>
      {paused && (
        <button type="button" className="chat-queue-resume" onClick={onResume}>
          Retomar
        </button>
      )}
      {items.map((q) => (
        <span key={q.id} className="chat-queue-item" title={q.text}>
          <span className="chat-queue-text">{q.text || `${q.attachments.length} anexo(s)`}</span>
          <button type="button" title="Editar" aria-label="Editar mensagem da fila" onClick={() => onEdit(q)}>
            ✎
          </button>
          <button type="button" title="Tirar da fila" aria-label="Tirar da fila" onClick={() => onRemove(q)}>
            ✕
          </button>
        </span>
      ))}
    </div>
  );
}
