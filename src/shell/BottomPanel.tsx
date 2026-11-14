import { useLayout } from "@/store";
import TerminalView from "@/views/terminal";

/**
 * Painel inferior. A tela de terminal desenha a própria faixa de abas
 * (classe `.tabs`) e os painéis (`.terms`); recolhido, só a faixa aparece.
 * Ela fica sempre montada para manter os processos vivos.
 */
export function BottomPanel() {
  const open = useLayout((s) => s.bottomOpen);
  return (
    <section className={open ? "bottom" : "bottom collapsed"} aria-label="Terminais">
      <TerminalView />
    </section>
  );
}
