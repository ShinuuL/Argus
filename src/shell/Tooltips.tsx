import { useEffect, useLayoutEffect, useRef, useState } from "react";
import "./select.css";

const DELAY_MS = 450;

/**
 * Dicas no visual do Argus para TODOS os elementos com `title`, sem mexer em
 * cada componente: ao passar o mouse (ou focar pelo teclado), o `title` vai
 * para `data-tip`, o que impede a dica nativa do sistema, e a dica própria
 * aparece depois de um instante. O nome acessível é preservado em `aria-label`
 * quando o elemento não tem texto.
 */
export function Tooltips() {
  const [tip, setTip] = useState<{ text: string; rect: DOMRect } | null>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let timer: number | undefined;
    let target: Element | null = null;

    const hide = () => {
      window.clearTimeout(timer);
      target = null;
      setTip(null);
      setPos(null);
    };
    const adopt = (el: Element): string | null => {
      const t = el.getAttribute("title");
      if (t) {
        el.setAttribute("data-tip", t);
        el.removeAttribute("title");
        if (!el.getAttribute("aria-label") && !(el.textContent ?? "").trim()) el.setAttribute("aria-label", t);
      }
      return el.getAttribute("data-tip");
    };
    const show = (from: EventTarget | null, delay: number) => {
      const el = from instanceof Element ? from.closest("[title], [data-tip]") : null;
      if (el === target) return;
      hide();
      if (!el) return;
      const text = adopt(el);
      if (!text) return;
      target = el;
      timer = window.setTimeout(() => {
        if (el.isConnected) setTip({ text, rect: el.getBoundingClientRect() });
      }, delay);
    };

    const onOver = (e: MouseEvent) => show(e.target, DELAY_MS);
    const onFocus = (e: FocusEvent) => {
      const el = e.target instanceof Element ? e.target : null;
      if (el?.matches(":focus-visible")) show(el, 0);
    };
    document.addEventListener("mouseover", onOver, true);
    document.addEventListener("focusin", onFocus, true);
    document.addEventListener("focusout", hide, true);
    document.addEventListener("mousedown", hide, true);
    document.addEventListener("keydown", hide, true);
    document.addEventListener("wheel", hide, { capture: true, passive: true });
    document.addEventListener("mouseleave", hide);
    window.addEventListener("blur", hide);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("mouseover", onOver, true);
      document.removeEventListener("focusin", onFocus, true);
      document.removeEventListener("focusout", hide, true);
      document.removeEventListener("mousedown", hide, true);
      document.removeEventListener("keydown", hide, true);
      document.removeEventListener("wheel", hide, true);
      document.removeEventListener("mouseleave", hide);
      window.removeEventListener("blur", hide);
    };
  }, []);

  // Abaixo do elemento, centralizada; acima quando não cabe; nunca fora da janela.
  useLayoutEffect(() => {
    const el = box.current;
    if (!tip || !el) return;
    const { rect } = tip;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const below = rect.bottom + 6 + h <= window.innerHeight - 4;
    setPos({
      left: Math.max(6, Math.min(rect.left + rect.width / 2 - w / 2, window.innerWidth - w - 6)),
      top: below ? rect.bottom + 6 : Math.max(4, rect.top - h - 6),
    });
  }, [tip]);

  if (!tip) return null;
  return (
    <div
      ref={box}
      className="tip"
      role="tooltip"
      style={pos ? { left: pos.left, top: pos.top, opacity: 1 } : { left: 0, top: 0, opacity: 0 }}
    >
      {tip.text}
    </div>
  );
}
