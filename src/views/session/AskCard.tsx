/**
 * Cartão de pergunta: o que a CLI pergunta no meio do turno aparece como
 * opções clicáveis, e não como JSON de chamada de ferramenta. Uma aba por
 * pergunta, opções com marcador (rádio ou caixa), "Outra" com texto livre e
 * "Enviar respostas". A escolha volta para o agente como mensagem do usuário.
 *
 * Teclado (com o foco no cartão): 1 a 9 escolhem a opção, ←/→ trocam de aba,
 * Ctrl+Enter envia.
 */
import { useRef, useState } from "react";
import { Icon } from "@/shell/icons";
import { answerText, type AskQuestion } from "./ask";
import { Dot, TimelineItem } from "./ToolRow";

export interface AskCardProps {
  questions: AskQuestion[];
  /** A pergunta ainda espera resposta. */
  pending: boolean;
  /** Envia a resposta; indefinido deixa o cartão só de leitura. */
  onAnswer?: (text: string) => Promise<void> | void;
  /**
   * Resposta ACP: leva as opções escolhidas (por pergunta) de volta ao
   * protocolo, sem transformar a escolha em nova mensagem do usuário.
   */
  onPick?: (picked: string[][], extras: string[]) => Promise<void> | void;
}

/** Rótulo da aba: o título curto da CLI, ou "Pergunta N". */
const tabLabel = (q: AskQuestion, i: number) => q.header ?? `Pergunta ${i + 1}`;

export function AskCard({ questions, pending, onAnswer, onPick }: AskCardProps) {
  const [picked, setPicked] = useState<string[][]>(() => questions.map(() => []));
  /** Texto de "Outra" por pergunta; `null` = "Outra" desmarcada. */
  const [other, setOther] = useState<(string | null)[]>(() => questions.map(() => null));
  const [tab, setTab] = useState(0);
  const [folded, setFolded] = useState(false);
  const [sending, setSending] = useState(false);
  const live = pending && (!!onAnswer || !!onPick);
  const otherInput = useRef<HTMLInputElement>(null);

  const q = questions[Math.min(tab, questions.length - 1)];
  const qi = questions.indexOf(q);

  const answered = (i: number) => (picked[i]?.length ?? 0) > 0 || !!other[i]?.trim();
  const done = questions.filter((_, i) => answered(i)).length;

  /** Respostas com o texto de "Outra" junto das opções escolhidas. */
  const merged = questions.map((_, i) => {
    const o = other[i]?.trim();
    return o ? [...(picked[i] ?? []), o] : (picked[i] ?? []);
  });
  const text = answerText(questions, merged, "");

  const next = () => {
    const after = questions.findIndex((_, i) => i > qi && !answered(i));
    if (after >= 0) setTab(after);
  };

  const toggle = (label: string) => {
    if (!live || sending) return;
    const single = !q.multi;
    setPicked((prev) =>
      prev.map((sel, i) => {
        if (i !== qi) return sel;
        if (!single) return sel.includes(label) ? sel.filter((l) => l !== label) : [...sel, label];
        return sel.includes(label) ? [] : [label];
      }),
    );
    if (single) {
      setOther((prev) => prev.map((o, i) => (i === qi ? null : o)));
      // Pergunta de uma escolha respondida: segue para a próxima sem resposta.
      if (!picked[qi]?.includes(label)) next();
    }
  };

  const toggleOther = () => {
    if (!live || sending) return;
    const on = other[qi] != null;
    setOther((prev) => prev.map((o, i) => (i === qi ? (on ? null : "") : o)));
    if (!on) {
      if (!q.multi) setPicked((prev) => prev.map((sel, i) => (i === qi ? [] : sel)));
      requestAnimationFrame(() => otherInput.current?.focus());
    }
  };

  const send = async () => {
    const chosen = merged.filter((g) => g.length > 0);
    if (sending || (!onPick && (!onAnswer || !text)) || (onPick && chosen.length === 0)) return;
    setSending(true);
    try {
      if (onPick) await onPick(picked, other.map((o) => o?.trim() ?? ""));
      else await onAnswer?.(text);
    } finally {
      setSending(false);
    }
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (!live) return;
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void send();
      return;
    }
    // Digitando em "Outra": as teclas são do texto.
    if (e.target instanceof HTMLInputElement) return;
    if (e.key === "ArrowRight" && qi < questions.length - 1) setTab(qi + 1);
    else if (e.key === "ArrowLeft" && qi > 0) setTab(qi - 1);
    else if (/^[1-9]$/.test(e.key)) {
      const n = Number(e.key) - 1;
      if (n < q.options.length) toggle(q.options[n].label);
      else if (n === q.options.length) toggleOther();
    } else return;
    e.preventDefault();
  };

  const otherOn = other[qi] != null;

  return (
    <TimelineItem dot={<Dot status={live ? "warn" : "none"} title={live ? "Esperando sua resposta" : "Pergunta respondida"} />}>
      <div className={`ses-ask${live ? " live" : ""}`} role="group" aria-label="Pergunta do agente" tabIndex={live ? 0 : undefined} onKeyDown={onKey}>
        <div className="ses-ask-bar">
          <div className="ses-ask-tabs" role="tablist" aria-label="Perguntas">
            {questions.map((x, i) => (
              <button
                key={i}
                type="button"
                role="tab"
                aria-selected={i === qi}
                className={`ses-ask-tab${i === qi ? " on" : ""}${answered(i) ? " ok" : ""}`}
                onClick={() => {
                  setTab(i);
                  setFolded(false);
                }}
              >
                {answered(i) && <Icon name="check" size={11} />}
                {tabLabel(x, i)}
              </button>
            ))}
          </div>
          {!live && <span className="small muted">já respondida</span>}
          <button
            type="button"
            className="ses-ask-ibtn"
            title={folded ? "Mostrar a pergunta" : "Recolher a pergunta"}
            aria-label={folded ? "Mostrar a pergunta" : "Recolher a pergunta"}
            aria-expanded={!folded}
            onClick={() => setFolded(!folded)}
          >
            <Icon name={folded ? "chevronRight" : "chevronDown"} size={14} />
          </button>
        </div>

        {!folded && (
          <>
            <div className="ses-ask-text" role="tabpanel">
              {q.question}
            </div>
            {q.multi && live && <div className="small muted">Pode escolher mais de uma.</div>}

            <div className="ses-ask-opts" role={q.multi ? "group" : "radiogroup"}>
              {q.options.map((o, n) => {
                const on = picked[qi]?.includes(o.label) ?? false;
                return (
                  <button
                    key={o.label}
                    type="button"
                    role={q.multi ? "checkbox" : "radio"}
                    aria-checked={on}
                    className={`ses-ask-opt${on ? " on" : ""}`}
                    disabled={!live || sending}
                    onClick={() => toggle(o.label)}
                  >
                    <span className={`ses-ask-mark${q.multi ? " box" : ""}`} aria-hidden="true">
                      {on && (q.multi ? <Icon name="check" size={11} /> : <i />)}
                    </span>
                    <span className="ses-ask-opt-body">
                      <span className="ses-ask-opt-label">
                        {live && n < 9 && <kbd className="ses-ask-key">{n + 1}</kbd>}
                        {o.label}
                      </span>
                      {o.description && <span className="ses-ask-opt-desc">{o.description}</span>}
                    </span>
                  </button>
                );
              })}
              {live && (
                <div className={`ses-ask-opt ses-ask-other${otherOn ? " on" : ""}`}>
                  <button
                    type="button"
                    role={q.multi ? "checkbox" : "radio"}
                    aria-checked={otherOn}
                    className="ses-ask-other-btn"
                    disabled={sending}
                    onClick={toggleOther}
                  >
                    <span className={`ses-ask-mark${q.multi ? " box" : ""}`} aria-hidden="true">
                      {otherOn && (q.multi ? <Icon name="check" size={11} /> : <i />)}
                    </span>
                    <span className="ses-ask-opt-label">Outra</span>
                  </button>
                  {otherOn && (
                    <input
                      ref={otherInput}
                      className="ses-ask-extra"
                      value={other[qi] ?? ""}
                      placeholder="Escreva a sua resposta"
                      aria-label="Outra resposta"
                      disabled={sending}
                      onChange={(e) => setOther((prev) => prev.map((o, i) => (i === qi ? e.target.value : o)))}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" && !e.ctrlKey && !e.metaKey) {
                          e.preventDefault();
                          if (done === questions.length) void send();
                          else next();
                        }
                      }}
                    />
                  )}
                </div>
              )}
            </div>
          </>
        )}

        {live && (
          <div className="ses-ask-foot">
            <button type="button" className="primary" disabled={!text || sending} onClick={() => void send()}>
              {sending ? "Enviando…" : done < questions.length ? `Enviar respostas (${done} de ${questions.length})` : "Enviar respostas"}
            </button>
            <span className="small muted">1–9 escolhe · ←/→ troca de pergunta · Ctrl+Enter envia</span>
          </div>
        )}
      </div>
    </TimelineItem>
  );
}
