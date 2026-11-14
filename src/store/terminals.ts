/**
 * Abas do painel inferior de terminais. O store só guarda a lista: quem cria o
 * PTY (`ptySpawn`) e o encerra é o componente de terminal de cada aba.
 */
import { create } from "zustand";
import { baseName, type CmdSpec, type CommandSpec, type Path } from "@/api";
import { useLayout } from "./layout";

export interface TermTab {
  id: number;
  title: string;
  cwd: Path | null;
  /** Sem comando, abre o shell padrão. */
  command: CmdSpec | CommandSpec | null;
}

export interface TerminalsState {
  tabs: TermTab[];
  active: number | null;
  nextId: number;

  /** Abre uma aba no painel inferior (e mostra o painel). Devolve o id da aba. */
  open(opts: { title?: string; cwd?: Path | null; command?: CmdSpec | CommandSpec | null }): number;
  close(id: number): void;
  activate(id: number): void;
}

export const useTerminals = create<TerminalsState>()((set, get) => ({
  tabs: [],
  active: null,
  nextId: 1,

  open: ({ title, cwd = null, command = null }) => {
    const id = get().nextId;
    const tab: TermTab = { id, title: title ?? (cwd ? baseName(cwd) : "terminal"), cwd, command };
    set((s) => ({ tabs: [...s.tabs, tab], active: id, nextId: id + 1 }));
    useLayout.getState().setBottomOpen(true);
    return id;
  },

  close: (id) =>
    set((s) => {
      const tabs = s.tabs.filter((t) => t.id !== id);
      const active = s.active === id ? (tabs.length ? tabs[tabs.length - 1].id : null) : s.active;
      return { tabs, active };
    }),

  activate: (id) => {
    set({ active: id });
    useLayout.getState().setBottomOpen(true);
  },
}));
