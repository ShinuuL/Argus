import { useEffect } from "react";
import { samePath, type Path, type TaskId } from "@/api";
import { activeTab, useApp, useWorkbench } from "@/store";

export interface ActiveContext {
  /** Sessão da aba em foco (null se a aba não é uma conversa com tarefa). */
  taskId: TaskId | null;
  /** Projeto ativo: o da aba em foco, ou o escolhido na barra esquerda. */
  project: Path | null;
}

/** Projeto que contém o caminho (o de caminho mais longo, se houver aninhados). */
function projectOf(path: Path, projects: { path: Path }[]): Path | null {
  const norm = (p: string) => p.replace(/[\\/]+/g, "/").replace(/\/$/, "").toLowerCase();
  const target = norm(path);
  let best: Path | null = null;
  for (const p of projects) {
    const base = norm(p.path);
    if ((target === base || target.startsWith(`${base}/`)) && (!best || base.length > norm(best).length)) best = p.path;
  }
  return best;
}

/** Sessão e projeto da aba em foco. */
export function useActiveContext(): ActiveContext {
  const kind = useWorkbench((s) => activeTab(s.groups.find((g) => g.id === s.activeGroup))?.kind);
  const tasks = useApp((s) => s.tasks);
  const projects = useApp((s) => s.projects);
  const selected = useApp((s) => s.selected);

  let taskId: TaskId | null = null;
  let tabProject: Path | null = null;
  if (kind?.type === "chat") {
    taskId = kind.task;
    const raw = kind.task != null ? (tasks.find((t) => t.id === kind.task)?.project ?? null) : kind.project;
    tabProject = raw ? (projects.find((p) => samePath(p.path, raw))?.path ?? null) : null;
  } else if (kind?.type === "file") {
    tabProject = projectOf(kind.path, projects);
  }
  return { taskId, project: tabProject ?? selected };
}

/**
 * Mantém o "projeto ativo" do app igual ao da aba em foco: é ele que o painel
 * direito (Arquivos), os terminais novos e as telas por projeto usam.
 */
export function useSyncActiveProject(): ActiveContext {
  const ctx = useActiveContext();
  const tabKey = useWorkbench((s) => {
    const g = s.groups.find((x) => x.id === s.activeGroup);
    return `${g?.id}:${g?.active}`;
  });
  useEffect(() => {
    const app = useApp.getState();
    if (ctx.project && !samePath(ctx.project, app.selected)) app.select(ctx.project);
    // Só ao trocar de aba ou quando o projeto dela muda; clicar num projeto na barra esquerda não é desfeito.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tabKey, ctx.project]);
  // Sem projeto ativo (primeira abertura): o primeiro da pasta raiz.
  const first = useApp((s) => (s.selected == null ? (s.projects[0]?.path ?? null) : null));
  useEffect(() => {
    if (first) useApp.getState().select(first);
  }, [first]);
  return ctx;
}
