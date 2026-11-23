/**
 * Bolinha de uso do plano na barra do compositor: um anel que enche com a
 * janela mais usada (5 horas, semanal...). Clicar abre o detalhe de cada
 * janela com a barra e quando reinicia. Os números vêm da própria CLI
 * (`rate_limit_event`), a cada turno; sem turno, fica o último visto.
 */
import { useEffect, useRef, useState } from "react";
import { api, onOrch, variantOf, type RateLimitInfo } from "@/api";
import { Popover } from "@/shell/Popover";

const WINDOW_LABEL: Record<string, string> = {
  five_hour: "Limite de 5 horas",
  seven_day: "Semanal · todos os modelos",
  seven_day_opus: "Semanal · Opus",
  seven_day_sonnet: "Semanal · Sonnet",
};
const ORDER = ["five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet"];

function tone(u: number, status: string): "ok" | "warn" | "err" {
  if (status === "rejected" || u >= 0.9) return "err";
  if (u >= 0.7 || status === "allowed_warning") return "warn";
  return "ok";
}

/** "Reinicia às 18:00" (hoje) ou "Reinicia ter., 10:00". */
function resetLabel(sec: number | null): string {
  if (sec == null) return "";
  const d = new Date(sec * 1000);
  const hm = d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
  if (d.toDateString() === new Date().toDateString()) return `Reinicia às ${hm}`;
  return `Reinicia ${d.toLocaleDateString("pt-BR", { weekday: "short" })}, ${hm}`;
}

const pct = (u: number | null) => Math.round((u ?? 0) * 100);

export function UsageRing() {
  const [limits, setLimits] = useState<RateLimitInfo[]>([]);
  const [open, setOpen] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    let alive = true;
    api.rateLimits().then((l) => alive && setLimits(l), () => undefined);
    const off = onOrch((ev) => {
      if (!("Agent" in ev)) return;
      const { kind, data } = variantOf(ev.Agent.event);
      if (kind !== "RateLimit") return;
      const info = data as RateLimitInfo;
      setLimits((cur) => [...cur.filter((x) => x.window !== info.window), info]);
    });
    return () => {
      alive = false;
      void off.then((f) => f()).catch(() => undefined);
    };
  }, []);

  // Só janelas que a CLI informou de verdade e cujo ciclo ainda não virou: um
  // valor de ciclo vencido não vale mais, e janela nenhuma é inventada aqui
  // para a lista parecer completa.
  const nowSec = Date.now() / 1000;
  const all = limits
    .filter((l) => l.utilization != null && !(l.resets_at != null && l.resets_at <= nowSec))
    .sort((a, b) => ((ORDER.indexOf(a.window) + 99) % 99) - ((ORDER.indexOf(b.window) + 99) % 99));
  if (all.length === 0) return null;

  const top = all.reduce((m, l) => ((l.utilization ?? 0) > (m.utilization ?? 0) ? l : m), all[0]);
  const u = Math.min(1, top.utilization ?? 0);
  const t = tone(u, top.status);
  const R = 6.5;
  const C = 2 * Math.PI * R;
  const summary = all.map((l) => `${WINDOW_LABEL[l.window] ?? l.window}: ${pct(l.utilization)}%`).join(" · ") + " · clique para detalhes";

  return (
    <span className="ur">
      <button
        ref={btn}
        type="button"
        className={`ur-btn ${t}`}
        aria-label={`Uso do plano: ${summary}`}
        title={summary}
        aria-expanded={open}
        aria-haspopup="dialog"
        onClick={() => setOpen((v) => !v)}
      >
        <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="8" cy="8" r={R} className="ur-track" />
          <circle cx="8" cy="8" r={R} className="ur-fill" strokeDasharray={C} strokeDashoffset={C * (1 - u)} transform="rotate(-90 8 8)" />
        </svg>
      </button>
      <Popover anchor={btn} open={open} onClose={() => setOpen(false)} ariaLabel="Seus limites de uso" width={360} className="ur-pop">
        <div className="ur-head">Seus limites de uso</div>
        {all.map((l) => {
          const lt = tone(l.utilization ?? 0, l.status);
          const name = WINDOW_LABEL[l.window] ?? l.window;
          return (
            <div key={l.window} className="ur-row">
              <div className="ur-line">
                <span className="ur-name">{name}</span>
                <span className="ur-meta">
                  {resetLabel(l.resets_at)}
                  <b>{`${pct(l.utilization)}%`}</b>
                </span>
              </div>
              <div className="ur-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct(l.utilization)} aria-label={name}>
                <i className={lt} style={{ transform: `scaleX(${Math.min(1, l.utilization ?? 0)})` }} />
              </div>
            </div>
          );
        })}
        <p className="ur-note">
          A CLI do Claude informa, a cada turno, só a janela mais perto do limite. As outras aparecem aqui quando forem informadas; nenhuma
          é estimada.
        </p>
      </Popover>
    </span>
  );
}
