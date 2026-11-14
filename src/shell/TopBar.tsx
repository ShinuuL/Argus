import { baseName } from "@/api";
import { activeTab, tabTitle, useApp, useLayout, useWorkbench } from "@/store";
import { sessionTitle } from "@/views/session/events";
import { Icon } from "./icons";
import type { ActiveContext } from "./useActiveContext";

/** Barra superior fina: alternar a barra esquerda, o que está em foco, e alternar terminal e painel direito. */
export function TopBar({ ctx }: { ctx: ActiveContext }) {
  const leftOpen = useLayout((s) => s.leftOpen);
  const rightOpen = useLayout((s) => s.rightOpen);
  const bottomOpen = useLayout((s) => s.bottomOpen);
  const kind = useWorkbench((s) => activeTab(s.groups.find((g) => g.id === s.activeGroup))?.kind);
  const task = useApp((s) => (ctx.taskId == null ? undefined : s.tasks.find((t) => t.id === ctx.taskId)));
  const layout = useLayout.getState();

  const what = !kind
    ? null
    : kind.type === "chat"
      ? task
        ? sessionTitle(task.prompt, 90)
        : "Nova conversa"
      : tabTitle(kind);

  return (
    <header className="tb">
      <button
        className="tb-btn"
        aria-pressed={leftOpen}
        title={leftOpen ? "Ocultar a barra de projetos (Ctrl+B)" : "Mostrar a barra de projetos (Ctrl+B)"}
        aria-label="Alternar a barra de projetos"
        onClick={layout.toggleLeft}
      >
        <Icon name="sidebarLeft" size={16} />
      </button>
      <div className="tb-crumbs" title={task?.prompt}>
        <span className="tb-project">{ctx.project ? baseName(ctx.project) : "Argus"}</span>
        {what && (
          <>
            <Icon name="chevronRight" size={12} />
            <span className="tb-what ellipsis">{what}</span>
          </>
        )}
      </div>
      <button
        className="tb-btn"
        aria-pressed={bottomOpen}
        title={bottomOpen ? "Ocultar o terminal (Ctrl+J)" : "Mostrar o terminal (Ctrl+J)"}
        aria-label="Alternar o terminal"
        onClick={layout.toggleBottom}
      >
        <Icon name="terminal" size={16} />
        <span>Terminal</span>
      </button>
      <button
        className="tb-btn"
        aria-pressed={rightOpen}
        title={rightOpen ? "Ocultar o painel direito" : "Mostrar o painel direito (arquivos, alterações, subagentes, servidores)"}
        aria-label="Alternar o painel direito"
        onClick={layout.toggleRight}
      >
        <Icon name="sidebarRight" size={16} />
      </button>
    </header>
  );
}
