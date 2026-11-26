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

/**
 * Ícones de arquivo do mockup (mesma ideia dos ícones do VS Code): uma plaqueta
 * colorida com a sigla da linguagem. A extensão decide a cor e o rótulo.
 */
function fileIcon(name: string): { cls: string; glyph: string } {
  const dot = name.lastIndexOf(".");
  const ext = dot < 0 ? "" : name.slice(dot + 1).toLowerCase();
  switch (ext) {
    case "ts":
    case "tsx":
      return { cls: "ts", glyph: "TS" };
    case "js":
    case "jsx":
    case "mjs":
    case "cjs":
      return { cls: "js", glyph: "JS" };
    case "css":
    case "scss":
    case "sass":
    case "less":
      return { cls: "css", glyph: "#" };
    case "html":
    case "htm":
    case "xml":
    case "svg":
      return { cls: "html", glyph: "<>" };
    case "rs":
      return { cls: "rs", glyph: "RS" };
    case "md":
    case "mdx":
      return { cls: "md", glyph: "MD" };
    case "json":
    case "jsonc":
    case "toml":
    case "yaml":
    case "yml":
    case "lock":
      return { cls: "json", glyph: "{}" };
    case "py":
      return { cls: "py", glyph: "PY" };
    case "go":
      return { cls: "go", glyph: "GO" };
    case "sh":
    case "ps1":
    case "bat":
      return { cls: "sh", glyph: "$_" };
    case "png":
    case "jpg":
    case "jpeg":
    case "gif":
    case "webp":
    case "ico":
      return { cls: "img", glyph: "IMG" };
    default:
      return { cls: "gen", glyph: name.slice(0, 1).toUpperCase() || "?" };
  }
}

function FileIcon({ name }: { name: string }) {
  const { cls, glyph } = fileIcon(name);
  return (
    <span className={`vsc-ico`}>
      <i className={`vsc-fi fi ${cls}`} aria-hidden="true">
        {glyph}
      </i>
    </span>
  );
}

interface TreeContext {
  /** Muda a cada evento do monitor de arquivos ou a cada atualização manual. */
  rev: unknown;
  files: Map<string, FileStatus>;
  touchedDirs: Set<string>;
  current: string | null;
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

/** Uma barra vertical por nível: o guia de indentação do explorer do VS Code. */
function Guides({ depth }: { depth: number }) {
  return (
    <>
      {Array.from({ length: depth }, (_, i) => (
        <span key={i} className="vsc-g" aria-hidden="true" />
      ))}
    </>
  );
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

  if (error) {
    return (
      <div className="vsc-note err">
        <Guides depth={depth} /> {error}
      </div>
    );
  }
  if (!entries) return null;
  if (entries.length === 0) {
    return (
      <div className="vsc-note muted">
        <Guides depth={depth} /> (vazia)
      </div>
    );
  }
  return (
    <>
      {entries.map((e) =>
        e.is_dir ? <DirNode key={e.path} entry={e} depth={depth} /> : <FileNode key={e.path} entry={e} depth={depth} />,
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
        className={touched ? "vsc-row folder touched" : "vsc-row folder"}
        role="treeitem"
        aria-expanded={open}
        tabIndex={0}
        title={entry.path}
        onClick={toggle}
        onKeyDown={(e) => onActivateKey(e, toggle)}
      >
        <span className="vsc-tw"><Icon name={open ? "chevronDown" : "chevronRight"} size={11} /></span>
        <Guides depth={depth} />
        <span className="vsc-ico">
          <Icon name={open ? "folderOpen" : "folder"} filled size={15} className="vsc-folder" />
        </span>
        <span className="vsc-name">{entry.name}</span>
        {touched && (
          <span className="vsc-mod" title="contém alterações">
            <Icon name="circleDot" size={8} filled />
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
  const cls = ["vsc-row", "file", mark?.cls, current === id ? "on" : ""].filter(Boolean).join(" ");
  return (
    <div
      className={cls}
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
      <span className="vsc-tw" />
      <Guides depth={depth} />
      <FileIcon name={entry.name} />
      <span className="vsc-name">{entry.name}</span>
      {mark && (
        <span className={`vsc-mod ${mark.cls}`} title={mark.title}>
          {mark.letter}
        </span>
      )}
    </div>
  );
}

function SectionHead({ label, open, onClick }: { label: string; open: boolean; onClick: () => void }) {
  return (
    <div className="vsc-sec-h">
      <button type="button" className="vsc-sec-btn" aria-expanded={open} onClick={onClick}>
        <span className="vsc-tw"><Icon name={open ? "chevronDown" : "chevronRight"} size={11} /></span>
        {label}
      </button>
    </div>
  );
}

/**
 * Aba Arquivos do inspetor no estilo do explorer do VS Code (mockup Bento):
 * barra EXPLORER, seção ABERTOS, árvore do projeto com ícones de pasta e de
 * linguagem, guias de indentação e rodapé de status.
 *
 * Ações que continuam funcionando: abrir arquivo (clique/duplo clique/teclado),
 * arrastar para o chat, status git, abrir terminal, atualizar e recolher pastas.
 */
export default function ExplorerView() {
  const statuses = useApp((s) => s.statuses);
  const selected = useApp((s) => s.selected);
  const openTerminal = useTerminals((s) => s.open);
  const groups = useWorkbench((s) => s.groups);
  const dirty = useWorkbench((s) => s.dirty);
  const openFile = useWorkbench((s) => s.openFile);
  const closeTab = useWorkbench((s) => s.closeTab);
  const currentFile = useWorkbench((s) => {
    const tab = activeTab(s.groups.find((g) => g.id === s.activeGroup));
    return tab?.kind.type === "file" ? tab.kind.path : null;
  });
  const [reload, setReload] = useState(0);
  const [treeKey, setTreeKey] = useState(0);
  const [openEditorsOpen, setOpenEditorsOpen] = useState(true);
  const [rootOpen, setRootOpen] = useState(true);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);

  const selectedStatus = selected ? statuses[selected] : undefined;
  // Cada evento do monitor traz um objeto de status novo; sem git, vale a troca do mapa inteiro.
  const rev = useMemo<unknown>(() => ({ status: selectedStatus ?? statuses, reload }), [selectedStatus, statuses, reload]);
  const index = useMemo(() => gitIndex(selected ?? "", selectedStatus), [selected, selectedStatus]);
  const tree = useMemo<TreeContext>(
    () => ({
      rev,
      ...index,
      current: currentFile ? norm(currentFile) : null,
      base: (selected ?? "").replace(/[\\/]+/g, "/").replace(/\/$/, ""),
    }),
    [rev, index, currentFile, selected],
  );

  const openFiles = useMemo(() => {
    const out: { gid: number; tab: number; path: Path }[] = [];
    const seen = new Set<string>();
    for (const g of groups) {
      for (const t of g.tabs) {
        if (t.kind.type !== "file") continue;
        const id = norm(t.kind.path);
        if (seen.has(id)) continue;
        seen.add(id);
        out.push({ gid: g.id, tab: t.id, path: t.kind.path });
      }
    }
    return out;
  }, [groups]);
  const dirtySet = useMemo(() => new Set(dirty.map(norm)), [dirty]);

  const refresh = () => setReload((v) => v + 1);
  const collapse = () => {
    expandedDirs.clear();
    setTreeKey((v) => v + 1);
  };
  const createFile = async () => {
    const name = newName.trim();
    if (!selected) return;
    if (!name || /[\\/:*?"<>|]/.test(name)) {
      setCreateError("Use um nome de arquivo válido, sem barras.");
      return;
    }
    const root = selected.replace(/[\\/]+$/g, "").replace(/\\/g, "/");
    const path = `${root}/${name}`;
    try {
      const exists = await api.fsStat(path);
      if (exists.exists) {
        setCreateError("Já existe um arquivo ou pasta com esse nome.");
        return;
      }
      await api.fsWriteFile(path, "");
      setCreating(false);
      setNewName("");
      setCreateError(null);
      refresh();
      openFile(path, false);
    } catch (e) {
      setCreateError(errorMessage(e));
    }
  };

  if (!selected) {
    return (
      <div className="explorer vsc">
        <p className="rp-empty">Escolha um projeto na barra esquerda para ver os arquivos.</p>
      </div>
    );
  }

  const changed = index.files.size;
  const statusText = `${changed} ${changed === 1 ? "arquivo modificado" : "arquivos modificados"} · ${openFiles.length} ${
    openFiles.length === 1 ? "aberto" : "abertos"
  }`;

  return (
    <div className="explorer vsc">
      <div className="vsc-title">
        <span>EXPLORER</span>
        <span className="vsc-acts">
          <button type="button" title="Novo arquivo" aria-label="Novo arquivo" onClick={() => setCreating((v) => !v)}>
            <Icon name="plus" size={15} />
          </button>
          <button type="button" title="Nova pasta (ainda não disponível)" aria-label="Nova pasta" disabled>
            <Icon name="folder" size={15} />
          </button>
          <button type="button" title="Atualizar" aria-label="Atualizar" onClick={refresh}>
            <Icon name="refresh" size={15} />
          </button>
          <button type="button" title="Recolher pastas" aria-label="Recolher pastas" onClick={collapse}>
            <Icon name="collapse" size={15} />
          </button>
          <button
            type="button"
            title="Abrir um terminal na pasta do projeto"
            aria-label="Abrir terminal na pasta do projeto"
            onClick={() => openTerminal({ title: baseName(selected), cwd: selected })}
          >
            <Icon name="terminal" size={15} />
          </button>
        </span>
      </div>

      {creating && (
        <div className="vsc-new">
          <input
            autoFocus
            aria-label="Nome do novo arquivo"
            placeholder="nome-do-arquivo.ts"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void createFile();
              if (e.key === "Escape") setCreating(false);
            }}
          />
          <button type="button" title="Criar arquivo" onClick={() => void createFile()}>
            <Icon name="check" size={14} />
          </button>
        </div>
      )}
      {createError && <div className="vsc-err">{createError}</div>}

      <SectionHead label="ABERTOS" open={openEditorsOpen} onClick={() => setOpenEditorsOpen((v) => !v)} />
      {openEditorsOpen &&
        openFiles.map(({ gid, tab, path }) => {
          const id = norm(path);
          const status = index.files.get(id);
          const mark = status ? STATUS_MARKS[status] : undefined;
          const isCurrent = currentFile ? norm(currentFile) === id : false;
          const cls = ["vsc-row", "file", isCurrent ? "on" : "", mark?.cls].filter(Boolean).join(" ");
          return (
            <div
              key={`${gid}:${tab}`}
              className={cls}
              role="treeitem"
              aria-selected={isCurrent}
              tabIndex={0}
              title={path}
              onClick={() => openFile(path, false)}
              onKeyDown={(e) => onActivateKey(e, () => openFile(path, false))}
            >
              <span className="vsc-tw" />
              <FileIcon name={baseName(path)} />
              <span className="vsc-name">{baseName(path)}</span>
              {dirtySet.has(id) && (
                <span className="vsc-mod" title="alterado">
                  <Icon name="circleDot" size={8} filled />
                </span>
              )}
              {mark && (
                <span className={`vsc-mod ${mark.cls}`} title={mark.title}>
                  {mark.letter}
                </span>
              )}
              <button
                type="button"
                className="vsc-x"
                title="Fechar aba"
                aria-label={`Fechar ${baseName(path)}`}
                onKeyDown={(e) => e.stopPropagation()}
                onClick={(e) => {
                  e.stopPropagation();
                  closeTab(gid, tab);
                }}
              >
                ×
              </button>
            </div>
          );
        })}

      <SectionHead label={baseName(selected).toUpperCase()} open={rootOpen} onClick={() => setRootOpen((v) => !v)} />
      {rootOpen && (
        <div className="vsc-tree" role="tree" aria-label={`Arquivos de ${baseName(selected)}`}>
          <TreeCtx.Provider value={tree}>
            <DirChildren key={`${selected}:${treeKey}`} path={selected} depth={0} />
          </TreeCtx.Provider>
        </div>
      )}

      <div className="vsc-status">{statusText}</div>
    </div>
  );
}
