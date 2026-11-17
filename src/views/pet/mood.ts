/** Humor do pet, derivado do uso. Funções puras (sem React), fáceis de testar. */

export type PetMood = "sleeping" | "working" | "sweating" | "happy" | "worried";

/** Quanto tempo o "feliz" (tarefa concluída) e o "preocupado" (erro) ficam na tela. */
export const RECENT_MS = 60_000;
/** Tokens no dia a partir dos quais o uso conta como pesado. */
export const HEAVY_TOKENS = 2_000_000;
/** CPU global (%) a partir da qual o pet sua. */
export const HEAVY_CPU = 85;

export interface PetSnapshot {
  runningAgents: number;
  tokensToday: number;
  /** CPU global em % (0 a 100). */
  cpu: number;
  /** Momento (ms) da última tarefa concluída e do último erro. */
  lastOkAt: number | null;
  lastErrAt: number | null;
  /** Momento (ms) em que o usuário clicou no pet: dispensa o que veio antes. */
  ackAt: number;
}

function isRecent(at: number | null, now: number, ackAt: number): at is number {
  return at != null && at > ackAt && now - at < RECENT_MS;
}

/**
 * Prioridade: erro recente > carga pesada > sucesso recente > trabalhando > dormindo.
 * Entre erro e sucesso recentes, vale o que aconteceu por último.
 */
export function moodOf(s: PetSnapshot, now: number): PetMood {
  const err = isRecent(s.lastErrAt, now, s.ackAt);
  const ok = isRecent(s.lastOkAt, now, s.ackAt);
  const errWins = err && (!ok || (s.lastErrAt as number) >= (s.lastOkAt as number));
  if (errWins) return "worried";
  if (s.cpu >= HEAVY_CPU || (s.runningAgents > 0 && s.tokensToday >= HEAVY_TOKENS)) return "sweating";
  if (ok) return "happy";
  return s.runningAgents > 0 ? "working" : "sleeping";
}

/** Milissegundos até o próximo humor temporário expirar (null = nenhum ativo). */
export function nextExpiry(s: PetSnapshot, now: number): number | null {
  const left = [s.lastOkAt, s.lastErrAt]
    .filter((at): at is number => isRecent(at, now, s.ackAt))
    .map((at) => at + RECENT_MS - now);
  return left.length ? Math.min(...left) : null;
}

export const MOOD_LABEL: Record<PetMood, string> = {
  sleeping: "Zzz… tudo tranquilo",
  working: "De olho no trabalho!",
  sweating: "Ufa, que carga!",
  happy: "Tarefa concluída!",
  worried: "Algo deu errado…",
};
