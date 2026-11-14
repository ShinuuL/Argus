/**
 * Layout da IDE:
 *
 *   barra superior fina
 *   barra esquerda (projetos e sessões) | centro (grupos de abas + terminais) | painel direito
 *   status bar
 *
 * O chat é a aba principal do centro. Cada região monta a tela correspondente
 * de `src/views/<tela>`.
 */
import { useEffect } from "react";
import { useApp, useLayout, useWorkbench } from "@/store";
import { InstallPrompt } from "@/views/deps";
import { Tooltips } from "./Tooltips";
import ProjectsView from "@/views/projects";
import WorkbenchView from "@/views/workbench";
import { BottomPanel } from "./BottomPanel";
import { Icon } from "./icons";
import { RightPanel } from "./RightPanel";
import { StatusBar } from "./StatusBar";
import { TopBar } from "./TopBar";
import { useSyncActiveProject } from "./useActiveContext";
import { useGlobalShortcuts } from "./useGlobalShortcuts";

export function Shell() {
  const error = useApp((s) => s.error);
  const setError = useApp((s) => s.setError);
  const ready = useApp((s) => s.ready);
  const leftOpen = useLayout((s) => s.leftOpen);
  const rightOpen = useLayout((s) => s.rightOpen);
  const ctx = useSyncActiveProject();
  useGlobalShortcuts();

  // O app abre numa conversa nova: o chat é o lugar principal.
  useEffect(() => {
    if (!ready) return;
    const wb = useWorkbench.getState();
    if (wb.groups.every((g) => g.tabs.length === 0)) wb.openNewChat(useApp.getState().selected);
  }, [ready]);

  return (
    <div className="shell">
      {error && (
        <div className="banner" role="alert">
          <Icon name="alert" size={15} />
          <span>
            <b>Erro: </b>
            {error}
          </span>
          <button title="Fechar aviso" aria-label="Fechar aviso" onClick={() => setError(null)}>
            <Icon name="close" size={13} />
          </button>
        </div>
      )}
      <TopBar ctx={ctx} />
      <div className="sh-main">
        {leftOpen && <ProjectsView />}
        <main className="wb-center">
          <div className="wb-area">
            <WorkbenchView />
          </div>
          <BottomPanel />
        </main>
        {rightOpen && <RightPanel ctx={ctx} />}
      </div>
      <StatusBar />
      <InstallPrompt />
      <Tooltips />
    </div>
  );
}
