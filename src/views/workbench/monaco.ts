/**
 * Monaco empacotado pelo vite (sem CDN) e o registro de documentos abertos.
 *
 * Há um modelo por arquivo, compartilhado por todos os grupos: o mesmo arquivo
 * em dois grupos tem o mesmo conteúdo e o mesmo estado de "modificado".
 * Este módulo é pesado; carregue-o por `loadMonaco()` (./lazy).
 */
import * as monaco from "monaco-editor";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import CssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import HtmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import JsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import TsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";
import { api, languageFor, type Path } from "@/api";
import { tokenColor, useTheme, useWorkbench } from "@/store";
import { MONACO_THEME } from "./theme";

export { monaco };

/** Tema do editor montado a partir dos tokens de cor; refeito quando o tema do app muda. */
function applyEditorTheme(): void {
  const c = tokenColor;
  monaco.editor.defineTheme(MONACO_THEME, {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "comment", foreground: c("text-2").slice(1), fontStyle: "italic" },
      { token: "keyword", foreground: c("accent-text").slice(1) },
      { token: "string", foreground: c("ok").slice(1) },
      { token: "number", foreground: c("warn").slice(1) },
      { token: "type", foreground: c("run").slice(1) },
    ],
    colors: {
      "editor.background": c("bg-editor"),
      "editor.foreground": c("text"),
      "editorLineNumber.foreground": c("border-strong"),
      "editorLineNumber.activeForeground": c("text"),
      "editorCursor.foreground": c("focus"),
      "editor.selectionBackground": c("selection"),
      "editor.inactiveSelectionBackground": c("bg-active"),
      "editor.lineHighlightBackground": c("bg-tabs"),
      "editor.lineHighlightBorder": c("bg-tabs"),
      "editorIndentGuide.background1": c("border"),
      "editorWidget.background": c("surface"),
      "editorWidget.border": c("border-strong"),
      "editorSuggestWidget.background": c("surface"),
      "editorSuggestWidget.selectedBackground": c("selection"),
      "editorHoverWidget.background": c("surface"),
      "input.background": c("bg"),
      "scrollbarSlider.background": c("surface-2") + "aa",
      "scrollbarSlider.hoverBackground": c("border-strong") + "aa",
      "minimap.background": c("bg-editor"),
      focusBorder: c("focus"),
    },
  });
  monaco.editor.setTheme(MONACO_THEME);
}
applyEditorTheme();
useTheme.subscribe(() => applyEditorTheme());

self.MonacoEnvironment = {
  getWorker(_id: string, label: string): Worker {
    switch (label) {
      case "json":
        return new JsonWorker();
      case "css":
      case "scss":
      case "less":
        return new CssWorker();
      case "html":
      case "handlebars":
      case "razor":
        return new HtmlWorker();
      case "typescript":
      case "javascript":
        return new TsWorker();
      default:
        return new EditorWorker();
    }
  },
};

// O editor vê um arquivo por vez, sem o projeto: a checagem semântica acusaria
// todo import como erro. Fica só a de sintaxe.
for (const defaults of [monaco.languages.typescript.typescriptDefaults, monaco.languages.typescript.javascriptDefaults]) {
  defaults.setDiagnosticsOptions({ noSemanticValidation: true, noSyntaxValidation: false });
}

export interface Doc {
  path: Path;
  model: monaco.editor.ITextModel;
  /** Versão (alternativa) do modelo que está gravada em disco. */
  savedVersion: number;
  /** true enquanto o conteúdo é trocado pelo que está em disco. */
  reloading: boolean;
}

const docs = new Map<string, Doc>();
const loading = new Map<string, Promise<Doc>>();

/** Chave do documento: barras normais e minúsculas (o Windows não diferencia). */
export function docKey(path: Path): string {
  return path.replace(/[\\/]+/g, "/").replace(/\/$/, "").toLowerCase();
}

export function getDoc(path: Path): Doc | undefined {
  return docs.get(docKey(path));
}

/** Devolve o documento do arquivo, lendo-o do disco na primeira vez. */
export function openDoc(path: Path): Promise<Doc> {
  const key = docKey(path);
  const ready = docs.get(key);
  if (ready) return Promise.resolve(ready);
  let pending = loading.get(key);
  if (!pending) {
    pending = api
      .fsReadFile(path)
      .then((text) => {
        const uri = monaco.Uri.file(path);
        // Um modelo órfão com a mesma URI impediria a criação.
        monaco.editor.getModel(uri)?.dispose();
        const model = monaco.editor.createModel(text, languageFor(path), uri);
        const doc: Doc = { path, model, savedVersion: model.getAlternativeVersionId(), reloading: false };
        model.onDidChangeContent((e) => {
          if (doc.reloading) return;
          const dirty = model.getAlternativeVersionId() !== doc.savedVersion;
          const wb = useWorkbench.getState();
          wb.setDirty(doc.path, dirty);
          // Editar fixa a aba de pré-visualização.
          if (!e.isFlush) wb.pinFile(doc.path);
        });
        docs.set(key, doc);
        return doc;
      })
      .finally(() => loading.delete(key));
    loading.set(key, pending);
  }
  return pending;
}

/** Grava o documento em disco e limpa o indicador de modificado. */
export async function saveDoc(path: Path): Promise<void> {
  const doc = getDoc(path);
  if (!doc) return;
  const version = doc.model.getAlternativeVersionId();
  await api.fsWriteFile(doc.path, doc.model.getValue());
  doc.savedVersion = version;
  if (!doc.model.isDisposed()) {
    useWorkbench.getState().setDirty(doc.path, doc.model.getAlternativeVersionId() !== version);
  }
}

function disposeDoc(key: string, doc: Doc): void {
  docs.delete(key);
  for (const editor of monaco.editor.getEditors()) {
    if (editor.getModel() === doc.model) editor.setModel(null);
  }
  doc.model.dispose();
  useWorkbench.getState().setDirty(doc.path, false);
}

/** Descarta os documentos que não estão mais abertos em nenhuma aba. */
export function retainDocs(openPaths: Path[]): void {
  const keep = new Set(openPaths.map(docKey));
  for (const [key, doc] of [...docs]) {
    if (!keep.has(key)) disposeDoc(key, doc);
  }
}

const lf = (text: string) => text.replace(/\r\n/g, "\n");

/**
 * Relê do disco os documentos sem alterações pendentes (ex.: um agente editou
 * o arquivo). Documentos modificados na IDE ficam como estão. `under` limita a
 * uma pasta de projeto.
 */
export async function reloadCleanDocs(under?: Path): Promise<void> {
  const prefix = under ? `${docKey(under)}/` : null;
  for (const [key, doc] of [...docs]) {
    if (prefix && !key.startsWith(prefix)) continue;
    if (doc.model.getAlternativeVersionId() !== doc.savedVersion) continue;
    let text: string;
    try {
      text = await api.fsReadFile(doc.path);
    } catch {
      continue; // Apagado ou ilegível: mantém o que está aberto.
    }
    if (doc.model.isDisposed() || docs.get(key) !== doc) continue;
    if (doc.model.getAlternativeVersionId() !== doc.savedVersion) continue;
    if (lf(text) === doc.model.getValue(monaco.editor.EndOfLinePreference.LF)) continue;
    doc.reloading = true;
    try {
      // applyEdits preserva cursor e rolagem dos editores que mostram o modelo.
      doc.model.applyEdits([{ range: doc.model.getFullModelRange(), text }]);
      doc.savedVersion = doc.model.getAlternativeVersionId();
    } finally {
      doc.reloading = false;
    }
    useWorkbench.getState().setDirty(doc.path, false);
  }
}
