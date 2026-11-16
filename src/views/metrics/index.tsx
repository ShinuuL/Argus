import { useEffect, useState } from "react";
import {
  api,
  baseName,
  errorMessage,
  formatTokensPt,
  formatUsdPt,
  totalTokens,
  type CostLine,
  type MetricsReport,
} from "@/api";
import { useApp, useWorkbench } from "@/store";
import { DayChart, shortDay, type Measure } from "./DayChart";
import "./metrics.css";

/** Período do filtro em dias (0 = tudo). */
const PERIODS: { days: number; label: string }[] = [
  { days: 7, label: "7 dias" },
  { days: 30, label: "30 dias" },
  { days: 0, label: "Tudo" },
];

/** Métricas na barra lateral: resumo de hoje e atalhos para as abas de métricas e do painel. */
export function MetricsSidebar() {
  const openMetrics = useWorkbench((s) => s.openMetrics);
  const openDashboard = useWorkbench((s) => s.openDashboard);
  const tokens = useApp((s) => s.today.tokens);
  const cost = useApp((s) => s.today.cost);
  return (
    <div className="side-pad">
      <div className="mx-side-kpi">
        <span className="small muted">Hoje, custo equivalente em API</span>
        <b>{formatUsdPt(cost)}</b>
        <span className="small">{formatTokensPt(tokens)} tokens</span>
      </div>
      <button onClick={openMetrics}>Abrir métricas de tokens e custo</button>
      <button onClick={openDashboard}>Abrir painel de projetos</button>
    </div>
  );
}

function Kpi({ label, value }: { label: string; value: string }) {
  return (
    <div className="m-kpi">
      <div className="m-kpi-v">{value}</div>
      <div className="muted small">{label}</div>
    </div>
  );
}

/** Barras horizontais em SVG (custo equivalente), com o custo e os tokens ao lado. */
function BarList({ lines, project = false }: { lines: CostLine[]; project?: boolean }) {
  if (lines.length === 0) return <div className="muted small">Sem dados.</div>;
  const maxCost = Math.max(...lines.map((l) => l.api_cost_usd));
  const maxTok = Math.max(...lines.map(totalTokens));
  // Sem preço conhecido (custo zero em tudo), a barra passa a representar os tokens.
  const share = (l: CostLine) =>
    maxCost > 0 ? l.api_cost_usd / maxCost : maxTok > 0 ? totalTokens(l) / maxTok : 0;
  return (
    <div className="m-bars">
      {lines.map((l) => {
        const pct = Math.max(1, share(l) * 100);
        const tip = `${l.key}\nCusto equivalente: ${formatUsdPt(l.api_cost_usd)}\nTokens: ${formatTokensPt(totalTokens(l))} (entrada ${formatTokensPt(l.input_tokens)}, saída ${formatTokensPt(l.output_tokens)})\nTurnos: ${l.turns}`;
        return (
          <div key={l.key} className="m-bar-row" title={tip}>
            <div className="m-bar-label ellipsis">{project ? baseName(l.key) : l.key || "(desconhecido)"}</div>
            <svg className="m-bar" viewBox="0 0 100 10" preserveAspectRatio="none" aria-hidden="true">
              <rect className="mx-track" x="0" y="0" width="100" height="10" />
              <rect className="mx-fill" x="0" y="0" width={pct} height="10" />
            </svg>
            <div className="m-bar-val">{formatUsdPt(l.api_cost_usd)}</div>
            <div className="m-bar-val muted">{formatTokensPt(totalTokens(l))}</div>
          </div>
        );
      })}
    </div>
  );
}

/**
 * Métricas (aba do workbench): tokens e custo equivalente em API por dia,
 * projeto, provedor e modelo, com filtro de período.
 */
export default function MetricsView() {
  const [period, setPeriod] = useState(7);
  const [measure, setMeasure] = useState<Measure>("cost");
  const [report, setReport] = useState<MetricsReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  // O store recalcula o uso de hoje a cada turno: serve de gatilho para recarregar.
  const todayTokens = useApp((s) => s.today.tokens);

  useEffect(() => {
    let alive = true;
    api.metricsReport(period).then(
      (r) => {
        if (!alive) return;
        setReport(r);
        setError(null);
      },
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [period, todayTokens]);

  const total = report?.total;
  const empty = report != null && report.has_db && report.total.turns === 0;

  return (
    <div className="metrics scroll pad">
      <div className="m-head">
        <h3>Uso e custo</h3>
        <div className="m-filter" role="group" aria-label="Período">
          {PERIODS.map((p) => (
            <button
              key={p.days}
              className={period === p.days ? "primary" : ""}
              aria-pressed={period === p.days}
              onClick={() => setPeriod(p.days)}
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>
      {error && <p className="err">{error}</p>}
      {report && !report.has_db && (
        <div className="warn small">
          Histórico desligado: o orquestrador está sem banco de dados, então o uso não é registrado.
        </div>
      )}
      <p className="muted small">
        Custo equivalente em API: o valor que os tokens custariam pela API paga. Com assinatura, não é o valor
        cobrado. Preços editáveis em pricing.json.
      </p>
      {!report && !error && <p className="muted">Carregando...</p>}
      {report && total && (
        <>
          <div className="m-kpis">
            <Kpi label="Custo equivalente em API" value={formatUsdPt(total.api_cost_usd)} />
            <Kpi label="Tokens" value={formatTokensPt(totalTokens(total))} />
            <Kpi
              label="Entrada / saída"
              value={`${formatTokensPt(total.input_tokens)} / ${formatTokensPt(total.output_tokens)}`}
            />
            <Kpi
              label="Cache (leitura / escrita)"
              value={`${formatTokensPt(total.cache_read)} / ${formatTokensPt(total.cache_write)}`}
            />
            <Kpi label="Turnos" value={String(total.turns)} />
          </div>
          {empty && <p className="muted">Nenhum uso registrado no período.</p>}

          <section>
            <div className="mx-section-head">
              <h4>{measure === "cost" ? "Custo equivalente em API por dia" : "Tokens por dia"}</h4>
              <div className="mx-seg" role="group" aria-label="Medida do gráfico">
                <button
                  className={measure === "cost" ? "primary" : ""}
                  aria-pressed={measure === "cost"}
                  onClick={() => setMeasure("cost")}
                >
                  Custo
                </button>
                <button
                  className={measure === "tokens" ? "primary" : ""}
                  aria-pressed={measure === "tokens"}
                  onClick={() => setMeasure("tokens")}
                >
                  Tokens
                </button>
              </div>
            </div>
            <DayChart days={report.days} measure={measure} />
            <details className="mx-table small">
              <summary className="muted">Ver os dias em tabela</summary>
              <table className="grid small">
                <thead>
                  <tr>
                    <th>Dia</th>
                    <th>Custo equivalente</th>
                    <th>Tokens</th>
                    <th>Turnos</th>
                  </tr>
                </thead>
                <tbody>
                  {[...report.days].reverse().map((d) => (
                    <tr key={d.day}>
                      <td>{shortDay(d.day)}</td>
                      <td>{formatUsdPt(d.line.api_cost_usd)}</td>
                      <td>{formatTokensPt(totalTokens(d.line))}</td>
                      <td>{d.line.turns}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </details>
          </section>

          <div className="m-cols">
            <section>
              <h4>Por projeto</h4>
              <BarList lines={report.projects} project />
            </section>
            <section>
              <h4>Por provedor</h4>
              <BarList lines={report.providers} />
              <h4>Por modelo</h4>
              <BarList lines={report.models} />
            </section>
          </div>
        </>
      )}
    </div>
  );
}
