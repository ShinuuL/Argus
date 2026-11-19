/** Tela 01 · Início: o que precisa de você, o que está rodando, a última sessão e os projetos recentes. */
import { useEffect, useMemo } from "react";
import { baseName, formatTokensPt, formatUsdPt, isTaskActive, samePath, type Path } from "@/api";
import { useApp, useLayout, useWorkbench } from "@/store";
import { Btn, StatusDot, type Tone } from "@/ui";
import { ensureRunsSync, useRuns } from "@/views/builds";
import { sessionTitle } from "@/views/session/events";
import "./home.css";

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
  const last = [...tasks].filter((t) => !isTaskActive(t.state)).sort((a, b) => b.id - a.id)[0];
  const open = (id: number) => useWorkbench.getState().openSession(id);
  const setArea = useLayout.getState().setArea;

  // Sem raiz e sem sessões: só a chamada para começar.
  if (projects.length === 0 && tasks.length === 0) {
    return (
      <div className="ui-area">
        <div className="ui-area-main hm">
          <div className="hm-start">
            <span className="ui-mut">Nenhum projeto aberto.</span>
            <Btn onClick={() => void useApp.getState().pickRoot()}>Abrir pasta raiz</Btn>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="ui-area">
      <div className="ui-area-main hm">
        <div className="hm-head">
          <p className="hm-sum">
            <button className="hm-link" onClick={() => setArea("agents")}>
              <b className={active.length ? "hm-ok" : undefined}>{active.length}</b> {active.length === 1 ? "agente rodando" : "agentes rodando"}
            </button>
            {waiting > 0 && (
              <>
                <span className="hm-dot">·</span>
                <b className="hm-warn">{waiting}</b> aguardando você
              </>
            )}
            <span className="hm-dot">·</span>
            <button className="hm-link" onClick={() => setArea("servers")}>
              <b>{servers.length}</b> {servers.length === 1 ? "servidor" : "servidores"}
            </button>
            <span className="hm-dot">·</span>
            <button className="hm-link" onClick={() => setArea("metrics")}>
              <b>{formatTokensPt(today.tokens)}</b> tokens hoje ({formatUsdPt(today.cost)})
            </button>
          </p>
          <span className="hm-sp" />
          <Btn onClick={newChat}>Nova conversa</Btn>
        </div>

        {needs.length > 0 && (
          <section className="hm-card hm-list hm-needs">
            <h2 className="hm-ttl">Precisa de você</h2>
            {needs.map((n) => (
              <button key={n.key} className="ui-it hm-need" onClick={n.run}>
                <StatusDot tone={n.tone} />
                <span className="grow">{n.text}</span>
              </button>
            ))}
          </section>
        )}

        <div className="hm-cols">
          <section className="hm-card hm-list">
            <h2 className="hm-ttl">Em andamento</h2>
            {active.length === 0 && <div className="ui-empty">Nenhum agente rodando.</div>}
            {active.map((t) => (
              <button key={t.id} className="ui-it hm-it" onClick={() => open(t.id)} title={t.project}>
                <StatusDot tone={attention[t.id] ? "warn" : "info"} />
                <span className="grow">#{t.id} {sessionTitle(t.prompt, 60)}</span>
                <span className="meta">{baseName(t.project)}</span>
              </button>
            ))}
            {last && (
              <div className="hm-resume">
                <span className="grow ui-mut">
                  Última: <span className="hm-resume-t">#{last.id} {sessionTitle(last.prompt, 50)}</span> · {baseName(last.project)}
                </span>
                <Btn variant="ghost" onClick={() => open(last.id)}>Retomar</Btn>
              </div>
            )}
          </section>
          <section className="hm-card hm-list">
            <h2 className="hm-ttl">Projetos recentes</h2>
            {recent.length === 0 && <div className="ui-empty">Nenhum projeto na raiz atual.</div>}
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
          </section>
        </div>
      </div>
    </div>
  );
}
