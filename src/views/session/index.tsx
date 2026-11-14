/**
 * Conversa com um agente (aba principal do workbench).
 *
 * Uma superfície só: a conversa nova (saudação + caixa de composição, com as
 * pílulas de projeto e provedor) e a sessão de uma tarefa (histórico, stream ao
 * vivo, fila, anexos, comandos de barra, consumo). O primeiro envio cria a
 * tarefa e a MESMA aba passa a ser a sessão dela. O chat de um subagente é
 * somente leitura: não tem caixa de resposta.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  baseName,
  errorMessage,
  formatTokens,
  formatUsd,
  isTaskActive,
  onTaskEvent,
  samePath,
  taskStateLabel,
  totalTokens,
  variantOf,
  type Path,
  type Pricing,
  type Task,
  type TaskId,
} from "@/api";
import { Icon } from "@/shell/icons";
import { useApp, useLayout, useWorkbench } from "@/store";
import { useProviders } from "@/views/agents";
import { AutoWorkBadge, AutoWorkPill } from "@/views/autowork";
import {
  Composer,
  DropZone,
  repeatMessage,
  RepeatDetector,
  RUN_FOR_REAL_PROMPT,
  splitPrompt,
  toolCallAsText,
  useChat,
  type ComposerHandle,
  type LocalCommandName,
} from "@/views/chat";
import { SessionServersBadge } from "@/views/servers";
import { applyTask, cancelTask, discardTask } from "./actions";
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
import { Transcript } from "./Transcript";
import { useTaskEvents } from "./useTaskEvents";
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

/** Ponto de status de uma sessão (rodando pulsa). */
export function StateDot({ task }: { task: Task }) {
  const { kind } = variantOf(task.state);
  const cls =
    kind === "Running" || kind === "Queued"
      ? "running"
      : kind === "Completed" || kind === "Applied"
        ? "ok"
        : kind === "Failed"
          ? "error"
          : "interrupted";
  return <span className={`ses-dot ${cls}`} title={taskStateLabel(task.state).label} aria-hidden="true" />;
}

// ------------------------------------------------------------ seletor de sessão

function SessionPicker(props: { current: Task | null; project: Path | null; onNew(): void }) {
  const { current, project, onNew } = props;
  const tasks = useApp((s) => s.tasks);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
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
                title={t.prompt}
                onClick={() => pick(t.id)}
              >
                <StateDot task={t} />
                <span className="ellipsis cv-pop-text">{sessionTitle(t.prompt)}</span>
                <span className="cv-pop-meta">
                  #{t.id} · {taskStateLabel(t.state).label.split(":")[0]}
                </span>
              </button>
            ))}
          </div>
        </div>
      )}
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

function Welcome(props: { project: Path | null; hasRoot: boolean; onPick(text: string): void }) {
  const { project, hasRoot, onPick } = props;
  const pickRoot = useApp((s) => s.pickRoot);
  return (
    <div className="cv-welcome">
      <Icon name="chat" size={30} className="cv-welcome-ic" />
      {project ? (
        <>
          <h2>
            O que vamos fazer em <b>{baseName(project)}</b>?
          </h2>
          <p>Escreva o pedido abaixo. O agente trabalha num worktree isolado; você revisa e aplica as alterações depois.</p>
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
  const projects = useApp((s) => s.projects);
  const root = useApp((s) => s.root);
  const selectedProject = useApp((s) => s.selected);
  const selectedSub = useApp((s) => (taskId == null ? null : (s.subagentOf[taskId] ?? null)));
  const unread = useApp((s) => taskId != null && !!s.unread[taskId]);
  const rightOpen = useLayout((s) => s.rightOpen);
  const providers = useProviders();

  const busy = task ? isTaskActive(task.state) : false;
  const { events, times, loaded, error: loadError } = useTaskEvents(taskId, busy);
  const pricing = usePricing();
  const [provider, setProvider] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  /** Quantidade de itens do chat quando o aviso de "chamada como texto" foi fechado. */
  const [dismissedAt, setDismissedAt] = useState(-1);
  const composer = useRef<ComposerHandle>(null);

  const providerId = provider || (providers.find((p) => p.id === "claude") ?? providers[0])?.id || "claude";
  /** Projeto da conversa: o da tarefa, ou o escolhido para a conversa nova. */
  const newProject =
    projects.find((p) => samePath(p.path, tabProject ?? selectedProject))?.path ?? projects[0]?.path ?? null;
  const project = task?.project ?? newProject;

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

  /** Envia: na conversa nova cria a tarefa e esta mesma aba vira a sessão dela. */
  const send = async (prompt: string) => {
    setError(null);
    if (taskId != null) {
      await api.taskReply(taskId, prompt);
      void useApp.getState().refreshTasks();
      return;
    }
    if (!newProject) throw new Error("Abra uma pasta raiz e escolha um projeto para conversar.");
    const id = await api.taskCreate(newProject, providerId, prompt);
    await useApp.getState().refreshTasks();
    useWorkbench.getState().bindChat(tabId, id);
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

  const state = task ? taskStateLabel(task.state) : null;
  const tokens = totalTokens(totals);
  const closed = task != null && (task.state === "Applied" || task.state === "Discarded");
  /** Sem sessão da CLI não há como continuar (com o turno rodando, a mensagem entra na fila). */
  const noSession = task != null && !busy && task.session_id == null;
  const missing = taskId != null && !task;
  const placeholder = closed
    ? "Esta tarefa foi encerrada. Use + para começar outra conversa."
    : noSession
      ? "Esta tarefa não tem sessão da CLI para continuar."
      : taskId == null
        ? "Descreva a tarefa…"
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

  const projectName = project ? baseName(project) : null;
  const providerName = (id: string) => providers.find((p) => p.id === id)?.display_name ?? id;

  const pills = (
    <>
      {taskId == null ? (
        <>
          <label className="chat-pill chat-pill-select" title="Projeto em que a conversa vai acontecer">
            <Icon name="folder" size={12} />
            <select
              aria-label="Projeto"
              value={newProject ?? ""}
              onChange={(e) => useWorkbench.getState().setChatProject(tabId, e.target.value || null)}
            >
              {!projects.length && <option value="">nenhum projeto</option>}
              {projects.map((p) => (
                <option key={p.path} value={p.path}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
          <label className="chat-pill chat-pill-select" title="Agente que vai atender (a CLI oficial, com o seu login)">
            <Icon name="bot" size={12} />
            <select aria-label="Provedor" value={providerId} onChange={(e) => setProvider(e.target.value)}>
              {!providers.length && <option value={providerId}>{providerId}</option>}
              {providers.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.display_name}
                </option>
              ))}
            </select>
          </label>
        </>
      ) : (
        task && (
          <>
            <span className="chat-pill static" title={task.workdir ?? task.project}>
              <Icon name="folder" size={12} />
              {projectName}
            </span>
            <span className="chat-pill static" title="Agente desta conversa">
              <Icon name="bot" size={12} />
              {providerName(task.provider)}
            </span>
          </>
        )
      )}
      <button
        type="button"
        className="chat-pill"
        title="Modo de permissão: o agente edita arquivos sem perguntar; o shell só roda os comandos permitidos do projeto. Clique para ver os comandos permitidos."
        onClick={() => useWorkbench.getState().openPage("settings")}
      >
        <Icon name="shield" size={12} />
        <span className="chat-pill-label">Edições automáticas</span>
      </button>
      <AutoWorkPill project={project} taskId={taskId} />
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
          {task && state && (
            <span className={`cv-state ${state.tone}`} title={state.label}>
              {state.label.split(":")[0]}
            </span>
          )}
          {taskId != null && <SessionServersBadge taskId={taskId} />}
          {totals.turns > 0 && (
            <span
              className="cv-total"
              title={`Total desta conversa\nentrada ${totals.input_tokens} · saída ${totals.output_tokens} · cache lido ${totals.cache_read} · cache gravado ${totals.cache_write}\nO custo é o equivalente em API (com assinatura você não paga por token).`}
            >
              <span className="cv-total-tok">
                {formatTokens(tokens)} tokens{totals.cost != null && " · "}
              </span>
              {totals.cost != null && <span>{formatUsd(totals.cost)} equivalente em API</span>}
            </span>
          )}
          <button
            className="cv-hbtn"
            aria-pressed={rightOpen}
            title={rightOpen ? "Ocultar o painel direito" : "Mostrar o painel direito (arquivos, alterações, subagentes, servidores)"}
            aria-label="Alternar o painel direito"
            onClick={() => useLayout.getState().toggleRight()}
          >
            <Icon name="sidebarRight" size={15} />
          </button>
        </header>

        {current && <SubagentBar node={current} taskActive={busy} now={now} onBack={() => selectSub(null)} />}

        {taskId == null ? (
          <div className="cv-scroll">
            <div className="cv-col cv-center">
              <Welcome project={newProject} hasRoot={root != null} onPick={(text) => composer.current?.insertText(text)} />
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
                  <div className="cv-working">o subagente está trabalhando…</div>
                ) : (
                  <div className="cv-note">O turno parou antes de o subagente terminar.</div>
                )
              ) : busy ? (
                <div className="cv-working">
                  <span className="ses-dot running" aria-hidden="true" /> o agente está trabalhando…
                </div>
              ) : null
            }
          />
        )}

        {!current && !missing && (
          <div className="cv-foot">
            <div className="cv-col">
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
              {taskId != null && (
                <div className="cv-strip">
                  <AutoWorkBadge taskId={taskId} readOnly={closed} />
                </div>
              )}
              <Composer
                ref={composer}
                taskId={taskId}
                busy={busy}
                disabled={closed || noSession || (taskId == null && !newProject)}
                workdir={task ? (task.workdir ?? task.project) : newProject}
                provider={task?.provider ?? providerId}
                placeholder={placeholder}
                onSend={send}
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
