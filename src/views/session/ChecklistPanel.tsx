/**
 * Inspetor › Pendências: o check-list do markdown do projeto (PROXIMOS_PASSOS.md,
 * PENDENCIAS.md ou TODO.md), com feito/total por seção, a lista de tarefas
 * (TodoWrite) do agente no turno e o item que ele está mexendo agora.
 * Clicar num item aberto põe "Trabalhe na pendência: …" na caixa da conversa.
 */
import { useEffect, useMemo, useState } from "react";
import { api, errorMessage, isTaskActive, type Checklist, type ChecklistItem, type Path, type TaskId } from "@/api";
import { Icon } from "@/shell/icons";
import { useApp, useLayout } from "@/store";
import { sessionTitle } from "./events";
import { agentTodos, checklistTouch, itemKey, type AgentTodo } from "./checklist";
import { useTaskEvents } from "./useTaskEvents";
import "./checklist.css";

/** Texto curto o bastante para virar pedido direto ao agente. */
const MAX_PROMPT_TEXT = 300;

export function ChecklistPanel({ taskId, project }: { taskId: TaskId | null; project: Path | null }) {
  const task = useApp((s) => (taskId == null ? undefined : s.tasks.find((t) => t.id === taskId)));
  const busy = task ? isTaskActive(task.state) : false;
  const { events } = useTaskEvents(taskId, busy);
  // A sessão pode rodar num worktree isolado: o arquivo dela é o que o agente edita.
  const dir = task?.workdir ?? task?.project ?? project;
  const watchRev = useApp((s) => (project ? s.statuses[project] : undefined));
  const [list, setList] = useState<Checklist | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const todos = useMemo(() => (taskId == null ? null : agentTodos(events)), [taskId, events]);
  const touch = useMemo(() => (list ? checklistTouch(events, list.name) : null), [events, list]);
  // Relê quando o watcher avisa mudança no projeto ou quando o agente mexe no arquivo.
  const touchRev = touch ? events.length : 0;

  useEffect(() => {
    if (!dir) return;
    let alive = true;
    const timer = setTimeout(
      () =>
        api.projectChecklist(dir).then(
          (l) => {
            if (!alive) return;
            setList(l);
            setError(null);
            setLoaded(true);
          },
          (e) => {
            if (!alive) return;
            setError(errorMessage(e));
            setLoaded(true);
          },
        ),
      touchRev ? 400 : 0,
    );
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [dir, watchRev, touchRev]);

  useEffect(() => {
    setList(null);
    setLoaded(false);
  }, [dir]);

  if (!dir) return <p className="rp-empty">Abra um projeto ou uma sessão para ver as pendências.</p>;

  const touched = new Set(touch?.keys ?? []);
  const doing = todos?.find((t) => t.status === "in_progress") ?? null;
  const current = touch && touch.keys.length > 0 ? findItem(list, touch.keys[0]) : null;
  const total = list ? list.sections.reduce((n, s) => n + s.items.length, 0) : 0;
  const done = list ? list.sections.reduce((n, s) => n + s.items.filter((i) => i.done).length, 0) : 0;

  return (
    <div className="rp-body ck">
      {task && (
        <div className={`ck-now${busy ? " live" : ""}`} aria-live="polite">
          <span className="ck-now-label">{busy ? "Trabalhando agora" : "Última sessão"}</span>
          <b className="ellipsis" title={task.prompt}>
            {sessionTitle(task.prompt, 60)}
          </b>
          {doing ? (
            <span className="ck-now-item">{doing.activeForm || doing.content}</span>
          ) : current ? (
            <span className="ck-now-item">{current.text}</span>
          ) : (
            <span className="ck-now-idle">{busy ? "Sem item marcado neste turno." : "Nenhum item em andamento."}</span>
          )}
        </div>
      )}

      {todos && todos.length > 0 && <AgentTodos todos={todos} />}

      <div className="rp-head">
        <b className="ellipsis" title={list?.path}>
          {list ? list.name : "Pendências"}
        </b>
        {list && total > 0 && (
          <span className="ck-sum" aria-label={`${done} de ${total} feitos`}>
            {done}/{total}
          </span>
        )}
      </div>
      {error && <p className="err">{error}</p>}
      {!loaded && !error && <p className="rp-empty">Carregando…</p>}
      {loaded && !list && !error && (
        <p className="rp-empty">Nenhum PROXIMOS_PASSOS.md, PENDENCIAS.md ou TODO.md na raiz do projeto.</p>
      )}
      {list && total === 0 && <p className="rp-empty">{list.name} não tem itens no formato “- [ ] tarefa”.</p>}
      {touch?.whole && busy && <p className="rp-hint">O agente reescreveu {list?.name} neste turno.</p>}
      {list?.sections.map((s, i) => (
        <Section key={`${i}:${s.title}`} title={s.title || "Sem título"} items={s.items} touched={touched} />
      ))}
      {list && total > 0 && <p className="rp-hint">Clique num item aberto para pedir ao agente que trabalhe nele.</p>}
    </div>
  );
}

function findItem(list: Checklist | null, key: string): ChecklistItem | null {
  for (const s of list?.sections ?? []) for (const it of s.items) if (itemKey(it.text) === key) return it;
  return null;
}

function AgentTodos({ todos }: { todos: AgentTodo[] }) {
  const done = todos.filter((t) => t.status === "completed").length;
  return (
    <section className="ck-sec ck-todos" aria-label="Tarefas do agente neste turno">
      <div className="ck-sec-head">
        <span className="ellipsis">Tarefas do agente</span>
        <span className="ck-sum">
          {done}/{todos.length}
        </span>
      </div>
      <Bar done={done} total={todos.length} />
      <ul className="ck-list">
        {todos.map((t, i) => (
          <li key={i} className={`ck-item ${t.status}`}>
            <Mark state={t.status === "completed" ? "done" : t.status === "in_progress" ? "doing" : "open"} />
            <span className="ck-text">{t.status === "in_progress" && t.activeForm ? t.activeForm : t.content}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function Section({ title, items, touched }: { title: string; items: ChecklistItem[]; touched: Set<string> }) {
  const done = items.filter((i) => i.done).length;
  const complete = done === items.length;
  const [open, setOpen] = useState(!complete);
  return (
    <section className="ck-sec">
      <button className="ck-sec-head" aria-expanded={open} onClick={() => setOpen(!open)}>
        <span className="ellipsis" title={title}>
          {title}
        </span>
        <span className={`ck-sum${complete ? " ok" : ""}`}>
          {done}/{items.length}
        </span>
      </button>
      <Bar done={done} total={items.length} />
      {open && (
        <ul className="ck-list">
          {items.map((it) => (
            <Item key={it.line} item={it} hot={touched.has(itemKey(it.text))} />
          ))}
        </ul>
      )}
    </section>
  );
}

function Item({ item, hot }: { item: ChecklistItem; hot: boolean }) {
  const cls = `ck-item${item.done ? " completed" : ""}${hot ? " hot" : ""}`;
  const style = item.depth > 0 ? { paddingLeft: 6 + Math.min(item.depth, 4) * 14 } : undefined;
  const canAsk = !item.done && item.text.length <= MAX_PROMPT_TEXT;
  const body = (
    <>
      <Mark state={item.done ? "done" : hot ? "doing" : "open"} />
      <span className="ck-text">{item.text}</span>
    </>
  );
  return (
    <li className={cls} style={style}>
      {canAsk ? (
        <button
          className="ck-ask"
          title={`Linha ${item.line}. Clique para pôr na caixa da conversa.`}
          onClick={() => useLayout.getState().insertInComposer(`Trabalhe na pendência: ${item.text}`)}
        >
          {body}
        </button>
      ) : (
        <span className="ck-ask" title={`Linha ${item.line}`}>
          {body}
        </span>
      )}
    </li>
  );
}

function Mark({ state }: { state: "open" | "doing" | "done" }) {
  return (
    <span className={`ck-mark ${state}`} aria-label={state === "done" ? "feito" : state === "doing" ? "em andamento" : "aberto"}>
      {state === "done" && <Icon name="check" size={10} />}
    </span>
  );
}

function Bar({ done, total }: { done: number; total: number }) {
  const pct = total > 0 ? Math.round((done / total) * 100) : 0;
  return (
    <div className="ck-bar" role="progressbar" aria-valuemin={0} aria-valuemax={total} aria-valuenow={done}>
      <span style={{ width: `${pct}%` }} />
    </div>
  );
}
