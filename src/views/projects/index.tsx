/**
 * Barra esquerda: os projetos da pasta raiz, cada um expandido nas suas
 * sessões (conversas com agentes). Busca no topo; cada sessão mostra o modelo
 * à direita e a selecionada fica num cartão. No rodapé, só ícones (com tooltip
 * e aria-label) que abrem as telas de apoio como abas.
 */
import { useMemo, useState, type KeyboardEvent, type MouseEvent } from "react";
import {
  api,
  baseName,
  errorMessage,
  isTaskActive,
  samePath,
  variantOf,
  type Path,
  type Project,
  type ProjectStatus,
  type Task,
} from "@/api";
import { ContextMenu, type MenuItem } from "@/shell/ContextMenu";
import { Icon, type IconName } from "@/shell/icons";
import { activeTab, PAGES, useApp, useLayout, useWorkbench, type PageId } from "@/store";
import { cancelTask, deleteClosedTasks, deleteTask, discardTask } from "@/views/session/actions";
import { sessionTitle } from "@/views/session/events";
import { StateDot } from "@/views/session";
import { agentStateOf, agentStateTitle } from "@/views/session/AgentState";
import { devServerMenuItems, ProjectServerButton } from "@/views/servers/devservers";
import "./projects.css";

const PAGE_ICONS: Record<PageId, IconName> = {
  deps: "box",
  builds: "hammer",
  metrics: "chart",
  remote: "phone",
  settings: "gear",
};

function onActivateKey(e: KeyboardEvent, action: () => void) {
  if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) {
    e.preventDefault();
    action();
  }
}

function GitInfo({ status, kinds, missing }: { status: ProjectStatus | null | undefined; kinds: string[]; missing?: boolean }) {
  if (missing)
    return (
      <span className="pj-git pj-missing" title="A pasta foi removida do disco; as sessões continuam aqui até serem excluídas">
        pasta ausente
      </span>
    );
  if (status === undefined) return <span className="pj-git">calculando…</span>;
  if (status === null) return <span className="pj-git">{kinds.join(", ") || "sem git"}</span>;
  const changed = status.changed_files.length;
  return (
    <span className="pj-git">
      <span className="ellipsis">{status.branch ?? "(detached)"}</span>
      {changed > 0 && (
        <span className="pj-changed" title={changed === 1 ? "1 arquivo alterado" : `${changed} arquivos alterados`}>
          ±{changed}
        </span>
      )}
    </span>
  );
}

/** Modelo da sessão em forma curta ("opus-4.5"); sem modelo fixo, o provedor. */
function shortModel(task: Task): string {
  const m = task.model?.trim();
  if (!m) return task.provider;
  return m.replace(/^claude-/, "").replace(/-\d{8}$/, "").replace(/(\d+)-(\d+)$/, "$1.$2");
}

interface SessionRowProps {
  task: Task;
  current: boolean;
  onMenu(e: MouseEvent, task: Task): void;
}

function SessionRow({ task, current, onMenu }: SessionRowProps) {
  const unread = useApp((s) => !!s.unread[task.id]);
  const auto = useApp((s) => s.autowork[task.id]);
  const denied = useApp((s) => s.attention[task.id]);
  const servers = useApp((s) => s.servers.filter((x) => x.owner.kind === "Session" && x.owner.id === task.id).length);
  const open = () => useWorkbench.getState().openSession(task.id);
  const { kind } = variantOf(task.state);
  const closed = kind === "Applied" || kind === "Discarded";
  const cls = ["pj-session", current && "sel", unread && "unread", closed && "closed"].filter(Boolean).join(" ");
  const autoKind = auto ? variantOf(auto.status).kind : null;
  return (
    <div
      className={cls}
      role="treeitem"
      tabIndex={0}
      aria-current={current}
      title={`#${task.id} · ${agentStateTitle(agentStateOf(task.state, denied), task.state, denied)}\n${task.provider}${task.model ? ` · ${task.model}` : ""}\n${task.prompt}`}
      onClick={open}
      onKeyDown={(e) => onActivateKey(e, open)}
      onContextMenu={(e) => onMenu(e, task)}
    >
      <StateDot task={task} />
      <span className="pj-session-title ellipsis">{sessionTitle(task.prompt)}</span>
      {auto && (
        <span
          className={autoKind === "Running" ? "pj-tag run" : "pj-tag"}
          title={`Auto-work: ${auto.done}/${auto.total} itens`}
        >
          auto
        </span>
      )}
      {servers > 0 && (
        <span className="pj-tag ok" title={servers === 1 ? "1 servidor rodando nesta sessão" : `${servers} servidores rodando nesta sessão`}>
          <Icon name="server" size={10} />
          {servers}
        </span>
      )}
      <span className="pj-provider ellipsis">{shortModel(task)}</span>
    </div>
  );
}

export default function ProjectsView() {
  const root = useApp((s) => s.root);
  const ready = useApp((s) => s.ready);
  const projects = useApp((s) => s.projects);
  const statuses = useApp((s) => s.statuses);
  const tasks = useApp((s) => s.tasks);
  const selected = useApp((s) => s.selected);
  const pickRoot = useApp((s) => s.pickRoot);
  const currentTask = useWorkbench((s) => {
    const kind = activeTab(s.groups.find((g) => g.id === s.activeGroup))?.kind;
    return kind?.type === "chat" ? kind.task : null;
  });
  const currentPage = useWorkbench((s) => {
    const kind = activeTab(s.groups.find((g) => g.id === s.activeGroup))?.kind;
    return kind?.type === "page" ? kind.page : null;
  });

  const [query, setQuery] = useState("");
  /** Projetos recolhidos (por caminho); o padrão é expandido. */
  const [collapsed, setCollapsed] = useState<Record<Path, boolean>>({});
  const [hideClosed, setHideClosed] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);

  const q = query.trim().toLowerCase();
  const rows = useMemo(() => {
    return projects
      .map((p: Project) => {
        const all = tasks.filter((t) => samePath(t.project, p.path));
        const nameHit = !q || p.name.toLowerCase().includes(q);
        const sessions = all.filter((t) => {
          const kind = variantOf(t.state).kind;
          if (hideClosed && (kind === "Applied" || kind === "Discarded")) return false;
          return nameHit || t.prompt.toLowerCase().includes(q) || `#${t.id}` === q;
        });
        return { project: p, sessions, total: all.length, show: nameHit || sessions.length > 0 };
      })
      .filter((r) => r.show);
  }, [projects, tasks, q, hideClosed]);

  const allCollapsed = projects.length > 0 && projects.every((p) => collapsed[p.path]);
  const toggleAll = () =>
    setCollapsed(allCollapsed ? {} : Object.fromEntries(projects.map((p) => [p.path, true])));

  const fail = (e: unknown) => useApp.getState().setError(errorMessage(e));

  const sessionMenu = (e: MouseEvent, task: Task) => {
    e.preventDefault();
    const active = isTaskActive(task.state);
    const kind = variantOf(task.state).kind;
    const closed = kind === "Applied" || kind === "Discarded";
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Abrir", onSelect: () => useWorkbench.getState().openSession(task.id) },
        {
          label: "Ver diff",
          onSelect: () => {
            useWorkbench.getState().openSession(task.id);
            useLayout.getState().showChanges();
          },
        },
        { label: "Cancelar o turno", disabled: !active, separator: true, onSelect: () => void cancelTask(task.id).catch(fail) },
        {
          label: "Descartar alterações…",
          danger: true,
          disabled: active || closed,
          onSelect: () =>
            void api
              .systemConfirm("Descartar alterações", `Descartar as alterações da sessão #${task.id}? Isso não pode ser desfeito.`)
              .then((ok) => (ok ? discardTask(task.id) : false))
              .catch(fail),
        },
        {
          label: "Excluir sessão…",
          danger: true,
          separator: true,
          onSelect: () => void deleteTask(task).catch(fail),
        },
      ],
    });
  };

  const projectMenu = (e: MouseEvent, p: Project) => {
    e.preventDefault();
    const closedCount = tasks.filter(
      (t) => samePath(t.project, p.path) && ["Applied", "Discarded"].includes(variantOf(t.state).kind),
    ).length;
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Nova conversa", disabled: !!p.missing, onSelect: () => newChat(p.path) },
        {
          label: closedCount > 0 ? `Excluir encerradas (${closedCount})…` : "Excluir encerradas",
          danger: true,
          separator: true,
          disabled: closedCount === 0,
          onSelect: () => void deleteClosedTasks(p.path).catch(fail),
        },
        ...(p.missing ? [] : devServerMenuItems(p.path)),
      ],
    });
  };

  const [refreshing, setRefreshing] = useState(false);
  const refresh = () => {
    setRefreshing(true);
    void useApp
      .getState()
      .refreshRoot()
      .finally(() => setRefreshing(false));
  };

  const newChat = (project: Path) => {
    useApp.getState().select(project);
    useWorkbench.getState().openNewChat(project);
  };

  return (
    <aside className="pj" aria-label="Projetos e sessões">
      <div className="pj-search">
        <Icon name="search" size={13} />
        <input
          placeholder="Buscar projeto ou sessão…"
          aria-label="Buscar projeto ou sessão"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {query && (
          <button className="pj-ibtn" title="Limpar a busca" aria-label="Limpar a busca" onClick={() => setQuery("")}>
            <Icon name="close" size={12} />
          </button>
        )}
      </div>

      <div className="pj-head">
        <span className="pj-head-title">Projetos</span>
        <button className="pj-ibtn" title="Abrir pasta raiz" aria-label="Abrir pasta raiz" onClick={() => void pickRoot()}>
          <Icon name="folderOpen" size={15} />
        </button>
        <button
          className="pj-ibtn"
          title="Atualizar a lista de projetos, branches e alterados"
          aria-label="Atualizar projetos"
          disabled={!root || refreshing}
          onClick={refresh}
        >
          <Icon name="refresh" size={15} className={refreshing ? "pj-spin" : undefined} />
        </button>
        <button
          className="pj-ibtn"
          title="Painel: status git de todos os projetos"
          aria-label="Painel de projetos"
          onClick={() => useWorkbench.getState().openDashboard()}
        >
          <Icon name="grid" size={15} />
        </button>
        <button
          className="pj-ibtn"
          aria-pressed={hideClosed}
          title={hideClosed ? "Mostrar também as sessões encerradas" : "Ocultar as sessões encerradas (aplicadas e descartadas)"}
          aria-label="Filtrar sessões encerradas"
          onClick={() => setHideClosed((v) => !v)}
        >
          <Icon name="filter" size={15} />
        </button>
        <button
          className="pj-ibtn"
          title={allCollapsed ? "Expandir todos os projetos" : "Recolher todos os projetos"}
          aria-label={allCollapsed ? "Expandir tudo" : "Recolher tudo"}
          onClick={toggleAll}
        >
          <Icon name="collapse" size={15} />
        </button>
      </div>

      <div className="pj-list" role="tree">
        {!root && ready && (
          <div className="pj-empty">
            <p>Escolha a pasta raiz onde ficam seus projetos.</p>
            <button className="primary" onClick={() => void pickRoot()}>
              <Icon name="folderOpen" size={14} /> Abrir pasta raiz
            </button>
          </div>
        )}
        {root && projects.length === 0 && <div className="pj-empty">Nenhum projeto encontrado nesta pasta.</div>}
        {root && projects.length > 0 && rows.length === 0 && <div className="pj-empty">Nada encontrado para “{query}”.</div>}
        {rows.map(({ project: p, sessions, total }) => {
          const open = q ? true : !collapsed[p.path];
          const isSel = samePath(p.path, selected);
          const toggle = () => {
            useApp.getState().select(p.path);
            // Projeto aberto mas não ativo: o clique só o torna ativo.
            if (open && !isSel) return;
            setCollapsed((c) => ({ ...c, [p.path]: open }));
          };
          return (
            <div key={p.path} className="pj-group">
              <div
                className={["pj-project", isSel && "sel", p.missing && "missing"].filter(Boolean).join(" ")}
                role="treeitem"
                aria-expanded={open}
                tabIndex={0}
                title={p.missing ? `${p.path}\n(pasta ausente)` : p.path}
                onClick={toggle}
                onContextMenu={(e) => projectMenu(e, p)}
                onKeyDown={(e) => onActivateKey(e, toggle)}
              >
                <Icon name={open ? "chevronDown" : "chevronRight"} size={13} className="pj-chev" />
                <span className="pj-name ellipsis">{p.name || baseName(p.path)}</span>
                <GitInfo status={statuses[p.path]} kinds={p.kinds} missing={p.missing} />
                {!p.missing && <ProjectServerButton project={p.path} />}
                {!p.missing && (
                <button
                  className="pj-ibtn pj-add"
                  title={`Nova conversa em ${p.name}`}
                  aria-label={`Nova conversa em ${p.name}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    newChat(p.path);
                  }}
                >
                  <Icon name="plus" size={14} />
                </button>
                )}
              </div>
              {open && (
                <div className="pj-sessions" role="group">
                  {sessions.map((t) => (
                    <SessionRow key={t.id} task={t} current={t.id === currentTask} onMenu={sessionMenu} />
                  ))}
                  {sessions.length === 0 && !p.missing && (
                    <button className="pj-none" onClick={() => newChat(p.path)}>
                      {total > 0 ? "Nenhuma sessão neste filtro" : "Nenhuma sessão ainda · começar uma conversa"}
                    </button>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>

      <nav className="pj-foot" aria-label="Telas">
        {PAGES.map((page) => (
          <button
            key={page.id}
            className={currentPage === page.id ? "pj-nav on" : "pj-nav"}
            title={page.title}
            aria-label={page.label}
            aria-current={currentPage === page.id ? "page" : undefined}
            onClick={() => useWorkbench.getState().openPage(page.id)}
          >
            <Icon name={PAGE_ICONS[page.id]} size={16} />
          </button>
        ))}
      </nav>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </aside>
  );
}
