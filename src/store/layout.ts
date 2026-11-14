/** Estado do layout: barra esquerda (projetos e sessões), painel direito, painel inferior. */
import { create } from "zustand";
import type { Owner } from "@/api";

/** Abas internas do painel direito. */
export type RightTab = "files" | "changes" | "subagents" | "servers";

export const RIGHT_TABS: { id: RightTab; label: string }[] = [
  { id: "files", label: "Arquivos" },
  { id: "changes", label: "Alterações" },
  { id: "subagents", label: "Subagentes" },
  { id: "servers", label: "Servidores" },
];

export interface LayoutState {
  /** Barra esquerda (projetos e sessões) visível. */
  leftOpen: boolean;
  /** Painel direito visível. */
  rightOpen: boolean;
  rightTab: RightTab;
  /** Painel inferior (terminais) aberto. */
  bottomOpen: boolean;
  /** Filtro da aba Servidores: null = todos os donos. */
  serversFilter: Owner | null;
  /** Muda a cada pedido de recarga do diff (ex.: comando /diff). */
  diffRev: number;

  toggleLeft(): void;
  toggleRight(): void;
  /** Abre o painel direito numa aba. */
  openRight(tab: RightTab): void;
  setRightTab(tab: RightTab): void;
  setBottomOpen(open: boolean): void;
  toggleBottom(): void;
  /** Abre a aba Servidores (filtrada por um dono, se informado). */
  openServersPanel(filter?: Owner | null): void;
  toggleServersPanel(): void;
  /** Abre a aba Alterações e recarrega o diff. */
  showChanges(): void;
}

export const useLayout = create<LayoutState>()((set) => ({
  leftOpen: true,
  rightOpen: typeof window !== "undefined" && window.innerWidth >= 1280,
  rightTab: "files",
  bottomOpen: false,
  serversFilter: null,
  diffRev: 0,

  toggleLeft: () => set((s) => ({ leftOpen: !s.leftOpen })),
  toggleRight: () => set((s) => ({ rightOpen: !s.rightOpen })),
  openRight: (rightTab) => set({ rightOpen: true, rightTab }),
  setRightTab: (rightTab) => set({ rightTab }),
  setBottomOpen: (bottomOpen) => set({ bottomOpen }),
  toggleBottom: () => set((s) => ({ bottomOpen: !s.bottomOpen })),
  openServersPanel: (filter = null) => set({ rightOpen: true, rightTab: "servers", serversFilter: filter }),
  toggleServersPanel: () =>
    set((s) =>
      s.rightOpen && s.rightTab === "servers"
        ? { rightOpen: false }
        : { rightOpen: true, rightTab: "servers", serversFilter: null },
    ),
  showChanges: () => set((s) => ({ rightOpen: true, rightTab: "changes", diffRev: s.diffRev + 1 })),
}));
