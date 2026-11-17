import type { ReactNode } from "react";

/** Cabeçalho de página de Configurações (.pad > .h + lead) com conteúdo abaixo. */
export function PageBody({ title, lead, right, children }: { title: string; lead?: string; right?: ReactNode; children: ReactNode }) {
  return (
    <>
      <div className="sa-head">
        <div className="sa-head-text">
          <p className="sa-h">{title}</p>
          {lead && <p className="ui-mut ui-sm sa-lead">{lead}</p>}
        </div>
        {right}
      </div>
      <div className="sa-body">{children}</div>
    </>
  );
}
