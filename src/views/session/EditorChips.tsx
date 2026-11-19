import { baseName, samePath, type Path } from "@/api";
import { useWorkbench } from "@/store";
import { focusBelongsTo, lineRange, relativeTo, useEditorContext } from "@/views/workbench/editorContext";

/**
 * Chips acima da caixa de mensagem com o que está no editor: "arquivo em foco"
 * (ligado por padrão) e "erros do editor" (desligado; aparece só com problemas).
 * Ligados, vão junto no texto do pedido ao enviar (`withEditorContext`).
 */
export function EditorChips({ root }: { root: Path | null }) {
  const focus = useEditorContext((s) => s.focus);
  const attachFocus = useEditorContext((s) => s.attachFocus);
  const attachMarkers = useEditorContext((s) => s.attachMarkers);
  // Aba do arquivo fechada: o chip some mesmo se o foco ainda não foi limpo.
  const open = useWorkbench((s) => !!focus && s.groups.some((g) => g.tabs.some((t) => t.kind.type === "file" && samePath(t.kind.path, focus.path))));
  // Arquivo de outro projeto: não oferece (iria junto num pedido de outra pasta).
  if (!focus || !open || !focusBelongsTo(focus.path, root)) return null;
  const lines = focus.selection ? ` · ${lineRange(focus.startLine, focus.endLine)}` : "";
  const errors = focus.markers.filter((m) => m.severity === "erro").length;
  const warns = focus.markers.length - errors;
  const rel = relativeTo(focus.path, root);
  const s = useEditorContext.getState();
  return (
    <div className="ed-chips" role="group" aria-label="Contexto do editor">
      <button
        type="button"
        className={`ed-chip${attachFocus ? " on" : ""}`}
        aria-pressed={attachFocus}
        title={
          attachFocus
            ? `Vai junto no pedido: ${rel}${focus.selection ? " e o trecho selecionado" : ""}. Clique para não enviar.`
            : `Clique para enviar ${rel}${focus.selection ? " e o trecho selecionado" : ""} junto com o pedido.`
        }
        onClick={() => s.setAttachFocus(!attachFocus)}
      >
        <span className="ed-chip-name">{baseName(focus.path)}</span>
        <span className="ed-chip-meta">em foco{lines}</span>
      </button>
      {focus.markers.length > 0 && (
        <button
          type="button"
          className={`ed-chip${attachMarkers ? " on" : ""}${errors ? " err" : ""}`}
          aria-pressed={attachMarkers}
          title={
            attachMarkers
              ? "Os problemas apontados pelo editor vão junto no pedido. Clique para não enviar."
              : "Clique para mandar ao agente os problemas apontados pelo editor neste arquivo."
          }
          onClick={() => s.setAttachMarkers(!attachMarkers)}
        >
          {[errors ? `${errors} ${errors === 1 ? "erro" : "erros"}` : "", warns ? `${warns} ${warns === 1 ? "aviso" : "avisos"}` : ""]
            .filter(Boolean)
            .join(", ")}{" "}
          do editor
        </button>
      )}
    </div>
  );
}
