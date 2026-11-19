/**
 * Pet em página inteira (Configurações › Extras › Pet): o mascote grande com o
 * humor do momento, os números que o movem e a legenda de todos os humores.
 * Os dados são os mesmos do widget da status bar.
 */
import { useEffect, useState, type ReactNode } from "react";
import { formatTokensPt, formatUsdPt, isTaskActive } from "@/api";
import { useApp } from "@/store";
import { Mascot } from "./Mascot";
import { HEAVY_CPU, HEAVY_TOKENS, MOOD_LABEL, moodOf, nextExpiry, type PetMood, type PetSnapshot } from "./mood";
import "./pet.css";

/** Quando cada humor aparece — a mesma regra de `moodOf`, em português. */
const MOOD_WHEN: Record<PetMood, string> = {
  sleeping: "Nenhum agente rodando: o Argus está de folga.",
  working: "Pelo menos um agente rodando.",
  sweating: `CPU acima de ${HEAVY_CPU}%, ou dia pesado (${formatTokensPt(HEAVY_TOKENS)} de tokens) com agente rodando.`,
  happy: "Uma tarefa terminou bem no último minuto.",
  worried: "Uma tarefa falhou no último minuto.",
};

const MOODS: PetMood[] = ["sleeping", "working", "sweating", "happy", "worried"];

function Stat({ label, value, children }: { label: string; value: string; children?: ReactNode }) {
  return (
    <div className="pet-stat">
      <span className="pet-stat-label">{label}</span>
      <strong className="pet-stat-value">{value}</strong>
      {children}
    </div>
  );
}

export default function PetPage() {
  const runningAgents = useApp((s) => s.tasks.filter((t) => isTaskActive(t.state)).length);
  const tokensToday = useApp((s) => s.today.tokens);
  const costToday = useApp((s) => s.today.cost);
  const cpu = useApp((s) => s.usage.cpu);
  const memUsed = useApp((s) => s.usage.mem_used_mb);
  const memTotal = useApp((s) => s.usage.mem_total_mb);
  const lastOkAt = useApp((s) => s.lastOkAt);
  const lastErrAt = useApp((s) => s.lastErrAt);

  // Muda quando um humor temporário expira, para recalcular.
  const [, setTick] = useState(0);

  const now = Date.now();
  const snap: PetSnapshot = { runningAgents, tokensToday, cpu, lastOkAt, lastErrAt, ackAt: 0 };
  const mood = moodOf(snap, now);
  const expiry = nextExpiry(snap, now);

  useEffect(() => {
    if (expiry == null) return;
    const timer = setTimeout(() => setTick((t) => t + 1), expiry + 50);
    return () => clearTimeout(timer);
    // `expiry` diminui a cada render; só rearmar quando a origem muda.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lastOkAt, lastErrAt]);

  const cpuPct = Math.min(100, Math.max(0, cpu));
  const memPct = memTotal > 0 ? Math.min(100, (memUsed / memTotal) * 100) : 0;

  return (
    <div className="pet-page">
      <div className={`pet-hero argo argo-${mood}`}>
        <Mascot mood={mood} size={128} />
        <div className="pet-hero-text">
          <p className="pet-mood">{MOOD_LABEL[mood]}</p>
          <p className="ui-mut ui-sm pet-why">{MOOD_WHEN[mood]}</p>
        </div>
      </div>

      <div className="pet-stats">
        <Stat label="Agentes rodando" value={String(runningAgents)} />
        <Stat label="Tokens hoje" value={formatTokensPt(tokensToday)} />
        <Stat label="Custo hoje (equivalente em API)" value={formatUsdPt(costToday)} />
        <Stat label="CPU" value={`${Math.round(cpu)}%`}>
          <span className="pet-meter" aria-hidden="true">
            <span style={{ width: `${cpuPct}%` }} />
          </span>
        </Stat>
        {memTotal > 0 && (
          <Stat label="Memória" value={`${(memUsed / 1024).toFixed(1)} / ${(memTotal / 1024).toFixed(1)} GB`}>
            <span className="pet-meter" aria-hidden="true">
              <span style={{ width: `${memPct}%` }} />
            </span>
          </Stat>
        )}
      </div>

      <div className="pet-legend">
        <strong>Os humores</strong>
        <ul>
          {MOODS.map((m) => (
            <li key={m} className={m === mood ? "active" : ""}>
              <span className={`argo argo-${m} pet-legend-face`}>
                <Mascot mood={m} size={32} />
              </span>
              <span className="pet-legend-text">
                <b>{MOOD_LABEL[m]}</b>
                <span className="ui-mut ui-sm">{MOOD_WHEN[m]}</span>
              </span>
            </li>
          ))}
        </ul>
      </div>

      <p className="ui-mut ui-sm pet-note">
        O mesmo bichinho fica na ponta direita da barra de status: clique nele para ver estes números sem sair do
        que estiver fazendo. As animações param quando o sistema pede menos movimento.
      </p>
    </div>
  );
}
