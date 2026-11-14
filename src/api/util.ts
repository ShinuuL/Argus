/** Utilitários puros para os tipos do contrato (sem chamadas ao backend). */
import type { AutoWorkStatus, CostLine, LoginStatus, Owner, Path, TaskState } from "./types";

/** Mensagem legível de um erro vindo de `invoke` (o backend rejeita com string). */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

/**
 * Lê um enum do serde com tag externa: `"Running"` vira `{ kind: "Running", data: undefined }`
 * e `{ Failed: "x" }` vira `{ kind: "Failed", data: "x" }`. Serve para qualquer variante,
 * inclusive as que ainda não estão nos tipos.
 */
export function variantOf(value: unknown): { kind: string; data: unknown } {
  if (typeof value === "string") return { kind: value, data: undefined };
  if (value && typeof value === "object") {
    const keys = Object.keys(value);
    if (keys.length === 1) return { kind: keys[0], data: (value as Record<string, unknown>)[keys[0]] };
  }
  return { kind: "", data: value };
}

/** Decodifica base64 em bytes (saída de terminal). */
export function decodeBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Último trecho de um caminho (aceita `\` e `/`). */
export function baseName(path: Path): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.length ? parts[parts.length - 1] : path;
}

/** Compara caminhos ignorando o tipo de barra e, no Windows, maiúsculas. */
export function samePath(a: Path | null | undefined, b: Path | null | undefined): boolean {
  if (a == null || b == null) return a == b;
  const norm = (p: string) => p.replace(/[\\/]+/g, "/").replace(/\/$/, "").toLowerCase();
  return norm(a) === norm(b);
}

/** Tarefa na fila ou rodando. */
export function isTaskActive(state: TaskState): boolean {
  return state === "Queued" || state === "Running";
}

/** Turno terminado, mas a tarefa ainda aceita diff, aplicar, descartar e resposta. */
export function isTaskFinished(state: TaskState): boolean {
  return state === "Completed" || state === "Cancelled" || (typeof state === "object" && "Failed" in state);
}

export type Tone = "ok" | "warn" | "err" | "muted";

/** Rótulo e tom (classe CSS) do estado de uma tarefa. */
export function taskStateLabel(state: TaskState): { label: string; tone: Tone } {
  const { kind, data } = variantOf(state);
  switch (kind) {
    case "Queued":
      return { label: "Na fila", tone: "muted" };
    case "Running":
      return { label: "Rodando", tone: "warn" };
    case "Completed":
      return { label: "Concluída", tone: "ok" };
    case "Failed":
      return { label: `Falhou: ${String(data ?? "")}`, tone: "err" };
    case "Cancelled":
      return { label: "Cancelada", tone: "muted" };
    case "Applied":
      return { label: "Aplicada", tone: "ok" };
    case "Discarded":
      return { label: "Descartada", tone: "muted" };
    default:
      return { label: kind || "Desconhecido", tone: "muted" };
  }
}

/** Rótulo e tom do status de login de um provedor. */
export function loginStatusLabel(status: LoginStatus | null | undefined): { label: string; tone: Tone } {
  switch (status) {
    case "NotInstalled":
      return { label: "não instalado", tone: "err" };
    case "LoggedOut":
      return { label: "sem login", tone: "warn" };
    case "LoggedIn":
      return { label: "conectado", tone: "ok" };
    case "Unknown":
      return { label: "desconhecido", tone: "muted" };
    default:
      return { label: "verificando…", tone: "muted" };
  }
}

/** Rótulo curto do status do auto-work. */
export function autoWorkStatusLabel(status: AutoWorkStatus): string {
  const { kind, data } = variantOf(status);
  switch (kind) {
    case "Running":
      return "rodando";
    case "Paused":
      return "pausado";
    case "Completed":
      return "concluído";
    case "Stopped":
      return `parado: ${String(data ?? "")}`;
    default:
      return kind;
  }
}

/** Rótulo do dono de um servidor. */
export function ownerLabel(owner: Owner): string {
  return owner.kind === "Session" ? `Sessão #${owner.id}` : `Terminal ${owner.id}`;
}

export function sameOwner(a: Owner, b: Owner): boolean {
  return a.kind === b.kind && a.id === b.id;
}

/** Soma de entrada, saída e cache de uma linha de custo. */
export function totalTokens(l: Pick<CostLine, "input_tokens" | "output_tokens" | "cache_read" | "cache_write">): number {
  return l.input_tokens + l.output_tokens + l.cache_read + l.cache_write;
}

/** 950, 12.3k, 2.50M, 1.20B. */
export function formatTokens(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(2)}M`;
  if (n >= 1e4) return `${(n / 1e3).toFixed(1)}k`;
  return String(Math.round(n));
}

/** Custo equivalente em API: $0.0042 abaixo de um centavo, senão $1.23. */
export function formatUsd(v: number): string {
  return v > 0 && v < 0.01 ? `$${v.toFixed(4)}` : `$${v.toFixed(2)}`;
}

/** Tempo desde `startedAt` (segundos Unix): 42s, 5min, 2h07. */
export function formatUptime(startedAt: number, nowMs: number = Date.now()): string {
  const s = Math.max(0, Math.floor(nowMs / 1000) - startedAt);
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}min`;
  return `${Math.floor(s / 3600)}h${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}`;
}

/** Linguagem do Monaco a partir do nome do arquivo. */
export function languageFor(path: Path): string {
  const name = baseName(path);
  const ext = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1).toLowerCase() : "";
  const map: Record<string, string> = {
    rs: "rust",
    js: "javascript",
    mjs: "javascript",
    cjs: "javascript",
    jsx: "javascript",
    ts: "typescript",
    mts: "typescript",
    cts: "typescript",
    tsx: "typescript",
    json: "json",
    toml: "ini",
    ini: "ini",
    py: "python",
    go: "go",
    java: "java",
    kt: "kotlin",
    kts: "kotlin",
    c: "c",
    h: "c",
    cpp: "cpp",
    hpp: "cpp",
    cc: "cpp",
    cs: "csharp",
    html: "html",
    htm: "html",
    css: "css",
    scss: "scss",
    md: "markdown",
    yml: "yaml",
    yaml: "yaml",
    xml: "xml",
    sql: "sql",
    sh: "shell",
    bash: "shell",
    ps1: "powershell",
    psm1: "powershell",
  };
  if (map[ext]) return map[ext];
  return name === "Dockerfile" ? "dockerfile" : "plaintext";
}
