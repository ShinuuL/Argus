import type { ReactNode } from "react";

interface Props {
  title: string;
  /** O que a tela vai fazer quando for portada. */
  hint?: string;
  /** "fill" centraliza na área; "inline" cabe na status bar. */
  variant?: "block" | "fill" | "inline";
  children?: ReactNode;
}

/** Marcador de tela ainda não portada. Some quando a tela real substituir o arquivo. */
export function Placeholder({ title, hint, variant = "block", children }: Props) {
  if (variant === "inline") {
    return (
      <span className="placeholder inline small" title={hint}>
        {title}
      </span>
    );
  }
  return (
    <div className={variant === "fill" ? "placeholder fill" : "placeholder"}>
      <b>{title}</b>
      <span className="small">Tela em construção.</span>
      {hint && <span className="small">{hint}</span>}
      {children}
    </div>
  );
}
