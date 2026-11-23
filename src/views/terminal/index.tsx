import { useEffect, useRef, useState } from "react";
import type { CmdSpec, CommandSpec, Path, PtyId } from "@/api";
import { defaultCwd, useLayout, useTerminals, useWorkbench } from "@/store";
import { Btn, Empty } from "@/ui";
import { Icon } from "@/shell/icons";
import { attachSession, detachSession, fitSession, freshTerminalKey, type TerminalSession } from "./session";
import "./terminal.css";

export { disposeSession as disposeTerminal } from "./session";

/**
 * Pedido de instalação por comando inexistente: o backend analisa a saída de
 * todo terminal e emite `pty-missing-command` (uma vez por comando), já com o
 * plano de instalação. Quem consome assina por aqui; o modal global
 * (`InstallPrompt`) é aberto por `startApp`, que já escuta o mesmo evento.
 */
export { onPtyMissingCommand as onTerminalMissingCommand } from "@/api";

export interface TerminalProps {
  cwd: Path | null;
  /** Sem comando, abre o shell padrão. */
  command?: CmdSpec | CommandSpec | null;
  /** Aba visível (para ajustar o tamanho do xterm ao aparecer). */
  active?: boolean;
  /**
   * Identidade do terminal fora do React. Com a mesma chave, o processo
   * continua vivo se o componente for desmontado e montado de novo em seguida
   * (aba arrastada para outro grupo). Sem chave, cada componente tem a sua.
   */
  persistKey?: string;
  /** Avisa quando o processo é criado (id do PTY) e quando termina (null). */
  onSession?: (id: PtyId | null) => void;
}

/**
 * Um terminal (xterm.js ligado a um PTY do backend). Usado no painel inferior
 * e nas abas de terminal do workbench. `cwd` e `command` valem só na criação.
 * O processo é encerrado quando o componente sai da tela de vez.
 */
export function Terminal({ cwd, command = null, active = true, persistKey, onSession }: TerminalProps) {
  const [ownKey] = useState(freshTerminalKey);
  const key = persistKey ?? ownKey;
  const slot = useRef<HTMLDivElement>(null);
  const session = useRef<TerminalSession | null>(null);
  const [error, setError] = useState<string | null>(null);
  const initial = useRef({ cwd, command });
  const onSessionRef = useRef(onSession);
  onSessionRef.current = onSession;

  useEffect(() => {
    const el = slot.current;
    if (!el) return;
    const s = attachSession(key, el, initial.current);
    session.current = s;
    const sync = () => {
      setError(s.error);
      onSessionRef.current?.(s.exited ? null : s.ptyId);
    };
    s.listeners.add(sync);
    sync();
    const observer = new ResizeObserver(() => fitSession(s));
    observer.observe(el);
    return () => {
      observer.disconnect();
      s.listeners.delete(sync);
      session.current = null;
      detachSession(key);
    };
  }, [key]);

  // Ao aparecer, o contêiner passa a ter tamanho: reajusta e foca.
  useEffect(() => {
    const s = session.current;
    if (!active || !s) return;
    const frame = requestAnimationFrame(() => {
      fitSession(s);
      s.term.focus();
    });
    return () => cancelAnimationFrame(frame);
  }, [active, key]);

  return (
    <div className="terminal-root">
      {error && <div className="terminal-err">{error}</div>}
      <div ref={slot} className="terminal-slot" />
    </div>
  );
}

/**
 * Painel inferior de terminais: faixa de abas (`.tabs`) e um painel por aba
 * (`.terms`). Os painéis ficam montados para manter os processos vivos.
 */
export default function TerminalView() {
  const open = useLayout((s) => s.bottomOpen);
  const toggleBottom = useLayout((s) => s.toggleBottom);
  const tabs = useTerminals((s) => s.tabs);
  const active = useTerminals((s) => s.active);
  const openTab = useTerminals((s) => s.open);
  const close = useTerminals((s) => s.close);
  const activate = useTerminals((s) => s.activate);

  return (
    <>
      <div className="tabs" role="tablist" aria-label="Terminais">
        <button className="tab" title="Mostrar/ocultar terminais" aria-expanded={open} onClick={toggleBottom}>
          {open ? "▾ Terminal" : "▸ Terminal"}
        </button>
        {tabs.map((t) => (
          <div
            key={t.id}
            role="tab"
            aria-selected={t.id === active}
            className={t.id === active ? "tab active" : "tab"}
            title={t.cwd ?? t.title}
            onClick={() => activate(t.id)}
            onMouseDown={(e) => {
              // Botão do meio fecha, como nas abas do editor.
              if (e.button === 1) e.preventDefault();
            }}
            onAuxClick={(e) => {
              if (e.button === 1) close(t.id);
            }}
          >
            <span className="tab-title">{t.title}</span>
            <span
              className="close"
              role="button"
              aria-label={`Fechar terminal ${t.title}`}
              title="Fechar terminal"
              onClick={(e) => {
                e.stopPropagation();
                close(t.id);
              }}
            >
              ×
            </span>
          </div>
        ))}
        <button
          className="tab"
          title="Novo terminal"
          aria-label="Novo terminal"
          onClick={() => openTab({ cwd: defaultCwd() })}
        >
          +
        </button>
        <button
          className="tab"
          title="Novo terminal como aba do editor (pode ser arrastado entre os grupos)"
          aria-label="Novo terminal como aba do editor"
          onClick={() => useWorkbench.getState().openTerminal({ cwd: defaultCwd() })}
        >
          ⧉
        </button>
      </div>
      <div className="terms">
        {tabs.length === 0 && (
          /*
           * O texto antigo mandava "clicar em +". Na área Terminal em tela
           * cheia esse `+` não aparece — a faixa de abas fica recolhida e o
           * botão se chama "Novo terminal" —, então a instrução apontava para
           * um botão inexistente. Agora a ação é o próprio botão.
           */
          <Empty
            icon={<Icon name="terminal" size={20} />}
            title="Nenhum terminal aberto"
            action={<Btn onClick={() => openTab({ cwd: defaultCwd() })}>Novo terminal</Btn>}
          >
            Um terminal abre na pasta do projeto, com o mesmo ambiente que o agente usa — útil para
            conferir à mão o que ele acabou de rodar.
          </Empty>
        )}
        {tabs.map((t) => (
          <div key={t.id} className="pane" style={{ display: t.id === active ? "block" : "none" }}>
            <Terminal cwd={t.cwd} command={t.command} active={t.id === active && open} persistKey={`bottom-${t.id}`} />
          </div>
        ))}
      </div>
    </>
  );
}
