import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent, type RefObject } from "react";
import { formatTokensPt, formatUsdPt, totalTokens, type DayLine } from "@/api";

export type Measure = "cost" | "tokens";

/** Piso da altura: abaixo disto as colunas viram risquinhos. */
const MIN_H = 170;
const M = { top: 10, right: 8, bottom: 20, left: 52 };

/**
 * Arredonda o topo da escala para um valor "redondo".
 *
 * Os degraus eram 1, 2, 2.5, 5 e 10: entre 5 e 10 não havia nada, então um dia
 * de pico de 6,5 mi tokens era desenhado contra um eixo de 10 mi e as colunas
 * usavam dois terços da altura, com um terço de grade vazia por cima. Com 6, 8
 * e 3 no meio, a barra mais alta fica sempre acima de 80% do gráfico.
 */
export function niceMax(v: number): number {
  if (!(v > 0)) return 1;
  const pow = Math.pow(10, Math.floor(Math.log10(v)));
  const f = v / pow;
  const step = f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 3 ? 3 : f <= 4 ? 4 : f <= 5 ? 5 : f <= 6 ? 6 : f <= 8 ? 8 : 10;
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

/**
 * Mede o espaço que o contêiner dá, nas duas direções.
 *
 * A altura era fixa em 170 px dentro de um cartão que ia até o fim da tela —
 * daí os quase 450 px de nada embaixo do gráfico na área de Métricas. Medir a
 * altura não entra em laço porque quem a define é o `flex` do cartão (ver
 * `.mxa-chart` em metrics-area.css), não o conteúdo.
 */
function useBox(): [RefObject<HTMLDivElement>, number, number] {
  const ref = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 600, h: MIN_H });
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const apply = () => setBox({ w: Math.max(240, Math.floor(el.clientWidth)), h: Math.max(MIN_H, Math.floor(el.clientHeight)) });
    apply();
    const ro = new ResizeObserver(apply);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return [ref, box.w, box.h];
}

/**
 * Colunas por dia em SVG, com uma medida por vez (custo equivalente em API ou
 * tokens) para manter um único eixo. Passar o mouse (ou as setas, com o gráfico
 * em foco) mostra os valores do dia.
 */
export function DayChart({ days, measure }: { days: DayLine[]; measure: Measure }) {
  const [ref, width, height] = useBox();
  const [hover, setHover] = useState<number | null>(null);

  if (days.length === 0) return <div className="muted small">Sem dados.</div>;

  const fmt = measure === "cost" ? formatUsdPt : formatTokensPt;
  const plotW = width - M.left - M.right;
  const plotH = height - M.top - M.bottom;
  const n = days.length;
  const slot = plotW / n;
  /*
   * A barra ocupa uma fração da casa, em vez de um teto fixo de 28 px: desde
   * que o gráfico passou a crescer em altura, 28 px de largura contra 600 px
   * de altura desenhava palitos separados por vãos maiores que eles. O teto
   * continua existindo (`slot - 6`) para as colunas não se encostarem.
   */
  const barW = Math.max(1, Math.min(slot - 6, Math.max(14, slot * 0.6)));
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
        height={height}
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
            y={height - 5}
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
          <span>Custo equivalente: {formatUsdPt(tipDay.line.api_cost_usd)}</span>
          <span>Tokens: {formatTokensPt(totalTokens(tipDay.line))}</span>
          <span>Turnos: {tipDay.line.turns}</span>
        </div>
      )}
    </div>
  );
}
