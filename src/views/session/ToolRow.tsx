/**
 * Linhas compactas da conversa, no estilo do painel de chat do Claude Code:
 * ferramenta (nome em negrito + resumo em mono, recolhida; abre IN/OUT ou
 * diff), raciocínio recolhido e a lista de tarefas (TodoWrite) como checklist.
 */
import { Fragment, useState, type ReactNode } from "react";
import { Icon, toolIcon } from "@/shell/icons";

type Input = Record<string, unknown>;

const str = (o: Input, ...keys: string[]): string => {
  for (const k of keys) {
    const v = o[k];
    if (typeof v === "string" && v.trim()) return v;
  }
  return "";
};
const num = (o: Input, k: string): number | null => (typeof o[k] === "number" ? (o[k] as number) : null);
const firstLine = (t: string) => t.trim().split(/\r?\n/)[0] ?? "";
const asInput = (input: unknown): Input => (input && typeof input === "object" && !Array.isArray(input) ? (input as Input) : {});

/** Caminho relativo a `base` (a pasta do projeto) quando estiver dentro dela. */
function relative(path: string, base: string | null): string {
  if (!base || !path) return path;
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");
  const p = norm(path);
  const b = norm(base);
  return p.toLowerCase().startsWith(b.toLowerCase() + "/") ? p.slice(b.length + 1) : path;
}

/** Resumo de uma linha da entrada, por ferramenta (comando, caminho, padrão...). */
export function toolSummary(name: string, input: unknown, base: string | null = null): string {
  if (input == null) return "";
  if (typeof input !== "object") return String(input);
  const raw = asInput(input);
  const o: Input = { ...raw };
  for (const k of ["file_path", "path", "filePath", "notebook_path"]) {
    if (typeof o[k] === "string") o[k] = relative(o[k] as string, base);
  }
  const t = name.toLowerCase();
  if (t === "read") {
    const path = str(o, "file_path", "path");
    const off = num(o, "offset");
    const lim = num(o, "limit");
    if (off == null && lim == null) return path;
    const from = off ?? 1;
    return lim != null ? `${path} (linhas ${from}–${from + lim - 1})` : `${path} (a partir da linha ${from})`;
  }
  if (t === "grep") {
    const pattern = str(o, "pattern");
    const where = str(o, "path", "glob");
    return where ? `${pattern}  em ${where}` : pattern;
  }
  if (t === "glob") {
    const pattern = str(o, "pattern");
    const where = str(o, "path");
    return where ? `${pattern}  em ${where}` : pattern;
  }
  return firstLine(
    str(o, "command", "file_path", "path", "filePath", "notebook_path", "pattern", "query", "url", "description", "prompt"),
  );
}

/** Rótulo do status da ferramenta. */
export type ToolStatus = "running" | "ok" | "error" | "none";

const STATUS_TITLE: Record<ToolStatus, string> = {
  running: "Rodando",
  ok: "Concluída",
  error: "Falhou",
  none: "Sem saída registrada",
};

/** Bolinha da linha do tempo; cor pelo status. */
export function Dot({ status, title }: { status: ToolStatus | "text" | "warn"; title?: string }) {
  return <span className={`tl-dot ${status}`} title={title} aria-hidden={title ? undefined : true} />;
}

/** Item da linha do tempo: bolinha à esquerda e conteúdo. */
export function TimelineItem({ dot, children, className }: { dot: ReactNode; children: ReactNode; className?: string }) {
  return (
    <div className={`tl-item${className ? ` ${className}` : ""}`}>
      <span className="tl-mark">{dot}</span>
      <div className="tl-body">{children}</div>
    </div>
  );
}

export interface DiffLine {
  sign: "+" | "-" | " ";
  text: string;
  /** Número da linha (no arquivo novo; no antigo para as removidas). Só no painel de diff. */
  no?: number;
}

/** Diff simples: linhas comuns no começo e no fim viram contexto; o meio, − e +. */
function diffLines(before: string, after: string): DiffLine[] {
  const a = before.split(/\r?\n/);
  const b = after.split(/\r?\n/);
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA--;
    endB--;
  }
  const ctx = 2;
  const out: DiffLine[] = [];
  for (let i = Math.max(0, start - ctx); i < start; i++) out.push({ sign: " ", text: a[i] });
  for (let i = start; i < endA; i++) out.push({ sign: "-", text: a[i] });
  for (let i = start; i < endB; i++) out.push({ sign: "+", text: b[i] });
  for (let i = endA; i < Math.min(a.length, endA + ctx); i++) out.push({ sign: " ", text: a[i] });
  return out;
}

/**
 * Linhas de diff (+/−/contexto). No chat, só as linhas; o painel de diff passa
 * `numbered` (coluna com o número da linha), `onLine` (clique no número para
 * comentar) e `after` (o que aparece embaixo de uma linha, como o comentário).
 */
export function Diff({
  lines,
  numbered = false,
  onLine,
  after,
  className,
}: {
  lines: DiffLine[];
  numbered?: boolean;
  onLine?: (i: number) => void;
  after?: (i: number) => ReactNode;
  className?: string;
}) {
  return (
    <div className={`tl-diff${numbered ? " numbered" : ""}${className ? ` ${className}` : ""}`} role="figure" aria-label="Alterações">
      {lines.map((l, i) => (
        <Fragment key={i}>
          <div className={`tl-diff-line ${l.sign === "+" ? "add" : l.sign === "-" ? "del" : "ctx"}`}>
            {numbered &&
              (onLine && l.no != null ? (
                <button className="tl-diff-no" title="Comentar esta linha" aria-label={`Comentar a linha ${l.no}`} onClick={() => onLine(i)}>
                  {l.no}
                </button>
              ) : (
                <span className="tl-diff-no">{l.no ?? ""}</span>
              ))}
            <span className="tl-diff-sign">{l.sign}</span>
            <span className="tl-diff-text">{l.text || " "}</span>
          </div>
          {after?.(i)}
        </Fragment>
      ))}
    </div>
  );
}

/** Diff de Edit/MultiEdit/Write; null para as demais ferramentas. */
function toolDiff(name: string, input: unknown): DiffLine[] | null {
  const o = asInput(input);
  const t = name.toLowerCase();
  if (t === "edit" && typeof o.old_string === "string" && typeof o.new_string === "string") {
    return diffLines(o.old_string, o.new_string);
  }
  if (t === "multiedit" && Array.isArray(o.edits)) {
    const out: DiffLine[] = [];
    for (const e of o.edits as Input[]) {
      if (typeof e?.old_string !== "string" || typeof e?.new_string !== "string") continue;
      if (out.length) out.push({ sign: " ", text: "⋯" });
      out.push(...diffLines(e.old_string, e.new_string));
    }
    return out;
  }
  if (t === "write" && typeof o.content === "string") {
    return o.content.split(/\r?\n/).map((text) => ({ sign: "+" as const, text }));
  }
  return null;
}

/** Texto do bloco IN: o comando para shell; senão "chave: valor" por linha. */
function inputText(name: string, input: unknown): string {
  if (input == null) return "";
  if (typeof input !== "object") return String(input);
  const o = asInput(input);
  const cmd = str(o, "command");
  if (cmd && /bash|shell|powershell/i.test(name)) return cmd;
  return Object.entries(o)
    .map(([k, v]) => `${k}: ${typeof v === "string" ? v : JSON.stringify(v, null, 2)}`)
    .join("\n");
}

function IoBlock({ label, text, error }: { label: "IN" | "OUT"; text: string; error?: boolean }) {
  return (
    <div className={`tl-io${error ? " error" : ""}`}>
      <span className="tl-io-label">{label}</span>
      <pre>{text}</pre>
    </div>
  );
}

interface TodoItem {
  content: string;
  status: string;
  activeForm?: string;
}

function TodoList({ input }: { input: unknown }) {
  const todos = (asInput(input).todos as TodoItem[] | undefined) ?? [];
  if (!Array.isArray(todos) || todos.length === 0) return null;
  return (
    <ul className="tl-todos">
      {todos.map((t, i) => (
        <li key={i} className={`tl-todo ${t.status}`}>
          {t.status === "completed" ? (
            <Icon name="circleCheck" size={14} />
          ) : t.status === "in_progress" ? (
            <Icon name="circleDot" size={14} />
          ) : (
            <Icon name="circle" size={14} />
          )}
          <span>{t.status === "in_progress" && t.activeForm ? t.activeForm : t.content}</span>
        </li>
      ))}
    </ul>
  );
}

export interface ToolRowProps {
  name: string;
  input: unknown;
  result: { ok: boolean; output: string } | null;
  /** O turno ainda roda: ferramenta sem saída está rodando. */
  live: boolean;
  /** Pasta do projeto: caminhos dentro dela aparecem relativos no resumo. */
  base?: string | null;
}

/** Uma chamada de ferramenta como item da linha do tempo. */
export function ToolRow({ name, input, result, live, base = null }: ToolRowProps) {
  const isTodo = /todo/i.test(name);
  const [open, setOpen] = useState(isTodo);
  const status: ToolStatus = result ? (result.ok ? "ok" : "error") : live ? "running" : "none";
  const summary = isTodo ? "" : toolSummary(name, input, base);
  const diff = open ? toolDiff(name, input) : null;
  const label = isTodo ? "Tarefas" : name;
  return (
    <TimelineItem dot={<Dot status={status} title={STATUS_TITLE[status]} />} className={`tl-tool${open ? " open" : ""}`}>
      <button type="button" className="tl-row" aria-expanded={open} onClick={() => setOpen((v) => !v)} title={summary || undefined}>
        <Icon name={toolIcon(name)} size={13} className="tl-row-ic" />
        <b>{label}</b>
        {summary && <span className="tl-row-sum">{summary}</span>}
        <Icon name="chevronRight" size={12} className="tl-chev" />
      </button>
      {open &&
        (isTodo ? (
          <TodoList input={input} />
        ) : (
          <div className="tl-detail">
            {diff ? <Diff lines={diff} /> : <IoBlock label="IN" text={inputText(name, input)} />}
            {result && (result.output.trim() || !result.ok) && (
              <IoBlock label="OUT" text={result.output.trim() || "(sem saída)"} error={!result.ok} />
            )}
            {!result && <div className="tl-io-note">{live ? "Rodando…" : "Sem saída registrada."}</div>}
          </div>
        ))}
    </TimelineItem>
  );
}

/** Raciocínio do modelo: "Pensou" recolhido; abre em itálico apagado. */
export function ThinkingRow({ text, live }: { text: string; live: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <TimelineItem dot={<Dot status={live ? "running" : "text"} />} className={`tl-think${open ? " open" : ""}`}>
      <button type="button" className="tl-row" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
        <Icon name="brain" size={13} className="tl-row-ic" />
        <span className="tl-think-label">{live ? "Pensando…" : "Pensou"}</span>
        <Icon name="chevronRight" size={12} className="tl-chev" />
      </button>
      {open && <div className="tl-think-text">{text}</div>}
    </TimelineItem>
  );
}
