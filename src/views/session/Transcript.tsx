/**
 * Transcrição de um chat (principal ou de subagente): coluna central, fala do
 * usuário em caixa, resposta em Markdown, ferramentas em linhas recolhidas e,
 * no turno terminado, os passos intermediários agrupados em "N passos".
 */
import { useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import { formatTokens, formatUsd, type Pricing, type TokenUsage } from "@/api";
import { Icon } from "@/shell/icons";
import { MessageView } from "@/views/chat";
import {
  noticeSummary,
  splitTurns,
  stepsCut,
  stepsLabel,
  subagentDuration,
  subagentStatus,
  subagentStatusLabel,
  usageCost,
  type ChatEntry,
  type SubagentNode,
} from "./events";

/** Avisos da CLI (stderr/sistema): discretos e recolhidos, com o texto completo ao expandir. */
function Notices({ lines }: { lines: string[] }) {
  return (
    <details className="wb-notice">
      <summary>
        <Icon name="alert" size={12} /> {noticeSummary(lines[0])}
        {lines.length > 1 && ` (+${lines.length - 1})`}
      </summary>
      <pre>{lines.join("\n")}</pre>
    </details>
  );
}

function UsageLine({ usage, pricing }: { usage: TokenUsage; pricing: Pricing | null }) {
  return (
    <div className="cv-usage">
      {formatTokens(usage.input_tokens)} entrada · {formatTokens(usage.output_tokens)} saída · cache{" "}
      {formatTokens(usage.cache_read)} lido · {formatTokens(usage.cache_write)} gravado
      {usage.model && ` · ${usage.model}`}
      {pricing && ` · ${formatUsd(usageCost(usage, pricing))} equivalente em API`}
    </div>
  );
}

export interface SubagentCardProps {
  node: SubagentNode;
  taskActive: boolean;
  now: number;
  onOpen(id: string): void;
}

/** Linha "subagente X" no chat de quem o criou; clicar abre o chat dele (somente leitura). */
export function SubagentCard({ node, taskActive, now, onOpen }: SubagentCardProps) {
  const status = subagentStatus(node, taskActive);
  const duration = subagentDuration(node, status, now);
  return (
    <button className="ses-sub-card" title="Abrir o chat do subagente (somente leitura)" onClick={() => onOpen(node.id)}>
      <Icon name="bot" size={15} className="ses-sub-arrow" />
      <span className="ses-sub-card-main">
        <span className="ellipsis">
          Subagente <b>{node.description || node.subagent_type || node.id}</b>
        </span>
        <span className="small ellipsis ses-sub-meta">
          {node.subagent_type && node.description ? `${node.subagent_type} · ` : ""}
          <span className={`ses-st ${status}`}>{subagentStatusLabel(status)}</span>
          {duration && ` · ${duration}`}
        </span>
      </span>
      <Icon name="chevronRight" size={14} />
    </button>
  );
}

export interface TranscriptProps {
  entries: ChatEntry[];
  /** Subagentes por id, para os cartões. */
  subagents: Map<string, SubagentNode>;
  taskActive: boolean;
  now: number;
  pricing: Pricing | null;
  onOpenSubagent(id: string): void;
  /** Agrupa os passos dos turnos terminados (desligado no chat de subagente). */
  group?: boolean;
  /** Conteúdo fixo antes e depois das mensagens (prompt do subagente, "trabalhando…"). */
  before?: ReactNode;
  after?: ReactNode;
  /** Muda quando o chat exibido troca: volta a rolagem para o fim. */
  resetKey?: string;
}

export function Transcript(props: TranscriptProps) {
  const { entries, subagents, taskActive, now, pricing, onOpenSubagent, group = true, before, after, resetKey } = props;
  const ref = useRef<HTMLDivElement>(null);
  /** Só acompanha o fim se o usuário não rolou para cima para ler. */
  const stick = useRef(true);

  useEffect(() => {
    stick.current = true;
  }, [resetKey]);

  useLayoutEffect(() => {
    const el = ref.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [entries.length, resetKey, after != null]);

  const onScroll = () => {
    const el = ref.current;
    if (el) stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
  };

  const item = (entry: ChatEntry, key: string): ReactNode => {
    switch (entry.type) {
      case "user":
        return <MessageView key={key} event={{ User: entry.text }} />;
      case "text":
        return <MessageView key={key} event={{ Text: entry.text }} />;
      case "tool":
        return <MessageView key={key} event={{ ToolUse: { name: entry.name, input: entry.input } }} />;
      case "result":
        // Sucesso não precisa de linha própria: a resposta já está acima.
        return entry.ok ? null : (
          <div key={key} className="cv-fail" role="alert">
            <Icon name="alert" size={14} />
            <span>
              <b>Erro: o turno falhou.</b>
              {entry.summary && <span className="cv-fail-text"> {entry.summary}</span>}
            </span>
          </div>
        );
      case "usage":
        return <UsageLine key={key} usage={entry.usage} pricing={pricing} />;
      case "notices":
        return <Notices key={key} lines={entry.lines} />;
      case "subagent": {
        const node = subagents.get(entry.id);
        return node ? <SubagentCard key={key} node={node} taskActive={taskActive} now={now} onOpen={onOpenSubagent} /> : null;
      }
      case "unknown":
        return (
          <div key={key} className="small muted">
            (evento {entry.kind || "desconhecido"})
          </div>
        );
    }
  };

  const turns = splitTurns(entries);

  return (
    <div ref={ref} className="cv-scroll" onScroll={onScroll}>
      <div className="cv-col cv-log">
        {before}
        {turns.map((turn, t) => {
          const finished = t < turns.length - 1 || !taskActive;
          const cut = group && finished ? stepsCut(turn.items) : -1;
          const steps = cut > 0 ? turn.items.slice(0, cut) : [];
          const rest = cut > 0 ? turn.items.slice(cut) : turn.items;
          return (
            <div key={t} className="cv-turn">
              {turn.user != null && <MessageView event={{ User: turn.user }} />}
              {steps.length > 0 && (
                <details className="cv-steps">
                  <summary title="Mostrar o que o agente fez antes de responder">
                    <Icon name="chevronRight" size={12} className="cv-steps-chev" />
                    {stepsLabel(steps)}
                  </summary>
                  <div className="cv-steps-body">{steps.map((e, i) => item(e, `s${i}`))}</div>
                </details>
              )}
              {rest.map((e, i) => item(e, `r${i}`))}
            </div>
          );
        })}
        {after}
      </div>
    </div>
  );
}
