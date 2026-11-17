/**
 * Pet de uso (status bar, à direita): um mascote original de um olho só, que
 * observa o que acontece na IDE. O humor reflete agentes rodando, tokens e
 * custo equivalente em API do dia, CPU e sucesso/erro recente. Clicar abre o
 * balão com os números. As animações param com `prefers-reduced-motion`.
 */
import { useEffect, useState, type ReactNode } from "react";
import { formatTokensPt, formatUsdPt, isTaskActive } from "@/api";
import { useApp } from "@/store";
import { MOOD_LABEL, moodOf, nextExpiry, type PetMood, type PetSnapshot } from "./mood";
import "./pet.css";

/** Olho (e boca) de cada humor. O olho fica em (32, 33), raio 12. */
function face(mood: PetMood): ReactNode {
  switch (mood) {
    case "sleeping":
      return (
        <>
          <circle className="argo-lid" cx="32" cy="33" r="12" />
          <path className="argo-line" d="M22 34 Q32 42 42 34" />
        </>
      );
    case "happy":
      return (
        <>
          <circle className="argo-lid" cx="32" cy="33" r="12" />
          <path className="argo-line" d="M22 37 Q32 25 42 37" />
          <path className="argo-ink" d="M27 49 Q32 55 37 49 Z" />
        </>
      );
    case "working":
      return (
        <>
          <circle className="argo-eye" cx="32" cy="33" r="12" />
          <g className="argo-pupil">
            <circle className="argo-ink" cx="32" cy="33" r="5.5" />
            <circle className="argo-glint" cx="34" cy="31" r="1.6" />
          </g>
          <path className="argo-line" d="M29 50 h6" />
        </>
      );
    case "sweating":
      return (
        <>
          <circle className="argo-eye" cx="32" cy="33" r="12" />
          <circle className="argo-ink" cx="32" cy="33" r="3.2" />
          <ellipse className="argo-ink" cx="32" cy="50.5" rx="3" ry="2.2" />
        </>
      );
    case "worried":
      return (
        <>
          <circle className="argo-eye" cx="32" cy="33" r="12" />
          <circle className="argo-ink" cx="29" cy="36" r="4.2" />
          <circle className="argo-glint" cx="30.5" cy="34.5" r="1.2" />
          <path className="argo-line" d="M21 21 L41 16" />
          <path className="argo-line" d="M28 52 Q32 48 36 52" />
        </>
      );
  }
}

function Mascot({ mood }: { mood: PetMood }) {
  return (
    <svg className="argo-svg" viewBox="0 0 64 64" width="22" height="22" aria-hidden="true">
      <ellipse className="argo-shadow" cx="32" cy="60.5" rx="15" ry="2.5" />
      <g className="argo-body">
        {/* antenas */}
        <path className="argo-stalk" d="M26 16 Q23 8 19 7" />
        <path className="argo-stalk" d="M38 16 Q41 8 45 7" />
        <circle className="argo-tip" cx="19" cy="7" r="2.6" />
        <circle className="argo-tip" cx="45" cy="7" r="2.6" />
        {/* pés e corpo */}
        <ellipse className="argo-foot" cx="23" cy="57" rx="5.5" ry="2.6" />
        <ellipse className="argo-foot" cx="41" cy="57" rx="5.5" ry="2.6" />
        <path
          className="argo-fill"
          d="M32 13 C46 13 54 24 54 38 C54 51 45 57 32 57 C19 57 10 51 10 38 C10 24 18 13 32 13 Z"
        />
        <circle className="argo-cheek" cx="15.5" cy="45" r="2.8" />
        <circle className="argo-cheek" cx="48.5" cy="45" r="2.8" />
        {face(mood)}
        {mood === "sweating" && <path className="argo-drop" d="M51 15 q3.2 5 0 7.2 q-3.2 -2.2 0 -7.2 Z" />}
      </g>
      {mood === "sleeping" && <path className="argo-z" d="M47 5 h7 l-7 8 h7" />}
      {mood === "happy" && (
        <>
          <path className="argo-spark" d="M8 6 L9.5 10.5 L14 12 L9.5 13.5 L8 18 L6.5 13.5 L2 12 L6.5 10.5 Z" />
          <path className="argo-spark late" d="M57 16 L58 19 L61 20 L58 21 L57 24 L56 21 L53 20 L56 19 Z" />
        </>
      )}
    </svg>
  );
}

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
