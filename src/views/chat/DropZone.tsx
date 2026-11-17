import { useRef, useState, type DragEvent, type ReactNode } from "react";

export interface DropZoneProps {
  /** Recebe os arquivos soltos. */
  onFiles: (files: File[]) => void;
  disabled?: boolean;
  className?: string;
  children: ReactNode;
}

const hasFiles = (e: DragEvent) => Array.from(e.dataTransfer.types).includes("Files");

/**
 * Área que aceita arquivos arrastados do sistema e mostra "SOLTE PARA ANEXAR".
 * Ignora o arrastar interno da IDE (abas), que não traz arquivos. Depende de
 * `dragDropEnabled: false` na janela, para o drop chegar como `File`.
 *
 * O `Composer` já é uma área dessas; use este componente para estender a área
 * à conversa inteira, repassando os arquivos com `composerRef.current.addFiles`.
 */
export function DropZone({ onFiles, disabled, className, children }: DropZoneProps) {
  // dragenter/dragleave disparam a cada filho: o contador diz quando saiu de verdade
  const depth = useRef(0);
  const [over, setOver] = useState(false);

  const enter = (e: DragEvent) => {
    if (disabled || !hasFiles(e)) return;
    e.preventDefault();
    e.stopPropagation();
    depth.current++;
    setOver(true);
  };
  const leave = (e: DragEvent) => {
    if (disabled || !hasFiles(e)) return;
    e.stopPropagation();
    depth.current = Math.max(0, depth.current - 1);
    if (!depth.current) setOver(false);
  };
  const dragOver = (e: DragEvent) => {
    if (disabled || !hasFiles(e)) return;
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = "copy";
  };
  const drop = (e: DragEvent) => {
    if (disabled || !hasFiles(e)) return;
    e.preventDefault();
    e.stopPropagation();
    depth.current = 0;
    setOver(false);
    const files = Array.from(e.dataTransfer.files);
    if (files.length) onFiles(files);
  };

  return (
    <div
      className={`chat-drop${className ? ` ${className}` : ""}`}
      onDragEnter={enter}
      onDragLeave={leave}
      onDragOver={dragOver}
      onDrop={drop}
    >
      {children}
      {over && <div className="chat-drop-overlay">SOLTE PARA ANEXAR</div>}
    </div>
  );
}
