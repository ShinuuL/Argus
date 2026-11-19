import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { api, errorMessage, type Path, type TaskId } from "@/api";
import { Icon } from "@/shell/icons";
import { AttachmentChips } from "./AttachmentChips";
import {
  buildPrompt,
  discard,
  fromFile,
  fromPaste,
  isBigPaste,
  type ChatAttachment,
  type Prepared,
  type SentAttachment,
} from "./attachments";
import { chatAgentCapabilities, chatAgents, chatAttachmentsCleanup, chatProjectFiles, type ChatAgent } from "./backend";
import { DropZone } from "./DropZone";
import { atCursor, isSafeRelative, replaceAt, score, type AtToken } from "./mentions";
import { QueueStrip } from "./QueueStrip";
import {
  allSlashItems,
  filterSlash,
  LOCAL_COMMANDS,
  readLocalCommand,
  replaceSlash,
  slashCursor,
  type LocalCommand,
  type LocalCommandName,
  type SlashToken,
} from "./slash";
import { chatKey, markUrgent, selectAttachments, selectQueue, sendNextQueued, useChat, type QueueItem } from "./store";
import { SuggestPopover, type SuggestItem } from "./SuggestPopover";

/** O que a caixa entrega em `onSend`. */
export interface OutgoingMessage {
  /** O que o usuário digitou, sem o bloco de anexos. */
  text: string;
  /** Anexos já gravados em disco. */
  attachments: SentAttachment[];
  /** Texto completo que vai ao agente (texto + bloco "Arquivos anexados pelo caminho"). */
  prompt: string;
}

/** Controle da caixa por quem a hospeda (`ref`). */
export interface ComposerHandle {
  /** Anexa arquivos (ex.: soltos numa `DropZone` em volta da conversa). */
  addFiles(files: File[]): void;
  focus(): void;
  /** Acrescenta texto ao rascunho. */
  insertText(text: string): void;
}

export interface ComposerProps {
  /**
   * Tarefa desta conversa; `null` = conversa ainda não criada. Rascunho, anexos
   * e fila são guardados por tarefa e sobrevivem à troca de aba.
   */
  taskId: TaskId | null;
  /** Id de rascunho da aba, enquanto a conversa não tem tarefa (separa caixa, fila e anexos por aba). */
  draftId?: string;
  /**
   * Envia uma mensagem agora. Recebe o texto pronto para o agente (`prompt`,
   * já com os anexos pelo caminho) e as partes separadas. Se rejeitar, a caixa
   * devolve o rascunho e mostra o erro.
   *
   * Só é chamado com o agente parado. Com `busy`, a mensagem entra na fila, e a
   * fila sai sozinha por `api.taskReply(taskId, prompt)` quando o turno termina
   * (mesmo com a aba fechada), sem passar por `onSend`.
   */
  onSend: (prompt: string, message: OutgoingMessage) => void | Promise<void>;
  /** O agente está trabalhando (tarefa na fila ou rodando): Enter enfileira. */
  busy?: boolean;
  /**
   * Pasta onde o agente roda (worktree da tarefa, ou o projeto antes de ela
   * existir). Base da busca de `@arquivo`, dos agentes e dos comandos da CLI.
   */
  workdir?: Path | null;
  /** "claude" ou "opencode": decide se os comandos de barra da CLI são consultados. */
  provider?: string | null;
  /** Interrompe o turno (botão Parar, `/parar` e Ctrl+Enter "parar e enviar"). */
  onStop?: () => void | Promise<void>;
  /**
   * Comandos locais que dependem da tela (`/nova`, `/diff`, `/aplicar`,
   * `/descartar`, `/sessoes`, `/modelo`). Devolva `false` para "não disponível
   * aqui". `/parar` e `/ajuda` são tratados pela própria caixa.
   */
  onCommand?: (command: LocalCommandName, args: string) => boolean | void | Promise<boolean | void>;
  /** Bloqueia a caixa (ex.: tarefa aplicada ou descartada). */
  disabled?: boolean;
  placeholder?: string;
  autoFocus?: boolean;
  /** Pílulas na barra de baixo (projeto, provedor, auto-work...), depois dos botões de anexar, "/" e "@". */
  toolbar?: ReactNode;
  /** Pedidos já enviados nesta conversa (o mais novo por último): seta para cima na caixa vazia os recupera. */
  recall?: string[];
}

interface Notice {
  tone: "info" | "err";
  text: string;
}

type Suggest =
  | { mode: "slash"; items: SuggestItem[]; index: number; loading: boolean; token: SlashToken }
  | { mode: "at"; items: SuggestItem[]; index: number; loading: boolean; token: AtToken };

/** Espera depois da última tecla antes de buscar arquivos. */
const SEARCH_DELAY_MS = 120;
const MAX_HEIGHT = 200;

/**
 * Caixa de composição do chat: texto com altura automática, menções `@arquivo`
 * e `@agente`, comandos `/`, anexos (clipe, arrastar, colar print ou texto
 * grande) e fila de mensagens enquanto o agente trabalha.
 */
export const Composer = forwardRef<ComposerHandle, ComposerProps>(function Composer(
  { taskId, draftId, onSend, busy = false, workdir, provider, onStop, onCommand, disabled = false, placeholder, autoFocus, toolbar, recall },
  ref,
) {
  const key = chatKey(taskId, draftId);
  const text = useChat((s) => s.drafts[key] ?? "");
  const attachments = useChat(selectAttachments(key));
  const queue = useChat(selectQueue(key));
  const paused = useChat((s) => s.paused[key] ?? null);

  const [notice, setNotice] = useState<Notice | null>(null);
  const [suggest, setSuggest] = useState<Suggest | null>(null);
  const [agents, setAgents] = useState<ChatAgent[]>([]);
  const [cliCommands, setCliCommands] = useState<string[]>([]);
  const [sending, setSending] = useState(false);

  const input = useRef<HTMLTextAreaElement>(null);
  const picker = useRef<HTMLInputElement>(null);
  const searchId = useRef(0);
  const searchTimer = useRef<number | undefined>(undefined);
  const capsAsked = useRef<string | null>(null);
  /** Posição na lista `recall` enquanto o usuário navega com as setas (null = fora dela). */
  const recallAt = useRef<number | null>(null);

  const setText = useCallback((value: string) => useChat.getState().setDraft(key, value), [key]);

  useEffect(() => {
    chatAttachmentsCleanup();
  }, []);

  useEffect(() => {
    let alive = true;
    void chatAgents(workdir ?? null).then((list) => {
      if (alive) setAgents(list);
    });
    return () => {
      alive = false;
    };
  }, [workdir]);

  // altura automática
  useLayoutEffect(() => {
    const el = input.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, MAX_HEIGHT)}px`;
  }, [text]);

  useEffect(() => {
    if (autoFocus) input.current?.focus();
  }, [autoFocus, key]);

  useEffect(() => () => window.clearTimeout(searchTimer.current), []);

  // ------------------------------------------------------------- anexos

  const addPrepared = useCallback(
    (p: Prepared) => {
      const store = useChat.getState();
      store.addAttachment(key, p.attachment);
      void p.save().then((saved) => useChat.getState().updateAttachment(key, saved));
    },
    [key],
  );

  const addFiles = useCallback(
    (files: File[]) => {
      if (disabled) return;
      for (const file of files) {
        fromFile(file, taskId, draftId ?? null)
          .then(addPrepared)
          .catch((e) => setNotice({ tone: "err", text: errorMessage(e) }));
      }
    },
    [addPrepared, disabled, taskId, draftId],
  );

  const removeAttachment = (att: ChatAttachment) => {
    discard(att);
    useChat.getState().removeAttachment(key, att.id);
  };

  const restoreText = (att: ChatAttachment) => {
    useChat.getState().removeAttachment(key, att.id);
    setText(text ? `${text}\n${att.text ?? ""}` : (att.text ?? ""));
    input.current?.focus();
  };

  useImperativeHandle(
    ref,
    () => ({
      addFiles,
      focus: () => input.current?.focus(),
      insertText: (extra: string) => {
        const current = useChat.getState().drafts[key] ?? "";
        setText(current && !/\s$/.test(current) ? `${current} ${extra}` : current + extra);
        input.current?.focus();
      },
    }),
    [addFiles, key, setText],
  );

  // ---------------------------------------------------------- sugestões

  const loadCliCommands = useCallback(() => {
    if (!workdir || provider !== "claude" || capsAsked.current === workdir) return;
    capsAsked.current = workdir;
    void chatAgentCapabilities(workdir, provider).then((caps) => {
      // sem resposta (CLI ausente, comando ainda não registrado): ficam só os locais
      const commands = caps?.slash_commands ?? [];
      if (commands.length) setCliCommands(commands);
      // a resposta chegou com o menu aberto: refaz com o que já está digitado
      const draft = useChat.getState().drafts[key] ?? "";
      const token = slashCursor(draft, input.current?.selectionStart ?? draft.length);
      if (token) {
        setSuggest((prev) =>
          prev?.mode === "slash"
            ? { mode: "slash", token, items: slashItems(commands, token.query), index: 0, loading: false }
            : prev,
        );
      }
    });
  }, [key, provider, workdir]);

  const refreshSuggest = useCallback(
    (value: string, cursor: number) => {
      window.clearTimeout(searchTimer.current);
      const slash = slashCursor(value, cursor);
      if (slash) {
        const waiting = !!workdir && provider === "claude" && !cliCommands.length && capsAsked.current !== workdir;
        setSuggest({ mode: "slash", token: slash, items: slashItems(cliCommands, slash.query), index: 0, loading: waiting });
        loadCliCommands();
        return;
      }
      const token = atCursor(value, cursor);
      if (!token) {
        searchId.current++;
        setSuggest(null);
        return;
      }
      const agentItems: SuggestItem[] = agents
        .map((a) => ({ a, points: score(a.name, token.query) }))
        .filter((x) => x.points > 0)
        .sort((x, y) => y.points - x.points)
        .slice(0, 8)
        .map(({ a }) => ({ key: `agent:${a.name}`, label: a.name, detail: a.description, tag: "agente" }));
      const id = ++searchId.current;
      setSuggest((prev) => ({
        mode: "at",
        token,
        // mantém os arquivos da busca anterior até a nova chegar (evita piscar)
        items: [...agentItems, ...(prev?.mode === "at" ? prev.items.filter((i) => i.key.startsWith("file:")) : [])],
        index: 0,
        loading: !!workdir,
      }));
      if (!workdir) return;
      searchTimer.current = window.setTimeout(() => {
        void chatProjectFiles(workdir, token.query, 30).then((files) => {
          if (id !== searchId.current) return; // resposta de uma busca velha
          const fileItems: SuggestItem[] = files.filter(isSafeRelative).map((rel) => {
            const slash = rel.lastIndexOf("/");
            return {
              key: `file:${rel}`,
              label: rel.slice(slash + 1),
              detail: slash >= 0 ? rel.slice(0, slash) : undefined,
              tag: "arquivo",
            };
          });
          setSuggest({ mode: "at", token, items: [...agentItems, ...fileItems], index: 0, loading: false });
        });
      }, SEARCH_DELAY_MS);
    },
    [agents, cliCommands, loadCliCommands, provider, workdir],
  );

  const closeSuggest = () => {
    window.clearTimeout(searchTimer.current);
    searchId.current++;
    setSuggest(null);
  };

  const pick = (item: SuggestItem) => {
    if (!suggest) return;
    // os dois modos trocam só o trecho sob o cursor: o resto da mensagem fica
    const next =
      suggest.mode === "slash"
        ? replaceSlash(text, suggest.token, item.label.slice(1))
        : replaceAt(text, suggest.token, item.key.startsWith("file:") ? item.key.slice(5) : item.label);
    setText(next.text);
    closeSuggest();
    requestAnimationFrame(() => {
      input.current?.focus();
      input.current?.setSelectionRange(next.cursor, next.cursor);
    });
  };

  // --------------------------------------------------------------- envio

  const runLocal = async (command: LocalCommand, args: string) => {
    if (command.name === "ajuda") {
      const lines = LOCAL_COMMANDS.map((c) => `/${c.name}${c.args ? ` ${c.args}` : ""} · ${c.description}`);
      setNotice({
        tone: "info",
        text:
          lines.join("\n") +
          "\n/ abre os comandos em qualquer ponto da mensagem; os do Argus acima só executam se a barra for a primeira coisa escrita." +
          "\n@ menciona arquivos e agentes · Enter envia · Shift+Enter quebra a linha" +
          "\nCom o agente trabalhando, Enter põe na fila e Ctrl+Enter para o turno e envia.",
      });
      return;
    }
    if (command.name === "parar") {
      if (busy && onStop) await onStop();
      else setNotice({ tone: "info", text: "Não há turno em andamento." });
      return;
    }
    if (command.needsTask && taskId == null) {
      setNotice({ tone: "info", text: `/${command.name} precisa de uma tarefa aberta.` });
      return;
    }
    const handled = onCommand ? await onCommand(command.name, args) : false;
    if (handled === false) setNotice({ tone: "info", text: `/${command.name} ainda não está disponível aqui.` });
  };

  const send = async (now: boolean) => {
    const raw = text.trim();
    if (disabled || sending || (!raw && !attachments.length)) return;
    if (attachments.some((a) => a.status === "saving")) {
      setNotice({ tone: "info", text: "Aguarde: os anexos ainda estão sendo gravados." });
      return;
    }
    if (attachments.some((a) => a.status === "error")) {
      setNotice({ tone: "err", text: "Tire os anexos com erro antes de enviar." });
      return;
    }
    closeSuggest();
    setNotice(null);
    const store = useChat.getState();

    // comandos locais executam na hora e nunca entram na fila; os anexos, se
    // houver, continuam na caixa (um comando do Argus nunca vai para o agente)
    const local = readLocalCommand(raw);
    if (local) {
      setText("");
      try {
        await runLocal(local.command, local.args);
      } catch (e) {
        setNotice({ tone: "err", text: errorMessage(e) });
      }
      return;
    }

    if (busy) {
      if (now && taskId != null && onStop) {
        const ok = await api
          .systemConfirm("Parar e enviar", "Interromper o turno em andamento e enviar esta mensagem agora?")
          .catch(() => false);
        if (!ok) return;
        store.enqueue(key, { text: raw, attachments }, true);
        store.setDraft(key, "");
        store.setAttachments(key, []);
        markUrgent(taskId);
        try {
          await onStop();
        } catch (e) {
          setNotice({ tone: "err", text: errorMessage(e) });
        }
        return;
      }
      store.enqueue(key, { text: raw, attachments });
      store.setDraft(key, "");
      store.setAttachments(key, []);
      return;
    }

    const prompt = buildPrompt(raw, attachments);
    const sent: SentAttachment[] = attachments
      .filter((a) => a.path)
      .map((a) => ({ kind: a.kind, name: a.name, path: a.path! }));
    store.setDraft(key, "");
    store.setAttachments(key, []);
    setSending(true);
    try {
      await onSend(prompt, { text: raw, attachments: sent, prompt });
    } catch (e) {
      // devolve o rascunho (se o usuário ainda não começou outro)
      const s = useChat.getState();
      if (!(s.drafts[key] ?? "")) s.setDraft(key, raw);
      s.setAttachments(key, [...attachments, ...(s.attachments[key] ?? [])]);
      setNotice({ tone: "err", text: errorMessage(e) });
    } finally {
      setSending(false);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing) return;
    if (suggest && suggest.items.length) {
      const n = suggest.items.length;
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        setSuggest({ ...suggest, index: (suggest.index + (e.key === "ArrowDown" ? 1 : -1) + n) % n });
        return;
      }
      if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey && !e.ctrlKey && !e.metaKey)) {
        e.preventDefault();
        pick(suggest.items[suggest.index]);
        return;
      }
    }
    if (suggest && e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      closeSuggest();
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      recallAt.current = null;
      void send(e.ctrlKey || e.metaKey);
      return;
    }
    // Seta para cima na caixa vazia (ou já navegando): recupera os pedidos anteriores.
    const list = recall ?? [];
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && list.length && !e.shiftKey && !e.altKey) {
      const browsing = recallAt.current != null && text === list[recallAt.current];
      if (e.key === "ArrowUp" && (text === "" || browsing)) {
        e.preventDefault();
        const next = browsing ? Math.max(0, recallAt.current! - 1) : list.length - 1;
        recallAt.current = next;
        setText(list[next]);
      } else if (e.key === "ArrowDown" && browsing) {
        e.preventDefault();
        const next = recallAt.current! + 1;
        recallAt.current = next < list.length ? next : null;
        setText(next < list.length ? list[next] : "");
      }
    }
  };

  /** Botões "/" e "@": escrevem o caractere na posição do cursor e abrem as sugestões. */
  const startToken = (ch: "/" | "@") => {
    const el = input.current;
    if (!el || disabled) return;
    const at = el.selectionStart ?? text.length;
    const before = text.slice(0, at);
    const sep = before && !/[\s(]$/.test(before) ? " " : "";
    const value = `${before}${sep}${ch}${text.slice(at)}`;
    const cursor = at + sep.length + 1;
    setText(value);
    el.focus();
    requestAnimationFrame(() => el.setSelectionRange(cursor, cursor));
    refreshSuggest(value, cursor);
  };

  const onPaste = (e: ClipboardEvent<HTMLTextAreaElement>) => {
    const files = Array.from(e.clipboardData.files);
    if (files.length) {
      e.preventDefault();
      addFiles(files);
      return;
    }
    const pasted = e.clipboardData.getData("text/plain");
    if (pasted && isBigPaste(pasted)) {
      e.preventDefault();
      addPrepared(fromPaste(pasted, taskId, draftId ?? null));
    }
  };

  // ---------------------------------------------------------------- fila

  const editQueued = (item: QueueItem) => {
    const store = useChat.getState();
    const taken = store.takeQueued(key, item.id);
    if (!taken) return;
    store.setDraft(key, text ? `${text}\n${taken.text}` : taken.text);
    store.setAttachments(key, [...attachments, ...taken.attachments]);
    input.current?.focus();
  };

  const removeQueued = (item: QueueItem) => {
    const taken = useChat.getState().takeQueued(key, item.id);
    taken?.attachments.forEach(discard);
    if (queue.length <= 1) useChat.getState().setPaused(key, null);
  };

  const resumeQueue = () => {
    if (taskId == null) return;
    if (busy) {
      // o turno atual vai terminar e puxar a fila
      useChat.getState().setPaused(key, null);
      return;
    }
    void sendNextQueued(taskId);
  };

  const canSend = !disabled && !sending && (!!text.trim() || attachments.length > 0);
  const hint = busy
    ? "Enter põe na fila · Ctrl+Enter para o turno e envia agora"
    : "Enter envia · Shift+Enter quebra a linha · ↑ recupera o último pedido";

  return (
    <DropZone className="chat-composer" onFiles={addFiles} disabled={disabled}>
      <QueueStrip items={queue} paused={paused} onEdit={editQueued} onRemove={removeQueued} onResume={resumeQueue} />
      {notice && (
        <div className={`chat-notice ${notice.tone}`}>
          <pre>{notice.text}</pre>
          <button type="button" title="Fechar" aria-label="Fechar aviso" onClick={() => setNotice(null)}>
            ✕
          </button>
        </div>
      )}
      <AttachmentChips attachments={attachments} onRemove={removeAttachment} onRestoreText={restoreText} />
      <div className="chat-box">
        {suggest && (
          <SuggestPopover
            items={suggest.items}
            index={suggest.index}
            onPick={pick}
            onHover={(index) => setSuggest((s) => (s ? { ...s, index } : s))}
            footer={
              suggest.loading
                ? suggest.mode === "slash"
                  ? "consultando os comandos da CLI…"
                  : "buscando arquivos…"
                : suggest.items.length
                  ? undefined
                  : suggest.mode === "at"
                    ? "nada encontrado"
                    : undefined
            }
          />
        )}
        <textarea
          ref={input}
          className="chat-input"
          rows={1}
          value={text}
          disabled={disabled}
          placeholder={placeholder ?? (busy ? "Escreva a próxima mensagem (entra na fila)…" : "Escreva para o agente…")}
          onChange={(e) => {
            setText(e.target.value);
            refreshSuggest(e.target.value, e.target.selectionStart ?? e.target.value.length);
          }}
          onKeyDown={onKeyDown}
          onPaste={onPaste}
          onDragOver={(e) => {
            if (Array.from(e.dataTransfer.types).includes("application/x-argus-path")) e.preventDefault();
          }}
          onDrop={(e) => {
            // Arquivo arrastado da árvore do painel direito: vira uma menção.
            const rel = e.dataTransfer.getData("application/x-argus-path");
            if (!rel) return;
            e.preventDefault();
            const current = useChat.getState().drafts[key] ?? "";
            setText(`${current}${current && !/\s$/.test(current) ? " " : ""}@${rel} `);
            input.current?.focus();
          }}
          onBlur={() => window.setTimeout(closeSuggest, 150)}
        />
        <div className="chat-bar">
          <input
            ref={picker}
            type="file"
            multiple
            hidden
            onChange={(e) => {
              addFiles(Array.from(e.target.files ?? []));
              e.target.value = "";
            }}
          />
          <button
            type="button"
            className="chat-ibtn"
            title="Anexar arquivos"
            aria-label="Anexar arquivos"
            disabled={disabled}
            onClick={() => picker.current?.click()}
          >
            <Icon name="clip" size={15} />
          </button>
          <button
            type="button"
            className="chat-ibtn"
            title="Comandos de barra (/)"
            aria-label="Comandos de barra"
            disabled={disabled}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => startToken("/")}
          >
            <Icon name="slash" size={15} />
          </button>
          <button
            type="button"
            className="chat-ibtn"
            title="Mencionar arquivo ou agente (@)"
            aria-label="Mencionar arquivo ou agente"
            disabled={disabled}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => startToken("@")}
          >
            <Icon name="at" size={15} />
          </button>
          <div className="chat-pills">{toolbar}</div>
          {busy && canSend && (
            <button type="button" className="chat-queue-btn" title="Pôr na fila (Enter)" onClick={() => void send(false)}>
              Pôr na fila
            </button>
          )}
          {busy && onStop ? (
            <button type="button" className="chat-round chat-stop" title="Parar o turno" aria-label="Parar o turno" onClick={() => void onStop()}>
              <Icon name="stop" size={14} filled />
            </button>
          ) : (
            <button
              type="button"
              className="chat-round chat-send"
              title={busy ? "Pôr na fila (Enter)" : "Enviar (Enter)"}
              aria-label={busy ? "Pôr na fila" : "Enviar"}
              disabled={!canSend}
              onClick={() => void send(false)}
            >
              <Icon name="send" size={16} />
            </button>
          )}
        </div>
      </div>
      <div className="chat-hint">{hint}</div>
    </DropZone>
  );
});

function slashItems(cliCommands: string[], q: string): SuggestItem[] {
  return filterSlash(allSlashItems(cliCommands), q).map((c) => ({
    key: `slash:${c.name}`,
    label: `/${c.name}`,
    detail: c.description,
    tag: c.local ? "Argus" : undefined,
  }));
}
