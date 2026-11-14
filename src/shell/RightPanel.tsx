import { RIGHT_TABS, useApp, useLayout, type RightTab } from "@/store";
import ExplorerView from "@/views/explorer";
import ServersView from "@/views/servers";
import { ChangesPanel, SubagentsPanel } from "@/views/session/panels";
import { Icon, type IconName } from "./icons";
import type { ActiveContext } from "./useActiveContext";

const TAB_ICONS: Record<RightTab, IconName> = {
  files: "file",
  changes: "diff",
  subagents: "tree",
  servers: "server",
};

/**
 * Painel direito, retrátil, sobre a sessão/projeto ativos: Arquivos,
 * Alterações (diff com Aplicar e Descartar), Subagentes e Servidores.
 */
export function RightPanel({ ctx }: { ctx: ActiveContext }) {
  const tab = useLayout((s) => s.rightTab);
  const setTab = useLayout((s) => s.setRightTab);
  const servers = useApp((s) => s.servers.length);

  return (
    <aside className="rp" aria-label="Painel direito">
      <div className="rp-tabs" role="tablist">
        {RIGHT_TABS.map((t) => (
          <button
            key={t.id}
            role="tab"
            aria-selected={tab === t.id}
            className={tab === t.id ? "rp-tab on" : "rp-tab"}
            title={t.label}
            onClick={() => setTab(t.id)}
          >
            <Icon name={TAB_ICONS[t.id]} size={14} className="rp-tab-ic" />
            <span>{t.label}</span>
            {t.id === "servers" && servers > 0 && <span className="rp-count">{servers}</span>}
          </button>
        ))}
      </div>
      <div className="rp-scroll" role="tabpanel">
        {tab === "files" && <ExplorerView />}
        {tab === "changes" && <ChangesPanel taskId={ctx.taskId} project={ctx.project} />}
        {tab === "subagents" && <SubagentsPanel taskId={ctx.taskId} />}
        {tab === "servers" && <ServersView />}
      </div>
    </aside>
  );
}
