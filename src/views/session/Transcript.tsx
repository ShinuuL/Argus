/**
 * Transcrição de um chat (principal ou de subagente): coluna central, fala do
 * usuário em caixa, resposta em Markdown, ferramentas em linhas recolhidas e,
 * no turno terminado, os passos intermediários agrupados em "N passos".
 */
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { api, errorMessage, formatTokens, formatUsd, type Path, type Pricing, type TokenUsage } from "@/api";
import { Icon } from "@/shell/icons";
import { MessageView } from "@/views/chat";
import { Dot, ThinkingRow, TimelineItem, ToolRow } from "./ToolRow";
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

/**
 * Ferramenta negada por falta de permissão: diz qual comando foi negado e
 * oferece liberá-lo no projeto (entra em "Comandos permitidos ao agente").
 */
function DeniedLine({ tool, command, project }: { tool: string; command: string; project: Path | null }) {
  const [state, setState] = useState<"idle" | "saving" | "done">("idle");
  const [error, setError] = useState<string | null>(null);
  const shell = tool === "Bash" || tool === "PowerShell";
  const canAllow = shell && !!command && !command.includes("\n") && project != null;
  const allow = async () => {
    if (!project) return;
    setState("saving");
    try {
      await api.permissionsAllowDenied(project, tool, command);
      setState("done");
      setError(null);
    } catch (e) {
      setState("idle");
      setError(errorMessage(e));
    }
  };
  return (
    <TimelineItem dot={<Dot status="warn" />}>
    <div className="cv-denied" role="status">
      <Icon name="shield" size={14} />
      <span className="cv-denied-text">
        <b>{shell ? "Comando negado" : `Ferramenta negada: ${tool}`}</b>
        {command && <code>{command}</code>}
        <span className="cv-denied-why">
          {state === "done"
            ? "Liberado neste projeto. Vale a partir do próximo turno: peça de novo."
            : shell
              ? "Não está entre os comandos liberados ao agente."
              : "O agente não tem permissão para isso nesta pasta."}
          {error && <span className="err"> {error}</span>}
        </span>
      </span>
      {canAllow && state !== "done" && (
        <button
          type="button"
          disabled={state === "saving"}
          title="Acrescenta este comando exato aos comandos permitidos do projeto (aba Configurações)"
          onClick={() => void allow()}
        >
          {state === "saving" ? "Liberando…" : "Liberar este comando neste projeto"}
        </button>
      )}
    </div>
    </TimelineItem>
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
    <TimelineItem dot={<Dot status={status === "interrupted" ? "none" : status} title={subagentStatusLabel(status)} />}>
      <button type="button" className="tl-row ses-sub-card" title="Abrir o chat do subagente (somente leitura)" onClick={() => onOpen(node.id)}>
        <Icon name="bot" size={13} className="tl-row-ic" />
        <b>{node.subagent_type || "Agent"}</b>
        <span className="tl-row-sum ses-sub-desc">{node.description || node.id}</span>
        <span className="ses-sub-meta">
          <span className={`ses-st ${status}`}>{subagentStatusLabel(status)}</span>
          {duration && ` · ${duration}`}
        </span>
        <Icon name="chevronRight" size={12} className="tl-chev" />
      </button>
    </TimelineItem>
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
  /** Nome de exibição de um provedor (para a divisória de troca de agente). */
  providerName?(id: string): string;
  /** Projeto da conversa (para liberar um comando negado). */
  project?: Path | null;
}

export function Transcript(props: TranscriptProps) {
  const { entries, subagents, taskActive, now, pricing, onOpenSubagent, group = true, before, after, resetKey } = props;
  const nameOf = props.providerName ?? ((id: string) => id);
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

  /** `live` = o turno do item ainda roda; `last` = é o último item do turno. */
  const item = (entry: ChatEntry, key: string, live: boolean, last: boolean): ReactNode => {
    switch (entry.type) {
      case "user":
        return <MessageView key={key} event={{ User: entry.text }} />;
      case "text":
        return (
          <TimelineItem key={key} dot={<Dot status="text" />} className="tl-text">
            <MessageView event={{ Text: entry.text }} />
          </TimelineItem>
        );
      case "thinking":
        return <ThinkingRow key={key} text={entry.text} live={live && last} />;
      case "tool":
        return <ToolRow key={key} name={entry.name} input={entry.input} result={entry.result} live={live} base={props.project ?? null} />;
      case "result":
        // Sucesso não precisa de linha própria: a resposta já está acima.
        return entry.ok ? null : (
          <TimelineItem key={key} dot={<Dot status="error" />}>
            <div className="cv-fail" role="alert">
              <Icon name="alert" size={14} />
              <span>
                <b>Erro: o turno falhou.</b>
                {entry.summary && <span className="cv-fail-text"> {entry.summary}</span>}
              </span>
            </div>
          </TimelineItem>
        );
      case "usage":
        return (
          <TimelineItem key={key} dot={null} className="tl-plain">
            <UsageLine usage={entry.usage} pricing={pricing} />
          </TimelineItem>
        );
      case "notices":
        return (
          <TimelineItem key={key} dot={null} className="tl-plain">
            <Notices lines={entry.lines} />
          </TimelineItem>
        );
      case "subagent": {
        const node = subagents.get(entry.id);
        return node ? <SubagentCard key={key} node={node} taskActive={taskActive} now={now} onOpen={onOpenSubagent} /> : null;
      }
      case "denied":
        return <DeniedLine key={key} tool={entry.tool} command={entry.command} project={props.project ?? null} />;
      case "switch":
        return (
          <div key={key} className="cv-switch" role="separator" title="A partir daqui, outro agente atende esta conversa. Ele recebeu um resumo do que veio antes.">
            <span>
              Agente trocado: {nameOf(entry.from)} → {nameOf(entry.to)}
            </span>
          </div>
        );
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
          const live = !finished;
          const cut = group && finished ? stepsCut(turn.items) : -1;
          const steps = cut > 0 ? turn.items.slice(0, cut) : [];
          const rest = cut > 0 ? turn.items.slice(cut) : turn.items;
          return (
            <div key={t} className="cv-turn">
              {turn.user != null && <MessageView event={{ User: turn.user }} />}
              {(steps.length > 0 || rest.length > 0) && (
                <div className="tl">
                  {steps.length > 0 && (
                    <TimelineItem dot={<Dot status="text" />} className="tl-steps">
                      <details className="cv-steps">
                        <summary title="Mostrar o que o agente fez antes de responder">
                          {stepsLabel(steps)}
                          <Icon name="chevronRight" size={12} className="cv-steps-chev" />
                        </summary>
                        <div className="cv-steps-body tl">{steps.map((e, i) => item(e, `s${i}`, false, false))}</div>
                      </details>
                    </TimelineItem>
                  )}
                  {rest.map((e, i) => item(e, `r${i}`, live, i === rest.length - 1))}
                </div>
              )}
            </div>
          );
        })}
        {after}
      </div>
    </div>
  );
}
