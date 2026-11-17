import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";

export type Tone = "ok" | "warn" | "bad" | "info";
const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(" ");

/** Card (.card). Props: title? (cabeçalho), actions? (à direita do título), + atributos de div. */
export function Card({ title, actions, className, children, ...rest }: Omit<HTMLAttributes<HTMLDivElement>, "title"> & { title?: ReactNode; actions?: ReactNode }) {
  return (
    <div className={cx("ui-card", className)} {...rest}>
      {(title || actions) && (
        <div className="ui-card-h">
          {title}
          <span className="ui-sp" />
          {actions}
        </div>
      )}
      {children}
    </div>
  );
}

/** Chip (.chip). Props: tone? ok|warn|bad|info, onClick? (vira botão), title?. */
export function Chip({ tone, className, children, onClick, title }: { tone?: Tone; className?: string; children: ReactNode; onClick?: () => void; title?: string }) {
  const cls = cx("ui-chip", tone, className);
  return onClick ? (
    <button type="button" className={cls} onClick={onClick} title={title}>
      {children}
    </button>
  ) : (
    <span className={cls} title={title}>
      {children}
    </span>
  );
}

/** Faixa de abas (.tabs). Props: label? (acessível). Filhos: <Tab>. */
export function Tabs({ children, className, label }: { children: ReactNode; className?: string; label?: string }) {
  return (
    <div className={cx("ui-tabs", className)} role="tablist" aria-label={label}>
      {children}
    </div>
  );
}

/** Aba (.tab). Props: on (ativa), onClick, count? (número ao lado, some se 0), icon?, title?. */
export function Tab({ on, onClick, count, icon, children, title }: { on?: boolean; onClick?: () => void; count?: number; icon?: ReactNode; children: ReactNode; title?: string }) {
  return (
    <button type="button" role="tab" aria-selected={!!on} className={cx("ui-tab", on && "on")} onClick={onClick} title={title}>
      {icon}
      {children}
      {count != null && count > 0 && <span className="ui-count">{count}</span>}
    </button>
  );
}

export interface SegOption<T extends string> {
  value: T;
  label: string;
  tone?: Exclude<Tone, "info">;
}

/**
 * Segmentado (.seg). Props: value, options [{value,label,tone?}], onChange?, label?.
 * A opção ativa usa on-<tone> (ok/warn/bad) ou "on" sem tom.
 * Para política use POLICY_OPTIONS (Permitir/Perguntar/Bloquear).
 */
export function Seg<T extends string>({ value, options, onChange, label }: { value: T; options: SegOption<T>[]; onChange?: (v: T) => void; label?: string }) {
  return (
    <div className="ui-seg" role="radiogroup" aria-label={label}>
      {options.map((o) => {
        const on = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={on}
            className={on ? (o.tone ? `on-${o.tone}` : "on") : undefined}
            onClick={() => onChange?.(o.value)}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

export type PolicyValue = "allow" | "ask" | "deny";
export const POLICY_OPTIONS: SegOption<PolicyValue>[] = [
  { value: "allow", label: "Permitir", tone: "ok" },
  { value: "ask", label: "Perguntar", tone: "warn" },
  { value: "deny", label: "Bloquear", tone: "bad" },
];

/** Botão (.btn). Props: variant? "primary" (padrão) | "ghost" (.btn.g) | "bad", small?, + atributos de button. */
export function Btn({ variant = "primary", small, className, type = "button", ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "ghost" | "bad"; small?: boolean }) {
  return <button type={type} className={cx("ui-btn", variant === "ghost" && "g", variant === "bad" && "bad", small && "sm", className)} {...rest} />;
}

export interface Column<R> {
  key: string;
  header: ReactNode;
  render: (row: R) => ReactNode;
  /** Alinha à direita (números). */
  right?: boolean;
  width?: number | string;
}

/**
 * Tabela (.t). Props: columns [{key,header,render,right?,width?}], rows, rowKey,
 * selected? (chave da linha marcada), onRowClick?, empty? (estado vazio).
 */
export function DataTable<R>({
  columns,
  rows,
  rowKey,
  selected,
  onRowClick,
  empty = "Nada para mostrar.",
}: {
  columns: Column<R>[];
  rows: R[];
  rowKey: (r: R) => string | number;
  selected?: string | number | null;
  onRowClick?: (r: R) => void;
  empty?: ReactNode;
}) {
  return (
    <table className="ui-t">
      <thead>
        <tr>
          {columns.map((c) => (
            <th key={c.key} className={c.right ? "r" : undefined} style={c.width != null ? { width: c.width } : undefined}>
              {c.header}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.length === 0 ? (
          <tr>
            <td className="ui-t-empty" colSpan={columns.length}>
              {empty}
            </td>
          </tr>
        ) : (
          rows.map((r) => {
            const k = rowKey(r);
            return (
              <tr key={k} className={cx(onRowClick && "click", selected === k && "sel") || undefined} onClick={onRowClick ? () => onRowClick(r) : undefined}>
                {columns.map((c) => (
                  <td key={c.key} className={c.right ? "r" : undefined}>
                    {c.render(r)}
                  </td>
                ))}
              </tr>
            );
          })
        )}
      </tbody>
    </table>
  );
}

/** Ponto de estado (.dot). Props: tone? ok|warn|bad|info, title? (também vira rótulo acessível). */
export function StatusDot({ tone, title }: { tone?: Tone; title?: string }) {
  return <span className={cx("ui-dot", tone)} title={title} aria-label={title} role={title ? "img" : undefined} />;
}

/** Barra de progresso (.bar). Props: value 0..1 (limitado), tone? ok|warn|bad, label? (acessível). */
export function Bar({ value, tone, label }: { value: number; tone?: Exclude<Tone, "info">; label?: string }) {
  const pct = Math.max(0, Math.min(1, Number.isFinite(value) ? value : 0)) * 100;
  return (
    <div className={cx("ui-bar", tone)} role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)} aria-label={label}>
      <i style={{ width: `${pct}%` }} />
    </div>
  );
}

/** Coluna lateral (.side). Props: width? (px, padrão 230), label? (acessível). Filhos: SideHeading/SideItem. */
export function SideList({ children, width, label }: { children: ReactNode; width?: number; label?: string }) {
  return (
    <nav className="ui-side" aria-label={label} style={width ? { width } : undefined}>
      {children}
    </nav>
  );
}

/** Título de grupo da coluna lateral (.side h4). */
export function SideHeading({ children }: { children: ReactNode }) {
  return <h4>{children}</h4>;
}

/** Item da coluna lateral (.it). Props: on?, onClick?, icon? (ícone ou StatusDot à esquerda), meta? (à direita), title?. */
export function SideItem({ on, onClick, icon, meta, children, title }: { on?: boolean; onClick?: () => void; icon?: ReactNode; meta?: ReactNode; children: ReactNode; title?: string }) {
  return (
    <button type="button" className={cx("ui-it", on && "on")} aria-current={on ? "true" : undefined} onClick={onClick} title={title}>
      {icon}
      <span className="grow">{children}</span>
      {meta != null && <span className="meta">{meta}</span>}
    </button>
  );
}

/** Cabeçalho de área (.h). Props: title, sub? (texto secundário), actions? (à direita), children? (chips ao lado do título). */
export function AreaHeader({ title, sub, actions, children }: { title: ReactNode; sub?: ReactNode; actions?: ReactNode; children?: ReactNode }) {
  return (
    <header className="ui-ah">
      <h1>{title}</h1>
      {sub && <span className="sub">{sub}</span>}
      {children}
      <span className="ui-sp" />
      {actions}
    </header>
  );
}
