/**
 * Cartão de pergunta: o que a CLI pergunta no meio do turno aparece como
 * opções clicáveis, e não como JSON de chamada de ferramenta. A escolha volta
 * para o agente como mensagem do usuário.
 */
import { useState } from "react";
import { Icon } from "@/shell/icons";
import { answerText, type AskQuestion } from "./ask";
import { Dot, TimelineItem } from "./ToolRow";

export interface AskCardProps {
  questions: AskQuestion[];
  /** A pergunta ainda espera resposta (sem saída da ferramenta). */
  pending: boolean;
  /** Envia a resposta; indefinido deixa o cartão só de leitura. */
  onAnswer?: (text: string) => Promise<void> | void;
}

export function AskCard({ questions, pending, onAnswer }: AskCardProps) {
  const [picked, setPicked] = useState<string[][]>(() => questions.map(() => []));
  const [extra, setExtra] = useState("");
  const [sending, setSending] = useState(false);
  const live = pending && !!onAnswer;

  const toggle = (qi: number, label: string) =>
    setPicked((prev) =>
      prev.map((sel, i) => {
        if (i !== qi) return sel;
        if (questions[i].multi) return sel.includes(label) ? sel.filter((l) => l !== label) : [...sel, label];
        return sel.includes(label) ? [] : [label];
      }),
    );

  const text = answerText(questions, picked, extra);
  const send = async () => {
    if (!onAnswer || !text) return;
    setSending(true);
    try {
      await onAnswer(text);
    } finally {
      setSending(false);
    }
  };

  return (
    <TimelineItem dot={<Dot status={live ? "warn" : "none"} title={live ? "Esperando sua resposta" : "Pergunta respondida"} />}>
      <div className={`ses-ask${live ? " live" : ""}`} role={live ? "group" : undefined}>
        <div className="ses-ask-head">
          <Icon name="chat" size={13} />
          <b>{live ? "O agente perguntou" : "Pergunta do agente"}</b>
          {!live && <span className="small muted">já respondida</span>}
        </div>

        {questions.map((q, qi) => (
          <div key={qi} className="ses-ask-q">
            {q.header && <div className="ses-ask-header">{q.header}</div>}
            <div className="ses-ask-text">{q.question}</div>
            {q.multi && live && <div className="small muted">Pode escolher mais de uma.</div>}
            <div className="ses-ask-opts">
              {q.options.map((o) => {
                const on = picked[qi]?.includes(o.label);
                return (
                  <button
                    key={o.label}
                    type="button"
                    className={`ses-ask-opt${on ? " on" : ""}`}
                    disabled={!live || sending}
                    aria-pressed={on}
                    onClick={() => toggle(qi, o.label)}
                  >
                    <span className="ses-ask-opt-label">{o.label}</span>
                    {o.description && <span className="ses-ask-opt-desc">{o.description}</span>}
                  </button>
                );
              })}
            </div>
          </div>
        ))}

        {live && (
          <div className="ses-ask-foot">
            <input
              className="ses-ask-extra"
              value={extra}
              placeholder="Outra resposta ou observação (opcional)"
              disabled={sending}
              onChange={(e) => setExtra(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && text) void send();
              }}
            />
            <button type="button" className="primary" disabled={!text || sending} onClick={() => void send()}>
              {sending ? "Enviando…" : "Responder"}
            </button>
          </div>
        )}
      </div>
    </TimelineItem>
  );
}
