/**
 * Sessões de terminal: um xterm ligado a um PTY do backend.
 *
 * A sessão vive fora do React, num registro por chave. O componente só encaixa
 * o elemento do xterm no próprio contêiner (`attach`) e o solta ao desmontar
 * (`detach`). Assim o processo sobrevive quando a aba muda de grupo no
 * workbench e à montagem dupla do StrictMode: a sessão só é encerrada se
 * ninguém a reencaixar logo depois de solta.
 */
import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { api, errorMessage, onPtyExit, onPtyOutput, type CmdSpec, type CommandSpec, type Path, type PtyId } from "@/api";
import { tokenColor, useTheme } from "@/store";

/** Tempo de tolerância entre soltar e reencaixar antes de encerrar o processo. */
const DISPOSE_DELAY_MS = 250;

export interface SessionOptions {
  cwd: Path | null;
  command: CmdSpec | CommandSpec | null;
}

export interface TerminalSession {
  readonly key: string;
  /** Elemento onde o xterm foi aberto; é movido entre contêineres. */
  readonly host: HTMLDivElement;
  readonly term: XTerm;
  readonly fit: FitAddon;
  ptyId: PtyId | null;
  error: string | null;
  exited: boolean;
  opened: boolean;
  disposed: boolean;
  disposeTimer: ReturnType<typeof setTimeout> | null;
  cleanup: (() => void)[];
  /** Avisados quando `error`, `ptyId` ou `exited` mudam. */
  listeners: Set<() => void>;
}

const sessions = new Map<string, TerminalSession>();
let nextKey = 1;

/** Chave nova para um terminal sem identidade própria. */
export function freshTerminalKey(): string {
  return `term-${nextKey++}`;
}

function notify(s: TerminalSession): void {
  s.listeners.forEach((cb) => cb());
}

/** Ajusta colunas/linhas ao tamanho do contêiner (ignora contêiner oculto). */
export function fitSession(s: TerminalSession): void {
  if (s.disposed || !s.opened || !s.host.isConnected) return;
  if (s.host.clientWidth === 0 || s.host.clientHeight === 0) return;
  try {
    s.fit.fit();
  } catch {
    // O xterm ainda não mediu a fonte; o próximo redimensionamento ajusta.
  }
}

async function start(s: TerminalSession, opts: SessionOptions): Promise<void> {
  try {
    const id = await api.ptySpawn({ cwd: opts.cwd, command: opts.command, cols: s.term.cols, rows: s.term.rows });
    if (s.disposed) {
      void api.ptyKill(id).catch(() => undefined);
      return;
    }
    s.ptyId = id;
    s.cleanup.push(
      onPtyOutput(id, (bytes) => s.term.write(bytes)),
      onPtyExit(id, (code) => {
        s.exited = true;
        s.term.write(`\r\n[processo finalizado: ${code ?? "?"}]\r\n`);
        notify(s);
      }),
    );
    // O tamanho pode ter mudado enquanto o processo era criado.
    void api.ptyResize(id, s.term.cols, s.term.rows).catch(() => undefined);
    notify(s);
  } catch (e) {
    s.error = `Falha ao iniciar terminal: ${errorMessage(e)}`;
    notify(s);
  }
}

/** Cores do xterm a partir dos tokens do tema. */
function termTheme() {
  return {
    background: tokenColor("bg-editor"),
    foreground: tokenColor("text"),
    cursor: tokenColor("focus"),
    cursorAccent: tokenColor("bg-editor"),
    selectionBackground: tokenColor("selection"),
    red: tokenColor("err"),
    green: tokenColor("ok"),
    yellow: tokenColor("warn"),
    blue: tokenColor("run"),
    brightBlack: tokenColor("text-2"),
  };
}

// Os terminais abertos acompanham a troca de tema.
useTheme.subscribe(() => {
  const theme = termTheme();
  for (const s of sessions.values()) s.term.options.theme = theme;
});

function create(key: string): TerminalSession {
  const host = document.createElement("div");
  host.className = "term-host";
  const term = new XTerm({
    cursorBlink: true,
    fontSize: 13,
    fontFamily: "Cascadia Mono, Consolas, monospace",
    theme: termTheme(),
    allowProposedApi: false,
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  const s: TerminalSession = {
    key,
    host,
    term,
    fit,
    ptyId: null,
    error: null,
    exited: false,
    opened: false,
    disposed: false,
    disposeTimer: null,
    cleanup: [],
    listeners: new Set(),
  };
  const data = term.onData((d) => {
    if (s.ptyId != null && !s.exited) void api.ptyWrite(s.ptyId, d).catch(() => undefined);
  });
  const resize = term.onResize(({ cols, rows }) => {
    if (s.ptyId != null && !s.exited) void api.ptyResize(s.ptyId, cols, rows).catch(() => undefined);
  });
  // Ctrl+C com seleção copia (sem seleção, interrompe o processo, como sempre);
  // Ctrl+V deixa o evento de colar do navegador chegar ao xterm.
  term.attachCustomKeyEventHandler((e) => {
    if (e.type !== "keydown" || !e.ctrlKey || e.altKey) return true;
    const k = e.key.toLowerCase();
    if (k === "c" && term.hasSelection()) {
      void navigator.clipboard.writeText(term.getSelection()).catch(() => undefined);
      term.clearSelection();
      e.preventDefault();
      return false;
    }
    if (k === "v") return false;
    return true;
  });
  s.cleanup.push(
    () => data.dispose(),
    () => resize.dispose(),
  );
  sessions.set(key, s);
  return s;
}

/**
 * Encaixa a sessão `key` em `container`, criando o xterm e o processo na
 * primeira vez. As opções só valem na criação.
 */
export function attachSession(key: string, container: HTMLElement, opts: SessionOptions): TerminalSession {
  const s = sessions.get(key) ?? create(key);
  if (s.disposeTimer != null) {
    clearTimeout(s.disposeTimer);
    s.disposeTimer = null;
  }
  container.appendChild(s.host);
  if (!s.opened) {
    s.opened = true;
    s.term.open(s.host);
    fitSession(s);
    void start(s, opts);
  } else {
    fitSession(s);
  }
  return s;
}

/** Solta a sessão do contêiner; se ninguém a reencaixar em seguida, encerra o processo. */
export function detachSession(key: string): void {
  const s = sessions.get(key);
  if (!s) return;
  s.host.remove();
  if (s.disposeTimer != null) clearTimeout(s.disposeTimer);
  s.disposeTimer = setTimeout(() => disposeSession(key), DISPOSE_DELAY_MS);
}

/** Encerra o processo e descarta o xterm. */
export function disposeSession(key: string): void {
  const s = sessions.get(key);
  if (!s) return;
  sessions.delete(key);
  s.disposed = true;
  if (s.disposeTimer != null) clearTimeout(s.disposeTimer);
  s.cleanup.forEach((off) => off());
  s.cleanup = [];
  s.listeners.clear();
  if (s.ptyId != null) void api.ptyKill(s.ptyId).catch(() => undefined);
  s.host.remove();
  s.term.dispose();
}
