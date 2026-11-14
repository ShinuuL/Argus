/**
 * Estado do chat por conversa: rascunho, anexos da caixa e fila de mensagens.
 *
 * Fica fora dos componentes para sobreviver à troca de aba. A fila vive só na
 * memória desta janela: não sobrevive a reinício nem aparece no acesso remoto.
 *
 * A fila sai sozinha: este módulo ouve o estado das tarefas e, quando um turno
 * termina bem, manda a próxima mensagem com `api.taskReply` 400 ms depois. Se o
 * turno falhou ou foi cancelado, a fila fica pausada até o usuário retomar.
 */
import { create } from "zustand";
import { api, errorMessage, isTaskActive, isTauri, onTaskState, variantOf, type TaskId, type TaskState } from "@/api";
import { buildPrompt, type ChatAttachment } from "./attachments";

/** Chave de conversa: o id da tarefa, ou "new" para a conversa ainda não criada. */
export type ChatKey = string;
export const chatKey = (taskId: TaskId | null | undefined): ChatKey => (taskId == null ? "new" : String(taskId));

export interface QueueItem {
  id: string;
  text: string;
  attachments: ChatAttachment[];
}

interface ChatState {
  drafts: Record<ChatKey, string>;
  attachments: Record<ChatKey, ChatAttachment[]>;
  queue: Record<ChatKey, QueueItem[]>;
  /** Fila parada (turno falhou ou foi cancelado); o valor é o motivo mostrado. */
  paused: Record<ChatKey, string>;

  setDraft(key: ChatKey, text: string): void;
  addAttachment(key: ChatKey, att: ChatAttachment): void;
  updateAttachment(key: ChatKey, att: ChatAttachment): void;
  removeAttachment(key: ChatKey, id: string): void;
  setAttachments(key: ChatKey, list: ChatAttachment[]): void;
  enqueue(key: ChatKey, item: Omit<QueueItem, "id">, first?: boolean): void;
  /** Tira um item da fila e o devolve (para editar ou enviar). */
  takeQueued(key: ChatKey, id?: string): QueueItem | null;
  setPaused(key: ChatKey, reason: string | null): void;
}

const EMPTY_ATTACHMENTS: ChatAttachment[] = [];
const EMPTY_QUEUE: QueueItem[] = [];
/** Listas vazias estáveis para os seletores (zustand 5 compara por referência). */
export const selectAttachments = (key: ChatKey) => (s: ChatState) => s.attachments[key] ?? EMPTY_ATTACHMENTS;
export const selectQueue = (key: ChatKey) => (s: ChatState) => s.queue[key] ?? EMPTY_QUEUE;

let seq = 0;

export const useChat = create<ChatState>()((set, get) => ({
  drafts: {},
  attachments: {},
  queue: {},
  paused: {},

  setDraft: (key, text) => set((s) => (s.drafts[key] === text ? s : { drafts: { ...s.drafts, [key]: text } })),

  addAttachment: (key, att) =>
    set((s) => ({ attachments: { ...s.attachments, [key]: [...(s.attachments[key] ?? []), att] } })),

  updateAttachment: (key, att) =>
    set((s) => ({
      attachments: { ...s.attachments, [key]: (s.attachments[key] ?? []).map((a) => (a.id === att.id ? att : a)) },
    })),

  removeAttachment: (key, id) =>
    set((s) => ({ attachments: { ...s.attachments, [key]: (s.attachments[key] ?? []).filter((a) => a.id !== id) } })),

  setAttachments: (key, list) => set((s) => ({ attachments: { ...s.attachments, [key]: list } })),

  enqueue: (key, item, first = false) =>
    set((s) => {
      const queued: QueueItem = { ...item, id: `q${Date.now().toString(36)}${(seq++).toString(36)}` };
      const list = s.queue[key] ?? [];
      return { queue: { ...s.queue, [key]: first ? [queued, ...list] : [...list, queued] } };
    }),

  takeQueued: (key, id) => {
    const list = get().queue[key] ?? [];
    const item = id ? list.find((q) => q.id === id) : list[0];
    if (!item) return null;
    set((s) => ({ queue: { ...s.queue, [key]: (s.queue[key] ?? []).filter((q) => q.id !== item.id) } }));
    return item;
  },

  setPaused: (key, reason) =>
    set((s) => {
      const paused = { ...s.paused };
      if (reason) paused[key] = reason;
      else delete paused[key];
      return { paused };
    }),
}));

// ------------------------------------------------------------ saída da fila

/** Espera entre o fim do turno e o envio da próxima mensagem. */
const DRAIN_DELAY_MS = 400;
/** Tarefas com envio da fila agendado ou em andamento. */
const draining = new Set<TaskId>();
/** Tarefas em que o usuário pediu "parar e enviar": a fila sai mesmo com o turno cancelado. */
const urgent = new Set<TaskId>();

/** Marca que a próxima mensagem da fila deve sair mesmo se o turno for cancelado. */
export function markUrgent(task: TaskId): void {
  urgent.add(task);
}

/**
 * Manda a primeira mensagem da fila da tarefa. Devolve false se não havia o que
 * mandar. Em caso de erro a mensagem volta para a frente da fila, que fica pausada.
 */
export async function sendNextQueued(task: TaskId): Promise<boolean> {
  const key = chatKey(task);
  const item = useChat.getState().takeQueued(key);
  if (!item) return false;
  useChat.getState().setPaused(key, null);
  try {
    await api.taskReply(task, buildPrompt(item.text, item.attachments));
    return true;
  } catch (e) {
    useChat.getState().enqueue(key, { text: item.text, attachments: item.attachments }, true);
    useChat.getState().setPaused(key, errorMessage(e));
    return false;
  }
}

function onState(task: TaskId, state: TaskState): void {
  if (isTaskActive(state)) return;
  const key = chatKey(task);
  if (!(useChat.getState().queue[key] ?? []).length) {
    urgent.delete(task);
    return;
  }
  const { kind } = variantOf(state);
  const forced = urgent.delete(task) && kind === "Cancelled";
  if (kind !== "Completed" && !forced) {
    const reason =
      kind === "Failed"
        ? "o turno falhou"
        : kind === "Cancelled"
          ? "o turno foi interrompido"
          : "a tarefa foi encerrada";
    useChat.getState().setPaused(key, reason);
    return;
  }
  if (draining.has(task)) return;
  draining.add(task);
  window.setTimeout(() => {
    void sendNextQueued(task).finally(() => draining.delete(task));
  }, DRAIN_DELAY_MS);
}

let started = false;
/** Liga (uma vez) a saída automática da fila. Chamado ao carregar o chat. */
export function startQueueDrain(): void {
  if (started || !isTauri()) return;
  started = true;
  onTaskState(onState).catch(() => {
    started = false;
  });
}
