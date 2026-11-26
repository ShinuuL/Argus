/** Cancelar e descartar uma tarefa, perguntando antes o que fazer com os servidores dela. */
import { api, baseName, isTaskActive, samePath, variantOf, type Path, type Task, type TaskId } from "@/api";
import { forgetSession, useApp, useWorkbench } from "@/store";
import { chatKey, useChat } from "@/views/chat/store";
import { sessionTitle } from "./events";
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

/**
 * Exclui a sessão, com confirmação nativa. Avisa se o turno vai ser cancelado e,
 * no modo isolado com worktree, que as alterações não aplicadas serão perdidas.
 * No modo direto a pasta do projeto não é tocada. false = o usuário desistiu.
 */
export async function deleteTask(task: Task): Promise<boolean> {
  const { kind } = variantOf(task.state);
  const closed = kind === "Applied" || kind === "Discarded";
  const lines = [`Excluir a sessão #${task.id} (“${sessionTitle(task.prompt)}”) e todo o histórico dela?`];
  if (isTaskActive(task.state)) lines.push("O turno em andamento será cancelado antes.");
  if (task.mode === "Isolated" && !closed && task.workdir) {
    lines.push(
      `As alterações ainda não aplicadas serão perdidas: o worktree (${task.workdir}) e a branch argus/${task.id} serão removidos.`,
    );
  } else if (task.mode === "Direct") {
    lines.push("A pasta do projeto não é alterada.");
  }
  lines.push("Tokens e custo continuam nas Métricas. Isso não pode ser desfeito.");
  if (!(await api.systemConfirm("Excluir sessão", lines.join("\n\n")))) return false;
  // Nada da fila sai enquanto o turno é cancelado.
  useChat.getState().forget(chatKey(task.id));
  await api.taskDelete(task.id);
  forgetSession(task.id);
  return true;
}

/** Exclui as sessões encerradas (aplicadas ou descartadas) do projeto, com confirmação. */
export async function deleteClosedTasks(project: Path): Promise<boolean> {
  const n = useApp
    .getState()
    .tasks.filter((t) => samePath(t.project, project) && ["Applied", "Discarded"].includes(variantOf(t.state).kind)).length;
  if (n === 0) return false;
  const what = n === 1 ? "a 1 sessão encerrada" : `as ${n} sessões encerradas`;
  const ok = await api.systemConfirm(
    "Excluir encerradas",
    `Excluir ${what} (aplicadas ou descartadas) de ${baseName(project)} e o histórico delas?\n\nTokens e custo continuam nas Métricas. Isso não pode ser desfeito.`,
  );
  if (!ok) return false;
  await api.taskDeleteClosed(project);
  await useApp.getState().refreshTasks();
  return true;
}

/** Turno curto depois de liberar um comando negado: pede ao agente que rode de novo. */
export function retryDeniedText(command: string): string {
  return `O comando \`${command}\` foi liberado neste projeto; rode de novo e continue de onde parou.`;
}

/** "Liberar e repetir": libera o comando no projeto e manda o turno curto. */
export async function allowAndRetry(project: Path, tool: string, command: string, send: (text: string) => Promise<void>): Promise<void> {
  await api.permissionsAllowDenied(project, tool, command);
  await send(retryDeniedText(command));
}

/**
 * Ramificar vale em "Direto na pasta", com sessão e parado. No Claude a CLI
 * faz o fork da sessão; nos outros provedores o Argus cria a cópia e manda um
 * resumo do histórico no primeiro turno (a CLI não tem fork utilizável).
 */
export function canFork(t: Task): boolean {
  return t.mode === "Direct" && t.session_id != null && !isTaskActive(t.state) && t.state !== "Applied" && t.state !== "Discarded";
}

/** Ramifica a conversa e abre o ramo numa aba. */
export async function forkTask(t: Task): Promise<void> {
  const id = await api.taskFork(t.id);
  await useApp.getState().refreshTasks();
  useWorkbench.getState().openSession(id);
}
