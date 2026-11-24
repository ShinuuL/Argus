import { useState } from "react";
import type { QueueItem } from "./store";

export interface QueueStripProps {
  items: QueueItem[];
  /** Motivo da pausa (turno falhou ou foi interrompido); null = fila andando. */
  paused: string | null;
  /** Só aparece enquanto o agente está ocupado (ou com a fila pausada, que precisa de "Retomar"). */
  busy?: boolean;
  /** Abertura controlada pelo menu "+" do Composer. */
  expanded?: boolean;
  onExpandedChange?: (open: boolean) => void;
  onEdit: (item: QueueItem) => void;
  onRemove: (item: QueueItem) => void;
  /** Manda a próxima agora (só aparece com a fila pausada). */
  onResume: () => void;
}

/** Badge compacto "N na fila"; clique abre a lista com editar/tirar/retomar. */
export function QueueStrip({ items, paused, busy = false, expanded, onExpandedChange, onEdit, onRemove, onResume }: QueueStripProps) {
  const [own, setOwn] = useState(false);
  if (!items.length || (!busy && !paused)) return null;
  const open = expanded ?? own;
  const setOpen = (value: boolean) => {
    if (expanded == null) setOwn(value);
    onExpandedChange?.(value);
  };
  const title = items.map((q) => q.text || `${q.attachments.length} anexo(s)`).join("\n");

  return (
    <div className={`chat-queue${paused ? " paused" : ""}`}>
      <button type="button" className="chat-queue-badge" aria-expanded={open} title={title} onClick={() => setOpen(!open)}>
        {items.length === 1 ? "1 na fila" : `${items.length} na fila`}
        {paused ? " · pausada" : ""}
      </button>
      {open && (
        <div className="chat-queue-list">
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
      )}
    </div>
  );
}
