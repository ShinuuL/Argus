/**
 * Abas do painel direito que dependem da sessão ativa: Alterações (diff da
 * tarefa, com Aplicar e Descartar) e Subagentes (árvore; clicar abre o chat
 * somente leitura do subagente na aba da conversa).
 */
import { useEffect, useMemo, useState } from "react";
import {
  api,
  baseName,
  errorMessage,
  isTaskActive,
  isTaskFinished,
  samePath,
  type FileStatus,
  type Path,
  type TaskId,
} from "@/api";
import { Icon } from "@/shell/icons";
import { useApp, useLayout, useWorkbench } from "@/store";
import { applyTask, discardTask } from "./actions";
import { subagentTree } from "./events";
import { SubagentTree } from "./SubagentTree";
import { useTaskEvents } from "./useTaskEvents";

const STATUS_LETTER: Record<FileStatus, string> = {
  New: "A",
  Untracked: "U",
  Modified: "M",
  Renamed: "R",
  TypeChange: "T",
  Deleted: "D",
  Conflicted: "!",
};

/** Arquivos citados num diff unificado, com linhas somadas e removidas. */
function diffFiles(diff: string): { file: string; add: number; del: number }[] {
  const out: { file: string; add: number; del: number }[] = [];
  for (const line of diff.split("\n")) {
    const m = /^diff --git a\/(.+?) b\/(.+)$/.exec(line);
    if (m) out.push({ file: m[2], add: 0, del: 0 });
    else if (out.length && line.startsWith("+") && !line.startsWith("+++")) out[out.length - 1].add++;
    else if (out.length && line.startsWith("-") && !line.startsWith("---")) out[out.length - 1].del++;
  }
  return out;
}

function lineClass(line: string): string {
  if (line.startsWith("diff --git") || line.startsWith("index ") || line.startsWith("+++") || line.startsWith("---")) return "df-meta";
  if (line.startsWith("@@")) return "df-hunk";
  if (line.startsWith("+")) return "df-add";
  if (line.startsWith("-")) return "df-del";
  return "";
}

/** Sem sessão ativa: os arquivos alterados do projeto (status git). */
function ProjectChanges({ project }: { project: Path | null }) {
  const status = useApp((s) => (project ? s.statuses[project] : undefined));
  if (!project) return <p className="rp-empty">Abra uma sessão para ver as alterações dela.</p>;
  const files = status?.changed_files ?? [];
  return (
    <div className="rp-body">
      <div className="rp-head">
        <b className="ellipsis">Projeto {baseName(project)}</b>
        <span className="rp-count">{files.length}</span>
      </div>
      <p className="rp-hint">Abra uma sessão para ver o diff dela. Abaixo, os arquivos alterados no projeto (git).</p>
      {files.length === 0 && <p className="rp-empty">Nenhum arquivo alterado.</p>}
      {files.map(([rel, st]) => (
        <button
          key={rel}
          className="df-file"
          title={`Abrir ${rel}`}
          onClick={() => useWorkbench.getState().openFile(`${project}/${rel}`.replace(/\\/g, "/"), true)}
        >
          <span className="df-letter">{STATUS_LETTER[st]}</span>
          <span className="ellipsis">{rel}</span>
        </button>
      ))}
    </div>
  );
}

export function ChangesPanel({ taskId, project }: { taskId: TaskId | null; project: Path | null }) {
  const task = useApp((s) => (taskId == null ? undefined : s.tasks.find((t) => t.id === taskId)));
  const rev = useLayout((s) => s.diffRev);
  const [diff, setDiff] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"apply" | "discard" | null>(null);
  const [reload, setReload] = useState(0);
  const state = task?.state;
  /** Aplicada ou descartada: o worktree não existe mais, não há diff para calcular. */
  const ended = state === "Applied" || state === "Discarded";

  useEffect(() => {
    setDiff(null);
    setError(null);
    if (taskId == null || ended) return;
    let alive = true;
    api.taskDiff(taskId).then(
      (d) => alive && setDiff(d),
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
    // Recarrega quando o turno termina (o estado muda), no /diff e no botão.
  }, [taskId, rev, reload, state, ended]);

  const files = useMemo(() => (diff ? diffFiles(diff) : []), [diff]);

  if (taskId == null || !task) return <ProjectChanges project={project} />;

  if (task.mode === "Direct") {
    return (
      <div className="rp-body">
        <div className="rp-head">
          <b className="ellipsis">Sessão #{taskId} · direto na pasta</b>
          <button className="wb-sbtn" title="Recarregar o diff" aria-label="Recarregar o diff" onClick={() => setReload((n) => n + 1)}>
            <Icon name="refresh" size={14} />
          </button>
        </div>
        <div className="rp-direct" role="note">
          <Icon name="folderOpen" size={14} />
          <span>
            Esta sessão trabalha direto na pasta do projeto: as alterações já valem, <b>não há o que aplicar nem descartar</b>.
            Abaixo, o diff da pasta em relação ao último commit (inclui o que já estava alterado antes da sessão).
          </span>
        </div>
        {error && (
          <div className="err small" role="alert">
            {error}
          </div>
        )}
        {diff === null && !error && <p className="rp-empty">Calculando o diff…</p>}
        {diff !== null && !diff.trim() && <p className="rp-empty">Nenhuma alteração em relação ao último commit.</p>}
        <DiffBody diff={diff} files={files} />
      </div>
    );
  }

  const finished = isTaskFinished(task.state);
  const hasWorktree = task.workdir != null && !samePath(task.workdir, task.project);
  const canAct = finished && hasWorktree && busy === null;
  const closed = task.state === "Applied" || task.state === "Discarded";

  const run = async (kind: "apply" | "discard", ask: [string, string], action: () => Promise<unknown>) => {
    if (!(await api.systemConfirm(ask[0], ask[1]).catch(() => false))) return;
    setBusy(kind);
    try {
      await action();
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="rp-body">
      <div className="rp-head">
        <b className="ellipsis">Sessão #{taskId}</b>
        <button className="wb-sbtn" title="Recarregar o diff" aria-label="Recarregar o diff" onClick={() => setReload((n) => n + 1)}>
          <Icon name="refresh" size={14} />
        </button>
      </div>
      <div className="rp-actions">
        <button
          className="primary"
          disabled={!canAct}
          title="Aplica as alterações desta sessão no projeto"
          onClick={() =>
            void run("apply", ["Aplicar alterações", "Aplicar as alterações desta sessão no projeto?"], () => applyTask(taskId))
          }
        >
          <Icon name="check" size={13} /> {busy === "apply" ? "Aplicando…" : "Aplicar"}
        </button>
        <button
          className="danger-btn"
          disabled={!canAct}
          title="Descarta as alterações desta sessão (remove o worktree)"
          onClick={() =>
            void run(
              "discard",
              ["Descartar alterações", "Descartar as alterações desta sessão? Isso não pode ser desfeito."],
              () => discardTask(taskId),
            )
          }
        >
          <Icon name="close" size={13} /> {busy === "discard" ? "Descartando…" : "Descartar"}
        </button>
      </div>
      {!canAct && busy === null && (
        <p className="rp-hint">
          {closed
            ? task.state === "Applied"
              ? "Esta sessão já foi encerrada: as alterações foram aplicadas no projeto."
              : "Esta sessão já foi encerrada: as alterações foram descartadas."
            : isTaskActive(task.state)
              ? "Aguarde o turno terminar para aplicar ou descartar."
              : !hasWorktree
                ? "Esta sessão não tem worktree próprio."
                : ""}
        </p>
      )}
      {error && (
        <div className="err small" role="alert">
          {error}
        </div>
      )}
      {diff === null && !error && !closed && <p className="rp-empty">Calculando o diff…</p>}
      {diff !== null && !diff.trim() && <p className="rp-empty">Sem alterações nesta sessão.</p>}
      <DiffBody diff={diff} files={files} />
    </div>
  );
}

/** Lista de arquivos do diff e o diff colorido. */
function DiffBody({ diff, files }: { diff: string | null; files: { file: string; add: number; del: number }[] }) {
  return (
    <>
      {files.length > 0 && (
        <div className="df-files">
          {files.map((f) => (
            <div key={f.file} className="df-file static" title={f.file}>
              <span className="ellipsis">{f.file}</span>
              <span className="df-add">+{f.add}</span>
              <span className="df-del">−{f.del}</span>
            </div>
          ))}
        </div>
      )}
      {diff?.trim() && (
        <pre className="df-pre">
          {diff.split("\n").map((line, i) => (
            <span key={i} className={lineClass(line)}>
              {line + "\n"}
            </span>
          ))}
        </pre>
      )}
    </>
  );
}

export function SubagentsPanel({ taskId }: { taskId: TaskId | null }) {
  const task = useApp((s) => (taskId == null ? undefined : s.tasks.find((t) => t.id === taskId)));
  const selected = useApp((s) => (taskId == null ? null : (s.subagentOf[taskId] ?? null)));
  const busy = task ? isTaskActive(task.state) : false;
  const { events, times, loaded } = useTaskEvents(taskId, busy);
  const nodes = useMemo(() => subagentTree(events, times), [events, times]);
  const [now, setNow] = useState(() => Date.now());
  const ticking = busy && nodes.some((n) => !n.done);
  useEffect(() => {
    if (!ticking) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [ticking]);

  if (taskId == null) return <p className="rp-empty">Abra uma sessão para ver os subagentes dela.</p>;
  if (!loaded) return <p className="rp-empty">Carregando…</p>;
  return (
    <div className="rp-body">
      <SubagentTree
        nodes={nodes}
        selected={selected}
        taskActive={busy}
        now={now}
        onSelect={(id) => {
          useApp.getState().selectSubagent(taskId, id);
          useWorkbench.getState().openSession(taskId);
        }}
      />
      {nodes.length === 0 && <p className="rp-empty">Esta sessão não criou subagentes.</p>}
      {nodes.length > 0 && <p className="rp-hint">Clique num subagente para ver a conversa dele (somente leitura).</p>}
      <button
        className="rp-map-btn"
        title="Abre o Mapa de agentes desta sessão numa aba do editor"
        onClick={() => useWorkbench.getState().openAgentMap(taskId)}
      >
        <Icon name="tree" size={13} /> Abrir mapa
      </button>
    </div>
  );
}
