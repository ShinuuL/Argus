import { useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";
import "./select.css";

export interface PopoverProps {
  /** Botão que abre o popover: a posição sai dele e o foco volta para ele ao fechar. */
  anchor: RefObject<HTMLElement | null>;
  open: boolean;
  /** `focus`: devolver o foco ao botão (Esc, escolha feita). */
  onClose(focus: boolean): void;
  ariaLabel: string;
  /** Largura do popover, em px. */
  width?: number;
  className?: string;
  children: ReactNode;
}

const MAX_HEIGHT = 440;

/**
 * Painel flutuante preso a um botão, no visual das listas do Argus (`.sel-pop`).
 * Abre abaixo do botão (acima quando falta espaço), fecha com Esc (o foco volta
 * ao botão), com clique fora e quando a janela muda de tamanho. Perder o foco
 * da janela não fecha: uma consulta à CLI pode tirar o foco por um instante.
 */
export function Popover({ anchor, open, onClose, ariaLabel, width = 320, className, children }: PopoverProps) {
  const pop = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; width: number; top?: number; bottom?: number; maxHeight: number }>();

  useLayoutEffect(() => {
    if (!open || !anchor.current) return setPos(undefined);
    const r = anchor.current.getBoundingClientRect();
    const below = window.innerHeight - r.bottom - 8;
    const above = r.top - 8;
    const up = below < 240 && above > below;
    const w = Math.min(width, window.innerWidth - 16);
    const left = Math.max(8, Math.min(r.left, window.innerWidth - w - 8));
    setPos({
      left,
      width: w,
      maxHeight: Math.min(MAX_HEIGHT, up ? above : below),
      ...(up ? { bottom: window.innerHeight - r.top + 4 } : { top: r.bottom + 4 }),
    });
  }, [open, width, anchor]);

  // Foco no primeiro controle (ou no painel) quando ele aparece.
  const placed = open && pos != null;
  useEffect(() => {
    if (!placed || !pop.current) return;
    const first = pop.current.querySelector<HTMLElement>("[data-autofocus], button:not(:disabled), input:not(:disabled)");
    (first ?? pop.current).focus();
  }, [placed]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!pop.current?.contains(t) && !anchor.current?.contains(t)) onClose(false);
    };
    const shut = () => onClose(false);
    document.addEventListener("mousedown", onDown, true);
    window.addEventListener("resize", shut);
    return () => {
      document.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("resize", shut);
    };
  }, [open, onClose, anchor]);

  if (!open || !pos) return null;
  return createPortal(
    <div
      ref={pop}
      className={`sel-pop pop${className ? ` ${className}` : ""}`}
      style={pos}
      role="dialog"
      aria-label={ariaLabel}
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          onClose(true);
        }
      }}
    >
      {children}
    </div>,
    document.body,
  );
}
