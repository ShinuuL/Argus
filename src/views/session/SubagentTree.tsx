/** Árvore de subagentes da sessão (somente leitura): status e duração de cada um. */
import { subagentDuration, subagentStatus, subagentStatusLabel, type SubagentNode } from "./events";

export interface SubagentTreeProps {
  nodes: SubagentNode[];
  /** Subagente aberto; null = chat principal. */
  selected: string | null;
  taskActive: boolean;
  now: number;
  onSelect(id: string | null): void;
}

export function SubagentTree({ nodes, selected, taskActive, now, onSelect }: SubagentTreeProps) {
  const running = nodes.filter((n) => subagentStatus(n, taskActive) === "running").length;
  return (
    <nav className="ses-tree" aria-label="Subagentes da sessão">
      <div className="ses-tree-title">
        <span>Subagentes</span>
        <span className="muted">{running > 0 ? `${running} de ${nodes.length} rodando` : nodes.length}</span>
      </div>
      <button
        className={selected === null ? "ses-tree-row sel" : "ses-tree-row"}
        aria-current={selected === null}
        onClick={() => onSelect(null)}
      >
        <span className={`ses-dot ${taskActive ? "running" : "ok"}`} aria-hidden="true" />
        <span className="ses-tree-name ellipsis">Sessão principal</span>
      </button>
      {nodes.map((node) => {
        const status = subagentStatus(node, taskActive);
        const duration = subagentDuration(node, status, now);
        const name = node.description || node.subagent_type || node.id;
        return (
          <button
            key={node.id}
            className={selected === node.id ? "ses-tree-row sel" : "ses-tree-row"}
            style={{ paddingLeft: 22 + node.depth * 14 }}
            aria-current={selected === node.id}
            title={`${name}\n${node.subagent_type} · ${subagentStatusLabel(status)}${duration ? ` · ${duration}` : ""}`}
            onClick={() => onSelect(node.id)}
          >
            <span className={`ses-dot ${status}`} aria-hidden="true" />
            <span className="ses-tree-name ellipsis">{name}</span>
            <span className="ses-tree-meta small muted">{duration ?? subagentStatusLabel(status)}</span>
          </button>
        );
      })}
    </nav>
  );
}
