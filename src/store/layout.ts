/**
 * Estado do layout. O app é dividido em ÁREAS (barra de ícones à esquerda):
 * a área "chat" mostra sessões + workbench + terminal inferior + inspetor;
 * as outras ocupam a tela inteira (src/areas/<area>).
 */
import { create } from "zustand";
import type { Owner } from "@/api";

/** Áreas do app. "agents" é a tela cheia de agentes, aberta pelo inspetor ou pela paleta. */
export type Area = "chat" | "home" | "git" | "servers" | "devices" | "metrics" | "terminal" | "settings" | "agents";

export const AREAS: { id: Area; label: string }[] = [
  { id: "chat", label: "Conversas" },
  { id: "home", label: "Início" },
  { id: "git", label: "Git" },
  { id: "servers", label: "Servidores" },
  { id: "devices", label: "Dispositivos" },
  { id: "metrics", label: "Métricas" },
  { id: "terminal", label: "Terminal" },
  { id: "settings", label: "Configurações" },
  { id: "agents", label: "Agentes" },
];

/** Abas do inspetor (painel direito da área chat). */
export type RightTab = "changes" | "files" | "agents";

export const RIGHT_TABS: { id: RightTab; label: string }[] = [
  { id: "changes", label: "Alterações" },
  { id: "files", label: "Arquivos" },
  { id: "agents", label: "Agentes" },
];

export interface LayoutState {
  /** Área visível. */
  area: Area;
  /** Lista de sessões (barra esquerda da área chat) visível. */
  leftOpen: boolean;
  /** Inspetor (painel direito da área chat) aberto. */
  inspectorOpen: boolean;
  /** Alias antigo de inspectorOpen (sempre igual). */
  rightOpen: boolean;
  rightTab: RightTab;
  /** Painel inferior (terminais) da área chat aberto. */
  bottomOpen: boolean;
  /** Filtro da área Servidores: null = todos os donos. */
  serversFilter: Owner | null;
  /** Muda a cada pedido de recarga do diff (ex.: comando /diff). */
  diffRev: number;
  /** Paleta de comandos (Ctrl+K) aberta. */
  paletteOpen: boolean;

  setArea(area: Area): void;
  toggleInspector(): void;
  setInspectorOpen(open: boolean): void;
  openPalette(): void;
  closePalette(): void;
  togglePalette(): void;

  toggleLeft(): void;
  /** Alias de toggleInspector. */
  toggleRight(): void;
  /** Abre o inspetor numa aba (vai para a área chat). */
  openRight(tab: RightTab): void;
  setRightTab(tab: RightTab): void;
  setBottomOpen(open: boolean): void;
  toggleBottom(): void;
  /** Vai para a área Servidores (filtrada por um dono, se informado). */
  openServersPanel(filter?: Owner | null): void;
  /** Alterna entre a área Servidores e a conversa. */
  toggleServersPanel(): void;
  /** Abre a aba Alterações do inspetor e recarrega o diff. */
  /** Abre a revisão de alterações (área Git, aba Alterações) e recarrega o diff. */
  showChanges(): void;
}


export const useLayout = create<LayoutState>()((set) => ({
  area: "chat",
  leftOpen: true,
  inspectorOpen: false,
  rightOpen: false,
  rightTab: "changes",
  bottomOpen: false,
  serversFilter: null,
  diffRev: 0,
  paletteOpen: false,

  setArea: (area) => set({ area, paletteOpen: false }),
  toggleInspector: () => set((s) => ({ inspectorOpen: !s.inspectorOpen, rightOpen: !s.inspectorOpen })),
  setInspectorOpen: (open) => set({ inspectorOpen: open, rightOpen: open }),
  openPalette: () => set({ paletteOpen: true }),
  closePalette: () => set({ paletteOpen: false }),
  togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),

  toggleLeft: () => set((s) => ({ leftOpen: !s.leftOpen })),
  toggleRight: () => set((s) => ({ inspectorOpen: !s.inspectorOpen, rightOpen: !s.inspectorOpen })),
  openRight: (rightTab) => set({ area: "chat", inspectorOpen: true, rightOpen: true, rightTab }),
  setRightTab: (rightTab) => set({ rightTab }),
  setBottomOpen: (bottomOpen) => set({ bottomOpen }),
  toggleBottom: () => set((s) => ({ bottomOpen: !s.bottomOpen })),
  openServersPanel: (filter = null) => set({ area: "servers", serversFilter: filter }),
  toggleServersPanel: () =>
    set((s) => (s.area === "servers" ? { area: "chat" } : { area: "servers", serversFilter: null })),
  showChanges: () => set((s) => ({ area: "git", paletteOpen: false, diffRev: s.diffRev + 1 })),
}));
