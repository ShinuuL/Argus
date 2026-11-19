import { BypassBadge } from "@/views/session/pills";
import { Fragment, useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type DragEvent, type MouseEvent, type PointerEvent } from "react";
import { languageFor, onProjectChanged, samePath, type Path, type Task } from "@/api";
import { Icon, type IconName } from "@/shell/icons";
import {
  activeTab,
  MAX_GROUPS,
  PAGES,
  tabTitle,
  tabTooltip,
  useWorkbench,
  useApp,
  type Group,
  type PageId,
  type TabKind,
  type WbTab,
} from "@/store";
import BuildsView from "@/views/builds";
import ConnectionsView from "@/views/connections";
import DepsView from "@/views/deps";
import DevicesView from "@/views/devices";
import MetricsView from "@/views/metrics";
import MirrorTab from "@/views/mirror";
import RemoteView from "@/views/remote";
import ChatTab from "@/views/session";
import AgentMap from "@/views/agentmap";
import { sessionTitle } from "@/views/session/events";
import { AgentState } from "@/views/session/AgentState";
import DiffTab from "@/views/session/DiffTab";
import SettingsView from "@/views/settings";
import { Terminal } from "@/views/terminal";
import { Dashboard } from "./Dashboard";
import { GroupEditor } from "./GroupEditor";
import { whenMonacoLoaded } from "./lazy";
import "./workbench.css";

// ------------------------------------------------------------------- ícones

/** Sigla do tipo de arquivo mostrada na aba. */
const FILE_ICONS: Record<string, string> = {
  rust: "RS",
  javascript: "JS",
  typescript: "TS",
  json: "{}",
  ini: "⚙",
  python: "PY",
  go: "GO",
  markdown: "M↓",
  html: "<>",
  css: "#",
  scss: "#",
  java: "J",
  kotlin: "KT",
  csharp: "C#",
  sql: "DB",
  shell: "$",
  powershell: "$",
  yaml: "≡",
  xml: "≡",
};

const PAGE_ICONS: Record<PageId, IconName> = {
  deps: "box",
  builds: "hammer",
  connections: "globe",
  devices: "devices",
  metrics: "chart",
  remote: "phone",
  settings: "gear",
};

function TabIcon({ kind, task }: { kind: TabKind; task: Task | undefined }) {
  switch (kind.type) {
    case "file":
      return (
        <span className="wb-ic" aria-hidden="true">
          {FILE_ICONS[languageFor(kind.path)] ?? "□"}
        </span>
      );
    case "chat":
      return task ? <AgentState task={task} size={13} /> : <Icon name="chat" size={14} className="wb-ic-svg" />;
    case "terminal":
      return <Icon name="terminal" size={14} className="wb-ic-svg" />;
    case "dashboard":
      return <Icon name="grid" size={14} className="wb-ic-svg" />;
    case "agentmap":
      return <Icon name="tree" size={14} className="wb-ic-svg" />;
    case "diff":
      return <Icon name="diff" size={14} className="wb-ic-svg" />;
    case "mirror":
      return <Icon name="phone" size={14} className="wb-ic-svg" />;
    case "page":
      return <Icon name={PAGE_ICONS[kind.page] ?? "gear"} size={14} className="wb-ic-svg" />;
  }
}

/** Título da aba: a conversa usa o primeiro pedido da tarefa. */
function titleOf(kind: TabKind, task: Task | undefined): string {
  return kind.type === "chat" && task ? sessionTitle(task.prompt, 32) : tabTitle(kind);
}

// -------------------------------------------------------------- ações de aba

/** Solta a aba arrastada no grupo `gid` (antes de `before`, ou no fim). */
function dropTab(gid: number, before: number | null): void {
  const s = useWorkbench.getState();
  if (s.drag == null) return;
  const tab = s.drag;
  s.setDrag(null);
  s.moveTab(tab, gid, before);
}

/** A aba arrastada pode virar um grupo novo? (há vaga e ela não está sozinha no grupo dela) */
function canDropAsNewGroup(groups: Group[], tab: number | null): boolean {
  if (tab == null || groups.length >= MAX_GROUPS) return false;
  const src = groups.find((g) => g.tabs.some((t) => t.id === tab));
  return !!src && src.tabs.length > 1;
}

/** Solta a aba arrastada num grupo novo, criado logo depois de `afterGid`. */
function dropTabAsNewGroup(afterGid: number): void {
  const s = useWorkbench.getState();
  const tab = s.drag;
  if (tab == null) return;
  if (!canDropAsNewGroup(s.groups, tab)) return dropTab(afterGid, null);
  const gid = s.nextId;
  useWorkbench.setState((st) => {
    const gi = st.groups.findIndex((g) => g.id === afterGid);
    const at = gi < 0 ? st.groups.length : gi + 1;
    const group: Group = { id: gid, tabs: [], active: null };
    return { groups: [...st.groups.slice(0, at), group, ...st.groups.slice(at)], nextId: st.nextId + 1, drag: null };
  });
  useWorkbench.getState().moveTab(tab, gid, null);
}

async function closeAll(gid: number): Promise<void> {
  const ids = (useWorkbench.getState().groups.find((g) => g.id === gid)?.tabs ?? []).map((t) => t.id);
  for (const id of ids) {
    if (!(await useWorkbench.getState().requestCloseTab(gid, id))) break;
  }
}

// ---------------------------------------------------------- menu de contexto

interface MenuState {
  x: number;
  y: number;
  tab: WbTab;
}

function TabMenu({ gid, menu, onClose }: { gid: number; menu: MenuState; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: menu.x, top: menu.y });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    // Mantém o menu dentro da janela.
    setPos({
      left: Math.max(4, Math.min(menu.x, window.innerWidth - el.offsetWidth - 4)),
      top: Math.max(4, Math.min(menu.y, window.innerHeight - el.offsetHeight - 4)),
    });
    el.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }, [menu.x, menu.y]);

  useEffect(() => {
    const onDown = (e: globalThis.MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDown, true);
    document.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    return () => {
      document.removeEventListener("mousedown", onDown, true);
      document.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  const wb = useWorkbench.getState();
  const group = wb.groups.find((g) => g.id === gid);
  const run = (action: () => void) => () => {
    onClose();
    action();
  };
  const onArrows = (e: React.KeyboardEvent) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const items = [...(ref.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = e.key === "ArrowDown" ? (i + 1) % items.length : (i + items.length - 1) % items.length;
    items[next]?.focus();
  };

  return (
    <div ref={ref} className="wb-menu" role="menu" style={pos} onKeyDown={onArrows}>
      <button role="menuitem" onClick={run(() => void wb.requestCloseTab(gid, menu.tab.id))}>
        <span>Fechar</span>
        <span className="key">Ctrl+W</span>
      </button>
      <button
        role="menuitem"
        disabled={(group?.tabs.length ?? 0) < 2}
        onClick={run(() => void wb.closeOthers(gid, menu.tab.id))}
      >
        Fechar outras
      </button>
      <button role="menuitem" onClick={run(() => void closeAll(gid))}>
        Fechar todas
      </button>
      <hr />
      <button role="menuitem" disabled={!menu.tab.preview} onClick={run(() => wb.pin(menu.tab.id))}>
        Manter aberta
      </button>
      <button
        role="menuitem"
        disabled={wb.groups.length >= MAX_GROUPS}
        onClick={run(() => {
          wb.activate(gid, menu.tab.id);
          wb.splitActive();
        })}
      >
        <span>Dividir editor</span>
        <span className="key">Ctrl+\</span>
      </button>
    </div>
  );
}

// ------------------------------------------------------------ corpo das abas

/** Tela de apoio aberta como aba: coluna de leitura com título. */
function Page({ page: raw }: { page: PageId }) {
  // Abas "providers" de um layout antigo viram Configurações (seção Provedores).
  const page: PageId = (raw as string) === "providers" ? "settings" : raw;
  const info = PAGES.find((p) => p.id === page);
  if (page === "metrics") return <MetricsView />;
  if (page === "devices") return <DevicesView />;
  return (
    <div className="page scroll">
      <div className="page-col">
        <h2 className="page-title">
          <Icon name={PAGE_ICONS[page]} size={18} />
          {info?.label}
        </h2>
        {page === "deps" && <DepsView />}
        {page === "builds" && <BuildsView />}
        {page === "connections" && <ConnectionsView />}
        {page === "remote" && <RemoteView />}
        {page === "settings" && <SettingsView />}
      </div>
    </div>
  );
}

/** Conteúdo de uma aba que não é arquivo (arquivos usam o editor do grupo). */
function TabBody({ tab, active }: { tab: WbTab; active: boolean }) {
  const kind = tab.kind;
  switch (kind.type) {
    case "chat":
      return <ChatTab tabId={tab.id} taskId={kind.task} project={kind.project} active={active} />;
    case "page":
      return <Page page={kind.page} />;
    case "terminal":
      return <Terminal cwd={kind.cwd} command={kind.command} active={active} persistKey={`wb-${tab.id}`} />;
    case "dashboard":
      return <Dashboard />;
    case "agentmap":
      return <AgentMap taskId={kind.task} active={active} />;
    case "diff":
      return <DiffTab taskId={kind.task} />;
    case "mirror":
      return <MirrorTab />;
    case "file":
      return null;
  }
}

// --------------------------------------------------------------------- grupo

/** `titleRow`: a faixa de abas está na linha de título da janela (a sobra dela arrasta a janela). */
function EditorGroup({
  group,
  focused,
  canSplit,
  titleRow,
  multi,
  style,
}: {
  group: Group;
  focused: boolean;
  canSplit: boolean;
  titleRow: boolean;
  /** Há mais de um grupo (mostra o botão de trocar a direção da divisão). */
  multi: boolean;
  style?: CSSProperties;
}) {
  const dirty = useWorkbench((s) => s.dirty);
  const tasks = useApp((s) => s.tasks);
  const taskOf = (kind: TabKind) => (kind.type === "chat" && kind.task != null ? tasks.find((t) => t.id === kind.task) : undefined);
  const drag = useWorkbench((s) => s.drag);
  const newGroupOk = useWorkbench((s) => canDropAsNewGroup(s.groups, s.drag));
  const split = useWorkbench((s) => s.split);
  const [menu, setMenu] = useState<MenuState | null>(null);
  /** Onde a aba arrastada cairia: antes de uma aba, no fim da faixa, ou nada. */
  const [over, setOver] = useState<number | "end" | null>(null);
  const [zone, setZone] = useState<"all" | "edge">("all");
  const strip = useRef<HTMLDivElement>(null);
  const gid = group.id;
  const active = activeTab(group);
  const filePath: Path | null = active?.kind.type === "file" ? active.kind.path : null;
  const hasFiles = group.tabs.some((t) => t.kind.type === "file");
  const wb = useWorkbench.getState;

  useEffect(() => {
    if (drag == null) {
      setOver(null);
      setZone("all");
    }
  }, [drag]);

  // Mantém a aba ativa visível na faixa.
  useEffect(() => {
    strip.current?.querySelector(".wb-tab.active")?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [group.active, group.tabs.length]);

  const allow = (e: DragEvent) => {
    if (drag == null) return false;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    return true;
  };

  const tabProps = (t: WbTab) => {
    const tid = t.id;
    return {
      draggable: true,
      onDragStart: (e: DragEvent) => {
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", tabTitle(t.kind));
        // Mexer no DOM dentro do dragstart cancela o arrasto; marca no próximo ciclo.
        setTimeout(() => wb().setDrag(tid), 0);
      },
      onDragEnd: () => wb().setDrag(null),
      onDragOver: (e: DragEvent) => {
        if (!allow(e)) return;
        e.stopPropagation();
        if (over !== tid) setOver(tid);
      },
      onDrop: (e: DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        dropTab(gid, tid);
      },
      onClick: () => wb().activate(gid, tid),
      onDoubleClick: () => wb().pin(tid),
      onMouseDown: (e: MouseEvent) => {
        // Evita a rolagem automática do botão do meio; o fechamento fica no auxclick.
        if (e.button === 1) e.preventDefault();
      },
      onAuxClick: (e: MouseEvent) => {
        if (e.button === 1) void wb().requestCloseTab(gid, tid);
      },
      onContextMenu: (e: MouseEvent) => {
        e.preventDefault();
        setMenu({ x: e.clientX, y: e.clientY, tab: t });
      },
    };
  };

  return (
    <div className={focused ? "wb-group focused" : "wb-group"} style={style} onMouseDownCapture={() => wb().focusGroup(gid)}>
      <div
        ref={strip}
        className={over === "end" ? "wb-tabs drop-end" : "wb-tabs"}
        role="tablist"
        onWheel={(e) => {
          if (e.deltaY !== 0 && strip.current) strip.current.scrollLeft += e.deltaY;
        }}
        onDragOver={(e) => {
          if (allow(e) && over !== "end") setOver("end");
        }}
        onDragLeave={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOver(null);
        }}
        onDrop={(e) => {
          e.preventDefault();
          dropTab(gid, null);
        }}
      >
        {group.tabs.map((t) => {
          const isActive = t.id === group.active;
          const kind = t.kind;
          const modified = kind.type === "file" && dirty.some((p) => samePath(p, kind.path));
          const task = taskOf(kind);
          const title = titleOf(kind, task);
          const cls = [
            "wb-tab",
            isActive && "active",
            t.preview && "preview",
            modified && "dirty",
            over === t.id && drag !== t.id && "drop-before",
            drag === t.id && "dragging",
          ]
            .filter(Boolean)
            .join(" ");
          return (
            <div
              key={t.id}
              className={cls}
              title={task ? task.prompt : tabTooltip(kind)}
              role="tab"
              aria-selected={isActive}
              tabIndex={isActive ? 0 : -1}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") wb().activate(gid, t.id);
              }}
              {...tabProps(t)}
            >
              <TabIcon kind={kind} task={task} />
              <span className="wb-title">{title}</span>
              {task?.access === "bypass" && <BypassBadge compact />}
              <span
                className="wb-x"
                role="button"
                aria-label={`Fechar ${title}`}
                title={modified ? "Modificado (fechar)" : "Fechar (Ctrl+W)"}
                onClick={(e) => {
                  e.stopPropagation();
                  void wb().requestCloseTab(gid, t.id);
                }}
                onDoubleClick={(e) => e.stopPropagation()}
              >
                <span className="wb-dot">●</span>
                <span className="wb-cross">×</span>
              </span>
            </div>
          );
        })}
        <div className="wb-tabs-fill" data-tauri-drag-region={titleRow || undefined} />
        <div className="wb-tab-actions">
            <button
              className="wb-sbtn"
              title="Nova conversa (Ctrl+N)"
              aria-label="Nova conversa"
              onClick={() => {
                wb().focusGroup(gid);
                wb().openNewChat(useApp.getState().selected);
              }}
            >
              <Icon name="plus" size={14} />
            </button>
          {canSplit && (
            <button
              className="wb-sbtn"
              title="Dividir editor (Ctrl+\)"
              aria-label="Dividir editor"
              onClick={() => {
                wb().focusGroup(gid);
                wb().splitActive();
              }}
            >
              <Icon name="splitH" size={14} />
            </button>
          )}
          {multi && (
            <button
              className="wb-sbtn"
              title={split === "horizontal" ? "Empilhar os grupos" : "Pôr os grupos lado a lado"}
              aria-label={split === "horizontal" ? "Empilhar os grupos" : "Pôr os grupos lado a lado"}
              onClick={() => wb().toggleSplitDir()}
            >
              <Icon name={split === "horizontal" ? "splitV" : "splitH"} size={14} />
            </button>
          )}
          {active && group.tabs.length > 1 && (
            <button
              className="wb-sbtn"
              title="Fechar outras"
              aria-label="Fechar outras abas"
              onClick={() => void wb().closeOthers(gid, active.id)}
            >
              ⋯
            </button>
          )}
        </div>
      </div>

      <div className="wb-body">
        {hasFiles && (
          <div className="pane" style={{ display: filePath ? "block" : "none" }}>
            <GroupEditor path={filePath} focused={focused} />
          </div>
        )}
        {/* As outras abas ficam montadas: terminais mantêm o processo e as demais, o estado. */}
        {group.tabs
          .filter((t) => t.kind.type !== "file")
          .map((t) => (
            <div
              key={t.id}
              className={t.kind.type === "dashboard" ? "pane scroll" : "pane"}
              role="tabpanel"
              style={{ display: t.id === group.active ? "block" : "none" }}
            >
              <TabBody tab={t} active={t.id === group.active} />
            </div>
          ))}
        {!active && (
          <div className="pane wb-empty">
            <div>Nenhuma aba aberta</div>
            <div className="small muted">
              Ctrl+N nova conversa · Ctrl+Tab alterna abas · Ctrl+W fecha · Ctrl+\ divide · arraste abas entre grupos
            </div>
            <div className="wb-empty-actions">
              <button className="primary" onClick={() => wb().openNewChat(useApp.getState().selected)}>
                Nova conversa
              </button>
              <button onClick={() => wb().openDashboard()}>Painel de projetos</button>
            </div>
          </div>
        )}
        {drag != null && (
          <div
            className="wb-drop"
            onDragOver={(e) => {
              if (!allow(e)) return;
              const r = e.currentTarget.getBoundingClientRect();
              const frac =
                split === "horizontal" ? (e.clientX - r.left) / r.width : (e.clientY - r.top) / r.height;
              const next = newGroupOk && frac > 0.67 ? "edge" : "all";
              if (next !== zone) setZone(next);
            }}
            onDragLeave={() => setZone("all")}
            onDrop={(e) => {
              e.preventDefault();
              if (zone === "edge") dropTabAsNewGroup(gid);
              else dropTab(gid, null);
            }}
          >
            <div className={`wb-drop-mark ${zone}`} />
          </div>
        )}
      </div>
      {menu && <TabMenu gid={gid} menu={menu} onClose={() => setMenu(null)} />}
    </div>
  );
}

// ----------------------------------------------------------------- workbench

/**
 * Workbench (área central): grupos de editores (split de até 3) com abas de
 * conversa com agente, arquivo (Monaco), terminal, painel e telas de apoio. As abas podem
 * ser arrastadas entre grupos ou para a borda de um grupo, criando outro.
 */
export default function WorkbenchView() {
  const groups = useWorkbench((s) => s.groups);
  const activeGroup = useWorkbench((s) => s.activeGroup);
  const split = useWorkbench((s) => s.split);
  const sizes = useWorkbench((s) => s.sizes);

  // Descarta os modelos dos arquivos que não estão mais em nenhuma aba.
  useEffect(() => {
    const open = groups.flatMap((g) => g.tabs.flatMap((t) => (t.kind.type === "file" ? [t.kind.path] : [])));
    whenMonacoLoaded((m) => m.retainDocs(open));
  }, [groups]);

  // Arquivo alterado em disco (ex.: por um agente): relê os que não têm edição pendente.
  useEffect(() => {
    const timers = new Map<string, ReturnType<typeof setTimeout>>();
    const off = onProjectChanged(({ path }) => {
      clearTimeout(timers.get(path));
      timers.set(
        path,
        setTimeout(() => {
          timers.delete(path);
          whenMonacoLoaded((m) => void m.reloadCleanDocs(path));
        }, 300),
      );
    });
    const onFocus = () => whenMonacoLoaded((m) => void m.reloadCleanDocs());
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener("focus", onFocus);
      timers.forEach((t) => clearTimeout(t));
      void off.then((f) => f());
    };
  }, []);

  const focusedGid = groups.some((g) => g.id === activeGroup) ? activeGroup : groups[0]?.id;
  const container = useRef<HTMLDivElement>(null);
  const weight = (gid: number) => sizes[gid] ?? 1;

  /** Arrasta o divisor entre os grupos `i` e `i + 1`: troca peso entre os dois (mínimo de 12% da área). */
  const startResize = (i: number) => (e: PointerEvent<HTMLDivElement>) => {
    const box = container.current;
    if (!box || e.button !== 0) return;
    e.preventDefault();
    const horiz = split === "horizontal";
    const total = horiz ? box.clientWidth : box.clientHeight;
    const a = groups[i].id;
    const b = groups[i + 1].id;
    const sum = groups.reduce((n, g) => n + weight(g.id), 0);
    const pair = weight(a) + weight(b);
    const startA = weight(a);
    const start = horiz ? e.clientX : e.clientY;
    const min = sum * 0.12;
    const sash = e.currentTarget;
    sash.setPointerCapture(e.pointerId);
    sash.classList.add("active");
    const move = (ev: globalThis.PointerEvent) => {
      const delta = (((horiz ? ev.clientX : ev.clientY) - start) / Math.max(1, total)) * sum;
      const na = Math.min(pair - min, Math.max(min, startA + delta));
      useWorkbench.getState().setSizes({ [a]: na, [b]: pair - na });
    };
    const up = () => {
      sash.classList.remove("active");
      sash.removeEventListener("pointermove", move);
      sash.removeEventListener("pointerup", up);
      sash.removeEventListener("pointercancel", up);
    };
    sash.addEventListener("pointermove", move);
    sash.addEventListener("pointerup", up);
    sash.addEventListener("pointercancel", up);
  };

  return (
    <div ref={container} className={split === "horizontal" ? "wb-groups wb-row" : "wb-groups wb-col"}>
      {groups.map((g, i) => (
        <Fragment key={g.id}>
          {i > 0 && (
            <div
              className="wb-sash"
              role="separator"
              aria-orientation={split === "horizontal" ? "vertical" : "horizontal"}
              aria-label="Redimensionar os grupos"
              title="Arraste para redimensionar · duplo clique iguala"
              onPointerDown={startResize(i - 1)}
              onDoubleClick={() => useWorkbench.getState().setSizes(Object.fromEntries(groups.map((x) => [x.id, 1])))}
            />
          )}
          <EditorGroup
            group={g}
            focused={g.id === focusedGid}
            canSplit={groups.length < MAX_GROUPS}
            titleRow={split === "horizontal" || i === 0}
            multi={groups.length > 1}
            style={{ flexGrow: weight(g.id) }}
          />
        </Fragment>
      ))}
    </div>
  );
}
