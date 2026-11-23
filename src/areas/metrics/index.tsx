/** Área "Uso e métricas" (tela 11 do mockup): 3 cards, gráfico por dia e seletor 7/30 dias. */
import { useEffect, useState } from "react";
import { api, baseName, errorMessage, formatTokensPt, formatUsdPt, totalTokens, type MetricsReport, type RateLimitInfo } from "@/api";
import { useApp } from "@/store";
import { AreaHeader, Card, Seg, AreaStatus } from "@/ui";
import { DayChart, type Measure } from "@/views/metrics/DayChart";
import "@/views/metrics/metrics.css";
import "./metrics-area.css";

type Period = "7" | "30";

const WINDOW_LABEL: Record<string, string> = {
  five_hour: "5 horas",
  seven_day: "semanal",
  seven_day_opus: "semanal · Opus",
  seven_day_sonnet: "semanal · Sonnet",
};

/** "reinicia às 18:00" (hoje) ou "reinicia ter., 10:00". */
function resetLabel(sec: number | null): string {
  if (sec == null) return "";
  const d = new Date(sec * 1000);
  const hm = d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
  if (d.toDateString() === new Date().toDateString()) return `reinicia às ${hm}`;
  return `reinicia ${d.toLocaleDateString("pt-BR", { weekday: "short" })}, ${hm}`;
}

export default function MetricsArea() {
  const [period, setPeriod] = useState<Period>("7");
  const [measure, setMeasure] = useState<Measure>("tokens");
  const [report, setReport] = useState<MetricsReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [limits, setLimits] = useState<RateLimitInfo[]>([]);
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

  // Os limites do plano vêm da CLI a cada turno; aqui basta o último visto.
  useEffect(() => {
    let alive = true;
    api.rateLimits().then((l) => alive && setLimits(l), () => undefined);
    return () => {
      alive = false;
    };
  }, [todayTokens]);

  /*
   * Janela mais apertada entre as que a CLI informou e cujo ciclo não virou.
   * Mesma regra do anel da conversa: ciclo vencido não vale, e nenhuma janela
   * é inventada para a lista parecer completa.
   */
  const nowSec = Date.now() / 1000;
  const janela = limits
    .filter((l) => l.utilization != null && !(l.resets_at != null && l.resets_at <= nowSec))
    .reduce<RateLimitInfo | null>((m, l) => (!m || (l.utilization ?? 0) > (m.utilization ?? 0) ? l : m), null);

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
            {/*
              * Este cartão dizia "o Argus ainda não lê o limite do plano" e
              * mostrava um traço — texto que ficou velho na 0.1.3, quando o
              * anel do compositor passou a mostrar exatamente esse número. Um
              * terço da fileira de cima ocupado para negar o que a tela do
              * lado já fazia. Agora é o dado; quando a CLI não informa
              * nenhuma janela, o cartão não aparece em vez de aparecer vazio.
              */}
            {janela && (
              <Card>
                <div className="ui-mut ui-sm">Janela do plano · {WINDOW_LABEL[janela.window] ?? janela.window}</div>
                <div className="mxa-big">{Math.round((janela.utilization ?? 0) * 100)}%</div>
                <div className="ui-sm ui-mut">{resetLabel(janela.resets_at) || "vindo da CLI a cada turno"}</div>
              </Card>
            )}
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
