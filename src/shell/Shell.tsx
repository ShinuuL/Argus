/**
 * Layout do app (docs/mockups/argus-telas.html):
 *
 *   barra superior (.tb)
 *   barra de ícones (48px) | área
 *   barra de status (.sb)
 *
 * A área "chat" (lista de sessões | workbench + terminal inferior | inspetor)
 * fica SEMPRE montada (escondida nas outras áreas) para preservar abas,
 * editores e processos dos terminais. Na área "terminal" o mesmo painel
 * inferior é expandido em tela cheia. As demais áreas montam
 * src/areas/<area>/index.tsx.
 */
import { useEffect, type ComponentType } from "react";
import { useApp, useLayout, useWorkbench, type Area } from "@/store";
import { InstallPrompt } from "@/views/deps";
import ProjectsView from "@/views/projects";
import WorkbenchView from "@/views/workbench";
import HomeArea from "@/areas/home";
import GitArea from "@/areas/git";
import ServersArea from "@/areas/servers";
import DevicesArea from "@/areas/devices";
import MetricsArea from "@/areas/metrics";
import TerminalArea from "@/areas/terminal";
import SettingsArea from "@/areas/settings";
import AgentsArea from "@/areas/agents";
import { Tooltips } from "./Tooltips";
import { BottomPanel } from "./BottomPanel";
import { CommandPalette } from "./CommandPalette";
import { Icon } from "./icons";
import { NavRail } from "./NavRail";
import { RightPanel } from "./RightPanel";
import { StatusBar } from "./StatusBar";
import { TopBar } from "./TopBar";
import { useSyncActiveProject } from "./useActiveContext";
import { useGlobalShortcuts } from "./useGlobalShortcuts";

/** Áreas em tela cheia ("chat" e "terminal" usam o layout da conversa). */
const FULL: Partial<Record<Area, ComponentType>> = {
  home: HomeArea,
  git: GitArea,
  servers: ServersArea,
  devices: DevicesArea,
  metrics: MetricsArea,
  settings: SettingsArea,
  agents: AgentsArea,
};

export function Shell() {
  const error = useApp((s) => s.error);
  const setError = useApp((s) => s.setError);
  const ready = useApp((s) => s.ready);
  const area = useLayout((s) => s.area);
  const leftOpen = useLayout((s) => s.leftOpen);
  const inspectorOpen = useLayout((s) => s.inspectorOpen);
  const ctx = useSyncActiveProject();
  useGlobalShortcuts();

  // O app abre numa conversa nova: o chat é o lugar principal.
  useEffect(() => {
    if (!ready) return;
    const wb = useWorkbench.getState();
    if (wb.groups.every((g) => g.tabs.length === 0)) wb.openNewChat(useApp.getState().selected);
  }, [ready]);

  const Full = FULL[area];
  const chatVisible = area === "chat" || area === "terminal";
  const termFull = area === "terminal";

  return (
    <div className="shell">
      <TopBar />
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
      <div className="sh-body">
        <NavRail />
        <div className={termFull ? "sh-chat term-full" : "sh-chat"} hidden={!chatVisible}>
          {termFull && <TerminalArea />}
          <div className="sh-main">
            {leftOpen && !termFull && <ProjectsView />}
            <main className="wb-center">
              <div className="wb-area">
                <WorkbenchView />
              </div>
              <BottomPanel />
            </main>
            {inspectorOpen && !termFull && <RightPanel ctx={ctx} />}
          </div>
        </div>
        {Full && <Full />}
      </div>
      <StatusBar />
      <CommandPalette />
      <InstallPrompt />
      <Tooltips />
    </div>
  );
}
