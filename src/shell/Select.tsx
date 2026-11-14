import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import "./select.css";

export interface SelectOption {
  value: string;
  label: string;
  /** Texto secundário, à direita do rótulo. */
  hint?: string;
}

export interface SelectProps {
  value: string;
  options: SelectOption[];
  onChange(value: string): void;
  ariaLabel: string;
  /** Mostrado quando `value` não está em `options`. */
  placeholder?: string;
  disabled?: boolean;
  id?: string;
  /** "field": campo de formulário (padrão). "pill": pílula da caixa de composição. */
  variant?: "field" | "pill";
  /** Ícone antes do rótulo. */
  icon?: ReactNode;
  /** Dica ao passar o mouse. */
  title?: string;
  /** Chamado cada vez que a lista abre (para carregar as opções sob demanda). */
  onOpen?(): void;
  /** Linha de aviso no fim da lista (ex.: "consultando…"). */
  note?: string | null;
  /** Largura mínima da lista aberta, em px (padrão 200). */
  popWidth?: number;
}

/** A partir de quantas opções a lista ganha campo de busca. */
const SEARCH_FROM = 8;
const MAX_HEIGHT = 320;

/**
 * Lista de escolha no visual do Argus, no lugar do <select> nativo (que o
 * sistema desenha com as cores dele). Teclado: setas, Enter, Esc, Home e End;
 * com muitas opções, digitar filtra.
 */
export function Select({
  value,
  options,
  onChange,
  ariaLabel,
  placeholder = "Escolher...",
  disabled,
  id,
  variant = "field",
  icon,
  title,
  onOpen,
  note,
  popWidth = 200,
}: SelectProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [pos, setPos] = useState<{ left: number; width: number; top?: number; bottom?: number; maxHeight: number }>();
  const btn = useRef<HTMLButtonElement>(null);
  const pop = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);

  const current = options.find((o) => o.value === value);
  const withSearch = options.length >= SEARCH_FROM;
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? options.filter((o) => `${o.label} ${o.hint ?? ""}`.toLowerCase().includes(q)) : options;
  }, [options, query]);

  const close = (focus = true) => {
    setOpen(false);
    setQuery("");
    if (focus) btn.current?.focus();
  };
  const pick = (o: SelectOption | undefined) => {
    if (!o) return;
    if (o.value !== value) onChange(o.value);
    close();
  };
  const openList = () => {
    if (disabled) return;
    setActive(Math.max(0, options.findIndex((o) => o.value === value)));
    setOpen(true);
    onOpen?.();
  };

  // Posição: abaixo do botão; acima quando não há espaço embaixo.
  useLayoutEffect(() => {
    if (!open || !btn.current) return;
    const r = btn.current.getBoundingClientRect();
    const below = window.innerHeight - r.bottom - 8;
    const above = r.top - 8;
    const up = below < 180 && above > below;
    const width = Math.min(Math.max(r.width, popWidth), window.innerWidth - 16);
    const left = Math.max(8, Math.min(r.left, window.innerWidth - width - 8));
    setPos({
      left,
      width,
      maxHeight: Math.min(MAX_HEIGHT, up ? above : below),
      ...(up ? { bottom: window.innerHeight - r.top + 4 } : { top: r.bottom + 4 }),
    });
  }, [open, popWidth]);

  // O foco só pode ir para a lista depois que ela existe (ela nasce quando a
  // posição é conhecida): sem isto, digitar para filtrar e as setas não valiam.
  const placed = open && pos != null;
  useEffect(() => {
    if (placed) (withSearch ? search.current : pop.current)?.focus();
  }, [placed, withSearch]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!pop.current?.contains(t) && !btn.current?.contains(t)) close(false);
    };
    // A janela perder o foco não fecha a lista: uma consulta à CLI (lista de
    // modelos) pode tirar o foco por um instante, e a lista sumia no meio.
    const shut = () => close(false);
    document.addEventListener("mousedown", onDown, true);
    window.addEventListener("resize", shut);
    return () => {
      document.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("resize", shut);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    if (open) pop.current?.querySelector(".sel-opt.on")?.scrollIntoView({ block: "nearest" });
  }, [open, active, shown]);

  const onKey = (e: React.KeyboardEvent) => {
    const last = shown.length - 1;
    if (e.key === "ArrowDown") setActive((i) => (i >= last ? 0 : i + 1));
    else if (e.key === "ArrowUp") setActive((i) => (i <= 0 ? last : i - 1));
    else if (e.key === "Home") setActive(0);
    else if (e.key === "End") setActive(last);
    else if (e.key === "Enter") pick(shown[active]);
    else if (e.key === "Escape" || e.key === "Tab") close();
    else return;
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <>
      <button
        ref={btn}
        id={id}
        type="button"
        className={`sel-btn sel-${variant}${open ? " open" : ""}`}
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        title={open ? undefined : title}
        disabled={disabled}
        onClick={() => (open ? close() : openList())}
        onKeyDown={(e) => {
          if (!open && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
            e.preventDefault();
            openList();
          }
        }}
      >
        {icon}
        <span className={`sel-label${current ? "" : " empty"}`}>{current?.label ?? placeholder}</span>
        <svg className="sel-chev" width="10" height="10" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M6 9l6 6 6-6" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      {open &&
        pos &&
        createPortal(
          <div
            ref={pop}
            className="sel-pop"
            style={pos}
            role="listbox"
            aria-label={ariaLabel}
            tabIndex={-1}
            onKeyDown={onKey}
          >
            {withSearch && (
              <input
                ref={search}
                className="sel-search"
                placeholder="Filtrar..."
                aria-label={`Filtrar ${ariaLabel.toLowerCase()}`}
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setActive(0);
                }}
              />
            )}
            <div className="sel-list">
              {shown.map((o, i) => (
                <div
                  key={o.value}
                  role="option"
                  aria-selected={o.value === value}
                  className={`sel-opt${i === active ? " on" : ""}${o.value === value ? " cur" : ""}`}
                  onMouseEnter={() => setActive(i)}
                  onClick={() => pick(o)}
                >
                  <span className="sel-opt-label">{o.label}</span>
                  {o.hint && <span className="sel-opt-hint">{o.hint}</span>}
                  {o.value === value && (
                    <svg className="sel-check" width="12" height="12" viewBox="0 0 24 24" aria-hidden="true">
                      <path d="M5 12l5 5 9-10" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                  )}
                </div>
              ))}
              {!shown.length && <div className="sel-empty">Nada encontrado.</div>}
            </div>
            {note && <div className="sel-note">{note}</div>}
          </div>,
          document.body,
        )}
    </>
  );
}
