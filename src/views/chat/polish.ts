/**
 * "Melhorar pedido": reescreve o rascunho antes de ele ir ao agente.
 *
 * A reescrita acontece no backend (`chat_polish_prompt`, que roda um `claude -p`
 * barato, sem ferramentas e fora da pasta do projeto). Aqui ficam só as partes
 * puras da tela: o que vai de contexto e a preferência em memória.
 */
import { polishModeGet, type PolishMode } from "./backend";

export type { PolishMode };

export const POLISH_LABELS: Record<PolishMode, string> = {
  off: "Desligado",
  button: "No botão (✨ ou Ctrl+I)",
  auto: "Em todo envio (com conferência)",
};

/** Nome do projeto a partir da pasta de trabalho (sem o caminho completo). */
export function projectName(workdir?: string | null): string | null {
  if (!workdir) return null;
  const parts = workdir.replace(/\\/g, "/").replace(/\/+$/, "").split("/");
  const last = parts[parts.length - 1] ?? "";
  // Pasta de worktree (".argus-worktrees/<projeto>-<n>"): usa o nome do projeto.
  const m = /^(.*)-\d+$/.exec(last);
  return (parts.includes(".argus-worktrees") && m ? m[1] : last) || null;
}

/** Arquivos citados com `@` no rascunho (até 10, sem repetir). */
export function mentionedFiles(text: string): string[] {
  const out: string[] = [];
  for (const m of text.matchAll(/(?:^|[\s(])@([\w./\\-]+)/g)) {
    const rel = m[1].replace(/\\/g, "/");
    if (rel.includes("/") || /\.\w+$/.test(rel)) {
      if (!out.includes(rel)) out.push(rel);
    }
    if (out.length >= 10) break;
  }
  return out;
}

let cached: Promise<PolishMode> | null = null;

/** Preferência gravada, consultada uma vez por execução. */
export function polishMode(): Promise<PolishMode> {
  cached ??= polishModeGet().catch(() => "button" as PolishMode);
  return cached;
}

/** Atualiza o que as caixas já abertas veem, depois de trocar em Configurações. */
export function setPolishModeCache(mode: PolishMode): void {
  cached = Promise.resolve(mode);
}
