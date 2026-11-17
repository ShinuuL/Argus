/** Área "Uso e métricas" (tela 11 do mockup): 3 cards, gráfico por dia e seletor 7/30 dias. */
import { useEffect, useState } from "react";
import { api, baseName, errorMessage, formatTokensPt, formatUsdPt, totalTokens, type MetricsReport } from "@/api";
import { useApp } from "@/store";
import { AreaHeader, Card, Seg, AreaStatus } from "@/ui";
import { DayChart, type Measure } from "@/views/metrics/DayChart";
import "@/views/metrics/metrics.css";
import "./metrics-area.css";

type Period = "7" | "30";

export default function MetricsArea() {
  const [period, setPeriod] = useState<Period>("7");
  const [measure, setMeasure] = useState<Measure>("tokens");
  const [report, setReport] = useState<MetricsReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  // O store recalcula o uso de hoje a cada turno: serve de gatilho para recarregar.
  const todayTokens = useApp((s) => s.today.tokens);

  useEffect(() => {
    let alive = true;
    api.metricsReport(Number(period)).then(
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

  const days = Number(period);
  const total = report?.total;
  const totalTok = total ? totalTokens(total) : 0;
  const top = report?.projects.reduce<(typeof report.projects)[number] | null>(
    (best, l) => (!best || totalTokens(l) > totalTokens(best) ? l : best),
    null,
  );
  const topShare = top && totalTok > 0 ? Math.round((totalTokens(top) / totalTok) * 100) : 0;

  return (
    <div className="ui-area">
      <div className="ui-area-main">
        <AreaHeader
          title="Uso"
          actions={
            <Seg<Period>
              label="Período"
              value={period}
              onChange={setPeriod}
              options={[
                { value: "7", label: "7 dias" },
                { value: "30", label: "30 dias" },
              ]}
            />
          }
        />
        <div className="mxa ui-pad">
          {error && <p className="mxa-err">{error}</p>}
          {report && !report.has_db && (
            <p className="mxa-warn ui-sm">
              Histórico desligado: o orquestrador está sem banco de dados, então o uso não é registrado.
            </p>
          )}

          <div className="ui-grid mxa-cards">
            <Card>
              <div className="ui-mut ui-sm">Limite da janela atual</div>
              <div className="mxa-big ui-mut">—</div>
              <div className="ui-sm ui-mut">O Argus ainda não lê o limite do plano.</div>
            </Card>
            <Card>
              <div className="ui-mut ui-sm">Turnos em {days} dias</div>
              <div className="mxa-big">{total ? total.turns : "—"}</div>
              <div className="ui-sm ui-mut">
                {total
                  ? `média ${Math.round(total.turns / days)} por dia · ${formatTokensPt(totalTok)} tokens · ${formatUsdPt(total.api_cost_usd)} eq. API`
                  : "carregando…"}
              </div>
            </Card>
            <Card>
              <div className="ui-mut ui-sm">Projeto que mais usou</div>
              <div className="mxa-big mxa-ellipsis" title={top?.key}>
                {top && totalTokens(top) > 0 ? baseName(top.key) : "—"}
              </div>
              <div className="ui-sm ui-mut">
                {top && totalTokens(top) > 0 ? `${topShare}% dos tokens` : "sem uso no período"}
              </div>
            </Card>
          </div>

          <Card
            className="mxa-chart"
            title={<span className="ui-mut ui-sm mxa-ct">{measure === "tokens" ? "Tokens por dia" : "Custo equivalente em API por dia"}</span>}
            actions={
              <Seg<Measure>
                label="Medida"
                value={measure}
                onChange={setMeasure}
                options={[
                  { value: "tokens", label: "Tokens" },
                  { value: "cost", label: "Custo" },
                ]}
              />
            }
          >
            {report ? (
              total && total.turns === 0 ? (
                <div className="ui-empty">Nenhum uso registrado no período.</div>
              ) : (
                <DayChart days={report.days} measure={measure} />
              )
            ) : (
              !error && <div className="ui-empty">Carregando…</div>
            )}
          </Card>
        </div>
        <AreaStatus>
          <span>dados locais</span>
        </AreaStatus>
      </div>
    </div>
  );
}
