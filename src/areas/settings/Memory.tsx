/**
 * Configurações › Preferências e memória: o prompt que vale para todos os projetos
 * (`<raiz>/_Argus/Preferencias.md`) e as notas que os agentes gravam em
 * `<raiz>/_Argus/Memoria/`. Tudo é Markdown na pasta raiz, abrível no Obsidian.
 */
import { useEffect, useMemo, useState } from "react";
import { api, errorMessage, type MemoryInfo } from "@/api";
import { useWorkbench } from "@/store";
import { Chip } from "@/ui";
import { PageBody } from "./PageBody";

const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

export default function MemoryPage() {
  const [info, setInfo] = useState<MemoryInfo | null>(null);
  const [text, setText] = useState("");
  const [saved, setSaved] = useState("");
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const openFile = useWorkbench((s) => s.openFile);

  const load = () =>
    api.memoryGet().then(
      (i) => {
        setInfo(i);
        setText(i.prefs ?? "");
        setSaved(i.prefs ?? "");
        setErr(null);
      },
      (e) => setErr(errorMessage(e)),
    );
  useEffect(() => void load(), []);

  const save = async () => {
    setBusy(true);
    try {
      await api.memorySetPrefs(text);
      await load();
    } catch (e) {
      setErr(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  const obsidian = (path: string) => api.memoryOpenObsidian(path).catch((e) => setErr(errorMessage(e)));

  const notes = useMemo(() => {
    const q = fold(query.trim());
    const all = info?.notes ?? [];
    if (!q) return all;
    return all.filter((n) => fold([n.name, n.summary, n.tipo ?? "", ...n.projetos].join(" ")).includes(q));
  }, [info, query]);

  const dirty = text !== saved;
  return (
    <PageBody
      title="Preferências e memória"
      lead="Instruções que valem para todos os projetos e agentes, e as notas que os agentes gravam ao aprender algo durável. Ficam em Markdown na pasta _Argus da raiz, abríveis no Obsidian."
    >
      {err && <div className="err" role="alert">{err}</div>}
      {info == null ? (
        !err && <p className="ui-mut ui-sm">Lendo a memória…</p>
      ) : (
        <>
          <div className="sa-form">
            <label className="sa-field">
              <span>
                Preferências <span className="ui-mut ui-sm ui-mono">{info.prefs_path}</span>
              </span>
              <textarea
                className="ui-mono"
                rows={12}
                value={text}
                placeholder={"Ex.: Responda em português do Brasil.\nPrefira mudanças pequenas e testadas."}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "s" && (e.ctrlKey || e.metaKey)) {
                    e.preventDefault();
                    if (dirty && !busy) void save();
                  }
                }}
              />
              <span className="ui-mut ui-sm">
                {info.active
                  ? "Cada agente é avisado para ler este arquivo e o índice da memória antes de começar. Vale a partir da próxima conversa (o Claude Code também nas atuais)."
                  : "Ao salvar, o Argus cria a pasta _Argus com este arquivo, o índice Memoria.md e a pasta de notas, e os agentes passam a ser avisados."}
              </span>
            </label>
            <div className="sa-row">
              <button className="primary" disabled={!dirty || busy} onClick={() => void save()}>
                {busy ? "Salvando…" : info.prefs == null ? "Criar e salvar" : "Salvar"}
              </button>
              {info.prefs != null && <button onClick={() => void obsidian(info.prefs_path)}>Abrir no Obsidian</button>}
              {dirty && <span className="ui-mut ui-sm">Alterações não salvas</span>}
            </div>
          </div>

          {/* Sem `.sa-head-text`: o flex: 1 dele esticava na coluna e empurrava a lista para o rodapé. */}
          <div className="sa-mem-head">
            <p className="sa-h">Memória</p>
            <p className="ui-mut ui-sm sa-lead">Uma nota por fato, gravada pelos agentes ou por você. O índice fica em {info.index_path}.</p>
          </div>
          <div className="sa-row">
            <div className="sa-search sa-mem-search">
              <input type="search" placeholder="Buscar nas notas…" aria-label="Buscar nas notas" value={query} onChange={(e) => setQuery(e.target.value)} />
            </div>
            {info.active && <button onClick={() => openFile(info.index_path)}>Abrir índice</button>}
            <button onClick={() => void load()}>Atualizar</button>
          </div>
          {info.notes.length === 0 ? (
            <p className="ui-mut ui-sm">Nenhuma nota ainda. Os agentes gravam aqui quando aprendem uma preferência ou uma decisão de projeto.</p>
          ) : notes.length === 0 ? (
            <p className="ui-mut ui-sm">Nenhuma nota com “{query}”.</p>
          ) : (
            <table className="ui-t sa-fixed">
              <thead>
                <tr>
                  <th style={{ width: 200 }}>Nota</th>
                  <th>Resumo</th>
                  <th style={{ width: 150 }}>Projetos</th>
                  <th style={{ width: 100 }}>Atualizada</th>
                  <th style={{ width: 190 }} />
                </tr>
              </thead>
              <tbody>
                {notes.map((n) => (
                  <tr key={n.path}>
                    <td title={n.path}>
                      <b>{n.name}</b> {n.tipo && <Chip>{n.tipo}</Chip>}
                    </td>
                    <td title={n.summary}>{n.summary || <span className="ui-mut">(vazia)</span>}</td>
                    <td title={n.projetos.join(", ")}>{n.projetos.length ? n.projetos.join(", ") : <span className="ui-mut">todos</span>}</td>
                    <td className="ui-mono">{n.atualizado ?? <span className="ui-mut">—</span>}</td>
                    <td>
                      <button onClick={() => openFile(n.path)}>Abrir</button> <button onClick={() => void obsidian(n.path)}>No Obsidian</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </>
      )}
    </PageBody>
  );
}
