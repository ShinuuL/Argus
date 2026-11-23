/**
 * Eventos do backend (src-tauri/src/events.rs).
 *
 * Os helpers `on*` globais devolvem `Promise<Unlisten>`; num `useEffect`:
 *
 *   useEffect(() => {
 *     const off = onOrch((ev) => ...);
 *     return () => void off.then((f) => f());
 *   }, []);
 *
 * Os de terminal (`onPtyOutput`, `onPtyExit`) são síncronos e devolvem `Unlisten` direto.
 */
import { listen as listenReal } from "@tauri-apps/api/event";
import { listenDaGaleria } from "@/dev/galeria";
import { decodeBase64 } from "./util";

/** Sem cenário de galeria, é o `listen` do Tauri (ver src/dev/galeria.ts). */
const listen = listenDaGaleria(listenReal);
import type {
  AgentEvent,
  AutoWorkEvent,
  LocalMediaEvent,
  OrchEvent,
  ProjectChanged,
  PtyExit,
  PtyId,
  PtyMissingCommand,
  PtyOutput,
  RootInfo,
  ServerInfo,
  RunInfo,
  TaskId,
  TaskState,
} from "./types";

export type Unlisten = () => void;

/** Nomes dos eventos, iguais aos de src-tauri/src/events.rs. */
export const EVENTS = {
  orch: "orch",
  orchLagged: "orch-lagged",
  projectChanged: "project-changed",
  workspaceChanged: "workspace-changed",
  ptyOutput: "pty-output",
  ptyExit: "pty-exit",
  ptyMissingCommand: "pty-missing-command",
  serversChanged: "servers-changed",
  mediaLocal: "media-local",
  runsChanged: "runs-changed",
  runFinished: "run-finished",
  schedulesChanged: "schedules-changed",
} as const;

function on<T>(event: string, cb: (payload: T) => void): Promise<Unlisten> {
  return listen<T>(event, (e) => cb(e.payload));
}

// ---------------------------------------------------------------- orquestrador

/** Todos os eventos do orquestrador (estado das tarefas, eventos de agente, stderr, auto-work). */
export const onOrch = (cb: (ev: OrchEvent) => void): Promise<Unlisten> => on(EVENTS.orch, cb);
/** A janela perdeu eventos do orquestrador: quem mostra histórico deve relê-lo. */
export const onOrchLagged = (cb: () => void): Promise<Unlisten> => on(EVENTS.orchLagged, () => cb());

/** Só as mudanças de estado das tarefas. */
export const onTaskState = (cb: (task: TaskId, state: TaskState) => void): Promise<Unlisten> =>
  onOrch((ev) => {
    if ("StateChanged" in ev) cb(ev.StateChanged.task, ev.StateChanged.state);
  });

/** Eventos de agente de uma tarefa; o stderr chega como `{ Raw: linha }`, como no chat antigo. */
export const onTaskEvent = (task: TaskId, cb: (event: AgentEvent) => void): Promise<Unlisten> =>
  onOrch((ev) => {
    if ("Agent" in ev) {
      if (ev.Agent.task === task) cb(ev.Agent.event);
    } else if ("Stderr" in ev) {
      if (ev.Stderr.task === task) cb({ Raw: ev.Stderr.line });
    }
  });

/** Só o progresso do auto-work. */
export const onAutoWork = (cb: (ev: AutoWorkEvent) => void): Promise<Unlisten> =>
  onOrch((ev) => {
    if ("AutoWork" in ev) cb(ev.AutoWork);
  });

// ------------------------------------------------------------------ projetos

/** Status git recalculado de um projeto (ao abrir a raiz e a cada mudança em disco). */
export const onProjectChanged = (cb: (ev: ProjectChanged) => void): Promise<Unlisten> =>
  on(EVENTS.projectChanged, cb);

/** A lista de projetos da pasta raiz mudou (subpasta criada, removida ou renomeada). */
export const onWorkspaceChanged = (cb: (info: RootInfo) => void): Promise<Unlisten> =>
  on(EVENTS.workspaceChanged, cb);

// ---------------------------------------------------------------- servidores

/** Lista completa de servidores sempre que ela muda. */
export const onServersChanged = (cb: (servers: ServerInfo[]) => void): Promise<Unlisten> =>
  on(EVENTS.serversChanged, cb);

// --------------------------------------------------------------------- mídia

/** Fim de faixa e erros do player de arquivos locais. */
export const onMediaLocal = (cb: (ev: LocalMediaEvent) => void): Promise<Unlisten> => on(EVENTS.mediaLocal, cb);

// ------------------------------------------------------------------ terminal

/** O terminal informou "comando não encontrado" para uma ferramenta instalável (uma vez por comando). */
export const onPtyMissingCommand = (cb: (ev: PtyMissingCommand) => void): Promise<Unlisten> =>
  on(EVENTS.ptyMissingCommand, cb);

interface PtyChannel {
  output: Set<(bytes: Uint8Array) => void>;
  exit: Set<(code: number | null) => void>;
  /** Saída recebida antes de alguém assinar. */
  pending: Uint8Array[];
  pendingBytes: number;
  exited: { code: number | null } | null;
}

/** Limite do que fica guardado por terminal enquanto ninguém assina. */
const MAX_PENDING_BYTES = 4 * 1024 * 1024;
const channels = new Map<PtyId, PtyChannel>();
const anyExit = new Set<(id: PtyId, code: number | null) => void>();
let ptyReady: Promise<void> | null = null;

function channel(id: PtyId): PtyChannel {
  let c = channels.get(id);
  if (!c) {
    c = { output: new Set(), exit: new Set(), pending: [], pendingBytes: 0, exited: null };
    channels.set(id, c);
  }
  return c;
}

/**
 * Registra (uma vez) os ouvintes globais de terminal. `ptySpawn` espera por
 * isto antes de criar o processo, então nenhuma saída inicial se perde.
 */
export function ensurePtyListeners(): Promise<void> {
  ptyReady ??= Promise.all([
    listen<PtyOutput>(EVENTS.ptyOutput, ({ payload }) => {
      const c = channel(payload.id);
      const bytes = decodeBase64(payload.data);
      if (c.output.size > 0) {
        c.output.forEach((cb) => cb(bytes));
      } else if (c.pendingBytes + bytes.length <= MAX_PENDING_BYTES) {
        c.pending.push(bytes);
        c.pendingBytes += bytes.length;
      }
    }),
    listen<PtyExit>(EVENTS.ptyExit, ({ payload }) => {
      const c = channel(payload.id);
      c.exited = { code: payload.code };
      c.exit.forEach((cb) => cb(payload.code));
      anyExit.forEach((cb) => cb(payload.id, payload.code));
    }),
  ]).then(() => undefined);
  return ptyReady;
}

/** Saída (bytes) de um terminal; entrega primeiro o que chegou antes da assinatura. */
export function onPtyOutput(id: PtyId, cb: (bytes: Uint8Array) => void): Unlisten {
  void ensurePtyListeners();
  const c = channel(id);
  const pending = c.pending;
  c.pending = [];
  c.pendingBytes = 0;
  pending.forEach(cb);
  c.output.add(cb);
  return () => void c.output.delete(cb);
}

/** Fim do processo de um terminal (chama na hora se ele já terminou). */
export function onPtyExit(id: PtyId, cb: (code: number | null) => void): Unlisten {
  void ensurePtyListeners();
  const c = channel(id);
  if (c.exited) cb(c.exited.code);
  c.exit.add(cb);
  return () => void c.exit.delete(cb);
}

/** Fim de qualquer terminal (ex.: reconsultar o login depois que a CLI termina). */
export function onAnyPtyExit(cb: (id: PtyId, code: number | null) => void): Unlisten {
  void ensurePtyListeners();
  anyExit.add(cb);
  return () => void anyExit.delete(cb);
}

/** Descarta assinantes e saída guardada de um terminal (chamado por `ptyKill`). */
export function forgetPty(id: PtyId): void {
  channels.delete(id);
}

// --------------------------------------------- builds e servidores de dev

/** Lista de execuções (builds e servidores de dev) sempre que muda. */
export const onRunsChanged = (cb: (runs: RunInfo[]) => void): Promise<Unlisten> => on(EVENTS.runsChanged, cb);

/** Uma execução terminou (ok, falhou, parada ou encerrada). */
export const onRunFinished = (cb: (run: RunInfo) => void): Promise<Unlisten> => on(EVENTS.runFinished, cb);

/** A lista de tarefas agendadas mudou (disparo, término, edição). */
export const onSchedulesChanged = (cb: () => void): Promise<Unlisten> => on(EVENTS.schedulesChanged, () => cb());
