/**
 * Liga o estado global aos eventos do backend. `startApp()` roda uma vez, no
 * `App`, e devolve a função que desfaz tudo.
 */
import {
  api,
  errorMessage,
  isTaskActive,
  onOrch,
  onProjectChanged,
  onWorkspaceChanged,
  onPtyMissingCommand,
  onServersChanged,
  variantOf,
  type OrchEvent,
  type Unlisten,
} from "@/api";
import { useApp } from "./app";
import { activeTab, useWorkbench } from "./workbench";
import { chatKey, useChat } from "@/views/chat/store";

/** A sessão está à vista (aba ativa de algum grupo)? Se não, a atividade nova a deixa "não lida". */
function noteActivity(task: number): void {
  const visible = useWorkbench.getState().groups.some((g) => {
    const kind = activeTab(g)?.kind;
    return kind?.type === "chat" && kind.task === task;
  });
  if (!visible || document.hidden) useApp.getState().markUnread(task);
}

/** Motivo usado pelo core quando o próprio usuário para o auto-work (não conta como erro). */
const STOP_BY_USER = "parado pelo usuário";
const USAGE_INTERVAL_MS = 2000;

function debounce(fn: () => void, ms: number): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  return () => {
    clearTimeout(timer);
    timer = setTimeout(fn, ms);
  };
}

function handleOrch(ev: OrchEvent, refreshTasks: () => void, refreshToday: () => void): void {
  if ("StateChanged" in ev) {
    const { task, state } = ev.StateChanged;
    const kind = variantOf(state).kind;
    useApp.setState((s) => ({
      // Atualiza já o que a lista mostra; workdir/sessão/turnos vêm no refresh.
      tasks: s.tasks.map((t) => (t.id === task ? { ...t, state } : t)),
      lastOkAt: kind === "Completed" ? Date.now() : s.lastOkAt,
      lastErrAt: kind === "Failed" ? Date.now() : s.lastErrAt,
    }));
    refreshTasks();
    if (!isTaskActive(state)) {
      refreshToday();
      noteActivity(task);
    }
  } else if ("Removed" in ev) {
    // Sessão excluída (aqui, por outra aba ou pelo auto-work): some das listas e das abas.
    forgetSession(ev.Removed.task);
  } else if ("AutoWork" in ev) {
    const aw = ev.AutoWork;
    const status = variantOf(aw.status);
    useApp.setState((s) => ({
      autowork: { ...s.autowork, [aw.task]: aw },
      lastOkAt: status.kind === "Completed" ? Date.now() : s.lastOkAt,
      lastErrAt: status.kind === "Stopped" && status.data !== STOP_BY_USER ? Date.now() : s.lastErrAt,
    }));
  } else if ("Agent" in ev) {
    const kind = variantOf(ev.Agent.event).kind;
    if (kind === "Usage") refreshToday();
    if ("Context" in ev.Agent.event) {
      const { task } = ev.Agent;
      const context = ev.Agent.event.Context;
      useApp.setState((s) => ({ tasks: s.tasks.map((t) => (t.id === task ? { ...t, context } : t)) }));
    }
    if (kind === "Text" || kind === "Result") noteActivity(ev.Agent.task);
  }
}

/** Tira a sessão excluída das listas, fecha as abas dela e esquece rascunho e fila. */
export function forgetSession(task: number): void {
  useApp.getState().forgetTask(task);
  useWorkbench.getState().closeSessionTabs(task);
  useChat.getState().forget(chatKey(task));
}

/** Intervalo mínimo entre duas atualizações da lista de projetos pelo foco da janela. */
const FOCUS_REFRESH_MS = 3000;

export function startApp(): () => void {
  const app = useApp.getState();
  const refreshTasks = debounce(() => void useApp.getState().refreshTasks(), 150);
  const refreshToday = debounce(() => void useApp.getState().refreshToday(), 500);

  const unlisten: Promise<Unlisten>[] = [
    onOrch((ev) => handleOrch(ev, refreshTasks, refreshToday)),
    onProjectChanged(({ path, status }) => useApp.getState().setStatus(path, status)),
    onWorkspaceChanged((info) => useApp.getState().setProjects(info)),
    onServersChanged((servers) => useApp.setState({ servers })),
    onPtyMissingCommand(({ command, plan }) =>
      useApp.getState().requestInstall({
        title: `Instalar ${command}?`,
        reason: `O terminal informou que \`${command}\` não foi encontrado. Deseja instalá-lo?`,
        plan,
      }),
    ),
  ];

  // Carga inicial (os ouvintes acima já estão sendo registrados).
  void (async () => {
    try {
      const [root, autowork, servers] = await Promise.all([
        api.workspaceCurrent(),
        api.autoworkAllStatus(),
        api.serversList(),
      ]);
      useApp.setState({ servers, autowork: Object.fromEntries(autowork.map((a) => [a.task, a])) });
      if (root) {
        app.applyRoot(root);
        // A raiz veio de ARGUS_ROOT: os eventos de status podem ter saído antes da interface ouvir.
        const statuses = await api.monitorStatuses(root.projects.map((p) => p.path));
        statuses.forEach(({ path, status }) => useApp.getState().setStatus(path, status));
      }
      await Promise.all([app.refreshTasks(), app.refreshToday()]);
    } catch (e) {
      useApp.setState({ error: errorMessage(e) });
    } finally {
      useApp.setState({ ready: true });
    }
  })();

  const pollUsage = () =>
    api.systemUsage().then(
      (usage) => useApp.setState({ usage }),
      () => undefined,
    );
  void pollUsage();
  const usageTimer = setInterval(pollUsage, USAGE_INTERVAL_MS);

  // A janela voltou ao foco: subpastas criadas fora do app, branch trocada no terminal…
  let lastFocus = 0;
  const onFocus = () => {
    const now = Date.now();
    if (now - lastFocus < FOCUS_REFRESH_MS) return;
    lastFocus = now;
    void useApp.getState().refreshRoot();
  };
  window.addEventListener("focus", onFocus);

  return () => {
    window.removeEventListener("focus", onFocus);
    clearInterval(usageTimer);
    unlisten.forEach((p) => void p.then((off) => off()));
  };
}
