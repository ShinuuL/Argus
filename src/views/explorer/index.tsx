import { createContext, useContext, useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { api, baseName, errorMessage, type DirEntry, type FileStatus, type Path, type ProjectStatus } from "@/api";
import { activeTab, useApp, useTerminals, useWorkbench } from "@/store";
import { Icon } from "@/shell/icons";
import "./explorer.css";

/** Caminho em forma comparável (barras normais, minúsculas: o Windows não diferencia). */
function norm(path: Path): string {
  return path.replace(/[\\/]+/g, "/").replace(/\/$/, "").toLowerCase();
}

/** Pastas abertas na árvore; fica fora do componente para sobreviver à troca de vista. */
const expandedDirs = new Set<string>();

interface StatusMark {
  letter: string;
  cls: string;
  title: string;
}

const STATUS_MARKS: Record<FileStatus, StatusMark> = {
  New: { letter: "A", cls: "st-new", title: "novo" },
  Untracked: { letter: "U", cls: "st-new", title: "não rastreado" },
  Modified: { letter: "M", cls: "st-mod", title: "modificado" },
  Renamed: { letter: "R", cls: "st-mod", title: "renomeado" },
  TypeChange: { letter: "T", cls: "st-mod", title: "tipo alterado" },
  Deleted: { letter: "D", cls: "st-del", title: "apagado" },
  Conflicted: { letter: "!", cls: "st-conf", title: "em conflito" },
};

interface TreeContext {
  /** Muda a cada evento do monitor de arquivos: as pastas abertas relistam. */
  rev: unknown;
  /** Status git por arquivo (caminho normalizado). */
  files: Map<string, FileStatus>;
  /** Pastas que contêm algum arquivo alterado. */
  touchedDirs: Set<string>;
  /** Arquivo da aba ativa do workbench. */
  current: string | null;
  /** Pasta do projeto, com barras normais e sem a barra final. */
  base: string;
}

const TreeCtx = createContext<TreeContext>({ rev: 0, files: new Map(), touchedDirs: new Set(), current: null, base: "" });

function gitIndex(project: Path, status: ProjectStatus | null | undefined) {
  const files = new Map<string, FileStatus>();
  const touchedDirs = new Set<string>();
  const base = norm(project);
  for (const [rel, st] of status?.changed_files ?? []) {
    const parts = norm(rel).split("/").filter(Boolean);
    files.set(`${base}/${parts.join("/")}`, st);
    for (let i = 1; i < parts.length; i++) touchedDirs.add(`${base}/${parts.slice(0, i).join("/")}`);
  }
  return { files, touchedDirs };
}

function onActivateKey(e: KeyboardEvent, action: () => void) {
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    action();
  }
}

/** Conteúdo de uma pasta: lista preguiçosa, relida quando o monitor avisa. */
function DirChildren({ path, depth }: { path: Path; depth: number }) {
  const { rev } = useContext(TreeCtx);
  const [entries, setEntries] = useState<DirEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    api.fsListDir(path).then(
      (list) => {
        if (!alive) return;
        setEntries(list);
        setError(null);
      },
      (e) => {
        if (alive) setError(errorMessage(e));
      },
    );
    return () => {
      alive = false;
    };
  }, [path, rev]);

  const pad = { paddingLeft: depth * 12 + 4 };
  if (error) {
    return (
      <div className="tree-note small err" style={pad}>
        {error}
      </div>
    );
  }
  if (!entries) return null;
  if (entries.length === 0) {
    return (
      <div className="tree-note small muted" style={pad}>
        (vazia)
      </div>
    );
  }
  return (
    <>
      {entries.map((e) =>
        e.is_dir ? (
          <DirNode key={e.path} entry={e} depth={depth} />
        ) : (
          <FileNode key={e.path} entry={e} depth={depth} />
        ),
      )}
    </>
  );
}

function DirNode({ entry, depth }: { entry: DirEntry; depth: number }) {
  const { touchedDirs } = useContext(TreeCtx);
  const id = norm(entry.path);
  const [open, setOpen] = useState(() => expandedDirs.has(id));
  const toggle = () => {
    setOpen((v) => {
      if (v) expandedDirs.delete(id);
      else expandedDirs.add(id);
      return !v;
    });
  };
  const touched = touchedDirs.has(id);
  return (
    <>
      <div
        className={touched ? "node dir touched" : "node dir"}
        style={{ paddingLeft: depth * 12 + 4 }}
        role="treeitem"
        aria-expanded={open}
        tabIndex={0}
        title={entry.path}
        onClick={toggle}
        onKeyDown={(e) => onActivateKey(e, toggle)}
      >
        <span className="arrow">{open ? "▾" : "▸"}</span>
        <span className="label">{entry.name}</span>
        {touched && (
          <span className="badge" title="contém alterações">
            ●
          </span>
        )}
      </div>
      {open && <DirChildren path={entry.path} depth={depth + 1} />}
    </>
  );
}

function FileNode({ entry, depth }: { entry: DirEntry; depth: number }) {
  const { files, current, base } = useContext(TreeCtx);
  const openFile = useWorkbench((s) => s.openFile);
  const id = norm(entry.path);
  const status = files.get(id);
  const mark = status ? STATUS_MARKS[status] : undefined;
  const cls = ["node", "file", mark?.cls, current === id ? "current" : ""].filter(Boolean).join(" ");
  return (
    <div
      className={cls}
      style={{ paddingLeft: depth * 12 + 18 }}
      role="treeitem"
      tabIndex={0}
      draggable
      onDragStart={(e) => {
        // Soltar na caixa do chat menciona o arquivo (@caminho relativo ao projeto).
        const rel = entry.path.replace(/\\/g, "/").slice(base.length + 1);
        e.dataTransfer.setData("application/x-argus-path", rel);
        e.dataTransfer.setData("text/plain", rel);
        e.dataTransfer.effectAllowed = "copy";
      }}
      title={mark ? `${entry.path} (${mark.title})` : entry.path}
      // Um clique abre em pré-visualização; duplo clique fixa a aba.
      onClick={() => openFile(entry.path, true)}
      onDoubleClick={() => openFile(entry.path, false)}
      onKeyDown={(e) => onActivateKey(e, () => openFile(entry.path, false))}
    >
      <span className="label">{entry.name}</span>
      {mark && <span className="badge">{mark.letter}</span>}
    </div>
  );
}

/**
 * Arquivos (aba do painel direito): a árvore do projeto ativo, com o status git
 * por arquivo. Arrastar um arquivo para a caixa do chat o anexa.
 */
export default function ExplorerView() {
  const statuses = useApp((s) => s.statuses);
  const selected = useApp((s) => s.selected);
  const openTerminal = useTerminals((s) => s.open);
  const currentFile = useWorkbench((s) => {
    const tab = activeTab(s.groups.find((g) => g.id === s.activeGroup));
    return tab?.kind.type === "file" ? tab.kind.path : null;
  });

  const selectedStatus = selected ? statuses[selected] : undefined;
  // Cada evento do monitor traz um objeto de status novo; sem git, vale a troca do mapa inteiro.
  const rev: unknown = selectedStatus ?? statuses;
  const tree = useMemo<TreeContext>(
    () => ({
      rev,
      ...gitIndex(selected ?? "", selectedStatus),
      current: currentFile ? norm(currentFile) : null,
      base: (selected ?? "").replace(/[\\/]+/g, "/").replace(/\/$/, ""),
    }),
    [selected, selectedStatus, rev, currentFile],
  );

  if (!selected) {
    return <p className="rp-empty">Escolha um projeto na barra esquerda para ver os arquivos.</p>;
  }
  return (
    <div className="explorer">
      <div className="rp-head">
        <b className="ellipsis" title={selected}>
          {baseName(selected)}
        </b>
        <button
          className="wb-sbtn"
          title="Abrir um terminal na pasta do projeto"
          onClick={() => openTerminal({ title: baseName(selected), cwd: selected })}
        >
          <Icon name="terminal" size={14} /> Terminal
        </button>
      </div>
      <div className="tree" role="tree" aria-label={`Arquivos de ${baseName(selected)}`}>
        <TreeCtx.Provider value={tree}>
          <DirChildren key={selected} path={selected} depth={0} />
        </TreeCtx.Provider>
      </div>
    </div>
  );
}
