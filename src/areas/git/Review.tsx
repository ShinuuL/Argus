/**
 * Tela 05: revisão das alterações da sessão em foco, em tela cheia. Lista de
 * arquivos com checkbox, mensagem do commit, diff lado a lado ou unificado e
 * as ações Aplicar e commitar / Descartar / Voltar à conversa.
 *
 * O backend aplica a sessão inteira (commit "argus: tarefa N" no worktree e
 * merge no projeto): por isso Aplicar só libera com todos os arquivos marcados
 * e a mensagem do commit é mostrada, não editada.
 */
import { useEffect, useMemo, useState } from "react";
import { api, baseName, errorMessage, isTaskActive, isTaskFinished, samePath, type Path, type TaskId } from "@/api";
import { useApp, useLayout, useWorkbench } from "@/store";
import { applyTask, discardTask } from "@/views/session/actions";
import { Btn, Chip, AreaStatus } from "@/ui";
import { parseDiff, sideBySide, type FileDiff, type Row } from "./diff";

type Mode = "split" | "unified";

export function useSessionDiff(taskId: TaskId | null) {
  const task = useApp((s) => (taskId == null ? undefined : s.tasks.find((t) => t.id === taskId)));
  const rev = useLayout((s) => s.diffRev);
  const [diff, setDiff] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  const state = task?.state;
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
  }, [taskId, rev, reload, state, ended]);

  const files = useMemo(() => (diff ? parseDiff(diff) : []), [diff]);
  return { task, diff, files, error, ended, reload: () => setReload((n) => n + 1) };
}

function Num({ n }: { n?: number }) {
  return <span className="gt-no">{n ?? ""}</span>;
}

function Line({ r, side }: { r: Row | null; side?: "old" | "new" }) {
  if (!r) return <div className="gt-ln gt-blank"> </div>;
  if (r.kind === "hunk") return <div className="gt-ln gt-hunk">⋯ {r.text}</div>;
  const cls = r.kind === "add" ? "a" : r.kind === "del" ? "d" : "";
  return (
    <div className={`gt-ln ${cls}`}>
      {side ? <Num n={side === "old" ? r.old : r.new} /> : (
        <>
          <Num n={r.old} />
          <Num n={r.new} />
        </>
      )}
      <span className="gt-sign">{r.kind === "add" ? "+" : r.kind === "del" ? "−" : " "}</span>
      {r.text}
    </div>
  );
}

function DiffView({ file, mode }: { file: FileDiff; mode: Mode }) {
  if (file.binary) return <p className="ui-empty">Arquivo binário: sem diff de texto.</p>;
  if (!file.rows.length) return <p className="ui-empty">Sem linhas alteradas (renomeação ou mudança de modo).</p>;
  if (mode === "unified")
    return (
      <div className="gt-diff gt-uni">
        {file.rows.map((r, i) => (
          <Line key={i} r={r} />
        ))}
      </div>
    );
  const pairs = sideBySide(file.rows);
  return (
    <div className="gt-diff gt-split">
      <div>
        {pairs.map(([l], i) => (
          <Line key={i} r={l} side="old" />
        ))}
      </div>
      <div>
        {pairs.map(([, r], i) => (
          <Line key={i} r={r} side="new" />
        ))}
      </div>
    </div>
  );
}

export function Review({ taskId, project }: { taskId: TaskId | null; project: Path | null }) {
  const { task, diff, files, error, ended, reload } = useSessionDiff(taskId);
  const status = useApp((s) => (project ? s.statuses[project] : undefined));
  const [sel, setSel] = useState<string | null>(null);
  const [off, setOff] = useState<Record<string, true>>({});
  const [mode, setMode] = useState<Mode>("split");
  const [busy, setBusy] = useState<"apply" | "discard" | null>(null);
  const [actErr, setActErr] = useState<string | null>(null);

  useEffect(() => {
    setOff({});
    setSel(null);
  }, [taskId]);

  const cur = files.find((f) => f.path === sel) ?? files[0] ?? null;
  const included = files.filter((f) => !off[f.path]).length;
  const backToChat = () => (taskId != null ? useWorkbench.getState().openSession(taskId) : useLayout.getState().setArea("chat"));

  // Sem sessão em foco: só a lista de arquivos alterados da pasta (o diff da pasta não tem comando próprio).
  if (taskId == null || !task) {
    const changed = status?.changed_files ?? [];
    return (
      <div className="gt-review">
        <div className="gt-side">
          <h4>Alterações na pasta · {changed.length}</h4>
          {changed.length === 0 && <p className="ui-empty">{project ? "Nada alterado em relação ao último commit." : "Selecione um projeto."}</p>}
          {changed.map(([p, st]) => (
            <div key={p} className="ui-it">
              <span className="grow ui-mono ui-sm" title={p}>{p}</span>
              <span className="meta">{st}</span>
            </div>
          ))}
        </div>
        <div className="gt-center">
          <p className="ui-empty">
            Abra uma conversa para revisar o diff da sessão aqui.
            <br />
            <Btn variant="ghost" small onClick={backToChat}>Voltar à conversa</Btn>
          </p>
        </div>
      </div>
    );
  }

  const direct = task.mode === "Direct";
  const hasWorktree = task.workdir != null && !samePath(task.workdir, task.project);
  const allOn = included === files.length;
  const canAct = !direct && !ended && isTaskFinished(task.state) && hasWorktree && busy === null && files.length > 0;
  const why = direct
    ? "Sessão direto na pasta: as alterações já valem, não há o que aplicar."
    : ended
      ? task.state === "Applied" ? "Sessão encerrada: as alterações já foram aplicadas." : "Sessão encerrada: as alterações foram descartadas."
      : isTaskActive(task.state)
        ? "Aguarde o turno terminar para aplicar."
        : !hasWorktree
          ? "Esta sessão não tem worktree próprio."
          : !allOn
            ? "O Argus aplica a sessão inteira: marque todos os arquivos para aplicar."
            : "";

  const run = async (kind: "apply" | "discard") => {
    if (kind === "apply") {
      const ok = await api
        .systemConfirm("Aplicar e commitar", `Commitar as alterações da sessão #${taskId} e fazer o merge em ${baseName(task.project)}?`)
        .catch(() => false);
      if (!ok) return;
    }
    setBusy(kind);
    try {
      if (kind === "apply") await applyTask(taskId);
      else await discardTask(taskId);
      setActErr(null);
    } catch (e) {
      setActErr(errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <div className="gt-rhead">
        <b>Revisar {files.length} {files.length === 1 ? "arquivo" : "arquivos"}</b>
        <span className="ui-mut">Sessão #{taskId} · {baseName(task.project)}</span>
        {why && <Chip tone={direct || ended ? undefined : "warn"}>{why}</Chip>}
        <span className="ui-sp" />
        <Btn variant="ghost" small onClick={reload} title="Recalcular o diff">Recarregar</Btn>
        <Btn variant="ghost" onClick={backToChat}>Voltar à conversa</Btn>
        {!direct && (
          <Btn variant="bad" disabled={!canAct} onClick={() => void run("discard")}>
            {busy === "discard" ? "Descartando…" : "Descartar"}
          </Btn>
        )}
        {!direct && (
          <Btn disabled={!canAct || !allOn} onClick={() => void run("apply")}>
            {busy === "apply" ? "Aplicando…" : "Aplicar e commitar"}
          </Btn>
        )}
      </div>
      {(error || actErr) && <div className="gt-err" role="alert">{actErr ?? error}</div>}
      <div className="gt-review">
        <div className="gt-side">
          <h4>Alterações · {files.length}</h4>
          {diff === null && !error && !ended && <p className="ui-empty">Calculando o diff…</p>}
          {diff !== null && files.length === 0 && <p className="ui-empty">Sem alterações nesta sessão.</p>}
          {files.map((f) => (
            <div key={f.path} className={`ui-it gt-file${cur?.path === f.path ? " on" : ""}`} onClick={() => setSel(f.path)}>
              <input
                type="checkbox"
                aria-label={`Incluir ${f.path}`}
                checked={!off[f.path]}
                disabled={direct}
                onClick={(e) => e.stopPropagation()}
                onChange={(e) =>
                  setOff((o) => {
                    const n = { ...o };
                    if (e.target.checked) delete n[f.path];
                    else n[f.path] = true;
                    return n;
                  })
                }
              />
              <button type="button" className="grow ui-mono ui-sm gt-fname" title={f.path}>{f.path}</button>
              <span className="ui-sm gt-add">+{f.add}</span>
              {f.del > 0 && <span className="ui-sm gt-del">−{f.del}</span>}
            </div>
          ))}
          {!direct && (
            <>
              <h4 className="gt-h4">Mensagem do commit</h4>
              <textarea
                className="gt-msg"
                readOnly
                value={`argus: tarefa ${taskId}`}
                title="Mensagem definida pelo Argus ao aplicar a sessão"
                aria-label="Mensagem do commit"
              />
              <p className="ui-mut ui-sm gt-note">Definida pelo Argus; o merge no projeto usa --no-ff.</p>
            </>
          )}
        </div>
        <div className="gt-center">
          <div className="gt-ftop">
            <span className="ui-mono ellipsis">{cur?.path ?? "—"}</span>
            {cur && (
              <span className="ui-sm">
                <span className="gt-add">+{cur.add}</span> <span className="gt-del">−{cur.del}</span>
              </span>
            )}
            <span className="ui-sp" />
            <div className="ui-tabs gt-modes" role="tablist">
              <button type="button" role="tab" aria-selected={mode === "split"} className={`ui-tab${mode === "split" ? " on" : ""}`} onClick={() => setMode("split")}>
                Lado a lado
              </button>
              <button type="button" role="tab" aria-selected={mode === "unified"} className={`ui-tab${mode === "unified" ? " on" : ""}`} onClick={() => setMode("unified")}>
                Unificado
              </button>
            </div>
          </div>
          <div className="gt-scroll">{cur ? <DiffView file={cur} mode={mode} /> : <p className="ui-empty">{ended ? "Sessão encerrada: não há diff." : "Nenhum arquivo para mostrar."}</p>}</div>
        </div>
      </div>
      <AreaStatus>
        <span>{files.length ? `${included} de ${files.length} arquivos selecionados` : "Sem arquivos"}</span>
        <span className="ui-sp" />
        <span className="ui-mono">{status?.branch ?? ""}</span>
      </AreaStatus>
    </>
  );
}
