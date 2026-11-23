/**
 * "Responder" sobre o trecho selecionado da conversa.
 *
 * Marcar um pedaço da resposta e perguntar sobre *aquilo* é o jeito natural de
 * continuar, e até agora só dava para copiar, colar na caixa e marcar o `>` na
 * mão. Aqui a seleção faz aparecer um botão junto do fim dela; clicar leva o
 * trecho para o rascunho como citação, com o cursor já na linha de baixo.
 *
 * Posição fixa (coordenadas de viewport), não absoluta dentro da transcrição:
 * a conversa rola, e um botão ancorado no conteúdo flutuaria junto, passando
 * por cima da caixa e da barra de status. Em troca, qualquer rolagem esconde o
 * botão — a seleção continua lá, basta soltar o mouse de novo.
 */
import { useCallback, useEffect, useState, type RefObject } from "react";
import { Icon } from "@/shell/icons";

/** Acima disso o trecho vira citação cortada: ninguém cita três telas de texto. */
const MAX = 2000;

export interface QuoteSpot {
  text: string;
  /** Canto inferior direito da seleção, em coordenadas de viewport. */
  x: number;
  y: number;
}

/** Converte o trecho em citação Markdown, com uma linha em branco para a resposta. */
export function asQuote(text: string): string {
  const corte = text.length > MAX ? `${text.slice(0, MAX).trimEnd()}…` : text;
  const linhas = corte
    .replace(/\r/g, "")
    .split("\n")
    .map((l) => `> ${l.trimEnd()}`.trimEnd());
  return `${linhas.join("\n")}\n\n`;
}

/**
 * Observa a seleção dentro de `ref` e devolve onde pôr o botão.
 *
 * Escuta `selectionchange` (e não só `mouseup`) porque a seleção também nasce
 * do teclado — Shift+seta, Ctrl+A — e porque é o único evento que avisa quando
 * ela some por um clique em qualquer outro lugar da janela.
 */
export function useQuoteSelection(ref: RefObject<HTMLElement | null>, enabled: boolean): [QuoteSpot | null, () => void] {
  const [spot, setSpot] = useState<QuoteSpot | null>(null);
  const limpar = useCallback(() => setSpot(null), []);

  useEffect(() => {
    if (!enabled) {
      setSpot(null);
      return;
    }
    const ver = () => {
      const sel = window.getSelection();
      const alvo = ref.current;
      if (!sel || sel.isCollapsed || sel.rangeCount === 0 || !alvo) return setSpot(null);
      const texto = sel.toString().trim();
      // Dois nós: arrastar da transcrição para fora (ou de fora para dentro)
      // selecionaria pedaço de menu e de barra de status junto.
      if (!texto || !alvo.contains(sel.anchorNode) || !alvo.contains(sel.focusNode)) return setSpot(null);
      const r = sel.getRangeAt(0).getBoundingClientRect();
      if (!r.width && !r.height) return setSpot(null);
      setSpot({ text: texto, x: r.right, y: r.bottom });
    };
    // Adiado um quadro: no `mouseup` que termina o arrasto a seleção ainda é a
    // anterior, e o botão apareceria no lugar da seleção de antes.
    const depois = () => setTimeout(ver, 0);
    document.addEventListener("selectionchange", depois);
    window.addEventListener("resize", limpar);
    return () => {
      document.removeEventListener("selectionchange", depois);
      window.removeEventListener("resize", limpar);
    };
  }, [enabled, ref, limpar]);

  return [spot, limpar];
}

/** O botão flutuante. Some ao ser usado: quem já citou não quer citar de novo. */
export function QuoteButton({ spot, onQuote, onClose }: { spot: QuoteSpot; onQuote(text: string): void; onClose(): void }) {
  // 8px abaixo e alinhado à direita do fim da seleção, preso dentro da janela.
  const x = Math.min(Math.max(spot.x - 96, 8), window.innerWidth - 112);
  const y = Math.min(spot.y + 8, window.innerHeight - 40);
  return (
    <button
      type="button"
      className="cv-quote"
      style={{ left: x, top: y }}
      title="Leva o trecho marcado para a caixa, como citação"
      // `mousedown` e não `click`: o clique só chega depois de o navegador
      // desfazer a seleção, e aí não há mais trecho para citar.
      onMouseDown={(e) => {
        e.preventDefault();
        onQuote(asQuote(spot.text));
        window.getSelection()?.removeAllRanges();
        onClose();
      }}
    >
      <Icon name="quote" size={12} /> Responder
    </button>
  );
}
