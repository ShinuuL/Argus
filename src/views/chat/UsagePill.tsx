import { useEffect, useState } from "react";
import { api, formatTokensPt, formatUsdPt, onOrch, totalTokens, variantOf, type CostLine } from "@/api";
import { useApp } from "@/store";

/** Mantidos por compatibilidade: delegam ao formatador único de @/api. */
export const tokensPt = formatTokensPt;
export const usdPt = formatUsdPt;

/**
 * Pílula "hoje: 1,2 mi tokens · ≈ US$ 3,40 (equivalente em API)". O detalhe
 * por modelo aparece ao passar o mouse. Atualiza a cada evento de uso.
 */
export function UsagePill() {
  const today = useApp((s) => s.today);
  const [models, setModels] = useState<CostLine[]>([]);

  useEffect(() => {
    let alive = true;
    let timer: number | undefined;
    const load = () => {
      void api
        .metricsReport(1)
        .then((r) => {
          if (alive) setModels(r.models);
        })
        .catch(() => undefined);
    };
    load();
    const off = onOrch((ev) => {
      if ("Agent" in ev && variantOf(ev.Agent.event).kind === "Usage") {
        window.clearTimeout(timer);
        timer = window.setTimeout(load, 1500);
      }
    });
    return () => {
      alive = false;
      window.clearTimeout(timer);
      void off.then((f) => f()).catch(() => undefined);
    };
  }, []);

  const detail = models.length
    ? "Por modelo (último dia), custo equivalente em API:\n" +
      models
        .map((m) => `${m.key || "modelo não informado"}: ${tokensPt(totalTokens(m))} tokens · ≈ ${usdPt(m.api_cost_usd)}`)
        .join("\n")
    : "Sem uso registrado hoje.";
  const tip = `Uso de hoje: ${tokensPt(today.tokens)} tokens · ≈ ${usdPt(today.cost)} (equivalente em API)

${detail}`;

  return (
    <span className="chat-usage" title={tip}>
      hoje {tokensPt(today.tokens)} tok · {usdPt(today.cost)}
    </span>
  );
}
