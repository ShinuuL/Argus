import { useEffect, useRef, useState } from "react";
import type * as Monaco from "monaco-editor";
import { errorMessage, type Path } from "@/api";
import { loadMonaco } from "./lazy";
import { MONACO_THEME } from "./theme";

type Editor = Monaco.editor.IStandaloneCodeEditor;

interface Props {
  /** Arquivo da aba ativa do grupo (null = a aba ativa não é arquivo). */
  path: Path | null;
  /** Grupo com o foco: o editor pega o teclado ao trocar de arquivo. */
  focused: boolean;
}

/**
 * Instância Monaco de um grupo: mostra o arquivo da aba ativa. Os modelos são
 * compartilhados entre os grupos (./monaco); posição do cursor e rolagem são
 * guardadas por arquivo neste grupo. Ctrl+S grava.
 */
export function GroupEditor({ path, focused }: Props) {
  const el = useRef<HTMLDivElement>(null);
  const [editor, setEditor] = useState<Editor | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const shown = useRef<string | null>(null);
  const viewStates = useRef(new Map<string, Monaco.editor.ICodeEditorViewState>());
  const focusedRef = useRef(focused);
  focusedRef.current = focused;

  useEffect(() => {
    let alive = true;
    let created: Editor | null = null;
    loadMonaco().then(
      ({ monaco, saveDoc }) => {
        if (!alive || !el.current) return;
        const ed = monaco.editor.create(el.current, {
          automaticLayout: true,
          theme: MONACO_THEME,
          model: null,
          fontSize: 13,
          dropIntoEditor: { enabled: false },
        });
        ed.onKeyDown((e) => {
          if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.keyCode === monaco.KeyCode.KeyS) {
            e.preventDefault();
            e.stopPropagation();
            const uri = ed.getModel()?.uri;
            if (!uri) return;
            saveDoc(uri.fsPath).then(
              () => setError(null),
              (err) => setError(`Erro ao salvar ${uri.fsPath}: ${errorMessage(err)}`),
            );
          }
        });
        created = ed;
        setLoading(false);
        setEditor(ed);
      },
      (err) => {
        if (!alive) return;
        setLoading(false);
        setError(`Editor indisponível: ${errorMessage(err)}`);
      },
    );
    return () => {
      alive = false;
      shown.current = null;
      created?.dispose();
      setEditor(null);
    };
  }, []);

  useEffect(() => {
    if (!editor) return;
    let alive = true;
    const keep = () => {
      const state = shown.current ? editor.saveViewState() : null;
      if (shown.current && state) viewStates.current.set(shown.current, state);
    };
    if (!path) {
      keep();
      shown.current = null;
      editor.setModel(null);
      return;
    }
    loadMonaco()
      .then((m) => m.openDoc(path).then((doc) => ({ doc, key: m.docKey(path) })))
      .then(
        ({ doc, key }) => {
          if (!alive || doc.model.isDisposed()) return;
          if (shown.current !== key || editor.getModel() !== doc.model) {
            keep();
            editor.setModel(doc.model);
            shown.current = key;
            const state = viewStates.current.get(key);
            if (state) editor.restoreViewState(state);
          }
          setError(null);
          if (focusedRef.current) editor.focus();
        },
        (err) => {
          if (!alive) return;
          keep();
          shown.current = null;
          editor.setModel(null);
          setError(`Erro ao abrir ${path}: ${errorMessage(err)}`);
        },
      );
    return () => {
      alive = false;
    };
  }, [editor, path]);

  return (
    <div className="wb-editor">
      {error && (
        <div className="err small wb-err" role="alert">
          {error}
        </div>
      )}
      {loading && <div className="muted small wb-err">Carregando editor…</div>}
      <div ref={el} className="wb-monaco" />
    </div>
  );
}
