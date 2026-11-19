/**
 * Pílulas e avisos da caixa de composição que dependem da conversa: a pílula
 * "provedor · modelo", o botão de opções (modo de trabalho, modo de permissão,
 * auto-work) e o aviso que segura o envio
 * quando a cópia isolada não teria o que o pedido precisa.
 */
import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { api, errorMessage, type AccessMode, type IsolationReport, type ModelInfo, type Path, type TaskId, type WorkMode, type WorkModeInfo } from "@/api";
import { Icon } from "@/shell/icons";
import { Popover } from "@/shell/Popover";
import { useWorkbench } from "@/store";
import { useAutoWork } from "@/views/autowork";

// ---------------------------------------------------------------------- modelo

/** Lista de modelos por provedor, guardada enquanto o app está aberto (só listas com modelos). */
const modelsCache = new Map<string, ModelInfo[]>();
/** Consulta em andamento por provedor: quem pede durante ela espera a mesma resposta. */
const modelsInflight = new Map<string, Promise<ModelInfo[]>>();

function fetchModels(provider: string, refresh: boolean): Promise<ModelInfo[]> {
  const running = modelsInflight.get(provider);
  if (running && !refresh) return running;
  const p = api
    .agentsModels(provider, refresh)
    .then((list) => {
      if (!list.length) throw new Error("A CLI não informou nenhum modelo.");
      modelsCache.set(provider, list);
      return list;
    })
    .finally(() => {
      if (modelsInflight.get(provider) === p) modelsInflight.delete(provider);
    });
  modelsInflight.set(provider, p);
  return p;
}

/**
 * Modelos do provedor, com cache em memória. A consulta começa assim que a
 * caixa de composição aparece (a CLI do claude leva de 5 a 20 s para
 * responder), não só quando a lista abre. Erro nunca vai para o cache.
 */
export function useModels(provider: string) {
  const [models, setModels] = useState<ModelInfo[]>(() => modelsCache.get(provider) ?? []);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const current = useRef(provider);
  current.current = provider;

  const load = useCallback(
    async (refresh = false): Promise<ModelInfo[]> => {
      if (!provider) return [];
      const cached = modelsCache.get(provider);
      if (cached?.length && !refresh) {
        setModels(cached);
        setError(null);
        return cached;
      }
      setLoading(true);
      setError(null);
      try {
        const list = await fetchModels(provider, refresh);
        if (current.current === provider) setModels(list);
        return list;
      } catch (e) {
        if (current.current === provider) setError(errorMessage(e));
        return [];
      } finally {
        if (current.current === provider) setLoading(false);
      }
    },
    [provider],
  );

  useEffect(() => {
    setModels(modelsCache.get(provider) ?? []);
    setError(null);
    setLoading(false);
    void load();
  }, [provider, load]);

  return { models, loading, error, load };
}

/** Mesma regra do backend (`valid_model_name`): nada de espaço nem começo com "-". */
export function isValidModelName(m: string): boolean {
  return m.length > 0 && m.length <= 200 && !m.startsWith("-") && /^[A-Za-z0-9._\-/:[\]#@+]+$/.test(m);
}

/**
 * Modelo que o item padrão resolve ("Opus 5 · 1M"), tirado da dica do item
 * marcado como `default`. Vazio enquanto a lista não carregou ou quando o
 * provedor não informa.
 */
export function defaultResolves(models: ModelInfo[]): string {
  const hint = models.find((m) => m.default)?.hint ?? "";
  const i = hint.indexOf(" — ");
  const which = i > 0 ? hint.slice(0, i).trim() : "";
  return which && which !== "o padrão da CLI" ? which : "";
}

export interface ProviderChoice {
  id: string;
  name: string;
}

export interface AgentModelPillProps {
  providers: ProviderChoice[];
  provider: string;
  onProvider(id: string): void;
  /** Troca de provedor bloqueada (turno em andamento, tarefa encerrada). */
  providerDisabled?: boolean;
  /** Por que o provedor não pode ser trocado agora (ou o que acontece ao trocar). */
  providerHint?: string;
  /** Modelo escolhido; "" = padrão da CLI. */
  model: string;
  onModel(model: string): void;
  disabled?: boolean;
  /** Dica do modelo (ex.: vale a partir do próximo turno). */
  modelHint?: string;
  /** Aviso no fim do menu (ex.: custo de trocar o modelo de uma conversa com histórico). */
  warning?: string | null;
}

/**
 * Pílula única "provedor · modelo": abre um menu com o agente (provedor) e o
 * modelo. Os modelos vêm da CLI; enquanto ela não responde aparece
 * "Carregando modelos…", em erro a mensagem e "Tentar de novo", e sempre dá
 * para digitar o nome de um modelo. `/modelo` abre este menu
 * (`[data-model-pill]`).
 */
export function AgentModelPill(props: AgentModelPillProps) {
  const { providers, provider, onProvider, providerDisabled, providerHint, model, onModel, disabled, modelHint, warning } = props;
  const { models, loading, error, load } = useModels(provider);
  const [open, setOpen] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);
  const close = useCallback((focus: boolean) => {
    setOpen(false);
    if (focus) btn.current?.focus();
  }, []);

  const providerName = providers.find((p) => p.id === provider)?.name ?? provider;
  const info = models.find((m) => !m.default && m.value === model);
  const modelLabel = model ? (info ? info.label + (info.badge ? ` · ${info.badge}` : "") : model) : "padrão";
  // Sem modelo escolhido: qual modelo o padrão resolve, se a lista já disse.
  const resolved = model ? "" : defaultResolves(models);
  const pickModel = (m: string) => {
    if (m !== model) onModel(m);
    close(true);
  };
  const pickProvider = (id: string) => {
    if (id !== provider) onProvider(id);
  };

  return (
    <>
      <button
        ref={btn}
        type="button"
        className={`sel-btn sel-pill am-pill${open ? " open" : ""}`}
        data-model-pill=""
        aria-label={`Agente e modelo: ${providerName}, ${model || "padrão da CLI"}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        title={open ? undefined : `${providerName} · ${model || "Padrão da CLI"}\nClique para trocar o agente ou o modelo.`}
        disabled={disabled}
        onClick={() => (open ? close(true) : (setOpen(true), void load()))}
      >
        <Icon name="bot" size={12} />
        <span className="sel-label">
          <span className="am-prov">{providerName} · </span>
          <span className="am-model">
            {modelLabel}
            {resolved && <span className="am-model-resolved"> ({resolved})</span>}
          </span>
        </span>
        <svg className="sel-chev" width="10" height="10" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M6 9l6 6 6-6" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      <Popover anchor={btn} open={open} onClose={close} ariaLabel="Agente e modelo" width={340} className="am-pop">
        <div className="pop-sec">
          <div className="pop-title">Agente</div>
          {providerHint && <p className="pop-help">{providerHint}</p>}
          <div className="pop-list" role="radiogroup" aria-label="Agente">
            {providers.map((p) => (
              <button
                key={p.id}
                type="button"
                role="radio"
                aria-checked={p.id === provider}
                className={`sel-opt pop-opt${p.id === provider ? " cur" : ""}`}
                disabled={providerDisabled && p.id !== provider}
                onClick={() => pickProvider(p.id)}
              >
                <span className="sel-opt-label">{p.name}</span>
                {p.id === provider && <Check />}
              </button>
            ))}
          </div>
        </div>
        <ModelPicker
          models={models}
          loading={loading}
          error={error}
          model={model}
          modelHint={modelHint}
          onPick={pickModel}
          onRetry={() => void load(true)}
        />
        {warning && (
          <div className="sel-note sel-warn" role="note">
            {warning}
          </div>
        )}
      </Popover>
    </>
  );
}

/** Sem acentos e sem caixa, para a busca. */
function fold(s: string): string {
  return s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

/** A partir de quantos modelos a lista é agrupada por provedor. */
const GROUP_FROM = 12;

interface PickRow {
  key: string;
  /** Valor escolhido ("" = padrão da CLI). */
  value: string;
  label: string;
  hint: string;
  badge: string;
  group: string;
  /** Id técnico, só na dica do mouse. */
  id?: string;
  kind: "default" | "model" | "current" | "typed";
  invalid?: boolean;
}

interface ModelPickerProps {
  models: ModelInfo[];
  loading: boolean;
  error: string | null;
  model: string;
  modelHint?: string;
  onPick(value: string): void;
  onRetry(): void;
}

/**
 * Seção "Selecione um modelo": busca fixa no topo (foco ao abrir; filtra por
 * nome, id, descrição e provedor, sem acentos nem caixa), lista em duas linhas
 * (nome em destaque, descrição abaixo), agrupada por provedor quando é grande.
 * Setas, PageUp/PageDown e Enter navegam; Esc fecha (o Popover cuida). Quando
 * o digitado não é um modelo da lista, oferece usá-lo como nome.
 */
function ModelPicker({ models, loading, error, model, modelHint, onPick, onRetry }: ModelPickerProps) {
  const [typed, setTyped] = useState("");
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  const typedName = typed.trim();
  const typedOk = isValidModelName(typedName);
  const searching = typedName.length > 0;

  const rows = useMemo<PickRow[]>(() => {
    const words = fold(typedName).split(/\s+/).filter(Boolean);
    const matches = (text: string) => words.every((w) => fold(text).includes(w));
    const def = models.find((m) => m.default);
    const list = models.filter((m) => !m.default);
    const out: PickRow[] = [];
    const defRow: PickRow = {
      key: "__default",
      value: "",
      label: def?.label ?? "Padrão da CLI",
      hint: def?.hint ?? "o padrão da CLI — usado quando nenhum é escolhido",
      badge: "",
      group: "",
      kind: "default",
    };
    if (matches(`${defRow.label} padrao default ${defRow.hint}`)) out.push(defRow);
    const grouped = list.length >= GROUP_FROM;
    for (const m of list) {
      if (!matches(`${m.label} ${m.value} ${m.hint} ${m.group} ${m.badge}`)) continue;
      out.push({
        key: m.value,
        value: m.value,
        label: m.label,
        // Agrupado, o provedor já está no cabeçalho do grupo.
        hint: grouped && m.hint === m.group ? "" : m.hint,
        badge: m.badge,
        group: grouped ? m.group : "",
        id: m.value,
        kind: "model",
      });
    }
    // O modelo em uso aparece mesmo fora da lista da CLI.
    if (!words.length && model && !list.some((m) => m.value === model)) {
      out.push({ key: "__current", value: model, label: model, hint: "em uso (fora da lista da CLI)", badge: "", group: "", kind: "current" });
    }
    if (typedName && !list.some((m) => m.value.toLowerCase() === typedName.toLowerCase())) {
      out.push({
        key: "__typed",
        value: typedName,
        label: `Usar “${typedName}”`,
        hint: typedOk ? "nome digitado, passado à CLI como está" : "nome inválido: sem espaços e sem começar com hífen",
        badge: "",
        group: "",
        kind: "typed",
        invalid: !typedOk,
      });
    }
    return out;
  }, [models, model, typedName, typedOk]);

  // Ao abrir ou filtrar: destaca o escolhido (sem busca) ou o primeiro resultado.
  useEffect(() => {
    const cur = rows.findIndex((r) => r.kind !== "typed" && r.value === model);
    setActive(searching ? 0 : Math.max(0, cur));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [typedName, models.length]);

  useEffect(() => {
    listRef.current?.querySelector(".am-opt.on")?.scrollIntoView({ block: "nearest" });
  }, [active, rows]);

  const choose = (r: PickRow | undefined) => {
    if (!r || r.invalid) return;
    onPick(r.value);
  };
  const optId = (i: number) => `${listId}-o${i}`;
  const at = Math.min(active, rows.length - 1);

  const onKey = (e: React.KeyboardEvent) => {
    const last = rows.length - 1;
    if (e.key === "ArrowDown") setActive((i) => (i >= last ? 0 : i + 1));
    else if (e.key === "ArrowUp") setActive((i) => (i <= 0 ? last : i - 1));
    else if (e.key === "PageDown") setActive((i) => Math.min(last, i + 8));
    else if (e.key === "PageUp") setActive((i) => Math.max(0, i - 8));
    else if (e.key === "Enter") choose(rows[at]);
    else return;
    e.preventDefault();
  };

  const items: React.ReactNode[] = [];
  let lastGroup = "";
  rows.forEach((r, i) => {
    if (r.group && r.group !== lastGroup) {
      items.push(
        <div key={`g-${r.group}`} className="am-group" role="presentation">
          {r.group}
        </div>,
      );
    }
    lastGroup = r.group;
    const selected = r.kind !== "typed" && r.value === model;
    items.push(
      <div
        key={r.key}
        id={optId(i)}
        role="option"
        aria-selected={selected}
        aria-disabled={r.invalid || undefined}
        title={r.id && r.id !== r.label ? `id: ${r.id}` : undefined}
        className={`sel-opt am-opt${i === at ? " on" : ""}${selected ? " cur" : ""}${r.invalid ? " off" : ""}`}
        onMouseEnter={() => setActive(i)}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => choose(r)}
      >
        <span className="am-opt-text">
          <span className="am-opt-name">
            <span className="am-opt-label">{r.label}</span>
            {r.badge && <span className="am-badge">{r.badge}</span>}
          </span>
          {r.hint && <span className="am-opt-hint">{r.hint}</span>}
        </span>
        {selected && <Check />}
      </div>,
    );
  });

  return (
    <div className="pop-sec am-models">
      <div className="pop-title">Selecione um modelo</div>
      {modelHint && <p className="pop-help">{modelHint}</p>}
      <input
        className="sel-search pop-input"
        placeholder="Buscar ou digitar um modelo…"
        aria-label="Buscar ou digitar o nome de um modelo"
        role="combobox"
        aria-expanded
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={rows.length ? optId(at) : undefined}
        value={typed}
        data-autofocus=""
        spellCheck={false}
        onChange={(e) => setTyped(e.target.value)}
        onKeyDown={onKey}
      />
      <div ref={listRef} id={listId} className="am-list" role="listbox" aria-label="Modelos">
        {items}
        {!rows.length && !loading && <div className="sel-empty">Nenhum modelo encontrado.</div>}
        {loading && (
          <div className="pop-status" role="status">
            Carregando modelos…
          </div>
        )}
        {!loading && error && (
          <div className="pop-status pop-err" role="alert">
            <span>Não foi possível listar os modelos: {error}</span>
            <button type="button" className="pop-link" onClick={onRetry}>
              Tentar de novo
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function Check() {
  return (
    <svg className="sel-check" width="12" height="12" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M5 12l5 5 9-10" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

// ------------------------------------------------------------- modo de trabalho

export const WORK_MODE_LABEL: Record<WorkMode, string> = {
  Isolated: "Isolado (cópia do projeto)",
  Direct: "Direto na pasta",
};

export const WORK_MODE_HELP: Record<WorkMode, string> = {
  Isolated:
    "Isolado: o agente trabalha numa cópia do projeto (git worktree) e você revisa antes de aplicar. A cópia só tem o que está commitado: pastas fora do git, arquivos ignorados (.env) e alterações não commitadas não aparecem para ele.",
  Direct:
    "Direto na pasta: o agente trabalha na própria pasta do projeto e enxerga tudo. As alterações valem na hora, sem etapa de aplicar ou descartar.",
};

/** Modo de trabalho padrão do projeto (preferência geral e do projeto), relido quando `rev` muda. */
export function useWorkModeInfo(project: Path | null, rev: unknown): WorkModeInfo | null {
  const [info, setInfo] = useState<WorkModeInfo | null>(null);
  useEffect(() => {
    setInfo(null);
    if (!project) return;
    let alive = true;
    api.workmodeGet(project).then(
      (i) => alive && setInfo(i),
      () => undefined,
    );
    return () => {
      alive = false;
    };
  }, [project, rev]);
  return info;
}

// ------------------------------------------------------------ opções da conversa

export interface ComposerOptionsProps {
  project: Path | null;
  taskId: TaskId | null;
  /** Modo de trabalho em vigor (o da tarefa, ou o escolhido para a conversa nova). */
  mode: WorkMode;
  /** Padrão do projeto para conversas novas (para marcar o que foge dele). */
  defaultMode: WorkMode;
  /** Conversa nova num repositório git: o modo pode ser escolhido. Fora disso, é fixo. */
  modeEditable: boolean;
  /** Projeto sem git (só existe "Direto na pasta"). */
  noGit?: boolean;
  onMode(mode: WorkMode): void;
  /** Modo de acesso da conversa (só com tarefa criada). */
  access?: AccessMode;
  onAccess?: (a: AccessMode) => void;
}

/**
 * Botão de opções da conversa (ícone de ajustes): modo de trabalho, modo de
 * permissão e auto-work, cada um com rótulo e explicação. O botão mostra
 * quantas opções fogem do padrão.
 */
/** Rótulos e explicações do modo de acesso da conversa. */
export const ACCESS_LABEL: Record<AccessMode, string> = { plan: "Planejar", normal: "Normal", bypass: "Bypass" };
/** Uma linha no menu; o texto completo (`ACCESS_HELP`) fica no tooltip. */
const ACCESS_SHORT: Record<AccessMode, string> = {
  plan: "Só lê e propõe; no fim, \"Executar o plano\".",
  normal: "Edita sem perguntar; shell só com comandos permitidos.",
  bypass: "Qualquer comando, sem aprovação. Só nesta conversa.",
};

/** Uma linha no menu; o texto completo (`WORK_MODE_HELP`) fica no tooltip. */
const WORK_MODE_SHORT: Record<WorkMode, string> = {
  Isolated: "Numa cópia (worktree); você revisa e aplica.",
  Direct: "Na própria pasta; vale na hora.",
};

export function ComposerOptions({ project, taskId, mode, defaultMode, modeEditable, noGit, onMode, access = "normal", onAccess }: ComposerOptionsProps) {
  const [open, setOpen] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);
  const aw = useAutoWork(project, taskId);
  const close = useCallback((focus: boolean) => {
    setOpen(false);
    if (focus) btn.current?.focus();
  }, []);

  const offMode = taskId == null && mode !== defaultMode;
  const changed = (offMode ? 1 : 0) + (aw.on ? 1 : 0) + (access !== "normal" ? 1 : 0);
  const summary = [
    mode === "Isolated" ? "isolado" : "direto na pasta",
    access === "normal" ? "edições automáticas" : `acesso ${ACCESS_LABEL[access]}`,
    aw.on ? `auto-work${aw.progress || " ligado"}` : null,
  ]
    .filter(Boolean)
    .join(", ");

  return (
    <>
      <button
        ref={btn}
        type="button"
        className={`chat-ibtn opt-btn${changed ? " on" : ""}${open ? " open" : ""}`}
        aria-label={`Opções da conversa: ${summary}${changed ? ` (${changed} fora do padrão)` : ""}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        title={open ? undefined : `Opções da conversa: ${summary}`}
        onClick={() => (open ? close(true) : setOpen(true))}
      >
        <Icon name="sliders" size={15} />
        {changed > 0 && <span className="opt-count">{changed}</span>}
      </button>
      <Popover anchor={btn} open={open} onClose={close} ariaLabel="Opções da conversa" width={360} className="opt-pop">
        {/* Sem projeto, modo de trabalho e auto-work não se aplicam: some a seção, fica uma dica. */}
        {!project && <p className="pop-help pop-note">Escolha o projeto na pílula ao lado para definir o modo de trabalho e o auto-work.</p>}
        {project && (
        <div className="pop-sec">
          <div className="pop-title">Modo de trabalho</div>
          {modeEditable ? (
            <div className="pop-list" role="radiogroup" aria-label="Modo de trabalho">
              {(["Isolated", "Direct"] as WorkMode[]).map((m) => (
                <button
                  key={m}
                  type="button"
                  role="radio"
                  aria-checked={m === mode}
                  className={`sel-opt pop-opt pop-opt-tall${m === mode ? " cur" : ""}`}
                  onClick={() => onMode(m)}
                >
                  <span className="pop-opt-text">
                    <span className="sel-opt-label">
                      {WORK_MODE_LABEL[m]}
                      {m === defaultMode && <span className="pop-tag">padrão</span>}
                    </span>
                    <span className="pop-help">{WORK_MODE_SHORT[m]}</span>
                  </span>
                  {m === mode && <Check />}
                </button>
              ))}
            </div>
          ) : (
            <p className="pop-help" title={WORK_MODE_HELP[mode]}>
              <b>{WORK_MODE_LABEL[mode]}</b>
              {" · "}
              {noGit ? "o projeto não é um repositório git" : "fixo nesta conversa"}
            </p>
          )}
        </div>
        )}
        <div className="pop-sec">
          <div className="pop-title">Acesso nesta conversa</div>
          {onAccess ? (
            <div className="pop-list" role="radiogroup" aria-label="Acesso nesta conversa">
              {(["plan", "normal", "bypass"] as AccessMode[]).map((a) => (
                <button
                  key={a}
                  type="button"
                  role="radio"
                  aria-checked={a === access}
                  className={`sel-opt pop-opt pop-opt-tall${a === access ? " cur" : ""}`}
                  onClick={() => {
                    if (a !== access) onAccess(a);
                  }}
                >
                  <span className="pop-opt-text">
                    <span className="sel-opt-label">
                      {ACCESS_LABEL[a]}
                      {a === "normal" && <span className="pop-tag">padrão</span>}
                      {a === "bypass" && <span className="pop-tag bypass-tag">perigoso</span>}
                    </span>
                    <span className="pop-help">{ACCESS_SHORT[a]}</span>
                  </span>
                  {a === access && <Check />}
                </button>
              ))}
            </div>
          ) : (
            <p className="pop-help">Conversa encerrada.</p>
          )}
          {/* O antigo "Modo de permissão" (só leitura) é o que vale no Normal: vira este atalho. */}
          <button
            type="button"
            className="pop-link"
            onClick={() => {
              close(false);
              useWorkbench.getState().openPage("settings");
            }}
          >
            Comandos permitidos
          </button>
        </div>
        {project && (
        <div className="pop-sec">
          <div className="pop-title">Auto-work</div>
          {(
            <>
              <div className="pop-row">
                <label
                  className="pop-switch"
                  title={`Ao fim de cada turno, o Argus verifica as pendências e dispara novos turnos sozinho, dentro dos limites.${aw.detail ? ` ${aw.detail}` : ""}`}
                >
                  <input type="checkbox" checked={aw.on} disabled={!aw.ready || aw.busy} onChange={() => void aw.toggle()} />
                  <span>{aw.on ? `Ligado neste projeto${aw.progress ? ` (${aw.progress.trim()})` : ""}` : "Desligado neste projeto"}</span>
                </label>
                <button
                type="button"
                className="pop-link"
                onClick={() => {
                  close(false);
                  useWorkbench.getState().openPage("settings");
                }}
              >
                  Limites
                </button>
              </div>
            </>
          )}
        </div>
        )}
      </Popover>
    </>
  );
}

// ------------------------------------------------------------ avisos de envio

/** O relatório tem algo que a cópia isolada não teria? */
export function hasGaps(r: IsolationReport): boolean {
  return r.untracked_total > 0 || r.ignored_total > 0 || r.modified_total > 0 || r.mentioned.length > 0;
}

function Group({ title, items, total }: { title: string; items: string[]; total: number }) {
  if (total === 0) return null;
  const MAX = 6;
  const shown = items.slice(0, MAX);
  return (
    <div className="cv-hold-group">
      <span className="cv-hold-label">
        {title} ({total})
      </span>
      <span className="cv-hold-items">
        {shown.map((p) => (
          <code key={p}>{p}</code>
        ))}
        {total > shown.length && <span className="cv-hold-more">e mais {total - shown.length}</span>}
      </span>
    </div>
  );
}

/** Aviso antes do envio no modo isolado: o que o agente não vai ver, com as duas saídas. */
export function IsolationHold(props: {
  report: IsolationReport;
  busy: boolean;
  onDirect(): void;
  onIsolated(): void;
  onCancel(): void;
}) {
  const { report, busy, onDirect, onIsolated, onCancel } = props;
  return (
    <div className="cv-hold" role="alertdialog" aria-label="O agente não vai ver tudo no modo isolado">
      <div className="cv-hold-head">
        <Icon name="alert" size={15} />
        <b>No modo Isolado o agente não vai ver parte do projeto</b>
        <button type="button" className="cv-hold-x" title="Cancelar e voltar a editar o pedido" aria-label="Cancelar" onClick={onCancel}>
          <Icon name="close" size={13} />
        </button>
      </div>
      <p>
        A cópia isolada só tem o que está commitado no git. O pedido ainda <b>não foi enviado</b>.
      </p>
      {report.mentioned.length > 0 && (
        <div className="cv-hold-cited">
          <span>
            {report.mentioned.length === 1 ? "O pedido cita um caminho que não existirá" : "O pedido cita caminhos que não existirão"} na
            cópia:
          </span>
          {report.mentioned.map((p) => (
            <code key={p}>{p}</code>
          ))}
        </div>
      )}
      <Group title="Fora do git" items={report.untracked} total={report.untracked_total} />
      <Group title="Ignorados pelo git" items={report.ignored} total={report.ignored_total} />
      <Group title="Alterações não commitadas" items={report.modified} total={report.modified_total} />
      <div className="cv-hold-actions">
        <button type="button" className="primary" disabled={busy} title={WORK_MODE_HELP.Direct} onClick={onDirect}>
          Usar Direto na pasta
        </button>
        <button type="button" disabled={busy} title="Envia assim mesmo: o agente trabalha só com o que está commitado" onClick={onIsolated}>
          Continuar isolado
        </button>
      </div>
    </div>
  );
}

/** Aviso antes de iniciar uma segunda sessão direto na mesma pasta. */
export function DirectHold(props: { others: TaskId[]; busy: boolean; onGo(): void; onCancel(): void }) {
  const { others, busy, onGo, onCancel } = props;
  const list = others.map((id) => `#${id}`).join(", ");
  return (
    <div className="cv-hold" role="alertdialog" aria-label="Já há uma sessão direto nesta pasta">
      <div className="cv-hold-head">
        <Icon name="alert" size={15} />
        <b>Já há outra sessão trabalhando direto nesta pasta</b>
        <button type="button" className="cv-hold-x" title="Cancelar e voltar a editar o pedido" aria-label="Cancelar" onClick={onCancel}>
          <Icon name="close" size={13} />
        </button>
      </div>
      <p>
        {others.length === 1 ? `A sessão ${list} está` : `As sessões ${list} estão`} rodando em "Direto na pasta" neste projeto. Dois
        agentes na mesma pasta podem editar o mesmo arquivo ao mesmo tempo. O pedido ainda <b>não foi enviado</b>.
      </p>
      <div className="cv-hold-actions">
        <button type="button" className="primary" disabled={busy} onClick={onGo}>
          Iniciar mesmo assim
        </button>
        <button type="button" disabled={busy} onClick={onCancel}>
          Cancelar
        </button>
      </div>
    </div>
  );
}

/** Selo vermelho "Bypass": a conversa roda sem pedir aprovação (cabeçalho, aba e barra de status). */
export function BypassBadge({ compact = false }: { compact?: boolean }) {
  return (
    <span
      className={`bypass-badge${compact ? " compact" : ""}`}
      title="Bypass ligado nesta conversa: o agente roda sem pedir aprovação e sem a política de shell do Argus."
    >
      Bypass
    </span>
  );
}
