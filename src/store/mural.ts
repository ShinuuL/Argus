/**
 * Mural de arquivos: quem está mexendo em quê, agora (crates/core/src/mural.rs).
 *
 * O backend anota sozinho cada ferramenta de arquivo que passa pelos agentes e
 * manda a anotação no evento `orch`. Aqui ela só vira lista para a tela — e
 * para o usuário ver, antes do diff, que duas sessões foram parar no mesmo
 * arquivo.
 */
import { create } from "zustand";
import { api, type MuralClaim, type TaskId } from "@/api";

/** Mesma validade do core: anotação velha não diz mais nada sobre "agora". */
const VALIDADE_MS = 15 * 60 * 1000;
/** Teto da lista. O painel mostra os primeiros; o resto é histórico sem uso. */
const MAX = 60;

/** Uma anotação por (tarefa, agente, arquivo): repetir no mesmo arquivo só renova. */
const chave = (c: MuralClaim) => `${c.task}|${c.agent ?? ""}|${c.path.replace(/\\/g, "/").toLowerCase()}`;

export type MuralStore = {
  claims: MuralClaim[];
  /** Arquivos em que duas tarefas diferentes esbarraram desde que o Argus abriu. */
  conflitos: string[];
  note(claim: MuralClaim, conflicts: MuralClaim[]): void;
  forget(task: TaskId): void;
  load(): Promise<void>;
};

/** Mais recente primeiro, sem o que venceu, no teto. */
function arrumar(lista: MuralClaim[]): MuralClaim[] {
  const corte = Date.now() / 1000 - VALIDADE_MS / 1000;
  return lista
    .filter((c) => c.at >= corte)
    .sort((a, b) => b.at - a.at)
    .slice(0, MAX);
}

export const useMural = create<MuralStore>((set) => ({
  claims: [],
  conflitos: [],
  note: (claim, conflicts) =>
    set((s) => ({
      claims: arrumar([claim, ...s.claims.filter((c) => chave(c) !== chave(claim))]),
      conflitos: conflicts.length && !s.conflitos.includes(claim.path) ? [...s.conflitos, claim.path] : s.conflitos,
    })),
  forget: (task) => set((s) => ({ claims: s.claims.filter((c) => c.task !== task) })),
  load: async () => {
    try {
      set({ claims: arrumar(await api.muralList()) });
    } catch {
      // Retrato inicial é conforto, não requisito: os eventos preenchem sozinhos.
    }
  },
}));
