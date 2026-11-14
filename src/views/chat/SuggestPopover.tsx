import { useEffect, useRef } from "react";

export interface SuggestItem {
  /** Chave única na lista. */
  key: string;
  /** Texto principal (ex.: "/diff", "src/main.rs"). */
  label: string;
  /** Texto secundário, à direita. */
  detail?: string;
  /** Etiqueta curta à esquerda (ex.: "agente", "arquivo"). */
  tag?: string;
}

export interface SuggestPopoverProps {
  items: SuggestItem[];
  /** Índice do item destacado. */
  index: number;
  onPick: (item: SuggestItem) => void;
  onHover?: (index: number) => void;
  /** Linha de rodapé (ex.: "buscando…"). */
  footer?: string;
}

/**
 * Lista de sugestões ancorada acima da caixa de mensagem (menções "@" e
 * comandos "/"). O teclado é tratado por quem usa; aqui só mouse e rolagem.
 */
export function SuggestPopover({ items, index, onPick, onHover, footer }: SuggestPopoverProps) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.querySelector(".on")?.scrollIntoView({ block: "nearest" });
  }, [index, items]);

  if (!items.length && !footer) return null;
  return (
    <div className="chat-suggest" ref={ref} role="listbox">
      {items.map((item, i) => (
        <button
          type="button"
          key={item.key}
          role="option"
          aria-selected={i === index}
          className={`chat-suggest-item${i === index ? " on" : ""}`}
          // mousedown: escolhe antes de a caixa perder o foco
          onMouseDown={(e) => {
            e.preventDefault();
            onPick(item);
          }}
          onMouseEnter={() => onHover?.(i)}
        >
          {item.tag && <span className="chat-suggest-tag">{item.tag}</span>}
          <b>{item.label}</b>
          {item.detail && <small>{item.detail}</small>}
        </button>
      ))}
      {footer && <div className="chat-suggest-foot">{footer}</div>}
    </div>
  );
}
