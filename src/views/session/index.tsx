/**
 * Conversa com um agente (aba principal do workbench).
 *
 * Uma superfície só: a conversa nova (saudação + caixa de composição, com as
 * pílulas de projeto e provedor) e a sessão de uma tarefa (histórico, stream ao
 * vivo, fila, anexos, comandos de barra, consumo). O primeiro envio cria a
 * tarefa e a MESMA aba passa a ser a sessão dela. O chat de um subagente é
 * somente leitura: não tem caixa de resposta.
 */
import { Select } from "@/shell/Select";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  baseName,
  errorMessage,
  formatIntPt,
  formatTokensPt,
  formatUsdPt,
  isTaskActive,
  onTaskEvent,
  samePath,
  totalTokens,
  variantOf,
  type Path,
  type AccessMode,
  type IsolationReport,
  type Pricing,
  type Task,
  type TaskId,
  type WorkMode,
} from "@/api";
import { Icon } from "@/shell/icons";
import { useApp, useLayout, useWorkbench } from "@/store";
import { useProviders } from "@/views/agents";
import { AutoWorkBadge } from "@/views/autowork";
import {
  Composer,
  DropZone,
  repeatMessage,
  RepeatDetector,
  RUN_FOR_REAL_PROMPT,
  splitPrompt,
  toolCallAsText,
  useChat,
  chatKey,
  draftIdFor,
  type ComposerHandle,
  type LocalCommandName,
} from "@/views/chat";
import { SessionServersBadge } from "@/views/servers";
import { applyTask, cancelTask, deleteTask, discardTask } from "./actions";
import { ContextMenu, type MenuItem } from "@/shell/ContextMenu";
import {
  chatEntries,
  sessionTitle,
  subagentDuration,
  subagentStatus,
  subagentStatusLabel,
  subagentTree,
  usageTotals,
  type ChatEntry,
  type SubagentNode,
} from "./events";
import { ServerStoppedNotice } from "./ServerStoppedNotice";
import { ACCESS_LABEL, AgentModelPill, BypassBadge, ComposerOptions, DirectHold, hasGaps, IsolationHold, useWorkModeInfo, WORK_MODE_LABEL } from "./pills";
import { ContextMeter } from "./ContextMeter";
import { UsagePill } from "@/views/chat/UsagePill";
import { DraftText, liveActivity, Transcript, WorkingLine } from "./Transcript";
import { useTaskEvents } from "./useTaskEvents";
import { EditorChips } from "./EditorChips";
import { withEditorContext } from "@/views/workbench/editorContext";
import { AgentState, agentStateLabel, agentStateOf, lastTurnDenied } from "./AgentState";
import "./session.css";

export interface ChatTabProps {
  /** Aba do workbench que hospeda a conversa. */
  tabId: number;
  /** Tarefa da conversa; null = conversa nova. */
  taskId: TaskId | null;
  /** Projeto escolhido para a conversa nova. */
  project: Path | null;
  /** A aba está à vista (ativa no grupo dela). */
  active: boolean;
}

/** Tabela de preços (custo equivalente em API), lida uma vez por conversa aberta. */
function usePricing(): Pricing | null {
  const [pricing, setPricing] = useState<Pricing | null>(null);
  useEffect(() => {
    let alive = true;
    api.pricingGet().then(
      (p) => alive && setPricing(p),
      () => undefined,
    );
    return () => {
      alive = false;
    };
  }, []);
  return pricing;
}

/** Relógio de 1 s, ligado só enquanto `on` (duração dos subagentes rodando). */
function useTick(on: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!on) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [on]);
  return now;
}

/** Indicador de estado de uma sessão (lista de conversas e barra lateral). */
export function StateDot({ task }: { task: Task }) {
  return <AgentState task={task} size={13} />;
}

export { AgentState } from "./AgentState";

// ------------------------------------------------------------ seletor de sessão

function SessionPicker(props: { current: Task | null; project: Path | null; onNew(): void }) {
  const { current, project, onNew } = props;
  const tasks = useApp((s) => s.tasks);
  const attention = useApp((s) => s.attention);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const menuOpen = useRef(false);
  menuOpen.current = menu != null;

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      // O menu de contexto fica fora da lista: clicar nele não fecha a lista.
      if (menuOpen.current) return;
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !menuOpen.current) setOpen(false);
    };
    document.addEventListener("mousedown", onDown, true);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("mousedown", onDown, true);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [open]);

  const list = useMemo(() => {
    const q = query.trim().toLowerCase();
    return tasks.filter(
      (t) => (!project || samePath(t.project, project)) && (!q || t.prompt.toLowerCase().includes(q) || `#${t.id}`.includes(q)),
    );
  }, [tasks, project, query]);

  const pick = (id: TaskId) => {
    setOpen(false);
    useWorkbench.getState().openSession(id);
  };

  return (
    <div className="cv-picker" ref={root}>
      <button
        className="cv-title"
        aria-haspopup="listbox"
        aria-expanded={open}
        title={current ? `${current.prompt}\n\nClique para trocar de conversa` : "Conversas anteriores"}
        onClick={() => {
          setQuery("");
          setOpen((v) => !v);
        }}
      >
        <span className="ellipsis">{current ? sessionTitle(current.prompt) : "Nova conversa"}</span>
        <Icon name="chevronDown" size={13} />
      </button>
      {open && (
        <div className="cv-pop" role="listbox" aria-label="Conversas">
          <div className="cv-pop-search">
            <Icon name="search" size={13} />
            <input
              autoFocus
              placeholder={project ? `Buscar nas conversas de ${baseName(project)}…` : "Buscar conversas…"}
              aria-label="Buscar conversas"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && list[0]) pick(list[0].id);
              }}
            />
          </div>
          <button
            className="cv-pop-item cv-pop-new"
            onClick={() => {
              setOpen(false);
              onNew();
            }}
          >
            <Icon name="plus" size={13} />
            <span>Nova conversa</span>
          </button>
          <div className="cv-pop-list">
            {list.length === 0 && <div className="cv-pop-empty">Nenhuma conversa encontrada.</div>}
            {list.map((t) => (
              <button
                key={t.id}
                role="option"
                aria-selected={t.id === current?.id}
                className={t.id === current?.id ? "cv-pop-item sel" : "cv-pop-item"}
                title={`${t.prompt}\n\nClique direito: excluir`}
                onClick={() => pick(t.id)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setMenu({
                    x: e.clientX,
                    y: e.clientY,
                    items: [
                      { label: "Abrir", onSelect: () => pick(t.id) },
                      {
                        label: "Excluir sessão…",
                        danger: true,
                        separator: true,
                        onSelect: () =>
                          void deleteTask(t).catch((err) => useApp.getState().setError(errorMessage(err))),
                      },
                    ],
                  });
                }}
              >
                <StateDot task={t} />
                <span className="ellipsis cv-pop-text">{sessionTitle(t.prompt)}</span>
                <span className="cv-pop-meta">
                  #{t.id} · {agentStateLabel(agentStateOf(t.state, attention[t.id]))}
                </span>
              </button>
            ))}
          </div>
        </div>
      )}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </div>
  );
}

// ------------------------------------------------------------ chat de subagente

/** Faixa do chat de um subagente: caminho de volta, status e o aviso de somente leitura. */
function SubagentBar(props: { node: SubagentNode; taskActive: boolean; now: number; onBack(): void }) {
  const { node, taskActive, now, onBack } = props;
  const status = subagentStatus(node, taskActive);
  const duration = subagentDuration(node, status, now);
  return (
    <div className="ses-sub-bar">
      <button className="wb-sbtn" title="Voltar ao chat principal" onClick={onBack}>
        ← Sessão principal
      </button>
      <span className="ses-sub-bar-name ellipsis" title={node.description}>
        <b>{node.description || node.id}</b>
        {node.subagent_type && <span className="ses-sub-meta"> · {node.subagent_type}</span>}
      </span>
      <span className="small">
        <span className={`ses-st ${status}`}>{subagentStatusLabel(status)}</span>
        {duration && <span className="ses-sub-meta"> · {duration}</span>}
      </span>
      <span className="ses-readonly" title="O chat de um subagente não aceita mensagens nem ações">
        somente leitura
      </span>
    </div>
  );
}

// ---------------------------------------------------------------- conversa nova

const SUGGESTIONS = [
  "Explique a estrutura deste projeto e por onde começar a ler o código.",
  "Procure erros prováveis e proponha as correções.",
  "Escreva testes para a parte menos coberta.",
  "Revise as alterações pendentes e aponte riscos.",
];

function Welcome(props: { project: Path | null; hasRoot: boolean; mode: WorkMode; onPick(text: string): void }) {
  const { project, hasRoot, mode, onPick } = props;
  const pickRoot = useApp((s) => s.pickRoot);
  return (
    <div className="cv-welcome">
      <Icon name="chat" size={30} className="cv-welcome-ic" />
      {project ? (
        <>
          <h2>
            O que vamos fazer em <b>{baseName(project)}</b>?
          </h2>
          <p>
            {mode === "Isolated"
              ? "Escreva o pedido abaixo. No modo Isolado o agente trabalha numa cópia do projeto, só com o que está commitado; você revisa e aplica as alterações depois."
              : "Escreva o pedido abaixo. No modo Direto na pasta o agente trabalha na própria pasta do projeto, enxerga tudo, e as alterações valem na hora."}
          </p>
          <div className="cv-suggest">
            {SUGGESTIONS.map((s) => (
              <button key={s} onClick={() => onPick(s)}>
                {s}
              </button>
            ))}
          </div>
        </>
      ) : (
        <>
          <h2>Comece abrindo seus projetos</h2>
          <p>
            {hasRoot
              ? "Nenhum projeto foi encontrado nesta pasta raiz. Escolha outra pasta."
              : "Escolha a pasta raiz onde ficam seus projetos. Cada conversa com um agente acontece dentro de um deles."}
          </p>
          <button className="primary" onClick={() => void pickRoot()}>
            <Icon name="folderOpen" size={14} /> Abrir pasta raiz
          </button>
        </>
      )}
    </div>
  );
}

// ------------------------------------------------------------------------- aba

export default function ChatTab({ tabId, taskId, project: tabProject, active }: ChatTabProps) {
  const task = useApp((s) => (taskId == null ? undefined : s.tasks.find((t) => t.id === taskId)));
  const allProjects = useApp((s) => s.projects);
  // Projeto com a pasta apagada não recebe conversa nova.
  const projects = useMemo(() => allProjects.filter((p) => !p.missing), [allProjects]);
  const root = useApp((s) => s.root);
  const selectedProject = useApp((s) => s.selected);
  const selectedSub = useApp((s) => (taskId == null ? null : (s.subagentOf[taskId] ?? null)));
  const unread = useApp((s) => taskId != null && !!s.unread[taskId]);
  const providers = useProviders();

  const busy = task ? isTaskActive(task.state) : false;
  const { events, times, loaded, error: loadError, draft } = useTaskEvents(taskId, busy);
  const pricing = usePricing();
  const [provider, setProvider] = useState("");
  /** Modelo escolhido para a conversa nova (undefined = o inicial do provedor). */
  const [modelChoice, setModelChoice] = useState<string | undefined>(undefined);
  /** Modo de trabalho escolhido para a conversa nova (null = o padrão do projeto). */
  const [modeChoice, setModeChoice] = useState<WorkMode | null>(null);
  /** Acesso escolhido para a conversa nova (vale já no primeiro turno). */
  const [accessChoice, setAccessChoice] = useState<AccessMode>("normal");
  /** Pedido segurado por um aviso, antes de ser enviado. */
  const [hold, setHold] = useState<
    | { kind: "isolation"; prompt: string; report: IsolationReport }
    | { kind: "direct"; prompt: string; mode: WorkMode | null; others: TaskId[] }
    | null
  >(null);
  const [holdBusy, setHoldBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Aviso de sucesso (ex.: pasta de volta a um ponto de retorno). */
  const [notice, setNotice] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  /** Quantidade de itens do chat quando o aviso de "chamada como texto" foi fechado. */
  const [dismissedAt, setDismissedAt] = useState(-1);
  const composer = useRef<ComposerHandle>(null);
  // Pedido de texto vindo do inspetor (clique numa pendência): só a aba visível atende.
  const insertReq = useLayout((s) => s.composerInsert);
  useEffect(() => {
    if (!insertReq || !active || !composer.current) return;
    composer.current.insertText(insertReq.text);
    // Consumido: outra aba que montar depois não repete o mesmo texto.
    useLayout.setState({ composerInsert: null });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [insertReq, active]);
  /** Rascunho próprio desta aba enquanto a conversa não tem tarefa (caixa, fila e pasta de anexos). */
  const draftId = draftIdFor(tabId);
  const handoff = useChat((s) => (taskId == null && active ? (s.drafts.new ?? "") : ""));
  // Texto deixado pelo `/nova` em outra aba: esta aba nova o adota.
  useEffect(() => {
    if (!handoff) return;
    const s = useChat.getState();
    s.setDraft(chatKey(null, draftId), handoff);
    s.setDraft("new", "");
  }, [handoff, draftId]);

  const providerId = provider || (providers.find((p) => p.id === "claude") ?? providers[0])?.id || "claude";
  /** Projeto da conversa: o da tarefa, ou o escolhido para a conversa nova. */
  const newProject =
    projects.find((p) => samePath(p.path, tabProject ?? selectedProject))?.path ?? projects[0]?.path ?? null;
  const project = task?.project ?? newProject;
  const modeInfo = useWorkModeInfo(taskId == null ? newProject : null, active);
  const isGit = modeInfo?.is_git ?? true;
  /** Modo da conversa nova: sem git só existe o direto. */
  const newMode: WorkMode = !isGit ? "Direct" : (modeChoice ?? modeInfo?.effective ?? "Isolated");
  const newModel = modelChoice ?? providers.find((p) => p.id === providerId)?.default_model ?? "";
  // Trocar de projeto ou de provedor desfaz a escolha feita para o anterior.
  useEffect(() => {
    setModeChoice(null);
  }, [newProject]);
  useEffect(() => {
    setModelChoice(undefined);
  }, [providerId]);

  const nodes = useMemo(() => subagentTree(events, times), [events, times]);
  const byId = useMemo(() => new Map(nodes.map((n) => [n.id, n])), [nodes]);
  const current = selectedSub ? (byId.get(selectedSub) ?? null) : null;
  const entries = useMemo(() => {
    const list = chatEntries(events, current ? current.id : null);
    // Histórico que não começa pela fala do usuário: mostra o pedido inicial da tarefa.
    if (!current && task && list[0]?.type !== "user") return [{ type: "user", text: task.prompt } as ChatEntry, ...list];
    return list;
  }, [events, current, task]);
  const totals = useMemo(() => usageTotals(events, pricing), [events, pricing]);
  const recall = useMemo(
    () => entries.flatMap((e) => (e.type === "user" ? [splitPrompt(e.text).text] : [])).filter((t) => t.trim()),
    [entries],
  );
  const anyRunning = busy && nodes.some((n) => !n.done);
  const now = useTick(anyRunning);

  // Pontos de retorno: relidos ao abrir e a cada turno que termina.
  const [checkpoints, setCheckpoints] = useState<number[]>([]);
  useEffect(() => {
    if (taskId == null || busy) return;
    let alive = true;
    api.taskCheckpoints(taskId).then((v) => alive && setCheckpoints(v)).catch(() => alive && setCheckpoints([]));
    return () => {
      alive = false;
    };
  }, [taskId, busy, events.length]);
  const restore = async (n: number) => {
    if (taskId == null) return;
    const ok = await api.systemConfirm(
      "Voltar a este ponto",
      `Desfazer na pasta tudo o que mudou a partir do pedido ${n}? Arquivos editados voltam como estavam e os criados depois são apagados. A conversa continua igual.`,
      true,
    );
    if (!ok) return;
    try {
      const removed = await api.taskRestoreCheckpoint(taskId, n);
      setError(null);
      setNotice(`Pasta de volta ao ponto antes do pedido ${n}${removed ? ` (${removed} ${removed === 1 ? "arquivo novo apagado" : "arquivos novos apagados"})` : ""}.`);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  // Comando negado no último turno: a sessão "precisa de atenção" (barra lateral e abas).
  useEffect(() => {
    if (taskId == null || !loaded) return;
    // Em Planejar a escrita é negada de propósito: não é algo a resolver.
    useApp.getState().setAttention(taskId, task?.access === "plan" ? null : lastTurnDenied(events));
  }, [taskId, loaded, events, task?.access]);

  // A conversa à vista deixa de estar "não lida".
  useEffect(() => {
    if (active && taskId != null && unread) useApp.getState().markSeen(taskId);
  }, [active, taskId, unread]);

  // Detecção de loop: cancela o turno quando o agente repete a mesma chamada (vale com a conversa aberta).
  useEffect(() => {
    if (taskId == null) return;
    const detector = new RepeatDetector((tool, reason) => {
      void api.taskCancel(taskId).catch(() => undefined);
      setError(repeatMessage(tool, reason));
    });
    const off = onTaskEvent(taskId, (ev) => {
      const { kind, data } = variantOf(ev);
      if (kind === "User") detector.reset();
      if (kind === "ToolUse") {
        const t = data as { name: string; input: unknown };
        detector.tool(t.name, t.input);
      }
    });
    return () => void off.then((f) => f()).catch(() => undefined);
  }, [taskId]);

  const selectSub = (id: string | null) => {
    if (taskId != null) useApp.getState().selectSubagent(taskId, id);
  };

  const cancel = async () => {
    if (taskId == null) return;
    setCancelling(true);
    try {
      await cancelTask(taskId);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setCancelling(false);
    }
  };

  /** Outras sessões rodando agora em "Direto na pasta" no mesmo projeto. */
  const directBusy = (where: Path): TaskId[] =>
    useApp
      .getState()
      .tasks.filter((t) => t.id !== taskId && t.mode === "Direct" && isTaskActive(t.state) && samePath(t.project, where))
      .map((t) => t.id);

  /**
   * Envia: na conversa nova cria a tarefa e esta mesma aba vira a sessão dela.
   * Antes, confere o que pode dar errado e segura o pedido num aviso (`hold`):
   * no modo isolado, o que a cópia não teria; no direto, outra sessão na pasta.
   */
  const send = async (prompt: string, opts: { mode?: WorkMode; skipIsolation?: boolean; skipDirect?: boolean } = {}) => {
    setError(null);
    if (taskId != null) {
      if (task?.mode === "Direct" && !opts.skipDirect) {
        const others = directBusy(task.project);
        if (others.length) return setHold({ kind: "direct", prompt, mode: null, others });
      }
      await api.taskReply(taskId, prompt);
      void useApp.getState().refreshTasks();
      return;
    }
    if (!newProject) throw new Error("Abra uma pasta raiz e escolha um projeto para conversar.");
    const mode = opts.mode ?? newMode;
    if (mode === "Isolated" && !opts.skipIsolation) {
      // Sem resposta do git, segue: o aviso é uma ajuda, não uma trava.
      const report = await api.isolationCheck(newProject, prompt).catch(() => null);
      if (report && hasGaps(report)) return setHold({ kind: "isolation", prompt, report });
    }
    if (mode === "Direct" && !opts.skipDirect) {
      const others = directBusy(newProject);
      if (others.length) return setHold({ kind: "direct", prompt, mode, others });
    }
    const id = await api.taskCreate(newProject, providerId, prompt, { model: newModel, mode, access: accessChoice });
    await useApp.getState().refreshTasks();
    useWorkbench.getState().bindChat(tabId, id);
  };

  /** Resolve o aviso: envia o pedido segurado com a escolha feita. */
  const releaseHold = async (opts: { mode?: WorkMode; skipIsolation?: boolean; skipDirect?: boolean }) => {
    if (!hold) return;
    const prompt = hold.prompt;
    setHoldBusy(true);
    try {
      setHold(null);
      await send(prompt, opts);
    } catch (e) {
      // Não foi: o pedido volta para a caixa.
      useChat.getState().setDraft(chatKey(taskId, draftId), prompt);
      setError(errorMessage(e));
    } finally {
      setHoldBusy(false);
    }
  };

  /** Desiste do envio: o pedido volta para a caixa de composição. */
  const cancelHold = () => {
    if (!hold) return;
    const key = chatKey(taskId, draftId);
    if (!(useChat.getState().drafts[key] ?? "")) useChat.getState().setDraft(key, hold.prompt);
    setHold(null);
    composer.current?.focus();
  };

  const changeModel = async (model: string) => {
    if (taskId == null) return setModelChoice(model);
    try {
      await api.taskSetModel(taskId, model || null);
      await useApp.getState().refreshTasks();
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  /** Troca o acesso da conversa. Ligar o Bypass pede confirmação no diálogo do sistema. */
  const changeAccess = async (access: AccessMode) => {
    try {
      if (access === "bypass") {
        const ok = await api.systemConfirm(
          "Ligar o Bypass",
          "Nesta conversa o agente vai rodar qualquer comando sem pedir aprovação e sem a política de shell do Argus. Ligar o Bypass?",
          true,
        );
        if (!ok) return;
      }
      if (taskId == null) return setAccessChoice(access);
      await api.taskSetAccess(taskId, access);
      await useApp.getState().refreshTasks();
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  /** Fim de um turno em Planejar: volta ao modo normal e pede a execução. */
  const runPlan = async () => {
    if (taskId == null) return;
    try {
      await api.taskSetAccess(taskId, "normal");
      await useApp.getState().refreshTasks();
      await send("Execute o plano que você propôs acima.");
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const changeProvider = async (id: string) => {
    if (taskId == null) return setProvider(id);
    try {
      await api.taskSetProvider(taskId, id);
      await useApp.getState().refreshTasks();
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const newChat = (draft?: string) => {
    if (draft) useChat.getState().setDraft("new", draft);
    useWorkbench.getState().openNewChat(project);
  };

  const onCommand = async (command: LocalCommandName, args: string): Promise<boolean | void> => {
    switch (command) {
      case "nova":
        newChat(args || undefined);
        return;
      case "modelo": {
        const wanted = args.trim();
        const pill = () => document.querySelector<HTMLButtonElement>(`[data-chat-tab="${tabId}"] [data-model-pill]`);
        if (!wanted) {
          // Sem nome: abre a lista da pílula de modelo.
          window.setTimeout(() => pill()?.click(), 0);
          return;
        }
        const current = task?.provider ?? providerId;
        if (/^(padr[aã]o|default)$/i.test(wanted)) return void (await changeModel(""));
        const list = await api.agentsModels(current);
        const found = list.find((m) => m.value.toLowerCase() === wanted.toLowerCase());
        if (!found) {
          window.setTimeout(() => pill()?.click(), 0);
          throw new Error(`A CLI não informa o modelo "${wanted}". Escolha um da lista.`);
        }
        await changeModel(found.value);
        return;
      }
      case "sessoes":
        document.querySelector<HTMLButtonElement>(`[data-chat-tab="${tabId}"] .cv-title`)?.click();
        return;
      case "diff":
        if (taskId == null) return false;
        useLayout.getState().showChanges();
        return;
      case "aplicar":
        if (taskId == null) return false;
        if (!(await api.systemConfirm("Aplicar alterações", "Aplicar as alterações desta tarefa no projeto?"))) return;
        await applyTask(taskId);
        return;
      case "descartar":
        if (taskId == null) return false;
        if (
          !(await api.systemConfirm("Descartar alterações", "Descartar as alterações desta tarefa? Isso não pode ser desfeito."))
        )
          return;
        await discardTask(taskId);
        return;
      default:
        return false;
    }
  };

  const tokens = totalTokens(totals);
  const closed = task != null && (task.state === "Applied" || task.state === "Discarded");
  /** Sem sessão da CLI não há como continuar (com o turno rodando, a mensagem entra na fila). */
  const noSession = task != null && !busy && task.session_id == null && !task.handoff;
  const missing = taskId != null && !task;
  const placeholder = closed
    ? "Esta tarefa foi encerrada. Use + para começar outra conversa."
    : noSession
      ? "Esta tarefa não tem sessão da CLI para continuar."
      : taskId == null
        ? newProject
          ? "Descreva a tarefa…"
          : "Escolha um projeto na pílula abaixo para escrever o pedido."
        : undefined;

  /** O último turno do agente principal escreveu a chamada de ferramenta como texto? */
  const callAsText = useMemo(() => {
    if (busy || current || taskId == null) return false;
    const parts: string[] = [];
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i];
      if (e.type === "user") break;
      if (e.type === "text") parts.unshift(e.text);
    }
    return toolCallAsText(parts.join("\n"));
  }, [busy, current, entries, taskId]);

  const providerName = (id: string) => providers.find((p) => p.id === id)?.display_name ?? id;

  const providerChoices = providers.map((p) => ({ id: p.id, name: p.display_name }));
  const pills = (
    <>
      {taskId == null && (
        <Select
          variant="pill"
          ariaLabel="Projeto"
          title="Projeto em que a conversa vai acontecer"
          icon={<Icon name="folder" size={12} />}
          placeholder="Nenhum projeto"
          value={newProject ?? ""}
          options={projects.map((p) => ({ value: p.path, label: p.name }))}
          onChange={(v) => useWorkbench.getState().setChatProject(tabId, v || null)}
        />
      )}
      {taskId == null ? (
        <AgentModelPill
          providers={providerChoices.length ? providerChoices : [{ id: providerId, name: providerName(providerId) }]}
          provider={providerId}
          onProvider={(v) => void changeProvider(v)}
          providerHint="Agente que vai atender (a CLI oficial, com o seu login)."
          model={newModel}
          onModel={(v) => void changeModel(v)}
          modelHint={
            providerId === "claude"
              ? "Custo: a 1ª mensagem de cada conversa grava o contexto no cache (cerca de 35 mil tokens, US$ 0,04 no Haiku); as seguintes leem esse cache, bem mais barato. Para perguntas avulsas, continue numa conversa aberta."
              : undefined
          }
        />
      ) : (
        task && (
          <AgentModelPill
            providers={providerChoices.length ? providerChoices : [{ id: task.provider, name: providerName(task.provider) }]}
            provider={task.provider}
            onProvider={(v) => void changeProvider(v)}
            providerDisabled={busy || closed}
            providerHint={
              busy
                ? "Aguarde o turno terminar para trocar de agente."
                : closed
                  ? "Esta tarefa foi encerrada."
                  : "Ao trocar, a tarefa e a pasta continuam as mesmas: o outro agente abre uma sessão nova e recebe um resumo da conversa até aqui."
            }
            model={task.model ?? ""}
            onModel={(v) => void changeModel(v)}
            disabled={closed}
            modelHint={
              busy
                ? "Vale a partir do próximo turno (o turno em andamento segue com o modelo com que começou)."
                : "Vale a partir do próximo turno desta conversa."
            }
            warning={
              task.provider === "claude" && (task.turns > 0 || task.session_id)
                ? "Trocar o modelo reprocessa a conversa inteira no próximo turno (fica mais caro)."
                : null
            }
          />
        )
      )}
      <span className="chat-pills-gap" />
      {task && <AgentState task={task} label />}
      {task?.context && <ContextMeter ctx={task.context} />}
      <UsagePill />
      <ComposerOptions
        project={project}
        taskId={taskId}
        mode={task ? task.mode : newMode}
        defaultMode={modeInfo?.effective ?? "Isolated"}
        modeEditable={taskId == null && !!newProject && isGit}
        noGit={taskId == null && !isGit}
        onMode={setModeChoice}
        access={task?.access ?? accessChoice}
        onAccess={closed ? undefined : (a) => void changeAccess(a)}
      />
    </>
  );

  return (
    <DropZone
      className="cv"
      onFiles={(files) => composer.current?.addFiles(files)}
      disabled={current != null || closed || noSession}
    >
      <div className="cv-root" data-chat-tab={tabId}>
        <header className="cv-head">
          <SessionPicker current={task ?? null} project={project} onNew={() => newChat()} />
          <button className="cv-hbtn" title="Nova conversa" aria-label="Nova conversa" onClick={() => newChat()}>
            <Icon name="plus" size={15} />
          </button>
          <span className="cv-head-space" />
          {task && (
            <span
              className="cv-agent"
              title={`Agente: ${providerName(task.provider)}\nModelo: ${task.model ?? "padrão da CLI"}\nModo de trabalho: ${WORK_MODE_LABEL[task.mode]}${task.handoff ? "\nO próximo turno leva o resumo da conversa para o agente novo." : ""}`}
            >
              {task.model ?? "modelo padrão"}
            </span>
          )}
          {task?.access === "bypass" && <BypassBadge />}
          {task?.access === "plan" && (
            <span className="cv-agent access-plan" title="Esta conversa está em Planejar: o agente só lê e propõe.">
              {ACCESS_LABEL.plan}
            </span>
          )}
          {taskId != null && <SessionServersBadge taskId={taskId} />}
          {taskId != null && (
            <button
              className="cv-hbtn"
              title="Mapa de agentes: subagentes e tarefas em segundo plano desta sessão (painel direito)"
              aria-label="Abrir o mapa de agentes"
              onClick={() => useLayout.getState().openRight("agents")}
            >
              <Icon name="tree" size={15} />
            </button>
          )}
          {totals.turns > 0 && (
            <span
              className="cv-total"
              title={`Total desta conversa\nentrada ${formatIntPt(totals.input_tokens)} · saída ${formatIntPt(totals.output_tokens)} · cache lido ${formatIntPt(totals.cache_read)} · cache gravado ${formatIntPt(totals.cache_write)}\nO custo é o equivalente em API (com assinatura você não paga por token).`}
            >
              <span className="cv-total-tok">
                {formatTokensPt(tokens)} tokens{totals.cost != null && " · "}
              </span>
              {totals.cost != null && <span>{formatUsdPt(totals.cost)} equivalente em API</span>}
            </span>
          )}
        </header>

        {current && <SubagentBar node={current} taskActive={busy} now={now} onBack={() => selectSub(null)} />}

        {taskId == null ? (
          <div className="cv-scroll">
            <div className="cv-col cv-center">
              <Welcome project={newProject} hasRoot={root != null} mode={newMode} onPick={(text) => composer.current?.insertText(text)} />
            </div>
          </div>
        ) : (
          <Transcript
            entries={entries}
            subagents={byId}
            taskActive={busy}
            now={now}
            pricing={current ? null : pricing}
            onOpenSubagent={selectSub}
            providerName={providerName}
            project={project}
            onRetry={current || closed || noSession || busy ? undefined : (text) => send(text)}
            hideDenied={task?.access === "plan"}
            checkpoints={current || closed ? undefined : checkpoints}
            onRestore={current || closed ? undefined : (n) => void restore(n)}
            onAnswer={
              current || closed || noSession
                ? undefined
                : (text) => send(text).catch((e) => setError(errorMessage(e)))
            }
            group={!current}
            resetKey={current ? current.id : "main"}
            before={
              current ? (
                <div className="ses-sub-block">
                  <div className="ses-sub-label">Prompt recebido</div>
                  <div className="ses-sub-box">{current.prompt || "(vazio)"}</div>
                </div>
              ) : missing ? (
                <div className="cv-note">A tarefa #{taskId} não foi encontrada.</div>
              ) : !loaded ? (
                <div className="cv-note">Carregando o histórico…</div>
              ) : null
            }
            follow={draft.length}
            liveItem={!current && busy && draft ? <DraftText text={draft} /> : null}
            after={
              current ? (
                current.done ? (
                  <div className="ses-sub-block">
                    <div className="ses-sub-label">
                      Resultado devolvido{" "}
                      <span className={current.ok === false ? "err" : "ok"}>{current.ok === false ? "✗ erro" : "✓"}</span>
                    </div>
                    <div className="ses-sub-box">{current.result || "(sem texto)"}</div>
                  </div>
                ) : busy ? (
                  <WorkingLine activity={liveActivity(entries, project)} />
                ) : (
                  <div className="cv-note">O turno parou antes de o subagente terminar.</div>
                )
              ) : busy ? (
                <WorkingLine activity={liveActivity(entries, project)} />
              ) : null
            }
          />
        )}

        {!current && !missing && (
          <div className="cv-foot">
            <div className="cv-col">
              {!closed && <ServerStoppedNotice events={events} project={project} busy={busy} />}
              {notice && !error && (
                <div className="chat-notice" role="status">
                  <pre>{notice}</pre>
                  <button type="button" title="Fechar" aria-label="Fechar aviso" onClick={() => setNotice(null)}>
                    ✕
                  </button>
                </div>
              )}
              {(error ?? loadError) && (
                <div className="chat-notice err" role="alert">
                  <pre>{error ?? loadError}</pre>
                  <button type="button" title="Fechar" aria-label="Fechar aviso" onClick={() => setError(null)}>
                    ✕
                  </button>
                </div>
              )}
              {callAsText && !closed && !noSession && dismissedAt !== entries.length && (
                <div className="chat-notice warn">
                  <pre>O agente escreveu a chamada de ferramenta como texto, então nada foi executado.</pre>
                  <button
                    type="button"
                    className="primary"
                    onClick={() => void send(RUN_FOR_REAL_PROMPT).catch((e) => setError(errorMessage(e)))}
                  >
                    Executar de verdade
                  </button>
                  <button type="button" title="Fechar" aria-label="Fechar aviso" onClick={() => setDismissedAt(entries.length)}>
                    ✕
                  </button>
                </div>
              )}
              {hold?.kind === "isolation" && (
                <IsolationHold
                  report={hold.report}
                  busy={holdBusy}
                  onDirect={() => void releaseHold({ mode: "Direct" })}
                  onIsolated={() => void releaseHold({ mode: "Isolated", skipIsolation: true })}
                  onCancel={cancelHold}
                />
              )}
              {hold?.kind === "direct" && (
                <DirectHold
                  others={hold.others}
                  busy={holdBusy}
                  onGo={() => void releaseHold({ mode: hold.mode ?? undefined, skipDirect: true })}
                  onCancel={cancelHold}
                />
              )}
              {task?.access === "plan" && !busy && !closed && task.session_id != null && (
                <div className="cv-note cv-plan">
                  <span>Modo Planejar: o agente só propôs, nada foi alterado.</span>
                  <button type="button" className="pop-link" onClick={() => void runPlan()}>
                    Executar o plano
                  </button>
                </div>
              )}
              {task?.handoff && !busy && !closed && (
                <div className="cv-note cv-handoff">
                  Agente trocado para {providerName(task.provider)}: o próximo pedido leva junto um resumo da conversa até aqui.
                </div>
              )}
              {taskId != null && (
                <div className="cv-strip">
                  <AutoWorkBadge taskId={taskId} readOnly={closed} />
                </div>
              )}
              {!closed && <EditorChips root={project} />}
              <Composer
                ref={composer}
                taskId={taskId}
                draftId={draftId}
                busy={busy}
                disabled={closed || noSession || hold != null || (taskId == null && !newProject)}
                workdir={task ? (task.workdir ?? task.project) : newProject}
                provider={task?.provider ?? providerId}
                placeholder={placeholder}
                onSend={(prompt) => send(withEditorContext(prompt, project))}
                onStop={taskId != null && !cancelling ? cancel : undefined}
                onCommand={onCommand}
                toolbar={pills}
                recall={recall}
                autoFocus={active}
              />
            </div>
          </div>
        )}
      </div>
    </DropZone>
  );
}
