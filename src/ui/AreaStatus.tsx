import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

/** Id do espaço da barra de status global onde cada área põe o próprio conteúdo. */
export const AREA_STATUS_SLOT = "sb-area";

/**
 * Conteúdo de barra de status de uma área em tela cheia. É desenhado dentro da
 * barra global (StatusBar), para a janela ter uma barra só, como no mockup.
 */
export function AreaStatus({ children }: { children: ReactNode }) {
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  useEffect(() => {
    setSlot(document.getElementById(AREA_STATUS_SLOT));
  }, []);
  return slot ? createPortal(<div className="sb-area-in">{children}</div>, slot) : null;
}
