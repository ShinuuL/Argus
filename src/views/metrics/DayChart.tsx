import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent, type RefObject } from "react";
import { formatTokens, formatUsd, totalTokens, type DayLine } from "@/api";

export type Measure = "cost" | "tokens";

const HEIGHT = 170;
const M = { top: 10, right: 8, bottom: 20, left: 52 };

/** Arredonda o topo da escala para um valor "redondo" (1, 2, 2.5, 5 x 10^n). */
export function niceMax(v: number): number {
  if (!(v > 0)) return 1;
  const pow = Math.pow(10, Math.floor(Math.log10(v)));
  const f = v / pow;
  const step = f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10;
  return step * pow;
}

/** AAAA-MM-DD -> DD/MM. */
export function shortDay(day: string): string {
  const [, m, d] = day.split("-");
  return d && m ? `${d}/${m}` : day;
}

function valueOf(d: DayLine, measure: Measure): number {
  return measure === "cost" ? d.line.api_cost_usd : totalTokens(d.line);
}

/** Coluna com o topo arredondado e a base reta, ancorada na linha de base. */
function barPath(x: number, y: number, w: number, h: number): string {
  const r = Math.min(4, w / 2, h);
  return `M${x},${y + h}V${y + r}Q${x},${y} ${x + r},${y}H${x + w - r}Q${x + w},${y} ${x + w},${y + r}V${y + h}Z`;
}

function useWidth(): [RefObject<HTMLDivElement>, number] {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(600);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const apply = () => setWidth(Math.max(240, Math.floor(el.clientWidth)));
    apply();
    const ro = new ResizeObserver(apply);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return [ref, width];
}

/**
 * Colunas por dia em SVG, com uma medida por vez (custo equivalente em API ou
 * tokens) para manter um único eixo. Passar o mouse (ou as setas, com o gráfico
 * em foco) mostra os valores do dia.
 */
export function DayChart({ days, measure }: { days: DayLine[]; measure: Measure }) {
  const [ref, width] = useWidth();
  const [hover, setHover] = useState<number | null>(null);

  if (days.length === 0) return <div className="muted small">Sem dados.</div>;

  const fmt = measure === "cost" ? formatUsd : formatTokens;
  const plotW = width - M.left - M.right;
  const plotH = HEIGHT - M.top - M.bottom;
  const n = days.length;
  const slot = plotW / n;
  const barW = Math.max(1, Math.min(slot - 2, 28));
  const max = niceMax(Math.max(...days.map((d) => valueOf(d, measure))));
  const y = (v: number) => M.top + plotH - (v / max) * plotH;
  const ticks = [0, max / 2, max];
  const labelIdx = Array.from(new Set([0, Math.floor((n - 1) / 2), n - 1]));
  const active = hover != null && hover < n ? hover : null;

  const onMove = (e: MouseEvent<SVGSVGElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const i = Math.floor((e.clientX - rect.left - M.left) / slot);
    setHover(i >= 0 && i < n ? i : null);
  };
  const onKey = (e: KeyboardEvent<SVGSVGElement>) => {
    if (e.key === "ArrowRight") setHover((h) => Math.min(n - 1, (h ?? -1) + 1));
    else if (e.key === "ArrowLeft") setHover((h) => Math.max(0, (h ?? n) - 1));
    else if (e.key === "Escape") setHover(null);
    else return;
    e.preventDefault();
  };

  const tipDay = active != null ? days[active] : null;
  const tipLeft = active != null ? M.left + slot * active + slot / 2 : 0;

  return (
    <div className="m-chart mx-chart" ref={ref}>
      <svg
        width={width}
        height={HEIGHT}
        className="mx-day-svg"
        role="img"
        tabIndex={0}
        aria-label={
          measure === "cost"
            ? "Custo equivalente em API por dia. Use as setas para percorrer os dias."
            : "Tokens por dia. Use as setas para percorrer os dias."
        }
        onMouseMove={onMove}
        onMouseLeave={() => setHover(null)}
        onBlur={() => setHover(null)}
        onKeyDown={onKey}
      >
        {ticks.map((t) => (
          <g key={t}>
            <line className="mx-grid" x1={M.left} x2={width - M.right} y1={y(t)} y2={y(t)} />
            <text className="mx-tick" x={M.left - 6} y={y(t)} textAnchor="end" dominantBaseline="middle">
              {fmt(t)}
            </text>
          </g>
        ))}
        {active != null && (
          <rect className="mx-band" x={M.left + slot * active} y={M.top} width={slot} height={plotH} />
        )}
        {days.map((d, i) => {
          const v = valueOf(d, measure);
          if (v <= 0) return null;
          const h = Math.max(1, (v / max) * plotH);
          const x = M.left + slot * i + (slot - barW) / 2;
          return (
            <path
              key={d.day}
              className={i === active ? "mx-bar on" : "mx-bar"}
              d={barPath(x, M.top + plotH - h, barW, h)}
            />
          );
        })}
        {labelIdx.map((i) => (
          <text
            key={i}
            className="mx-tick"
            x={M.left + slot * i + slot / 2}
            y={HEIGHT - 5}
            textAnchor={i === 0 && n > 1 ? "start" : i === n - 1 && n > 1 ? "end" : "middle"}
          >
            {shortDay(days[i].day)}
          </text>
        ))}
      </svg>
      {tipDay && (
        <div
          className="mx-tip"
          style={tipLeft > width / 2 ? { right: width - tipLeft + 10 } : { left: tipLeft + 10 }}
          role="status"
        >
          <b>{shortDay(tipDay.day)}</b>
          <span>Custo equivalente: {formatUsd(tipDay.line.api_cost_usd)}</span>
          <span>Tokens: {formatTokens(totalTokens(tipDay.line))}</span>
          <span>Turnos: {tipDay.line.turns}</span>
        </div>
      )}
    </div>
  );
}
