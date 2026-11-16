/**
 * Área central estilo editor: grupos (split de até 3) com abas de vários tipos.
 * Porte da lógica de `crates/app/src/ui/workbench.rs`.
 *
 * As funções `*In` são puras (recebem e devolvem os grupos) e o store as aplica.
 */
import { create } from "zustand";
import { api, baseName, samePath, type CmdSpec, type CommandSpec, type Path, type TaskId } from "@/api";

export const MAX_GROUPS = 3;

export type SplitDir = "horizontal" | "vertical";

export type TabKind =
  | { type: "file"; path: Path }
  /** Conversa com um agente. `task: null` = conversa nova, ainda sem tarefa (em `project`). */
  | { type: "chat"; task: TaskId | null; project: Path | null }
  | { type: "terminal"; title: string; cwd: Path | null; command: CmdSpec | CommandSpec | null }
  | { type: "dashboard" }
  /** Mapa de agentes de uma sessão (subagentes e tarefas em segundo plano). */
  | { type: "agentmap"; task: TaskId }
  /** Diff de uma sessão num painel do workbench (com comentário por linha). */
  | { type: "diff"; task: TaskId }
  /** Telas que abrem como aba pelo rodapé da barra esquerda. */
  | { type: "page"; page: PageId };

export type PageId = "deps" | "builds" | "connections" | "devices" | "metrics" | "remote" | "settings";

/**
 * "providers" era uma aba própria; hoje é a seção Provedores de Configurações.
 * Aceito em `openPage` e nas abas restauradas por compatibilidade.
 */
export type LegacyPageId = "providers";

/** Seções da tela de Configurações. */
export type SettingsSection = "theme" | "agents" | "providers" | "project";

export const SETTINGS_SECTIONS: { id: SettingsSection; label: string }[] = [
  { id: "theme", label: "Tema" },
  { id: "agents", label: "Agentes" },
  { id: "providers", label: "Provedores" },
  { id: "project", label: "Por projeto" },
];

/** Troca uma aba de página antiga ("providers") pela equivalente atual (Configurações). */
export function normalizeKind(kind: TabKind): TabKind {
  if (kind.type === "page" && (kind.page as string) === "providers") return { type: "page", page: "settings" };
  return kind;
}

export const PAGES: { id: PageId; label: string; short: string; title: string }[] = [
  { id: "deps", label: "Dependências", short: "Deps.", title: "Dependências dos projetos" },
  { id: "builds", label: "Builds", short: "Builds", title: "Builds, instaladores e servidores de dev dos projetos" },
  { id: "connections", label: "Conexões", short: "Conexões", title: "Conexões: GitHub, Google Cloud, Firebase e Vercel pelas CLIs" },
  { id: "devices", label: "Dispositivos", short: "Disp.", title: "Celulares Android: espelhamento e controle (Screen-Mirror)" },
  { id: "metrics", label: "Métricas", short: "Métricas", title: "Métricas de tokens e custo equivalente em API" },
  { id: "remote", label: "Remoto", short: "Remoto", title: "Acesso remoto pelo celular" },
  { id: "settings", label: "Configurações", short: "Config.", title: "Configurações: tema, agentes, provedores e projetos" },
];

export interface WbTab {
  id: number;
  kind: TabKind;
  /**
   * Aba de pré-visualização (título em itálico): é substituída pelo próximo
   * arquivo aberto até ser editada, receber duplo clique ou ser fixada.
   */
  preview: boolean;
}

export interface Group {
  id: number;
  tabs: WbTab[];
  active: number | null;
}

/** Mesma "identidade", para reaproveitar uma aba já aberta. Terminais nunca coincidem. */
export function sameKind(a: TabKind, b: TabKind): boolean {
  if (a.type === "file" && b.type === "file") return samePath(a.path, b.path);
  if (a.type === "chat" && b.type === "chat") return a.task === b.task;
  if (a.type === "agentmap" && b.type === "agentmap") return a.task === b.task;
  if (a.type === "diff" && b.type === "diff") return a.task === b.task;
  if (a.type === "page" && b.type === "page") return a.page === b.page;
  if (a.type === "terminal" || b.type === "terminal") return false;
  return a.type === b.type;
}

export function tabTitle(kind: TabKind): string {
  switch (kind.type) {
    case "file":
      return baseName(kind.path);
    case "chat":
      return kind.task == null ? "Nova conversa" : `Sessão #${kind.task}`;
    case "terminal":
      return kind.title;
    case "dashboard":
      return "Painel";
    case "agentmap":
      return `Mapa de agentes #${kind.task}`;
    case "diff":
      return `Alterações #${kind.task}`;
    case "page":
      return PAGES.find((p) => p.id === (normalizeKind(kind) as typeof kind).page)?.label ?? kind.page;
  }
}

export function tabTooltip(kind: TabKind): string {
  if (kind.type === "file") return kind.path;
  if (kind.type === "terminal") return `Terminal em ${kind.cwd ?? "pasta atual"}`;
  return tabTitle(kind);
}

export function activeTab(g: Group | undefined): WbTab | undefined {
  return g?.tabs.find((t) => t.id === g.active);
}

/** Ativa a próxima/anterior aba do grupo (circular). */
export function cycleIn(g: Group, back: boolean): Group {
  const n = g.tabs.length;
  if (n === 0) return g;
  const cur = Math.max(0, g.tabs.findIndex((t) => t.id === g.active));
  const next = back ? (cur + n - 1) % n : (cur + 1) % n;
  return { ...g, active: g.tabs[next].id };
}

/** Remove a aba do grupo e escolhe a vizinha como ativa. */
function withoutTab(g: Group, tab: number): Group {
  const pos = g.tabs.findIndex((t) => t.id === tab);
  if (pos < 0) return g;
  const tabs = g.tabs.filter((t) => t.id !== tab);
  const active = g.active === tab ? (tabs[Math.min(pos, tabs.length - 1)]?.id ?? null) : g.active;
  return { ...g, tabs, active };
}

/** Fecha a aba; grupo que fica vazio some (se não for o único). */
export function closeTabIn(groups: Group[], gid: number, tab: number): Group[] {
  const gi = groups.findIndex((g) => g.id === gid);
  if (gi < 0) return groups;
  const g = withoutTab(groups[gi], tab);
  if (g === groups[gi]) return groups;
  if (g.tabs.length === 0 && groups.length > 1) return groups.filter((_, i) => i !== gi);
  return groups.map((x, i) => (i === gi ? g : x));
}

/**
 * Move a aba para o grupo `toGid` (antes de `before`, ou no fim). Se o destino
 * já tem a mesma aba (ex.: mesmo arquivo), só foca lá e fecha a origem. Grupo
 * de origem vazio é removido (se não for o único). Devolve null se nada mudou.
 */
export function moveTabIn(groups: Group[], tab: number, toGid: number, before: number | null): Group[] | null {
  if (before === tab) return null;
  const fromGi = groups.findIndex((g) => g.tabs.some((t) => t.id === tab));
  const toGi = groups.findIndex((g) => g.id === toGid);
  if (fromGi < 0 || toGi < 0) return null;
  const moved = groups[fromGi].tabs.find((t) => t.id === tab)!;
  let next = groups.map((g, i) => (i === fromGi ? withoutTab(g, tab) : g));
  const dest = next[toGi];
  const existing = dest.tabs.find((t) => sameKind(t.kind, moved.kind));
  let newDest: Group;
  if (existing) {
    newDest = { ...dest, active: existing.id };
  } else {
    const idx = before == null ? -1 : dest.tabs.findIndex((t) => t.id === before);
    const at = idx < 0 ? dest.tabs.length : idx;
    const tabs = [...dest.tabs.slice(0, at), moved, ...dest.tabs.slice(at)];
    newDest = { ...dest, tabs, active: moved.id };
  }
  next = next.map((g, i) => (i === toGi ? newDest : g));
  if (next[fromGi].tabs.length === 0 && next.length > 1) next = next.filter((_, i) => i !== fromGi);
  return next;
}

/** Abre (ou foca) uma aba no grupo de índice `gi`. `newId` é usado se a aba for criada. */
export function openIn(groups: Group[], gi: number, kind: TabKind, preview: boolean, newId: number): Group[] {
  const g = groups[gi];
  if (!g) return groups;
  let ng: Group;
  const same = g.tabs.find((t) => sameKind(t.kind, kind));
  const previewTab = preview ? g.tabs.find((t) => t.preview) : undefined;
  if (same) {
    ng = {
      ...g,
      active: same.id,
      tabs: preview ? g.tabs : g.tabs.map((t) => (t.id === same.id ? { ...t, preview: false } : t)),
    };
  } else if (previewTab) {
    ng = { ...g, active: previewTab.id, tabs: g.tabs.map((t) => (t.id === previewTab.id ? { ...t, kind } : t)) };
  } else {
    const cur = g.tabs.findIndex((t) => t.id === g.active);
    const pos = cur < 0 ? g.tabs.length : cur + 1;
    const tab: WbTab = { id: newId, kind, preview };
    ng = { ...g, active: newId, tabs: [...g.tabs.slice(0, pos), tab, ...g.tabs.slice(pos)] };
  }
  return groups.map((x, i) => (i === gi ? ng : x));
}

export interface WorkbenchState {
  groups: Group[];
  activeGroup: number;
  split: SplitDir;
  /** Peso (flex-grow) de cada grupo, por id; quem não está aqui pesa 1. */
  sizes: Record<number, number>;
  /** Arquivos com alterações não salvas. */
  dirty: Path[];
  /** Aba sendo arrastada (drag-and-drop entre grupos). */
  drag: number | null;
  nextId: number;

  /** Abre (ou foca) uma aba no grupo ativo. */
  open(kind: TabKind, preview?: boolean): void;
  openFile(path: Path, preview?: boolean): void;
  /** Abre (ou foca) a aba de chat de uma tarefa. */
  openSession(task: TaskId): void;
  /** Abre (ou foca) a aba "Mapa de agentes" de uma tarefa. */
  openAgentMap(task: TaskId): void;
  /** Abre (ou foca) o diff da sessão como aba. */
  openDiff(task: TaskId): void;
  /** Abre uma conversa nova (reaproveita a aba de conversa vazia do grupo, se houver). */
  openNewChat(project?: Path | null): void;
  /** A conversa nova da aba `tab` virou a tarefa `task` (a aba continua a mesma). */
  bindChat(tab: number, task: TaskId): void;
  /** Troca o projeto de uma conversa ainda sem tarefa. */
  setChatProject(tab: number, project: Path | null): void;
  /** Abre uma tela; "providers" abre Configurações › Provedores. */
  openPage(page: PageId | LegacyPageId): void;
  /** Seção visível em Configurações. */
  settingsSection: SettingsSection;
  setSettingsSection(section: SettingsSection): void;
  /** Abre (ou foca) Configurações já na seção pedida. */
  openSettings(section?: SettingsSection): void;
  openTerminal(opts: { title?: string; cwd?: Path | null; command?: CmdSpec | CommandSpec | null }): void;
  openDashboard(): void;
  openMetrics(): void;
  activate(gid: number, tab: number): void;
  focusGroup(gid: number): void;
  /** Fixa a aba (deixa de ser pré-visualização). */
  pin(tab: number): void;
  /** Fixa as abas de pré-visualização desse arquivo (chamado quando ele é editado). */
  pinFile(path: Path): void;
  setDirty(path: Path, dirty: boolean): void;
  isDirty(path: Path): boolean;
  /** Fecha sem perguntar. Prefira `requestCloseTab` na interface. */
  closeTab(gid: number, tab: number): void;
  /** Fecha todas as abas de conversa da sessão (sessão excluída). */
  closeSessionTabs(task: TaskId): void;
  /** Fecha a aba; se for o último editor de um arquivo modificado, pede confirmação. false = cancelado. */
  requestCloseTab(gid: number, tab: number): Promise<boolean>;
  closeOthers(gid: number, keep: number): Promise<void>;
  closeActive(): Promise<void>;
  /** Próxima/anterior aba do grupo ativo (Ctrl+Tab / Ctrl+Shift+Tab). */
  cycle(back: boolean): void;
  /** Divide o editor (Ctrl+\): novo grupo com uma cópia da aba ativa (terminais não são duplicados). */
  splitActive(): void;
  /** Move uma aba para outro grupo (antes de `before`, ou no fim). */
  moveTab(tab: number, toGid: number, before?: number | null): void;
  toggleSplitDir(): void;
  /** Novos pesos dos grupos (arrastar o divisor entre eles). */
  setSizes(sizes: Record<number, number>): void;
  setDrag(tab: number | null): void;
}

export const useWorkbench = create<WorkbenchState>()((set, get) => {
  const activeIndex = () => {
    const s = get();
    return Math.max(0, s.groups.findIndex((g) => g.id === s.activeGroup));
  };

  return {
    groups: [{ id: 0, tabs: [], active: null }],
    activeGroup: 0,
    split: "horizontal",
    sizes: {},
    dirty: [],
    drag: null,
    nextId: 1,
    settingsSection: "theme",

    open: (kind, preview = false) => {
      if (kind.type === "page" && (kind.page as string) === "providers") set({ settingsSection: "providers" });
      kind = normalizeKind(kind);
      set((s) => ({ groups: openIn(s.groups, activeIndex(), kind, preview, s.nextId), nextId: s.nextId + 1 }));
    },
    openFile: (path, preview = false) => get().open({ type: "file", path }, preview),
    openSession: (task) => {
      const s = get();
      // Já aberta em algum grupo: foca lá em vez de duplicar.
      for (const g of s.groups) {
        const tab = g.tabs.find((t) => t.kind.type === "chat" && t.kind.task === task);
        if (tab) return s.activate(g.id, tab.id);
      }
      s.open({ type: "chat", task, project: null });
    },
    openAgentMap: (task) => get().open({ type: "agentmap", task }),
    openDiff: (task) => get().open({ type: "diff", task }),
    openNewChat: (project = null) => {
      const s = get();
      const g = s.groups[activeIndex()];
      const empty = g?.tabs.find((t) => t.kind.type === "chat" && t.kind.task == null);
      if (g && empty) {
        if (project) s.setChatProject(empty.id, project);
        return s.activate(g.id, empty.id);
      }
      s.open({ type: "chat", task: null, project });
    },
    bindChat: (tab, task) =>
      set((s) => ({
        groups: s.groups.map((g) => ({
          ...g,
          tabs: g.tabs.map((t) => (t.id === tab && t.kind.type === "chat" ? { ...t, kind: { ...t.kind, task } } : t)),
        })),
      })),
    setChatProject: (tab, project) =>
      set((s) => ({
        groups: s.groups.map((g) => ({
          ...g,
          tabs: g.tabs.map((t) =>
            t.id === tab && t.kind.type === "chat" && t.kind.task == null ? { ...t, kind: { ...t.kind, project } } : t,
          ),
        })),
      })),
    openPage: (page) => (page === "providers" ? get().openSettings("providers") : get().open({ type: "page", page })),
    setSettingsSection: (settingsSection) => set({ settingsSection }),
    openSettings: (section) => {
      if (section) set({ settingsSection: section });
      get().open({ type: "page", page: "settings" });
    },
    openTerminal: ({ title, cwd = null, command = null }) =>
      get().open({ type: "terminal", title: title ?? (cwd ? baseName(cwd) : "terminal"), cwd, command }),
    openDashboard: () => get().open({ type: "dashboard" }),
    openMetrics: () => get().open({ type: "page", page: "metrics" }),

    activate: (gid, tab) =>
      set((s) => ({
        activeGroup: gid,
        groups: s.groups.map((g) => (g.id === gid ? { ...g, active: tab } : g)),
      })),
    focusGroup: (gid) => {
      if (get().activeGroup !== gid) set({ activeGroup: gid });
    },

    pin: (tab) =>
      set((s) => ({
        groups: s.groups.map((g) => ({ ...g, tabs: g.tabs.map((t) => (t.id === tab ? { ...t, preview: false } : t)) })),
      })),
    pinFile: (path) => {
      const isPreview = (t: WbTab) => t.preview && t.kind.type === "file" && samePath(t.kind.path, path);
      if (!get().groups.some((g) => g.tabs.some(isPreview))) return;
      set((s) => ({
        groups: s.groups.map((g) => ({ ...g, tabs: g.tabs.map((t) => (isPreview(t) ? { ...t, preview: false } : t)) })),
      }));
    },

    setDirty: (path, dirty) =>
      set((s) => {
        const has = s.dirty.some((p) => samePath(p, path));
        if (dirty === has) return s;
        return { dirty: dirty ? [...s.dirty, path] : s.dirty.filter((p) => !samePath(p, path)) };
      }),
    isDirty: (path) => get().dirty.some((p) => samePath(p, path)),

    closeTab: (gid, tab) =>
      set((s) => {
        const groups = closeTabIn(s.groups, gid, tab);
        const activeGroup = groups.some((g) => g.id === s.activeGroup)
          ? s.activeGroup
          : groups[Math.min(Math.max(0, s.groups.findIndex((g) => g.id === gid)), groups.length - 1)].id;
        return { groups, activeGroup };
      }),

    closeSessionTabs: (task) => {
      const found = get().groups.flatMap((g) =>
        g.tabs
          .filter((t) => (t.kind.type === "chat" || t.kind.type === "agentmap" || t.kind.type === "diff") && t.kind.task === task).map((t) => [g.id, t.id] as const),
      );
      found.forEach(([gid, tab]) => {
        if (get().groups.some((g) => g.id === gid)) get().closeTab(gid, tab);
      });
    },

    requestCloseTab: async (gid, tab) => {
      const s = get();
      const kind = s.groups.find((g) => g.id === gid)?.tabs.find((t) => t.id === tab)?.kind;
      if (kind?.type === "file" && s.isDirty(kind.path)) {
        const path = kind.path;
        const elsewhere = s.groups.some((g) =>
          g.tabs.some((t) => t.id !== tab && t.kind.type === "file" && samePath(t.kind.path, path)),
        );
        if (!elsewhere) {
          const discard = await api.systemConfirm(
            "Alterações não salvas",
            `${path} tem alterações não salvas. Fechar e descartar?`,
          );
          if (!discard) return false;
          get().setDirty(path, false);
        }
      }
      get().closeTab(gid, tab);
      return true;
    },

    closeOthers: async (gid, keep) => {
      const ids = (get().groups.find((g) => g.id === gid)?.tabs ?? []).filter((t) => t.id !== keep).map((t) => t.id);
      for (const id of ids) await get().requestCloseTab(gid, id);
    },

    closeActive: async () => {
      const s = get();
      const g = s.groups.find((x) => x.id === s.activeGroup);
      if (g?.active != null) await s.requestCloseTab(g.id, g.active);
    },

    cycle: (back) =>
      set((s) => {
        const gi = activeIndex();
        return { groups: s.groups.map((g, i) => (i === gi ? cycleIn(g, back) : g)) };
      }),

    splitActive: () =>
      set((s) => {
        if (s.groups.length >= MAX_GROUPS) return s;
        const gi = activeIndex();
        const gid = s.nextId;
        const tid = s.nextId + 1;
        const copy = activeTab(s.groups[gi]);
        const tabs: WbTab[] =
          copy && copy.kind.type !== "terminal" ? [{ id: tid, kind: copy.kind, preview: false }] : [];
        const group: Group = { id: gid, tabs, active: tabs[0]?.id ?? null };
        return {
          groups: [...s.groups.slice(0, gi + 1), group, ...s.groups.slice(gi + 1)],
          activeGroup: gid,
          nextId: s.nextId + 2,
        };
      }),

    moveTab: (tab, toGid, before = null) =>
      set((s) => {
        const groups = moveTabIn(s.groups, tab, toGid, before);
        return groups ? { groups, activeGroup: toGid } : s;
      }),

    toggleSplitDir: () => set((s) => ({ split: s.split === "horizontal" ? "vertical" : "horizontal" })),
    setSizes: (sizes) => set((s) => ({ sizes: { ...s.sizes, ...sizes } })),
    setDrag: (drag) => set({ drag }),
  };
});
