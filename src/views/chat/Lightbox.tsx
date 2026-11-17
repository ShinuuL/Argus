import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import type { Path } from "@/api";
import { previewFor } from "./attachments";

/** Imagem em tamanho real por cima da janela; clique ou Esc fecha. */
export function Lightbox({ src, alt, onClose }: { src: string; alt?: string; onClose: () => void }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  return createPortal(
    <div className="chat-zoom" onClick={onClose} title="Clique ou Esc para fechar">
      <img src={src} alt={alt ?? ""} />
    </div>,
    document.body,
  );
}

/**
 * Miniatura de uma imagem anexada (até 160×120). Lê a imagem da pasta de
 * anexos; clique amplia. Sem a imagem (arquivo apagado), mostra só o nome.
 */
export function Thumb({ path, name, url }: { path?: Path | null; name: string; url?: string }) {
  const [src, setSrc] = useState<string | null>(url ?? null);
  const [zoom, setZoom] = useState(false);

  useEffect(() => {
    if (url || !path) return;
    let alive = true;
    void previewFor(path).then((u) => {
      if (alive) setSrc(u);
    });
    return () => {
      alive = false;
    };
  }, [path, url]);

  if (!src) return <span className="chat-att-chip" title={path ?? name}>{name}</span>;
  return (
    <>
      <img className="chat-shot" src={src} alt={name} title={name} onClick={() => setZoom(true)} />
      {zoom && <Lightbox src={src} alt={name} onClose={() => setZoom(false)} />}
    </>
  );
}
