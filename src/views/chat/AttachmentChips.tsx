import { useState } from "react";
import { Icon } from "@/shell/icons";
import { kindLabel, sizeText, type ChatAttachment } from "./attachments";
import { Lightbox } from "./Lightbox";

export interface AttachmentChipsProps {
  attachments: ChatAttachment[];
  onRemove: (att: ChatAttachment) => void;
  /** Devolve um texto colado à caixa (só aparece em anexos vindos de colagem). */
  onRestoreText?: (att: ChatAttachment) => void;
}

/** Chips dos anexos acima da caixa: miniatura ou tipo, nome, tamanho e botão de remover. */
export function AttachmentChips({ attachments, onRemove, onRestoreText }: AttachmentChipsProps) {
  const [zoom, setZoom] = useState<ChatAttachment | null>(null);
  if (!attachments.length) return null;
  return (
    <div className="chat-atts">
      {attachments.map((a) => (
        <div key={a.id} className={`chat-att ${a.kind}${a.status === "error" ? " err" : ""}`} title={a.error ?? a.path ?? a.name}>
          {a.previewUrl ? (
            <img src={a.previewUrl} alt="" onClick={() => setZoom(a)} />
          ) : (
            <span className="chat-att-icon">{a.kind === "pdf" ? "PDF" : a.kind === "text" ? "TXT" : "ARQ"}</span>
          )}
          <span className="chat-att-info">
            <b>{a.name}</b>
            <small>
              {a.status === "saving"
                ? "gravando…"
                : a.status === "error"
                  ? (a.error ?? "não foi possível anexar")
                  : `${kindLabel(a.kind)} · ${sizeText(a.size)}${a.path ? "" : " · vai no texto"}`}
            </small>
          </span>
          {a.pasted && onRestoreText && (
            <button type="button" className="chat-att-btn" title="Colar como texto na mensagem" onClick={() => onRestoreText(a)}>
              <Icon name="undo" size={13} />
            </button>
          )}
          <button type="button" className="chat-att-btn" title="Tirar o anexo" aria-label={`Tirar ${a.name}`} onClick={() => onRemove(a)}>
            <Icon name="close" size={13} />
          </button>
        </div>
      ))}
      {zoom?.previewUrl && <Lightbox src={zoom.previewUrl} alt={zoom.name} onClose={() => setZoom(null)} />}
    </div>
  );
}
