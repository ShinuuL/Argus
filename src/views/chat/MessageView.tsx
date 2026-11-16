import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode } from "react";
import { api, formatTokensPt, variantOf, type AgentEvent, type TokenUsage } from "@/api";
import { splitPrompt } from "./attachments";
import { Thumb } from "./Lightbox";
import { md } from "./markdown";
import { Icon, toolIcon } from "@/shell/icons";
import { RUN_FOR_REAL_PROMPT, toolCallAsText } from "./toolcall";

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // sem permissão de área de transferência: cai no jeito antigo
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    return ok;
  }
}

function flash(button: HTMLElement, ok: boolean) {
  // Botão só com ícone (copiar a mensagem): troca o ícone pela classe, sem mexer no conteúdo.
  const iconOnly = button.classList.contains("chat-msg-copy");
  const before = button.textContent;
  if (!iconOnly) button.textContent = ok ? "Copiado" : "Falhou";
  button.classList.add(ok ? "done" : "fail");
  window.setTimeout(() => {
    if (!iconOnly) button.textContent = before;
    button.classList.remove("done", "fail");
  }, 1200);
}

export interface MarkdownProps {
  /** Texto em Markdown (é escapado antes de virar HTML). */
  text: string;
  /** Mostra o botão "Copiar a mensagem" no canto. */
  copyable?: boolean;
  className?: string;
}

/**
 * Markdown de uma resposta: títulos, listas, tabelas, citações, código com
 * rótulo da linguagem e botão de copiar. Links http(s) abrem no navegador.
 */
export const Markdown = memo(function Markdown({ text, copyable = false, className }: MarkdownProps) {
  const html = useMemo(() => md(text), [text]);

  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    const copy = target.closest<HTMLElement>("button.chat-copy");
    if (copy) {
      e.preventDefault();
      const code = copy.closest(".chat-code")?.querySelector("pre")?.textContent ?? "";
      void copyText(code).then((ok) => flash(copy, ok));
      return;
    }
    const link = target.closest<HTMLAnchorElement>("a[href]");
    if (link) {
      // nunca navega a própria janela: o endereço vai para o navegador do sistema
      e.preventDefault();
      void api.systemOpenUrl(link.getAttribute("href") ?? "").catch(() => undefined);
    }
  };

  return (
    <div className={`chat-md-wrap${className ? ` ${className}` : ""}`}>
      <div className="chat-md" onClick={onClick} dangerouslySetInnerHTML={{ __html: html }} />
      {copyable && (
        <div className="chat-msg-actions">
          <button
            type="button"
            className="chat-msg-copy"
            title="Copiar a mensagem"
            aria-label="Copiar a mensagem"
            onClick={(e) => {
              const button = e.currentTarget;
              void copyText(text).then((ok) => flash(button, ok));
            }}
          >
            <svg className="i-copy" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M9 9h10a1 1 0 011 1v10a1 1 0 01-1 1H9a1 1 0 01-1-1V10a1 1 0 011-1zM5 15H4a1 1 0 01-1-1V4a1 1 0 011-1h10a1 1 0 011 1v1" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
            <svg className="i-done" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M4 12l5 5L20 6" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
        </div>
      )}
    </div>
  );
});

/** Resumo de uma linha da entrada de uma ferramenta (o campo mais informativo). */
function toolSummary(input: unknown): string {
  if (input == null) return "";
  if (typeof input !== "object") return String(input);
  const o = input as Record<string, unknown>;
  for (const k of ["file_path", "path", "filePath", "command", "pattern", "query", "url", "description", "prompt"]) {
    const v = o[k];
    if (typeof v === "string" && v.trim()) return v.trim().split("\n")[0];
  }
  return "";
}

function usageLine(u: TokenUsage): string {
  const parts = [`${formatTokensPt(u.input_tokens)} entrada`, `${formatTokensPt(u.output_tokens)} saída`];
  if (u.cache_read) parts.push(`${formatTokensPt(u.cache_read)} cache lido`);
  if (u.cache_write) parts.push(`${formatTokensPt(u.cache_write)} cache gravado`);
  if (u.model) parts.push(u.model);
  return parts.join(" · ");
}

export interface MessageViewProps {
  /** Um evento do agente (de `taskHistory` ou do stream ao vivo). */
  event: AgentEvent;
  /**
   * Mostra o resumo de um `Result` bem-sucedido. Normalmente ele repete o
   * último texto do turno, então o padrão é esconder (falhas sempre aparecem).
   */
  showResultSummary?: boolean;
  /** Mostra a linha de tokens dos eventos `Usage` (padrão: sim). */
  showUsage?: boolean;
  /** Mostra as linhas brutas/stderr (`Raw`) (padrão: sim). */
  showRaw?: boolean;
  /** Mostra os eventos internos de subagentes (`Scoped`), recuados (padrão: não). */
  showScoped?: boolean;
  /** Torna clicável o cartão "→ subagente" (abre a visão somente leitura). */
  onOpenSubagent?: (id: string) => void;
}

/**
 * Renderiza UM evento da conversa:
 *
 * - `User`: balão do usuário; os anexos enviados pelo caminho viram miniaturas
 *   (imagens, clique amplia) ou chips;
 * - `Text`: Markdown com copiar;
 * - `ToolUse`: linha recolhida com o nome da ferramenta e o alvo, que abre a entrada completa;
 * - `Result`, `Usage`, `Raw`: linhas discretas;
 * - `SubagentStart` / `SubagentEnd`: cartão do subagente;
 * - `Session` e variantes desconhecidas: nada.
 */
export const MessageView = memo(function MessageView({
  event,
  showResultSummary = false,
  showUsage = true,
  showRaw = true,
  showScoped = false,
  onOpenSubagent,
}: MessageViewProps): ReactNode {
  const { kind, data } = variantOf(event);
  switch (kind) {
    case "User": {
      const { text, attachments } = splitPrompt(String(data ?? ""));
      return (
        <div className="chat-msg user">
          {attachments.length > 0 && (
            <div className="chat-msg-atts">
              {attachments.map((a) =>
                a.kind === "image" ? (
                  <Thumb key={a.path} path={a.path} name={a.name} />
                ) : (
                  <span key={a.path} className="chat-att-chip" title={a.path}>
                    {a.name}
                  </span>
                ),
              )}
            </div>
          )}
          {text && <div className="chat-user-text">{text}</div>}
        </div>
      );
    }
    case "Text":
      return (
        <div className="chat-msg agent">
          <Markdown text={String(data ?? "")} copyable />
        </div>
      );
    case "ToolUse": {
      const tool = data as { name: string; input: unknown };
      const summary = toolSummary(tool.input);
      return (
        <details className="chat-tool">
          <summary>
            <Icon name={toolIcon(tool.name)} size={13} className="chat-tool-ic" />
            <b>{tool.name}</b>
            {summary && <span title={summary}>{summary}</span>}
            <Icon name="chevronRight" size={12} className="chat-tool-chev" />
          </summary>
          <pre>{JSON.stringify(tool.input, null, 2)}</pre>
        </details>
      );
    }
    case "Result": {
      const r = data as { ok: boolean; summary: string; cost: number | null };
      if (r.ok && !(showResultSummary && r.summary.trim())) return <div className="chat-line ok">Turno concluído</div>;
      if (r.ok) {
        return (
          <div className="chat-msg agent">
            <Markdown text={r.summary} copyable />
          </div>
        );
      }
      return <div className="chat-line err">O turno falhou{r.summary.trim() ? `: ${r.summary.trim()}` : ""}</div>;
    }
    case "Usage":
      return showUsage ? <div className="chat-line dim">{usageLine(data as TokenUsage)}</div> : null;
    case "Raw": {
      const line = String(data ?? "");
      if (/^(\[stderr\]\s*)?warning:\s*claude\.ai mcp servers blocked/i.test(line.trim())) return null;
      return showRaw && line.trim() ? <pre className="chat-raw">{line}</pre> : null;
    }
    case "SubagentStart": {
      const s = data as { id: string; description: string; subagent_type: string };
      const label = (
        <>
          <b>→ subagente {s.subagent_type}</b>
          {s.description && <span>{s.description}</span>}
        </>
      );
      return onOpenSubagent ? (
        <button type="button" className="chat-sub" title="Ver a conversa do subagente (somente leitura)" onClick={() => onOpenSubagent(s.id)}>
          {label}
        </button>
      ) : (
        <div className="chat-sub">{label}</div>
      );
    }
    case "SubagentEnd": {
      const s = data as { id: string; ok: boolean };
      return <div className={`chat-line ${s.ok ? "dim" : "err"}`}>← subagente {s.ok ? "concluiu" : "falhou"}</div>;
    }
    case "Scoped": {
      if (!showScoped) return null;
      const s = data as { subagent: string; event: AgentEvent };
      return (
        <div className="chat-scoped">
          <MessageView event={s.event} showUsage={showUsage} showRaw={showRaw} showScoped />
        </div>
      );
    }
    default:
      return null;
  }
});

export interface MessageListProps {
  /** Eventos da conversa, em ordem. */
  events: AgentEvent[];
  /** O agente está trabalhando: mostra "trabalhando…" e não avalia o fim do turno. */
  busy?: boolean;
  /** Pedido inicial da tarefa, mostrado se o histórico não começar por uma fala do usuário. */
  initialPrompt?: string | null;
  /**
   * Envia uma resposta ao agente. Liga o aviso de chamada de ferramenta escrita
   * como texto, com o botão "Executar de verdade".
   */
  onReply?: (prompt: string) => void | Promise<void>;
  onOpenSubagent?: (id: string) => void;
  /** Texto do estado vazio. */
  emptyHint?: string;
  showUsage?: boolean;
  showRaw?: boolean;
  /** Conteúdo extra no fim da lista (ex.: cartões da tela). */
  children?: ReactNode;
}

/** Texto do agente desde a última fala do usuário. */
function lastTurnText(events: AgentEvent[]): string {
  const parts: string[] = [];
  for (let i = events.length - 1; i >= 0; i--) {
    const { kind, data } = variantOf(events[i]);
    if (kind === "User") break;
    if (kind === "Text") parts.unshift(String(data ?? ""));
  }
  return parts.join("\n");
}

/**
 * Conversa inteira: um `MessageView` por evento, rolagem que acompanha o fim
 * enquanto o usuário não sobe, "trabalhando…" e o aviso de chamada de
 * ferramenta escrita como texto no fim do turno.
 */
export function MessageList({
  events,
  busy = false,
  initialPrompt,
  onReply,
  onOpenSubagent,
  emptyHint,
  showUsage,
  showRaw,
  children,
}: MessageListProps) {
  const box = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const [dismissedAt, setDismissedAt] = useState(-1);

  useLayoutEffect(() => {
    const el = box.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [events.length, busy]);

  // conversa nova (lista esvaziou): volta a acompanhar o fim
  useEffect(() => {
    if (!events.length) stick.current = true;
  }, [events.length]);

  const needsPrompt = !!initialPrompt && variantOf(events[0]).kind !== "User";
  const callAsText = useMemo(() => !busy && !!onReply && toolCallAsText(lastTurnText(events)), [busy, events, onReply]);

  // resumo do Result só aparece quando não repete o último texto do turno
  const resultIsNew = (index: number): boolean => {
    const r = variantOf(events[index]).data as { summary?: string } | undefined;
    const summary = r?.summary?.trim();
    if (!summary) return false;
    for (let i = index - 1; i >= 0; i--) {
      const { kind, data } = variantOf(events[i]);
      if (kind === "User") break;
      if (kind === "Text") return String(data ?? "").trim() !== summary;
    }
    return true;
  };

  return (
    <div
      className="chat-list"
      ref={box}
      onScroll={(e) => {
        const el = e.currentTarget;
        stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
      }}
    >
      {!events.length && !needsPrompt && !busy && <div className="chat-empty">{emptyHint ?? "Nenhuma mensagem ainda."}</div>}
      {needsPrompt && <MessageView event={{ User: initialPrompt! }} />}
      {events.map((ev, i) => (
        <MessageView
          key={i}
          event={ev}
          showResultSummary={"Result" in ev && resultIsNew(i)}
          showUsage={showUsage}
          showRaw={showRaw}
          onOpenSubagent={onOpenSubagent}
        />
      ))}
      {busy && <div className="chat-working">trabalhando…</div>}
      {callAsText && dismissedAt !== events.length && (
        <div className="chat-notice warn">
          <pre>O agente escreveu a chamada de ferramenta como texto, então nada foi executado.</pre>
          <button type="button" className="primary" onClick={() => void onReply!(RUN_FOR_REAL_PROMPT)}>
            Executar de verdade
          </button>
          <button type="button" title="Fechar" aria-label="Fechar aviso" onClick={() => setDismissedAt(events.length)}>
            ✕
          </button>
        </div>
      )}
      {children}
    </div>
  );
}
