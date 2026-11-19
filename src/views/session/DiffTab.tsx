/**
 * Diff de uma sessão como aba do workbench (pode ir para qualquer grupo do
 * split). Usa as mesmas linhas +/− do chat (`Diff` de ToolRow), com o número da
 * linha: clicar nele abre um comentário, que vai para o rascunho da conversa
 * da sessão ("arquivo:linha — comentário"). Nada é enviado sozinho: o usuário
 * revisa e envia pela caixa de composição. Cada arquivo e cada trecho podem ser
 * rejeitados (voltam ao último commit), com confirmação.
 */
import { useEffect, useMemo, useState } from "react";
import { api, errorMessage, type TaskId } from "@/api";
import { Icon } from "@/shell/icons";
import { useApp, useLayout, useWorkbench } from "@/store";
import { chatKey, useChat } from "@/views/chat";
import { Diff, type DiffLine } from "./ToolRow";

interface DiffFile {
  path: string;
  add: number;
  del: number;
  lines: DiffLine[];
  /** Arquivo novo ou apagado: só a rejeição do arquivo inteiro. */
  whole: boolean;
  /** Trechos: cabeçalho `@@ -a,b +c,d @@` e índice da última linha do trecho em `lines`. */
  hunks: { header: string; last: number }[];
}

/** Diff unificado (git) em arquivos com linhas numeradas; cabeçalhos de hunk viram uma linha "…". */
export function parseUnifiedDiff(diff: string): DiffFile[] {
  const files: DiffFile[] = [];
  let cur: DiffFile | null = null;
  let oldNo = 0;
  let newNo = 0;
  for (const raw of diff.split("\n")) {
    const line = raw.replace(/\r$/, "");
    const head = /^diff --git a\/(.+?) b\/(.+)$/.exec(line);
    if (head) {
      cur = { path: head[2], add: 0, del: 0, lines: [], whole: false, hunks: [] };
      files.push(cur);
      continue;
    }
    if (!cur) continue;
    const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)$/.exec(line);
    if (hunk) {
      const prev = cur.hunks[cur.hunks.length - 1];
      if (prev) prev.last = cur.lines.length - 1;
      oldNo = Number(hunk[1]);
      newNo = Number(hunk[2]);
      if (cur.lines.length) cur.lines.push({ sign: " ", text: `⋯${hunk[3] ? ` ${hunk[3].trim()}` : ""}` });
      cur.hunks.push({ header: line.slice(0, line.indexOf(" @@", 2) + 3), last: -1 });
      continue;
    }
    if (/^(new|deleted) file mode|^Binary files/.test(line) || line === "--- /dev/null" || line === "+++ /dev/null") cur.whole = true;
    if (line.startsWith("+++") || line.startsWith("---") || line.startsWith("index ") || line.startsWith("\\")) continue;
    if (/^(new|deleted) file mode|^similarity|^rename |^old mode|^new mode|^Binary files/.test(line)) continue;
    if (line.startsWith("+")) {
      cur.add++;
      cur.lines.push({ sign: "+", text: line.slice(1), no: newNo++ });
    } else if (line.startsWith("-")) {
      cur.del++;
      cur.lines.push({ sign: "-", text: line.slice(1), no: oldNo++ });
    } else if (line.startsWith(" ")) {
      cur.lines.push({ sign: " ", text: line.slice(1), no: newNo++ });
      oldNo++;
    }
  }
  for (const f of files) {
    const prev = f.hunks[f.hunks.length - 1];
    if (prev) prev.last = f.lines.length - 1;
  }
  return files;
}

export default function DiffTab({ taskId }: { taskId: TaskId }) {
  const task = useApp((s) => s.tasks.find((t) => t.id === taskId));
  const rev = useLayout((s) => s.diffRev);
  const [diff, setDiff] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  /** Linha com a caixa de comentário aberta: arquivo e índice da linha. */
  const [editing, setEditing] = useState<{ file: string; i: number } | null>(null);
  const [text, setText] = useState("");
  const [sent, setSent] = useState<Record<string, true>>({});
  const state = task?.state;
  const ended = state === "Applied" || state === "Discarded";

  useEffect(() => {
    if (ended) return;
    let alive = true;
    api.taskDiff(taskId).then(
      (d) => {
        if (!alive) return;
        setDiff(d);
        setError(null);
      },
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [taskId, rev, reload, state, ended]);

  const files = useMemo(() => (diff ? parseUnifiedDiff(diff) : []), [diff]);
  const busy = state === "Running" || state === "Queued";
  const [reverting, setReverting] = useState(false);

  /** Rejeita um arquivo inteiro ou um trecho, com confirmação do sistema. */
  const reject = async (file: DiffFile, hunk?: string) => {
    const what = hunk ? `o trecho ${hunk} de ${file.path}` : file.whole ? `${file.path} (o arquivo novo será apagado)` : `todas as alterações em ${file.path}`;
    const ok = await api.systemConfirm("Rejeitar alterações", `Desfazer ${what}? Não dá para voltar atrás pelo Argus.`);
    if (!ok) return;
    setReverting(true);
    try {
      if (hunk) await api.taskRevertHunk(taskId, file.path, hunk);
      else await api.taskRevertFile(taskId, file.path);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setReverting(false);
      setReload((n) => n + 1);
    }
  };

  const addComment = (file: DiffFile, i: number) => {
    const body = text.trim();
    const line = file.lines[i];
    if (!body || !line) return;
    const key = chatKey(taskId);
    const chat = useChat.getState();
    const prev = chat.drafts[key] ?? "";
    const note = `${file.path}:${line.no ?? "?"} — ${body}`;
    chat.setDraft(key, prev.trim() ? `${prev.replace(/\s+$/, "")}\n${note}` : note);
    setSent((s) => ({ ...s, [`${file.path}#${i}`]: true }));
    setEditing(null);
    setText("");
  };

  return (
    <div className="dt scroll">
      <div className="dt-head">
        <Icon name="diff" size={15} />
        <b className="ellipsis">Alterações da sessão #{taskId}</b>
        <span className="dt-sum">
          {files.length} {files.length === 1 ? "arquivo" : "arquivos"}
          <span className="df-add">+{files.reduce((n, f) => n + f.add, 0)}</span>
          <span className="df-del">−{files.reduce((n, f) => n + f.del, 0)}</span>
        </span>
        <span className="dt-hint">Clique no número da linha para comentar; o comentário vai para o rascunho da conversa.</span>
        <button className="wb-sbtn" title="Abrir a conversa da sessão" aria-label="Abrir a conversa da sessão" onClick={() => useWorkbench.getState().openSession(taskId)}>
          <Icon name="chat" size={14} />
        </button>
        <button className="wb-sbtn" title="Recarregar o diff" aria-label="Recarregar o diff" onClick={() => setReload((n) => n + 1)}>
          <Icon name="refresh" size={14} />
        </button>
      </div>
      {error && (
        <div className="err small dt-msg" role="alert">
          {error}
        </div>
      )}
      {ended && <p className="rp-empty dt-msg">Sessão encerrada: não há mais diff para mostrar.</p>}
      {!ended && diff === null && !error && <p className="rp-empty dt-msg">Calculando o diff…</p>}
      {!ended && diff !== null && files.length === 0 && <p className="rp-empty dt-msg">Sem alterações nesta sessão.</p>}
      {!ended &&
        files.map((f) => (
          <section key={f.path} className="dt-file">
            <header className="dt-file-head" title={f.path}>
              <span className="ellipsis">{f.path}</span>
              <span className="df-add">+{f.add}</span>
              <span className="df-del">−{f.del}</span>
              <button
                type="button"
                className="wb-sbtn dt-reject"
                disabled={reverting || busy}
                title={busy ? "Aguarde o turno terminar." : f.whole ? "Rejeitar: apaga o arquivo novo (ou restaura o apagado)" : "Rejeitar todas as alterações deste arquivo"}
                onClick={() => void reject(f)}
              >
                Rejeitar arquivo
              </button>
            </header>
            <Diff
              lines={f.lines}
              numbered
              className="dt-diff"
              onLine={(i) => {
                setEditing({ file: f.path, i });
                setText("");
              }}
              after={(i) => {
                const h = !f.whole && f.hunks.length > 1 ? f.hunks.find((x) => x.last === i) : undefined;
                const rejectHunk = h ? (
                  <div className="dt-hunk-act">
                    <button
                      type="button"
                      className="wb-sbtn dt-reject"
                      disabled={reverting || busy}
                      title={`Desfazer só o trecho ${h.header}`}
                      onClick={() => void reject(f, h.header)}
                    >
                      Rejeitar trecho acima
                    </button>
                  </div>
                ) : null;
                const note = editing?.file === f.path && editing.i === i ? (
                  <form
                    className="dt-note"
                    onSubmit={(e) => {
                      e.preventDefault();
                      addComment(f, i);
                    }}
                  >
                    <textarea
                      autoFocus
                      rows={2}
                      value={text}
                      placeholder={`Comentário sobre ${f.path}:${f.lines[i]?.no ?? ""}`}
                      aria-label="Comentário da linha"
                      onChange={(e) => setText(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Escape") setEditing(null);
                        if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                          e.preventDefault();
                          addComment(f, i);
                        }
                      }}
                    />
                    <div className="dt-note-actions">
                      <button type="button" onClick={() => setEditing(null)}>
                        Cancelar
                      </button>
                      <button type="submit" className="primary" disabled={!text.trim()} title="Ctrl+Enter">
                        Adicionar à conversa
                      </button>
                    </div>
                  </form>
                ) : sent[`${f.path}#${i}`] ? (
                  <div className="dt-sent">
                    <Icon name="check" size={12} /> Comentário no rascunho da conversa
                  </div>
                ) : null;
                return note || rejectHunk ? (
                  <>
                    {note}
                    {rejectHunk}
                  </>
                ) : null;
              }}
            />
          </section>
        ))}
    </div>
  );
}
