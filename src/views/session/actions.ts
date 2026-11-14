/** Cancelar e descartar uma tarefa, perguntando antes o que fazer com os servidores dela. */
import { api, type TaskId } from "@/api";
import { useApp } from "@/store";
import { askStopSession, stopSessionServers } from "@/views/servers";

/**
 * Cancela o turno em andamento. Se a sessão tem servidores, pergunta: só parar
 * o turno, parar e derrubar os servidores, ou voltar. false = o usuário voltou.
 */
export async function cancelTask(id: TaskId): Promise<boolean> {
  const choice = await askStopSession(id, "cancel");
  if (choice === "back") return false;
  await api.taskCancel(id);
  if (choice === "kill") await stopSessionServers(id);
  return true;
}

/** Descarta as alterações da tarefa (remove o worktree), com a mesma pergunta. false = o usuário voltou. */
export async function discardTask(id: TaskId): Promise<boolean> {
  const choice = await askStopSession(id, "discard");
  if (choice === "back") return false;
  if (choice === "kill") await stopSessionServers(id);
  await api.taskDiscard(id);
  await useApp.getState().refreshTasks();
  return true;
}

/** Aplica as alterações da tarefa no projeto. */
export async function applyTask(id: TaskId): Promise<void> {
  await api.taskApply(id);
  await useApp.getState().refreshTasks();
}
