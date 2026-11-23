/**
 * "Minhas tarefas" no painel Agentes: lista escrita pelo usuário. Vai ao agente
 * no começo de cada pedido, e ele marca um item como feito escrevendo
 * `argus-feito: N` na resposta (ver `crates/core/src/agents/todos.rs`).
 */
import { useEffect, useRef, useState } from "react";
import { api, errorMessage, type TaskId, type Todo } from "@/api";
import { Icon } from "@/shell/icons";

export function TodoList({ taskId, busy }: { taskId: TaskId; busy: boolean }) {
  const [list, setList] = useState<Todo[]>([]);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);

  // Relê ao abrir, a cada 3 s com o turno rodando (o agente pode marcar) e quando ele para.
  useEffect(() => {
    let alive = true;
    const load = () => api.taskTodos(taskId).then((l) => alive && setList(l), () => undefined);
    void load();
    const timer = busy ? setInterval(load, 3000) : null;
    return () => {
      alive = false;
      if (timer) clearInterval(timer);
    };
  }, [taskId, busy]);

  const save = async (next: Todo[]) => {
    setList(next);
    try {
      setList(await api.taskSetTodos(taskId, next));
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const add = () => {
    const t = text.trim();
    if (!t) return;
    setText("");
    void save([...list, { text: t, done: false }]);
    input.current?.focus();
  };

  const pending = list.filter((t) => !t.done).length;

  return (
    <section className="ap-todos" aria-label="Minhas tarefas">
      <header className="ap-head">
        <div className="ap-head-t">
          <h3>Minhas tarefas</h3>
          <span className="ap-sub">
            {list.length === 0 ? "o agente vê a lista a cada pedido" : `${pending} pendente${pending === 1 ? "" : "s"} de ${list.length}`}
          </span>
        </div>
        <button
          className="ap-ibtn"
          title="Tirar as concluídas da lista"
          aria-label="Tirar as concluídas"
          disabled={list.length === pending}
          onClick={() => void save(list.filter((t) => !t.done))}
        >
          <Icon name="trash" size={14} />
        </button>
      </header>

      <form
        className="ap-todo-add"
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <input
          ref={input}
          value={text}
          placeholder="Nova tarefa (Enter para adicionar)"
          aria-label="Nova tarefa"
          maxLength={300}
          onChange={(e) => setText(e.target.value)}
        />
        <button type="submit" className="ap-ibtn" title="Adicionar" aria-label="Adicionar" disabled={!text.trim()}>
          <Icon name="plus" size={14} />
        </button>
      </form>

      {list.length > 0 && (
        <ul className="ap-todo-list" role="list">
          {list.map((t, i) => (
            <li key={i} className={t.done ? "ap-todo done" : "ap-todo"}>
              <label>
                <input
                  type="checkbox"
                  checked={t.done}
                  onChange={() => void save(list.map((x, j) => (j === i ? { ...x, done: !x.done } : x)))}
                />
                <span className="ap-todo-n">{i + 1}.</span>
                <span className="ap-todo-t">{t.text}</span>
              </label>
              <button
                className="ap-ibtn"
                title="Remover"
                aria-label={`Remover a tarefa ${i + 1}`}
                onClick={() => void save(list.filter((_, j) => j !== i))}
              >
                <Icon name="close" size={12} />
              </button>
            </li>
          ))}
        </ul>
      )}
      {error && <p className="ap-hint err">{error}</p>}
    </section>
  );
}
