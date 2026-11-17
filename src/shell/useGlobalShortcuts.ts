import { useEffect } from "react";
import { useApp, useLayout, useWorkbench } from "@/store";

/**
 * Atalhos globais, capturados antes do Monaco e do xterm:
 * Ctrl+Tab / Ctrl+Shift+Tab (alternar abas), Ctrl+W (fechar), Ctrl+\ (dividir).
 * Ctrl+K (paleta). Fora de um terminal: Ctrl+. (inspetor), Ctrl+B (barra de projetos), Ctrl+J (terminal) e Ctrl+N (nova conversa).
 */
export function useGlobalShortcuts(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.altKey || e.metaKey) return;
      const wb = useWorkbench.getState();
      const inTerminal = (e.target as HTMLElement | null)?.closest?.(".xterm") != null;
      const key = e.key.toLowerCase();
      let action: (() => void) | null = null;
      if (!e.shiftKey && key === "k") action = () => useLayout.getState().togglePalette();
      else if (!inTerminal && !e.shiftKey && (e.key === "." || e.code === "Period"))
        action = () => {
          const l = useLayout.getState();
          if (l.area !== "chat") (l.setArea("chat"), l.setInspectorOpen(true));
          else l.toggleInspector();
        };
      else if (e.key === "Tab") action = () => wb.cycle(e.shiftKey);
      else if (key === "w") action = () => void wb.closeActive();
      else if (e.key === "\\" || e.code === "Backslash" || e.code === "IntlBackslash") action = () => wb.splitActive();
      else if (!inTerminal && !e.shiftKey && key === "b") action = () => useLayout.getState().toggleLeft();
      else if (!inTerminal && !e.shiftKey && key === "j") action = () => useLayout.getState().toggleBottom();
      else if (!inTerminal && !e.shiftKey && key === "n") action = () => wb.openNewChat(useApp.getState().selected);
      if (!action) return;
      e.preventDefault();
      e.stopPropagation();
      action();
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, []);
}
