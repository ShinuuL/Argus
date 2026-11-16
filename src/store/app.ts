/**
 * Estado global da IDE: pasta raiz, projetos, status git, tarefas, auto-work,
 * servidores e uso do sistema. É alimentado por `bootstrap.ts` (eventos do backend).
 *
 * Zustand 5: selecione um campo por vez (`useApp((s) => s.tasks)`); um seletor
 * que monta objeto/array novo a cada chamada causa laço de renderização
 * (use `useShallow` nesses casos).
 */
import { create } from "zustand";
import {
  api,
  errorMessage,
  samePath,
  type AutoWorkEvent,
  type InstallPlan,
  type Path,
  type Project,
  type ProjectStatus,
  type RootInfo,
  type ServerInfo,
  type SystemUsage,
  type Task,
  type TaskId,
  type TodayUsage,
} from "@/api";

/** Instalação aguardando confirmação explícita do usuário (modal global). */
export interface PendingInstall {
  title: string;
  reason: string;
  plan: InstallPlan;
}

export interface AppState {
  /** true depois da carga inicial (`startApp`). */
  ready: boolean;
  /** Erro global exibido na faixa do topo. */
  error: string | null;
  root: Path | null;
  projects: Project[];
  /** Status git por caminho de projeto; `undefined` = ainda calculando, `null` = sem git. */
  statuses: Record<Path, ProjectStatus | null>;
  /** Projeto ativo (o da aba em foco ou o clicado na barra esquerda): base do painel direito e dos terminais novos. */
  selected: Path | null;
  tasks: Task[];
  /** Último progresso de auto-work por tarefa. */
  autowork: Record<TaskId, AutoWorkEvent>;
  servers: ServerInfo[];
  usage: SystemUsage;
  /** Tokens e custo equivalente em API de hoje. */
  today: TodayUsage;
  /** Momento (ms) da última tarefa concluída / com erro, para o humor do pet. */
  lastOkAt: number | null;
  lastErrAt: number | null;
  pendingInstall: PendingInstall | null;
  /** Sessões com atividade nova que o usuário ainda não abriu. */
  unread: Record<TaskId, true>;
  /** Sessões cujo último turno teve um comando negado (texto do comando); some no turno seguinte. */
  attention: Record<TaskId, string>;
  /** Subagente aberto (somente leitura) em cada sessão; ausente = chat principal. */
  subagentOf: Record<TaskId, string>;

  markUnread(task: TaskId): void;
  markSeen(task: TaskId): void;
  /** Marca (texto) ou limpa (null) o "precisa de atenção" de uma sessão. */
  setAttention(task: TaskId, what: string | null): void;
  selectSubagent(task: TaskId, subagent: string | null): void;
  setError(error: string | null): void;
  /** Abre o seletor de pasta e, se o usuário escolher, abre como raiz. */
  pickRoot(): Promise<void>;
  openRoot(path: Path): Promise<void>;
  applyRoot(info: RootInfo): void;
  /** Troca só a lista de projetos (varredura nova da mesma raiz), sem mexer no erro global. */
  setProjects(info: RootInfo): void;
  /** Varre a raiz de novo e recalcula branch/alterados (botão "Atualizar" e foco da janela). */
  refreshRoot(): Promise<void>;
  /** Tira a tarefa excluída das listas (não lida, auto-work, subagente aberto). */
  forgetTask(task: TaskId): void;
  select(path: Path | null): void;
  setStatus(path: Path, status: ProjectStatus | null): void;
  /** Recarrega a lista de tarefas do backend. */
  refreshTasks(): Promise<void>;
  refreshToday(): Promise<void>;
  /** Pede confirmação para um plano de instalação (abre o modal; não executa nada). */
  requestInstall(install: PendingInstall): void;
  clearInstall(): void;
}

export const useApp = create<AppState>()((set, get) => ({
  ready: false,
  error: null,
  root: null,
  projects: [],
  statuses: {},
  selected: null,
  tasks: [],
  autowork: {},
  servers: [],
  usage: { cpu: 0, mem_used_mb: 0, mem_total_mb: 0 },
  today: { tokens: 0, cost: 0 },
  lastOkAt: null,
  lastErrAt: null,
  pendingInstall: null,
  unread: {},
  attention: {},
  subagentOf: {},

  markUnread: (task) => set((s) => (s.unread[task] ? s : { unread: { ...s.unread, [task]: true } })),
  markSeen: (task) =>
    set((s) => {
      if (!s.unread[task]) return s;
      const unread = { ...s.unread };
      delete unread[task];
      return { unread };
    }),
  setAttention: (task, what) =>
    set((s) => {
      if ((s.attention[task] ?? null) === what) return s;
      const attention = { ...s.attention };
      if (what == null) delete attention[task];
      else attention[task] = what;
      return { attention };
    }),
  selectSubagent: (task, subagent) =>
    set((s) => {
      const subagentOf = { ...s.subagentOf };
      if (subagent) subagentOf[task] = subagent;
      else delete subagentOf[task];
      return { subagentOf };
    }),
  setError: (error) => set({ error }),

  pickRoot: async () => {
    try {
      const dir = await api.workspacePickRoot();
      if (dir) await get().openRoot(dir);
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  openRoot: async (path) => {
    try {
      get().applyRoot(await api.workspaceOpenRoot(path));
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  applyRoot: (info) =>
    set((s) => ({
      root: info.root,
      projects: info.projects,
      // Mantém o status já conhecido dos projetos que continuam na lista.
      statuses: Object.fromEntries(
        info.projects.filter((p) => p.path in s.statuses).map((p) => [p.path, s.statuses[p.path]]),
      ),
      selected: info.projects.some((p) => samePath(p.path, s.selected)) ? s.selected : null,
      error: info.watch_error,
    })),

  setProjects: (info) =>
    set((s) => {
      if (!samePath(info.root, s.root)) return s;
      return {
        projects: info.projects,
        statuses: Object.fromEntries(
          info.projects.filter((p) => p.path in s.statuses).map((p) => [p.path, s.statuses[p.path]]),
        ),
        selected: info.projects.some((p) => samePath(p.path, s.selected)) ? s.selected : null,
      };
    }),

  refreshRoot: async () => {
    if (!get().root) return;
    try {
      const info = await api.workspaceRefresh();
      if (info) get().setProjects(info);
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  forgetTask: (task) =>
    set((s) => {
      const drop = <T,>(r: Record<TaskId, T>) => {
        if (!(task in r)) return r;
        const out = { ...r };
        delete out[task];
        return out;
      };
      return {
        tasks: s.tasks.filter((t) => t.id !== task),
        unread: drop(s.unread),
        attention: drop(s.attention),
        autowork: drop(s.autowork),
        subagentOf: drop(s.subagentOf),
      };
    }),

  select: (selected) => set({ selected }),

  setStatus: (path, status) => set((s) => ({ statuses: { ...s.statuses, [path]: status } })),

  refreshTasks: async () => {
    try {
      const tasks = await api.tasksList();
      tasks.sort((a, b) => b.id - a.id);
      set({ tasks });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  refreshToday: async () => {
    try {
      set({ today: await api.metricsToday() });
    } catch {
      // Sem banco ou erro de leitura: mantém o último valor.
    }
  },

  requestInstall: (pendingInstall) => {
    if (!get().pendingInstall) set({ pendingInstall });
  },
  clearInstall: () => set({ pendingInstall: null }),
}));

/** Pasta de trabalho padrão para terminais novos: projeto selecionado ou a raiz. */
export function defaultCwd(): Path | null {
  const s = useApp.getState();
  return s.selected ?? s.root;
}
