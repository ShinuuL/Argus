/**
 * Barra superior (.tb do mockup). A janela não tem decorações do sistema:
 *
 *   [logo] Argus / <área> · <contexto>   [Buscar ou executar… Ctrl K]   [terminal] [inspetor] [— □ ✕]
 *
 * Os espaços vazios têm `data-tauri-drag-region` (arrastar e duplo clique para maximizar).
 * Os botões de terminal e inspetor só aparecem na área de conversas.
 */
import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { baseName, isTaskActive } from "@/api";
import { Chip } from "@/ui";
import { sessionTitle } from "@/views/session/events";
import { activeTab, AREAS, tabTitle, useApp, useLayout, useWorkbench } from "@/store";
import { ArgusMark, Icon } from "./icons";
import { useActiveContext } from "./useActiveContext";

/** A janela do Tauri, ou null fora dele (ex.: navegador comum no `vite`). */
function appWindow() {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

function useMaximized(): boolean {
  const [max, setMax] = useState(false);
  useEffect(() => {
    const w = appWindow();
    if (!w) return;
    let alive = true;
    const sync = () => void w.isMaximized().then((m) => alive && setMax(m), () => {});
    sync();
    const off = w.onResized(sync);
    return () => {
      alive = false;
      void off.then((f) => f(), () => {});
    };
  }, []);
  return max;
}

/** Breadcrumb: na conversa, o título da sessão em foco (como no mockup); nas outras áreas, área · projeto. */
function Crumb() {
  const area = useLayout((s) => s.area);
  const kind = useWorkbench((s) => activeTab(s.groups.find((g) => g.id === s.activeGroup))?.kind);
  const ctx = useActiveContext();
  const task = useApp((s) => (ctx.taskId == null ? undefined : s.tasks.find((t) => t.id === ctx.taskId)));
  const label = AREAS.find((a) => a.id === area)?.label ?? area;
  const project = ctx.project ? baseName(ctx.project) : null;
  if (area === "chat") {
    // Aba que não é conversa (Celular, terminal, arquivo...): o nome dela.
    const other = kind && kind.type !== "chat" ? tabTitle(kind) : null;
    const title = other ?? (task ? sessionTitle(task.prompt) : project ? `Nova conversa · ${project}` : "Conversas");
    return (
      <>
        <span className="tb-crumb" data-tauri-drag-region title={task ? `#${task.id} · ${project ?? ""}` : undefined}>
          <b>{title}</b>
        </span>
        {task && isTaskActive(task.state) && <Chip tone="ok">trabalhando</Chip>}
      </>
    );
  }
  return (
    <span className="tb-crumb" data-tauri-drag-region>
      <b>{label}</b>
      {project && <span> · {project}</span>}
    </span>
  );
}

export function TopBar() {
  const area = useLayout((s) => s.area);
  const inspectorOpen = useLayout((s) => s.inspectorOpen);
  const bottomOpen = useLayout((s) => s.bottomOpen);
  const root = useApp((s) => s.root);
  const max = useMaximized();
  const layout = useLayout.getState();
  const win = (fn: (w: NonNullable<ReturnType<typeof appWindow>>) => Promise<void>) => () => {
    const w = appWindow();
    if (w) void fn(w).catch(() => {});
  };

  return (
    <header className="tb" data-tauri-drag-region>
      <ArgusMark className="tb-logo" />
      <span className="tb-app" data-tauri-drag-region title={root ?? undefined}>
        Argus
      </span>
      <span className="tb-sep" aria-hidden="true">/</span>
      <Crumb />
      <span className="tb-sp" data-tauri-drag-region />
      <button className="tb-search" onClick={layout.openPalette} title="Buscar ou executar (Ctrl+K)">
        <Icon name="search" size={13} />
        <span className="tb-search-t">Buscar ou executar…</span>
        <span className="kbd">Ctrl K</span>
      </button>
      <span className="tb-sp" data-tauri-drag-region />
      {/* Fora de Conversas os botões ficam reservados (invisíveis) para a busca não mudar de lugar. */}
      <span className="tb-chat-ctl" style={area === "chat" ? undefined : { visibility: "hidden" }} aria-hidden={area !== "chat"}>
        <>
          <button
            className="tb-btn"
            tabIndex={area === "chat" ? undefined : -1}
            aria-pressed={bottomOpen}
            title={bottomOpen ? "Ocultar o terminal (Ctrl+J)" : "Mostrar o terminal (Ctrl+J)"}
            aria-label="Alternar o terminal"
            onClick={layout.toggleBottom}
          >
            <Icon name="terminal" size={15} />
          </button>
          <button
            className="tb-btn"
            tabIndex={area === "chat" ? undefined : -1}
            aria-pressed={inspectorOpen}
            title={inspectorOpen ? "Ocultar o inspetor (Ctrl+.)" : "Mostrar o inspetor: alterações, arquivos e agentes (Ctrl+.)"}
            aria-label="Alternar o inspetor"
            onClick={layout.toggleInspector}
          >
            <Icon name="sidebarRight" size={15} />
          </button>
        </>
      </span>
      <div className="tb-win" role="group" aria-label="Janela">
        <button className="tb-wbtn" title="Minimizar" aria-label="Minimizar" onClick={win((w) => w.minimize())}>
          <Icon name="minus" size={15} />
        </button>
        <button
          className="tb-wbtn"
          title={max ? "Restaurar" : "Maximizar"}
          aria-label={max ? "Restaurar" : "Maximizar"}
          onClick={win((w) => w.toggleMaximize())}
        >
          <Icon name={max ? "restore" : "maximize"} size={12} />
        </button>
        <button className="tb-wbtn close" title="Fechar" aria-label="Fechar" onClick={win((w) => w.close())}>
          <Icon name="close" size={15} />
        </button>
      </div>
    </header>
  );
}
