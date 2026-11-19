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
import { Btn, Chip } from "@/ui";
import "./inspector.css";
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

export interface DiffFile {
  file: string;
  add: number;
  del: number;
  /** A (novo), D (removido) ou M (alterado). */
  kind: "A" | "D" | "M";
  /** Linhas do diff deste arquivo (a partir do cabeçalho "diff --git"). */
  lines: string[];
}

/** Arquivos citados num diff unificado, com linhas somadas e removidas. */
export function diffFiles(diff: string): DiffFile[] {
  const out: DiffFile[] = [];
  for (const line of diff.split("\n")) {
    const m = /^diff --git a\/(.+?) b\/(.+)$/.exec(line);
    if (m) {
      out.push({ file: m[2], add: 0, del: 0, kind: "M", lines: [line] });
      continue;
    }
    const cur = out[out.length - 1];
    if (!cur) continue;
    cur.lines.push(line);
    if (line.startsWith("new file mode")) cur.kind = "A";
    else if (line.startsWith("deleted file mode")) cur.kind = "D";
    else if (line.startsWith("+") && !line.startsWith("+++")) cur.add++;
    else if (line.startsWith("-") && !line.startsWith("---")) cur.del++;
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
      {groupByTopFolder(files).map((g) => (
        <ChangeGroup key={g.dir} project={project} group={g} />
      ))}
    </div>
  );
}

/** Quantos arquivos uma pasta mostra antes do "Ver todos". */
const GROUP_PAGE = 50;

interface FolderGroup {
  dir: string;
  files: [string, FileStatus][];
}

/** Agrupa pela pasta de primeiro nível; arquivos da raiz ficam em "(raiz)". Maiores primeiro. */
function groupByTopFolder(files: [string, FileStatus][]): FolderGroup[] {
  const map = new Map<string, [string, FileStatus][]>();
  for (const f of files) {
    const rel = f[0].replace(/\\/g, "/");
    const cut = rel.indexOf("/");
    const dir = cut > 0 ? rel.slice(0, cut) : "(raiz)";
    const list = map.get(dir);
    if (list) list.push(f);
    else map.set(dir, [f]);
  }
  return [...map]
    .map(([dir, list]) => ({ dir, files: list }))
    .sort((a, b) => b.files.length - a.files.length || a.dir.localeCompare(b.dir));
}

/** Uma pasta: cabeçalho com contagem que abre e fecha; pasta com mais de 20 começa fechada. */
function ChangeGroup({ project, group }: { project: Path; group: FolderGroup }) {
  const [open, setOpen] = useState(group.files.length <= 20);
  const [all, setAll] = useState(false);
  const shown = all ? group.files : group.files.slice(0, GROUP_PAGE);
  return (
    <div className="df-group">
      <button className="df-group-head" aria-expanded={open} onClick={() => setOpen(!open)}>
        <Icon name={open ? "chevronDown" : "chevronRight"} size={12} />
        <span className="ellipsis">{group.dir}</span>
        <span className="rp-count">{group.files.length}</span>
      </button>
      {open &&
        shown.map(([rel, st]) => (
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
      {open && !all && group.files.length > GROUP_PAGE && (
        <button className="df-more" onClick={() => setAll(true)}>
          Ver todos os {group.files.length}
        </button>
      )}
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
          <span className="rp-head-actions">
            <button
              className="wb-sbtn"
              title="Abrir o diff num painel (pode ficar ao lado do chat e ser comentado por linha)"
              aria-label="Abrir o diff num painel"
              onClick={() => useWorkbench.getState().openDiff(taskId)}
            >
              <Icon name="splitH" size={14} />
            </button>
            <button className="wb-sbtn" title="Recarregar o diff" aria-label="Recarregar o diff" onClick={() => setReload((n) => n + 1)}>
              <Icon name="refresh" size={14} />
            </button>
          </span>
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
  // Sem diff (erro ao ler o worktree, ou ainda calculando) não há o que revisar: nada de Aplicar.
  const canAct = finished && hasWorktree && busy === null && !error && diff !== null;
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
    <div className="ins">
     <div className="rp-body ins-body">
      <div className="rp-head">
        <b className="ellipsis">Sessão #{taskId}</b>
        <span className="rp-head-actions">
          <button
            className="wb-sbtn"
            title="Abrir o diff num painel (pode ficar ao lado do chat e ser comentado por linha)"
            aria-label="Abrir o diff num painel"
            onClick={() => useWorkbench.getState().openDiff(taskId)}
          >
            <Icon name="splitH" size={14} />
          </button>
          <button className="wb-sbtn" title="Recarregar o diff" aria-label="Recarregar o diff" onClick={() => setReload((n) => n + 1)}>
            <Icon name="refresh" size={14} />
          </button>
          <Btn variant="ghost" small title="Abrir a revisão completa na área Git" onClick={() => useLayout.getState().showChanges()}>
            Revisar
          </Btn>
        </span>
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
          {/os error 267|os error 3|not a git repository/i.test(error)
            ? `O worktree desta sessão não existe mais (${task.workdir ?? "pasta"}). Não há o que aplicar; descarte a sessão para limpar.`
            : error}
        </div>
      )}
      {diff === null && !error && !closed && <p className="rp-empty">Calculando o diff…</p>}
      {diff !== null && !diff.trim() && <p className="rp-empty">Sem alterações nesta sessão.</p>}
      <DiffBody diff={diff} files={files} />
     </div>
      <div className="ins-foot">
        <Btn
          variant="ghost"
          // Descartar continua possível com o diff quebrado: é o jeito de limpar a sessão.
          disabled={!(finished && hasWorktree && busy === null)}
          title="Descarta as alterações desta sessão (remove o worktree)"
          onClick={() =>
            void run(
              "discard",
              ["Descartar alterações", "Descartar as alterações desta sessão? Isso não pode ser desfeito."],
              () => discardTask(taskId),
            )
          }
        >
          {busy === "discard" ? "Descartando…" : "Descartar"}
        </Btn>
        <span className="ins-sp" />
        <Btn
          variant="primary"
          disabled={!canAct}
          title="Aplica todas as alterações desta sessão no projeto"
          onClick={() =>
            void run("apply", ["Aplicar alterações", "Aplicar as alterações desta sessão no projeto?"], () => applyTask(taskId))
          }
        >
          {busy === "apply" ? "Aplicando…" : "Aplicar tudo"}
        </Btn>
      </div>
    </div>
  );
}

/** Lista de arquivos (chip A/M/D, +/−) e o diff resumido do arquivo escolhido. */
function DiffBody({ diff, files }: { diff: string | null; files: DiffFile[] }) {
  const [pick, setPick] = useState<string | null>(null);
  const cur = files.find((f) => f.file === pick) ?? files[0];
  if (!diff?.trim() || !cur) return null;
  // Resumido: sem as linhas de cabeçalho do git, só os trechos.
  const body = cur.lines.filter((l) => !/^(diff --git|index |\+\+\+|---|new file mode|deleted file mode|similarity|rename )/.test(l));
  return (
    <>
      <div className="ins-files" role="listbox" aria-label="Arquivos alterados">
        {files.map((f) => (
          <button
            key={f.file}
            role="option"
            aria-selected={f === cur}
            className={f === cur ? "ins-file on" : "ins-file"}
            title={f.file}
            onClick={() => setPick(f.file)}
          >
            <Chip tone={f.kind === "A" ? "ok" : f.kind === "D" ? "bad" : undefined}>{f.kind}</Chip>
            <span className="ins-file-name">{f.file.split("/").slice(-2).join("/")}</span>
            {f.add > 0 && <span className="ins-add">+{f.add}</span>}
            {f.del > 0 && <span className="ins-del">−{f.del}</span>}
          </button>
        ))}
      </div>
      <div className="ins-diff" aria-label={`Diff de ${cur.file}`}>
        {body.map((line, i) => (
          <div key={i} className={lineClass(line)}>
            {line || " "}
          </div>
        ))}
      </div>
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
