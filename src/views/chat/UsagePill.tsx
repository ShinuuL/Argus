import { useEffect, useState } from "react";
import { api, onOrch, totalTokens, variantOf, type CostLine } from "@/api";
import { useApp } from "@/store";

/** 950 · 12,3 mil · 1,2 mi (português). */
export function tokensPt(n: number): string {
  const f = (v: number) => v.toLocaleString("pt-BR", { maximumFractionDigits: 1 });
  if (n >= 1e9) return `${f(n / 1e9)} bi`;
  if (n >= 1e6) return `${f(n / 1e6)} mi`;
  if (n >= 1e3) return `${f(n / 1e3)} mil`;
  return String(Math.round(n));
}

/** US$ 3,40. */
export function usdPt(v: number): string {
  const digits = v > 0 && v < 0.01 ? 4 : 2;
  return `US$ ${v.toLocaleString("pt-BR", { minimumFractionDigits: digits, maximumFractionDigits: digits })}`;
}

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

  return (
    <span className="chat-usage" title={detail}>
      hoje: {tokensPt(today.tokens)} tokens · ≈ {usdPt(today.cost)} <small>(equivalente em API)</small>
    </span>
  );
}
