/** Tela 01 · Início: saudação, 4 cards de estado, projetos recentes e "Precisa de você". */
import { useEffect, useMemo } from "react";
import { baseName, formatTokensPt, formatUsdPt, isTaskActive, samePath, type Path } from "@/api";
import { useApp, useLayout, useWorkbench } from "@/store";
import { Btn, StatusDot, type Tone } from "@/ui";
import { ensureRunsSync, useRuns } from "@/views/builds";
import { sessionTitle } from "@/views/session/events";
import "./home.css";

function greeting(h: number) {
  return h < 12 ? "Bom dia" : h < 18 ? "Boa tarde" : "Boa noite";
}

interface Need {
  key: string;
  tone: Tone;
  text: React.ReactNode;
  run(): void;
}

export default function HomeArea() {
  const projects = useApp((s) => s.projects);
  const tasks = useApp((s) => s.tasks);
  const statuses = useApp((s) => s.statuses);
  const servers = useApp((s) => s.servers);
  const today = useApp((s) => s.today);
  const attention = useApp((s) => s.attention);
  const unread = useApp((s) => s.unread);
  const selected = useApp((s) => s.selected);
  const runs = useRuns((s) => s.runs);

  useEffect(() => {
    ensureRunsSync();
  }, []);

  const now = new Date();
  const date = now.toLocaleDateString("pt-BR", { weekday: "long", day: "numeric", month: "long" });

  const active = tasks.filter((t) => isTaskActive(t.state));
  const waiting = Object.keys(attention).length;

  // Projetos recentes: ordem pela sessão mais nova (ids crescem com o tempo).
  const recent = useMemo(() => {
    const last = new Map<Path, number>();
    for (const t of tasks) last.set(t.project, Math.max(last.get(t.project) ?? 0, t.id));
    return [...projects]
      .filter((p) => !p.missing)
      .sort((a, b) => (last.get(b.path) ?? 0) - (last.get(a.path) ?? 0))
      .slice(0, 6)
      .map((p) => ({
        p,
        sessions: tasks.filter((t) => samePath(t.project, p.path)).length,
        running: tasks.some((t) => samePath(t.project, p.path) && isTaskActive(t.state)),
      }));
  }, [projects, tasks]);

  const needs: Need[] = [];
  for (const [id, what] of Object.entries(attention)) {
    const task = tasks.find((t) => t.id === Number(id));
    needs.push({
      key: `a${id}`,
      tone: "warn",
      text: (
        <>
          Comando negado em #{id}{task ? ` ${sessionTitle(task.prompt, 30)}` : ""}: <span className="ui-mono">{what}</span>
        </>
      ),
      run: () => useWorkbench.getState().openSession(Number(id)),
    });
  }
  for (const r of runs.filter((r) => r.kind === "build" && r.state === "failed").slice(-4)) {
    needs.push({ key: `b${r.id}`, tone: "bad", text: `Build falhou em ${baseName(r.project)} · ${r.label}`, run: () => useWorkbench.getState().openPage("builds") });
  }
  for (const id of Object.keys(unread).slice(0, 4)) {
    if (attention[Number(id)]) continue;
    const task = tasks.find((t) => t.id === Number(id));
    if (!task) continue;
    needs.push({ key: `u${id}`, tone: "info", text: `Atividade nova em #${id} ${sessionTitle(task.prompt, 40)}`, run: () => useWorkbench.getState().openSession(task.id) });
  }

  const newChat = () => useWorkbench.getState().openNewChat(useApp.getState().selected);
  const openProject = (path: Path) => {
    useApp.getState().select(path);
    useWorkbench.getState().openNewChat(path);
  };
  const ports = servers.map((s) => `:${s.port}`);

  return (
    <div className="ui-area">
      <div className="ui-area-main hm">
        <div className="hm-head">
          <div>
            <p className="ui-mut ui-sm hm-date">{date}</p>
            <p className="hm-hello">{greeting(now.getHours())}</p>
          </div>
          <span className="hm-sp" />
          <Btn variant="ghost" onClick={() => void useApp.getState().pickRoot()}>Abrir pasta</Btn>
          <Btn onClick={newChat}>Nova conversa</Btn>
        </div>

        <div className="hm-stats">
          <button className="hm-card hm-stat" onClick={() => useLayout.getState().setArea("agents")}>
            <div className="ui-mut ui-sm">Conversas ativas</div>
            <div className="hm-num">{active.length}</div>
            <div className="ui-sm ui-mut">{waiting ? `${waiting} aguardando você` : `${tasks.length} sessões no total`}</div>
          </button>
          <button className="hm-card hm-stat" onClick={() => useLayout.getState().setArea("servers")}>
            <div className="ui-mut ui-sm">Servidores</div>
            <div className="hm-num">
              {servers.length} {servers.length > 0 && <span className="ui-sm hm-ok">no ar</span>}
            </div>
            <div className="ui-sm ui-mut hm-ell">{ports.length ? ports.slice(0, 4).join(" · ") : "nenhuma porta aberta"}</div>
          </button>
          <button className="hm-card hm-stat" onClick={() => useLayout.getState().setArea("git")}>
            <div className="ui-mut ui-sm">PRs abertos</div>
            <div className="hm-num ui-mut">—</div>
            <div className="ui-sm ui-mut">sem dados do GitHub ainda</div>
          </button>
          <button className="hm-card hm-stat" onClick={() => useLayout.getState().setArea("metrics")}>
            <div className="ui-mut ui-sm">Uso hoje</div>
            <div className="hm-num">{formatTokensPt(today.tokens)}</div>
            <div className="ui-sm ui-mut">tokens · {formatUsdPt(today.cost)} em API</div>
          </button>
        </div>

        <div className="hm-cols">
          <div className="hm-card hm-list">
            <div className="ui-mut ui-sm hm-ttl">Projetos recentes</div>
            {recent.length === 0 && <div className="ui-empty">Nenhum projeto. Use "Abrir pasta" para escolher a raiz.</div>}
            {recent.map(({ p, sessions, running }) => {
              const st = statuses[p.path];
              const dirty = !!st && st.changed_files.length > 0;
              return (
                <button key={p.path} className={samePath(p.path, selected) ? "ui-it on hm-it" : "ui-it hm-it"} onClick={() => openProject(p.path)} title={p.path}>
                  <StatusDot tone={running ? "ok" : dirty ? "warn" : undefined} title={running ? "agente rodando" : dirty ? "alterações pendentes" : undefined} />
                  <span className="grow">
                    {p.name}
                    {st?.branch && <span className="ui-mut"> · {st.branch}</span>}
                  </span>
                  <span className="meta">{sessions ? `${sessions} ${sessions === 1 ? "sessão" : "sessões"}` : "sem sessões"}</span>
                </button>
              );
            })}
          </div>
          <div className="hm-card hm-list hm-needs">
            <div className="ui-mut ui-sm hm-ttl">Precisa de você</div>
            {needs.length === 0 && <div className="ui-empty">Nada pendente.</div>}
            {needs.map((n) => (
              <button key={n.key} className="ui-it hm-need" onClick={n.run}>
                <StatusDot tone={n.tone} />
                <span className="grow">{n.text}</span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
