/**
 * Paleta de comandos (Ctrl+K), tela 04 do mockup: busca em sessões, projetos,
 * arquivos alterados, servidores/portas, ações e telas, em grupos. Setas
 * navegam, Enter abre, Tab troca o filtro por tipo, Esc fecha.
 */
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { baseName, isTaskActive, samePath } from "@/api";
import { requestNewTask } from "@/areas/tasks/intent";
import { AREAS, PAGES, useApp, useLayout, useWorkbench, type Area } from "@/store";
import { sessionTitle } from "@/views/session/events";
import { ErrorBoundary } from "./ErrorBoundary";
import { Icon, type IconName } from "./icons";
import { areaIcon } from "./NavRail";
import "./CommandPalette.css";

type Group = "Sessões" | "Projetos" | "Arquivos" | "Portas" | "Ações" | "Ir para";
const GROUPS: Group[] = ["Portas", "Sessões", "Projetos", "Arquivos", "Ações", "Ir para"];

interface Cmd {
  id: string;
  group: Group;
  label: string;
  /** Texto extra usado só na busca. */
  keys?: string;
  lead?: ReactNode;
  meta?: ReactNode;
  run(): void;
}

/** Ícone padrão de cada grupo (itens sem `lead` próprio). */
const GROUP_ICON: Record<string, IconName> = {
  Sessões: "chat",
  Projetos: "folder",
  Arquivos: "file",
  Ações: "plus",
  "Ir para": "gear",
};

function commands(): Cmd[] {
  const layout = useLayout.getState();
  const wb = useWorkbench.getState();
  const app = useApp.getState();
  const go = (a: Area) => () => layout.setArea(a);
  const list: Cmd[] = [];

  for (const s of app.servers) {
    list.push({
      id: `port-${s.pid}-${s.port}`,
      group: "Portas",
      label: `${s.name}${s.cwd ? ` · ${baseName(s.cwd)}` : ""}`,
      keys: `porta ${s.port} ${s.cmdline}`,
      lead: <span className="ui-mono">:{s.port}</span>,
      meta: <span className="ui-chip ok">em uso</span>,
      run: () => layout.openServersPanel(),
    });
  }

  for (const t of [...app.tasks].sort((a, b) => b.id - a.id)) {
    list.push({
      id: `ses-${t.id}`,
      group: "Sessões",
      label: `#${t.id} ${sessionTitle(t.prompt, 60)}`,
      keys: `sessao ${baseName(t.project)}`,
      meta: (
        <span className="pal-meta">
          {isTaskActive(t.state) ? "rodando" : baseName(t.project)}
          {!app.projects.some((p) => samePath(p.path, t.project)) && " · fora da raiz"}
        </span>
      ),
      run: () => wb.openSession(t.id),
    });
  }

  for (const p of app.projects) {
    const st = app.statuses[p.path];
    list.push({
      id: `proj-${p.path}`,
      group: "Projetos",
      label: p.name,
      keys: `projeto ${p.path}`,
      meta: <span className="pal-meta">{st?.branch ?? p.kinds.join(" · ")}</span>,
      run: () => {
        app.select(p.path);
        wb.openNewChat(p.path);
      },
    });
    for (const [rel] of st?.changed_files ?? []) {
      list.push({
        id: `file-${p.path}-${rel}`,
        group: "Arquivos",
        label: rel,
        keys: `arquivo ${p.name}`,
        meta: <span className="pal-meta">{p.name} · alterado</span>,
        run: () => {
          layout.setArea("chat");
          wb.openFile(`${p.path.replace(/[\\/]$/, "")}/${rel}`);
        },
      });
    }
  }

  list.push(
    { id: "new-chat", group: "Ações", label: "Nova conversa", meta: <span className="kbd">Ctrl N</span>, run: () => wb.openNewChat(app.selected) },
    { id: "open-folder", group: "Ações", label: "Abrir pasta", run: () => void app.pickRoot() },
    { id: "refresh", group: "Ações", label: "Atualizar projetos", run: () => void app.refreshRoot() },
    { id: "inspector", group: "Ações", label: "Alternar o inspetor", meta: <span className="kbd">Ctrl .</span>, run: () => (layout.setArea("chat"), layout.toggleInspector()) },
    { id: "changes", group: "Ações", label: "Ver alterações da sessão", run: () => layout.showChanges() },
    { id: "agents", group: "Ações", label: "Ver agentes em tela cheia", run: go("agents") },
    {
      id: "new-schedule",
      group: "Ações",
      label: "Nova tarefa agendada",
      keys: "agendar agendamento lembrete cron recorrente tarefas",
      run: () => (layout.setArea("tasks"), requestNewTask()),
    },
    { id: "mirror", group: "Ações", label: "Espelhar celular como aba", keys: "celular espelhamento screen mirror android", run: () => wb.openMirror() },
  );

  for (const a of AREAS) {
    const I = areaIcon(a.id);
    list.push({ id: `area-${a.id}`, group: "Ir para", label: a.label, keys: "tela area", lead: I ? <I size={14} className="pal-ic" aria-hidden="true" /> : undefined, run: go(a.id) });
  }
  for (const s of [
    ["theme", "Tema"],
    ["agents", "Política de agentes"],
    ["providers", "Provedores"],
    ["project", "Projeto"],
  ] as const) {
    list.push({ id: `set-${s[0]}`, group: "Ir para", label: `Configurações › ${s[1]}`, run: () => wb.openSettings(s[0]) });
  }
  for (const p of PAGES.filter((p) => p.id === "builds" || p.id === "deps" || p.id === "remote")) {
    list.push({ id: `page-${p.id}`, group: "Ir para", label: p.label, keys: p.title, meta: <span className="pal-meta">extras</span>, run: () => (layout.setArea("chat"), wb.openPage(p.id)) });
  }
  list.push(
    { id: "x-autowork", group: "Ir para", label: "Autowork", keys: "auto work", meta: <span className="pal-meta">extras · na conversa</span>, run: () => (layout.setArea("chat"), layout.openRight("agents")) },
    { id: "x-player", group: "Ir para", label: "Player", keys: "musica", meta: <span className="pal-meta">extras · barra de status</span>, run: () => undefined },
    { id: "x-pet", group: "Ir para", label: "Pet", meta: <span className="pal-meta">extras · barra de status</span>, run: () => undefined },
  );
  return list;
}

const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
const LIMIT = 8;

export function CommandPalette() {
  const open = useLayout((s) => s.paletteOpen);
  if (!open) return null;
  return (
    <ErrorBoundary>
      <Palette />
    </ErrorBoundary>
  );
}

function Palette() {
  const [q, setQ] = useState("");
  const [filter, setFilter] = useState<Group | null>(null);
  const [sel, setSel] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const all = useMemo(commands, []);
  const close = () => useLayout.getState().closePalette();

  const groups = useMemo(() => {
    const words = norm(q.trim()).split(/\s+/).filter(Boolean);
    const hit = (c: Cmd) => {
      const hay = norm(`${c.group} ${c.label} ${c.keys ?? ""}`);
      return words.every((w) => hay.includes(w));
    };
    return GROUPS.filter((g) => !filter || g === filter)
      .map((g) => {
        const items = all.filter((c) => c.group === g && hit(c));
        return { g, items: filter || words.length ? items.slice(0, filter ? 50 : LIMIT * 2) : items.slice(0, g === "Ir para" || g === "Ações" ? 40 : 4) };
      })
      .filter((x) => x.items.length > 0);
  }, [all, q, filter]);
  const flat = groups.flatMap((x) => x.items);

  useEffect(() => {
    input.current?.focus();
  }, []);
  useEffect(() => {
    setSel(0);
  }, [q, filter]);
  useEffect(() => {
    listRef.current?.querySelector(".pal-it.on")?.scrollIntoView({ block: "nearest" });
  }, [sel]);

  const run = (c: Cmd | undefined) => {
    if (!c) return;
    close();
    c.run();
  };
  const cycle = (dir: 1 | -1) => {
    const opts: (Group | null)[] = [null, ...GROUPS];
    setFilter((f) => opts[(opts.indexOf(f) + dir + opts.length) % opts.length]);
  };

  let i = -1;
  return (
    <div className="pal-scrim" onMouseDown={close}>
      <div className="pal" role="dialog" aria-label="Paleta de comandos" onMouseDown={(e) => e.stopPropagation()}>
        <div className="pal-top">
          {filter && <span className="ui-chip info">{filter}</span>}
          <input
            ref={input}
            className="pal-input"
            placeholder="Buscar ou executar…"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") close();
              else if (e.key === "ArrowDown") (e.preventDefault(), setSel((s) => Math.min(s + 1, flat.length - 1)));
              else if (e.key === "ArrowUp") (e.preventDefault(), setSel((s) => Math.max(s - 1, 0)));
              else if (e.key === "Enter") (e.preventDefault(), run(flat[sel]));
              else if (e.key === "Tab") (e.preventDefault(), cycle(e.shiftKey ? -1 : 1));
              else if (e.key === "Backspace" && !q && filter) setFilter(null);
            }}
          />
        </div>
        <div className="pal-list" role="listbox" ref={listRef}>
          {flat.length === 0 && <div className="ui-empty">Nada encontrado.</div>}
          {groups.map(({ g, items }) => (
            <div key={g} role="group" aria-label={g}>
              <h4 className="pal-h">{g}</h4>
              {items.map((c) => {
                i += 1;
                const n = i;
                return (
                  <button key={c.id} role="option" aria-selected={n === sel} className={n === sel ? "pal-it on" : "pal-it"} onMouseMove={() => n !== sel && setSel(n)} onClick={() => run(c)}>
                    {c.lead ?? <Icon name={GROUP_ICON[c.group] ?? "chevronRight"} size={14} className="pal-ic" />}
                    <span className="grow">{c.label}</span>
                    {n === sel && !c.meta ? <span className="kbd">Enter</span> : c.meta}
                  </button>
                );
              })}
            </div>
          ))}
        </div>
        <div className="pal-foot">
          <span>↑↓ navegar</span>
          <span>Enter abrir</span>
          <span>Tab filtrar por tipo</span>
          <span>Esc fechar</span>
        </div>
      </div>
    </div>
  );
}
