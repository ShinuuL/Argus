/**
 * Pet de uso (status bar, à direita): um mascote original de um olho só, que
 * observa o que acontece na IDE. O humor reflete agentes rodando, tokens e
 * custo equivalente em API do dia, CPU e sucesso/erro recente. Clicar abre o
 * balão com os números. As animações param com `prefers-reduced-motion`.
 */
import { useEffect, useState } from "react";
import { formatTokensPt, formatUsdPt, isTaskActive } from "@/api";
import { useApp } from "@/store";
import { Mascot } from "./Mascot";
import { MOOD_LABEL, moodOf, nextExpiry, type PetSnapshot } from "./mood";
import "./pet.css";

export default function PetView() {
  const runningAgents = useApp((s) => s.tasks.filter((t) => isTaskActive(t.state)).length);
  const tokensToday = useApp((s) => s.today.tokens);
  const costToday = useApp((s) => s.today.cost);
  const cpu = useApp((s) => s.usage.cpu);
  const memUsed = useApp((s) => s.usage.mem_used_mb);
  const memTotal = useApp((s) => s.usage.mem_total_mb);
  const lastOkAt = useApp((s) => s.lastOkAt);
  const lastErrAt = useApp((s) => s.lastErrAt);

  const [open, setOpen] = useState(false);
  const [ackAt, setAckAt] = useState(0);
  // Muda quando um humor temporário expira, para recalcular.
  const [, setTick] = useState(0);

  const now = Date.now();
  const snap: PetSnapshot = { runningAgents, tokensToday, cpu, lastOkAt, lastErrAt, ackAt };
  const mood = moodOf(snap, now);
  const expiry = nextExpiry(snap, now);

  useEffect(() => {
    if (expiry == null) return;
    const timer = setTimeout(() => setTick((t) => t + 1), expiry + 50);
    return () => clearTimeout(timer);
    // `expiry` diminui a cada render; só rearmar quando a origem muda.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lastOkAt, lastErrAt, ackAt]);

  const label = MOOD_LABEL[mood];
  const cpuPct = Math.min(100, Math.max(0, cpu));
  const summary = `${label} Agentes rodando: ${runningAgents}. Tokens hoje: ${formatTokensPt(tokensToday)}. Custo de hoje, equivalente em API: ${formatUsdPt(costToday)}. CPU: ${Math.round(cpu)}%.`;

  const toggle = () => {
    // Clicar conta como "visto": o feliz/preocupado dá lugar ao estado real.
    if (mood === "happy" || mood === "worried") setAckAt(Date.now());
    setOpen((o) => !o);
  };

  return (
    <span
      className={`argo argo-${mood}`}
      onMouseLeave={() => setOpen(false)}
      onKeyDown={(e) => e.key === "Escape" && setOpen(false)}
      onBlur={(e) => !e.currentTarget.contains(e.relatedTarget as Node | null) && setOpen(false)}
    >
      <button className="argo-btn" title={open ? undefined : summary} aria-label={summary} aria-expanded={open} onClick={toggle}>
        <Mascot mood={mood} />
      </button>
      {open && (
        <div className="argo-bubble" role="status">
          <strong>{label}</strong>
          <div className="argo-row">
            <span>Agentes rodando</span>
            <b>{runningAgents}</b>
          </div>
          <div className="argo-row">
            <span>Tokens hoje</span>
            <b>{formatTokensPt(tokensToday)}</b>
          </div>
          <div className="argo-row">
            <span>Custo hoje (equivalente em API)</span>
            <b>{formatUsdPt(costToday)}</b>
          </div>
          <div className="argo-row">
            <span>CPU</span>
            <span className="argo-meter" aria-hidden="true">
              <span style={{ width: `${cpuPct}%` }} />
            </span>
            <b>{Math.round(cpu)}%</b>
          </div>
          {memTotal > 0 && (
            <div className="argo-row">
              <span>Memória</span>
              <b>
                {(memUsed / 1024).toFixed(1)} / {(memTotal / 1024).toFixed(1)} GB
              </b>
            </div>
          )}
        </div>
      )}
    </span>
  );
}
