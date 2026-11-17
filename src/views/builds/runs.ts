/**
 * Estado compartilhado de builds e servidores de dev: execuções (evento
 * `runs-changed`), comandos detectados por projeto, servidores lembrados e o
 * último aviso (status bar). Tudo roda num terminal da IDE: `startCommand` pede
 * ao backend o comando validado e abre uma aba no painel de terminais.
 */
import { create } from "zustand";
import {
  api,
  baseName,
  onRunFinished,
  onRunsChanged,
  samePath,
  type DetectedCommand,
  type Path,
  type RememberedServer,
  type RunInfo,
} from "@/api";
import { useApp, useTerminals } from "@/store";

export interface RunsFlash {
  text: string;
  tone: "ok" | "err" | "info";
  at: number;
}

interface RunsState {
  runs: RunInfo[];
  /** Comandos detectados, por caminho do projeto. */
  commands: Record<Path, DetectedCommand[]>;
  remembered: RememberedServer[];
  /** Último aviso (build terminou, porta trocada...). */
  flash: RunsFlash | null;
  /** Incrementa quando um build termina (para recarregar o histórico). */
  finishedRev: number;
}

export const useRuns = create<RunsState>()(() => ({
  runs: [],
  commands: {},
  remembered: [],
  flash: null,
  finishedRev: 0,
}));

export function flash(text: string, tone: RunsFlash["tone"] = "info"): void {
  useRuns.setState({ flash: { text, tone, at: Date.now() } });
}

export const isActive = (r: RunInfo) => r.state === "running" || r.state === "ready";

/** Servidor de dev ativo do projeto. */
export function serverOf(runs: RunInfo[], project: Path): RunInfo | undefined {
  return runs.find((r) => r.kind === "servidor" && isActive(r) && samePath(r.project, project));
}

/** Build em andamento do projeto. */
export function buildOf(runs: RunInfo[], project: Path): RunInfo | undefined {
  return runs.find((r) => r.kind === "build" && isActive(r) && samePath(r.project, project));
}

/** Recarrega os comandos detectados de todos os projetos. */
export async function loadAllCommands(): Promise<void> {
  const all = await api.runsDetectAll();
  const commands: Record<Path, DetectedCommand[]> = {};
  for (const p of all) commands[p.project] = p.commands;
  useRuns.setState({ commands });
}

export async function loadRemembered(): Promise<void> {
  useRuns.setState({ remembered: await api.runsRemembered() });
}

export function commandsOf(commands: Record<Path, DetectedCommand[]>, project: Path): DetectedCommand[] {
  const key = Object.keys(commands).find((k) => samePath(k, project));
  return key ? commands[key] : [];
}

/** Servidor preferido do projeto: o último usado ou o primeiro detectado. */
export function preferredServer(state: Pick<RunsState, "commands" | "remembered">, project: Path): DetectedCommand | undefined {
  const servers = commandsOf(state.commands, project).filter((c) => c.kind === "servidor");
  const last = state.remembered.find((r) => samePath(r.project, project))?.command_id;
  return servers.find((c) => c.id === last) ?? servers[0];
}

let synced = false;

/** Liga os eventos do backend (uma vez só). */
export function ensureRunsSync(): void {
  if (synced) return;
  synced = true;
  void api.runsList().then((runs) => useRuns.setState({ runs }), () => undefined);
  void onRunsChanged((runs) => useRuns.setState({ runs }));
  void onRunFinished((run) => {
    if (run.kind !== "build") return;
    const name = baseName(run.project);
    const ok = run.state === "ok";
    const failed = run.state === "failed";
    const text = ok
      ? `Build concluído em ${name}: ${run.label}`
      : failed
        ? `Build falhou em ${name}: ${run.label} (código ${run.exit_code ?? "?"})`
        : `Build ${run.state === "stopped" ? "interrompido" : "encerrado"} em ${name}: ${run.label}`;
    useRuns.setState((s) => ({ flash: { text, tone: ok ? "ok" : failed ? "err" : "info", at: Date.now() }, finishedRev: s.finishedRev + 1 }));
    // O pet fica feliz ou preocupado, como no fim de uma tarefa de agente.
    if (ok) useApp.setState({ lastOkAt: Date.now() });
    if (failed) useApp.setState({ lastErrAt: Date.now() });
  });
  void loadAllCommands().catch(() => undefined);
  void loadRemembered().catch(() => undefined);
  // A lista de projetos mudou: detecta de novo.
  let projects = useApp.getState().projects;
  useApp.subscribe((s) => {
    if (s.projects === projects) return;
    projects = s.projects;
    void loadAllCommands().catch(() => undefined);
    void loadRemembered().catch(() => undefined);
  });
}

/**
 * Roda um comando detectado num terminal novo da IDE (painel inferior).
 * Rejeita com a mensagem do backend (build rodando, porta ocupada...).
 */
export async function startCommand(project: Path, commandId: string): Promise<void> {
  const p = await api.runsPrepare(project, commandId);
  useTerminals.getState().open({ title: p.title, cwd: project, command: p.command });
  if (p.notice) flash(p.notice, "info");
  void loadRemembered().catch(() => undefined);
}

/** Inicia o servidor lembrado de cada projeto (os que já estão no ar ficam como estão). */
export async function startAllRemembered(): Promise<string[]> {
  await loadRemembered();
  const { remembered, runs } = useRuns.getState();
  const errors: string[] = [];
  for (const r of remembered) {
    if (serverOf(runs, r.project)) continue;
    try {
      await startCommand(r.project, r.command_id);
    } catch (e) {
      errors.push(`${baseName(r.project)}: ${String(e)}`);
    }
  }
  return errors;
}

export function formatBytes(n: number | null | undefined): string {
  if (n == null) return "";
  if (n < 1024) return `${n} B`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(2)} GB`;
}

export function formatDuration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}min${String(s % 60).padStart(2, "0")}s`;
  return `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}`;
}

export const STATE_LABEL: Record<string, string> = {
  running: "em andamento",
  ready: "pronto",
  ok: "concluído",
  failed: "falhou",
  stopped: "parado",
  ended: "encerrado",
  interrompido: "interrompido",
};

export const ORIGIN_LABEL: Record<string, string> = {
  ide: "botão",
  terminal: "terminal",
  agente: "agente",
};
