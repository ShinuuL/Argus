import { useEffect, useRef, useState } from "react";
import type * as Monaco from "monaco-editor";
import { errorMessage, samePath, type Path } from "@/api";
import { loadMonaco } from "./lazy";
import { MONACO_THEME } from "./theme";
import { useApp, useLayout, useTheme, useWorkbench } from "@/store";
import { addToChatText, useEditorContext, type EditorFocus } from "./editorContext";

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
 * guardadas por arquivo neste grupo. Ctrl+S grava. Alt+K leva a seleção (ou o
 * arquivo) para a conversa visível; o arquivo e a seleção em foco ficam em
 * `useEditorContext` para os chips do composer.
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
          fontSize: useTheme.getState().codeFont,
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
        /** Arquivo, seleção e problemas deste editor para o chat. */
        const snapshot = (): EditorFocus | null => {
          const model = ed.getModel();
          if (!model) return null;
          const sel = ed.getSelection();
          const empty = !sel || sel.isEmpty();
          const start = sel ? sel.startLineNumber : 1;
          // Seleção que termina no começo de uma linha não inclui essa linha.
          const end = sel && !empty && sel.endColumn === 1 && sel.endLineNumber > start ? sel.endLineNumber - 1 : (sel?.endLineNumber ?? start);
          const markers = monaco.editor
            .getModelMarkers({ resource: model.uri })
            .filter((m) => m.severity >= monaco.MarkerSeverity.Warning)
            .sort((a, b) => b.severity - a.severity || a.startLineNumber - b.startLineNumber)
            .map((m) => ({
              line: m.startLineNumber,
              severity: m.severity === monaco.MarkerSeverity.Error ? ("erro" as const) : ("aviso" as const),
              message: m.message,
            }));
          return {
            path: model.uri.fsPath,
            startLine: start,
            endLine: end,
            selection: empty || !sel ? "" : model.getValueInRange(sel),
            language: model.getLanguageId(),
            markers,
          };
        };
        let published: string | null = null;
        // Só a aba do arquivo fechada apaga o foco: trocar para a conversa no mesmo
        // grupo esconde o editor, mas o arquivo continua sendo o "em foco" do chat.
        const forget = () => {
          const cur = useEditorContext.getState().focus;
          const stillOpen =
            cur != null &&
            useWorkbench.getState().groups.some((g) => g.tabs.some((t) => t.kind.type === "file" && samePath(t.kind.path, cur.path)));
          if (cur && cur.path === published && !stillOpen) useEditorContext.getState().setFocus(null);
          published = null;
        };
        const publish = () => {
          const f = snapshot();
          // Editor sem arquivo (aba trocada para a conversa): não publica vazio por cima.
          if (!f) return forget();
          published = f.path;
          useEditorContext.getState().setFocus(f);
        };
        // Editor fechado (aba fechada, grupo removido): o chip "em foco" não fica preso.
        ed.onDidFocusEditorText(publish);
        ed.onDidChangeCursorSelection(() => {
          if (ed.hasTextFocus() || focusedRef.current) publish();
        });
        ed.onDidChangeModel(() => {
          if (focusedRef.current) publish();
          else if (!ed.getModel()) forget();
        });
        const markersSub = monaco.editor.onDidChangeMarkers((uris) => {
          const m = ed.getModel();
          const cur = useEditorContext.getState().focus;
          if (m && cur?.path === m.uri.fsPath && uris.some((u) => u.toString() === m.uri.toString())) publish();
        });
        ed.onDidDispose(() => {
          markersSub.dispose();
          forget();
        });
        ed.addAction({
          id: "argus.addToChat",
          label: "Adicionar à conversa",
          keybindings: [monaco.KeyMod.Alt | monaco.KeyCode.KeyK],
          contextMenuGroupId: "navigation",
          contextMenuOrder: 0,
          run: () => {
            const f = snapshot();
            if (!f) return;
            useEditorContext.getState().setFocus(f);
            const root = useApp.getState().selected;
            // Sem conversa à vista, mostra uma (a do projeto, se houver) antes de inserir.
            useWorkbench.getState().revealChat(root);
            const text = addToChatText(f, root);
            setTimeout(() => useLayout.getState().insertInComposer(text), 0);
          },
        });
        // Fonte do código escolhida em Aparência: vale na hora para editores abertos.
        const fontSub = useTheme.subscribe((st, prev) => {
          if (st.codeFont !== prev.codeFont) ed.updateOptions({ fontSize: st.codeFont });
        });
        ed.onDidDispose(fontSub);
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
