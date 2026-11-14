/**
 * Pergunta feita ao cancelar ou descartar uma sessão que tem servidores
 * rodando: só parar, parar e derrubar os servidores, ou voltar.
 */
import { useEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import { api, type TaskId } from "@/api";
import { useApp } from "@/store";
import "./servers.css";

export type StopChoice = "keep" | "kill" | "back";

/** O que o usuário está fazendo com a sessão; muda só os textos. */
export type StopAction = "cancel" | "discard";

const TEXTS: Record<StopAction, { title: string; keep: string; kill: string }> = {
  cancel: { title: "Cancelar o turno", keep: "Só parar o turno", kill: "Parar e derrubar servidores" },
  discard: { title: "Descartar a sessão", keep: "Só descartar", kill: "Descartar e derrubar servidores" },
};

interface DialogProps {
  taskId: TaskId;
  count: number;
  action: StopAction;
  onChoose(choice: StopChoice): void;
}

function StopDialog({ taskId, count, action, onChoose }: DialogProps) {
  const texts = TEXTS[action];
  const backRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    backRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onChoose("back");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onChoose]);

  return (
    <div className="srv-ask-back" onMouseDown={(e) => e.target === e.currentTarget && onChoose("back")}>
      <div className="srv-ask" role="alertdialog" aria-modal="true" aria-labelledby="srv-ask-title">
        <b id="srv-ask-title">{texts.title}</b>
        <p>
          A sessão #{taskId} tem {count === 1 ? "1 servidor rodando" : `${count} servidores rodando`}. O que fazer com{" "}
          {count === 1 ? "ele" : "eles"}?
        </p>
        <div className="srv-ask-actions">
          <button ref={backRef} onClick={() => onChoose("back")}>
            Voltar
          </button>
          <button onClick={() => onChoose("keep")}>{texts.keep}</button>
          <button className="srv-danger" onClick={() => onChoose("kill")}>
            {texts.kill}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * Devolve a escolha do usuário. Sem servidores na sessão, devolve "keep" sem
 * perguntar. Não executa nada: quem chama cancela/descarta e, se a escolha for
 * "kill", chama `stopSessionServers`.
 */
export function askStopSession(taskId: TaskId, action: StopAction): Promise<StopChoice> {
  const count = useApp
    .getState()
    .servers.filter((s) => s.owner.kind === "Session" && s.owner.id === taskId).length;
  if (count === 0) return Promise.resolve("keep");

  return new Promise((resolve) => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);
    const choose = (choice: StopChoice) => {
      root.unmount();
      host.remove();
      resolve(choice);
    };
    root.render(<StopDialog taskId={taskId} count={count} action={action} onChoose={choose} />);
  });
}

/** Derruba todos os servidores da sessão. */
export function stopSessionServers(taskId: TaskId): Promise<number> {
  return api.serversStopAll({ kind: "Session", id: taskId });
}
