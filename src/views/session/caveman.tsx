/**
 * "Comprimir com caveman" sobre o trecho selecionado da conversa.
 *
 * A CLI comprime o texto de verdade (ou diz que não conseguiu). As duas versões
 * aparecem lado a lado e quem decide é o usuário: nada substitui a resposta
 * recebida sem confirmação.
 */
import { useEffect, useState } from "react";
import { cavemanCompress, cavemanStatus, errorMessage, type CavemanResult } from "@/api";
import { Icon } from "@/shell/icons";
import type { QuoteSpot } from "./quote";

interface Props {
  spot: QuoteSpot;
  onPick(text: string): void;
  onClose(): void;
  /** Abaixo do botão "Responder", que usa a mesma seleção. */
  offsetY?: number;
}

export function CavemanButton({ spot, onPick, onClose, offsetY = 38 }: Props) {
  const [result, setResult] = useState<CavemanResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [hint, setHint] = useState<string | null>(null);

  useEffect(() => {
    let vivo = true;
    cavemanStatus()
      .then((s) => { if (vivo && !s.installed) setHint(s.hint); })
      .catch((e) => { if (vivo) setError(errorMessage(e)); });
    return () => { vivo = false; };
  }, []);

  const compress = async () => {
    setBusy(true);
    setError(null);
    try {
      setResult(await cavemanCompress(spot.text));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const x = Math.min(Math.max(spot.x - 200, 8), window.innerWidth - 232);
  const y = Math.min(spot.y + offsetY, window.innerHeight - 40);

  if (result) {
    return (
      <div className="cv-caveman" role="dialog" aria-label="Escolher a saída">
        <div className="cv-caveman-head">
          <strong>caveman</strong>
          <span className="muted">
            {result.compressed_for_real
              ? `${Math.round(result.report.ratio * 100)}% menor · ${result.report.engine}`
              : "sem compressão (pass-through)"}
          </span>
          <button type="button" className="ses-ask-ibtn" aria-label="Fechar" onClick={onClose}>
            <Icon name="close" size={12} />
          </button>
        </div>
        {!result.compressed_for_real && result.report.note && (
          <div className="cv-caveman-note">{result.report.note}</div>
        )}
        <div className="cv-caveman-cols">
          <div>
            <div className="cv-caveman-label">Original</div>
            <pre>{result.original}</pre>
            <button type="button" onClick={() => { onPick(result.original); onClose(); }}>Usar original</button>
          </div>
          <div>
            <div className="cv-caveman-label">Comprimido</div>
            <pre>{result.compressed}</pre>
            <button type="button" className="primary" onClick={() => { onPick(result.compressed); onClose(); }}>
              Usar comprimido
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="cv-caveman-bar" style={{ left: x, top: y }}>
      <button type="button" className="cv-quote" disabled={busy} onMouseDown={(e) => { e.preventDefault(); void compress(); }}>
        <Icon name="sparkles" size={12} /> {busy ? "Comprimindo…" : "Comprimir com caveman"}
      </button>
      {(hint || error) && <div className="cv-caveman-hint">{error ?? hint}</div>}
      <button type="button" className="ses-ask-ibtn" aria-label="Fechar" onClick={onClose}>
        <Icon name="close" size={12} />
      </button>
    </div>
  );
}
