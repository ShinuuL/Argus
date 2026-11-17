import { useEffect, useState } from "react";
import { api, type Path, type TaskId } from "@/api";
import { RIGHT_TABS, useApp, useLayout } from "@/store";
import ExplorerView from "@/views/explorer";
import { AgentsPanel } from "@/views/agentmap/AgentsPanel";
import { ChangesPanel, diffFiles } from "@/views/session/panels";
import { Tab, Tabs } from "@/ui";
import { Icon } from "./icons";
import type { ActiveContext } from "./useActiveContext";


/** Nº de arquivos alterados: o diff da sessão ou, sem sessão, o status git do projeto. */
function useChangeCount(taskId: TaskId | null, project: Path | null): number {
  const state = useApp((s) => (taskId == null ? undefined : s.tasks.find((t) => t.id === taskId)?.state));
  const projectCount = useApp((s) => (project ? (s.statuses[project]?.changed_files.length ?? 0) : 0));
  const rev = useLayout((s) => s.diffRev);
  const [count, setCount] = useState(0);
  const ended = state === "Applied" || state === "Discarded";
  useEffect(() => {
    setCount(0);
    if (taskId == null || ended) return;
    let alive = true;
    api.taskDiff(taskId).then(
      (d) => alive && setCount(diffFiles(d).length),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [taskId, state, rev, ended]);
  return taskId == null ? projectCount : count;
}

/**
 * Inspetor da área de conversas (Ctrl+.): Alterações (diff com Aplicar e
 * Descartar), Arquivos e Agentes. "Tela cheia" abre a área Agentes.
 * Servidores virou área própria (barra de ícones).
 */
export function RightPanel({ ctx }: { ctx: ActiveContext }) {
  const tab = useLayout((s) => s.rightTab);
  const setTab = useLayout((s) => s.setRightTab);
  const changes = useChangeCount(ctx.taskId, ctx.project);

  return (
    <aside className="rp" aria-label="Inspetor">
      <Tabs label="Inspetor">
        {RIGHT_TABS.map((t) => (
          <Tab key={t.id} on={tab === t.id} title={t.label} onClick={() => setTab(t.id)} count={t.id === "changes" ? changes : undefined}>
            {t.label}
          </Tab>
        ))}
        {tab === "agents" && (
          <button className="tb-btn rp-full" title="Abrir agentes em tela cheia" aria-label="Abrir agentes em tela cheia" onClick={() => useLayout.getState().setArea("agents")}>
            <Icon name="expand" size={13} />
          </button>
        )}
      </Tabs>
      <div className="rp-scroll" role="tabpanel">
        {tab === "files" && <ExplorerView />}
        {tab === "changes" && <ChangesPanel taskId={ctx.taskId} project={ctx.project} />}
        {tab === "agents" && <AgentsPanel taskId={ctx.taskId} />}
      </div>
    </aside>
  );
}
