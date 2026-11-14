/**
 * Auto-work na interface: selo de progresso por sessão (pausar, retomar,
 * parar, reiniciar) e painel de configuração por projeto (liga/desliga e limites).
 */
import { useEffect, useState } from "react";
import {
  api,
  autoWorkStatusLabel,
  errorMessage,
  formatTokens,
  formatUsd,
  variantOf,
  type AutoWorkEvent,
  type AutoWorkSettings,
  type Path,
  type TaskId,
} from "@/api";
import { useApp, useWorkbench } from "@/store";
import { Icon } from "@/shell/icons";
import "./autowork.css";

export interface AutoWorkPanelProps {
  project: Path;
}

export interface AutoWorkBadgeProps {
  taskId: TaskId;
}

/** Resumo de um evento de auto-work (dica do selo). */
export function describeAutoWork(e: AutoWorkEvent): string {
  const item = e.current_item ? ` · item: ${e.current_item}` : "";
  return `auto-work ${autoWorkStatusLabel(e.status)} · iteração ${e.iteration}/${e.max_iterations} · ${e.done}/${e.total} itens${item}`;
}

function pill(e: AutoWorkEvent): { cls: string; label: string } {
  const { kind, data } = variantOf(e.status);
  switch (kind) {
    case "Running":
      return { cls: "run", label: `auto ${e.done}/${e.total} · it ${e.iteration}/${e.max_iterations}` };
    case "Paused":
      return { cls: "pause", label: `auto pausado ${e.done}/${e.total}` };
    case "Completed":
      return { cls: "done", label: `auto concluído ${e.done}/${e.total}` };
    case "Stopped":
      return { cls: "stop", label: `auto parado: ${String(data ?? "")}` };
    default:
      return { cls: "", label: `auto ${kind}` };
  }
}

type Action = "start" | "pause" | "resume" | "stop";

const ACTIONS: Record<Action, (task: TaskId) => Promise<boolean>> = {
  start: api.autoworkStart,
  pause: api.autoworkPause,
  resume: api.autoworkResume,
  stop: api.autoworkStop,
};

/** Progresso do auto-work de uma tarefa, com pausar/retomar/parar. Vazio se não houver execução. */
export function AutoWorkBadge({ taskId }: AutoWorkBadgeProps) {
  const ev = useApp((s) => s.autowork[taskId]);
  const [busy, setBusy] = useState(false);
  if (!ev) return null;

  const run = async (action: Action) => {
    setBusy(true);
    try {
      const accepted = await ACTIONS[action](taskId);
      if (!accepted && action === "start") {
        useApp.getState().setError("O auto-work não pôde ser iniciado: ligue-o no projeto e aguarde o turno terminar.");
      }
      // O progresso também chega pelo evento `orch`; isto cobre o caso de ele não mudar.
      const status = await api.autoworkStatus(taskId);
      if (status) useApp.setState((s) => ({ autowork: { ...s.autowork, [taskId]: status } }));
    } catch (e) {
      useApp.getState().setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const { cls, label } = pill(ev);
  const kind = variantOf(ev.status).kind;
  const tip = `${describeAutoWork(ev)}\ntokens: ${formatTokens(ev.tokens)} · ${formatUsd(ev.cost_usd)} equivalente em API`;
  const total = Math.max(ev.total, 1);

  return (
    <span className="aw-badge" onClick={(e) => e.stopPropagation()}>
      <span className={`aw-pill ${cls}`} title={tip}>
        {label}
      </span>
      {kind === "Running" && (
        <progress className="aw-progress" max={total} value={Math.min(ev.done, total)} aria-label="Itens concluídos" />
      )}
      {ev.blocked > 0 && (
        <span className="small warn" title="Itens que o agente não conseguiu resolver">
          {ev.blocked === 1 ? "1 travado" : `${ev.blocked} travados`}
        </span>
      )}
      {ev.current_item && (
        <span className="aw-item small muted" title={ev.current_item}>
          {ev.current_item}
        </span>
      )}
      <span className="aw-actions">
        {kind === "Running" && (
          <>
            <button disabled={busy} onClick={() => void run("pause")}>
              Pausar
            </button>
            <button disabled={busy} onClick={() => void run("stop")}>
              Parar
            </button>
          </>
        )}
        {kind === "Paused" && (
          <>
            <button disabled={busy} onClick={() => void run("resume")}>
              Retomar
            </button>
            <button disabled={busy} onClick={() => void run("stop")}>
              Parar
            </button>
          </>
        )}
        {kind !== "Running" && kind !== "Paused" && (
          <button disabled={busy} onClick={() => void run("start")}>
            Reiniciar auto-work
          </button>
        )}
      </span>
    </span>
  );
}

/** Avisa as outras telas (pílula do chat, aba Configurações) que o auto-work de um projeto mudou. */
const SETTINGS_EVENT = "argus-autowork-settings";
function announce(project: Path): void {
  window.dispatchEvent(new CustomEvent(SETTINGS_EVENT, { detail: project }));
}
function onSettingsChanged(project: Path, cb: () => void): () => void {
  const handler = (e: Event) => {
    if ((e as CustomEvent<Path>).detail === project) cb();
  };
  window.addEventListener(SETTINGS_EVENT, handler);
  return () => window.removeEventListener(SETTINGS_EVENT, handler);
}

export interface AutoWorkPillProps {
  project: Path | null;
  /** Sessão aberta, para mostrar o progresso quando o auto-work está rodando nela. */
  taskId?: TaskId | null;
}

/**
 * Pílula da caixa de composição: liga/desliga o auto-work do projeto e mostra o
 * progresso quando ativo. Os limites ficam na aba Configurações.
 */
export function AutoWorkPill({ project, taskId = null }: AutoWorkPillProps) {
  const ev = useApp((s) => (taskId != null ? s.autowork[taskId] : undefined));
  const [settings, setSettings] = useState<AutoWorkSettings | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setSettings(null);
    if (!project) return;
    let alive = true;
    const load = () =>
      api.autoworkGetSettings(project).then(
        (s) => alive && setSettings(s),
        () => undefined,
      );
    void load();
    const off = onSettingsChanged(project, () => void load());
    return () => {
      alive = false;
      off();
    };
  }, [project]);

  if (!project) return null;
  const on = settings?.enabled ?? false;
  const kind = ev ? variantOf(ev.status).kind : null;
  const progress = ev && (kind === "Running" || kind === "Paused") ? ` ${ev.done}/${ev.total}` : "";

  const toggle = async () => {
    if (!settings || busy) return;
    setBusy(true);
    try {
      setSettings(await api.autoworkSetSettings(project, { ...settings, enabled: !settings.enabled }));
      announce(project);
    } catch (e) {
      useApp.getState().setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <span className="chat-pill-group">
      <button
        type="button"
        className={on ? "chat-pill on" : "chat-pill"}
        aria-pressed={on}
        disabled={!settings || busy}
        title={
          (on
            ? "Auto-work ligado neste projeto: ao fim de cada turno, o Argus verifica as pendências e dispara novos turnos. Clique para desligar."
            : "Auto-work desligado neste projeto. Clique para ligar.") + (ev ? `\n${describeAutoWork(ev)}` : "")
        }
        onClick={() => void toggle()}
      >
        <Icon name="loop" size={12} />
        Auto-work{on ? (progress || " ligado") : ""}
      </button>
      <button
        type="button"
        className="chat-pill chat-pill-ic"
        title="Limites do auto-work (aba Configurações)"
        aria-label="Limites do auto-work"
        onClick={() => useWorkbench.getState().openPage("settings")}
      >
        <Icon name="gear" size={12} />
      </button>
    </span>
  );
}

/** Campo numérico que só grava ao sair ou no Enter; vazio = sem valor. */
function NumberField(props: {
  id: string;
  label: string;
  value: number | null;
  min: number;
  step?: number;
  placeholder?: string;
  disabled: boolean;
  onCommit(value: number | null): void;
}) {
  const { id, label, value, min, step, placeholder, disabled, onCommit } = props;
  const shown = value == null ? "" : String(value);
  const [text, setText] = useState(shown);
  useEffect(() => setText(shown), [shown]);

  const commit = () => {
    const t = text.trim();
    const n = t === "" ? null : Number(t.replace(",", "."));
    if (n !== null && (!Number.isFinite(n) || n < min)) {
      setText(shown);
      return;
    }
    if (n !== value) onCommit(n);
  };

  return (
    <>
      <label className="small" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        type="number"
        min={min}
        step={step}
        placeholder={placeholder}
        value={text}
        disabled={disabled}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
      />
    </>
  );
}

/** Auto-work de um projeto: liga/desliga e limites (iterações, tokens, custo equivalente em API). */
export default function AutoWorkPanel({ project }: AutoWorkPanelProps) {
  const [settings, setSettings] = useState<AutoWorkSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let alive = true;
    setSettings(null);
    setError(null);
    const load = () =>
      api.autoworkGetSettings(project).then(
        (s) => alive && setSettings(s),
        (e) => alive && setError(errorMessage(e)),
      );
    void load();
    const off = onSettingsChanged(project, () => void load());
    return () => {
      alive = false;
      off();
    };
  }, [project]);

  const save = async (patch: Partial<AutoWorkSettings>) => {
    if (!settings) return;
    const previous = settings;
    setSettings({ ...settings, ...patch });
    setSaving(true);
    try {
      setSettings(await api.autoworkSetSettings(project, { ...previous, ...patch }));
      announce(project);
      setError(null);
    } catch (e) {
      setSettings(previous);
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  if (!settings) {
    return <div className={error ? "small err" : "small muted"}>{error ?? "Carregando…"}</div>;
  }

  const idBase = `aw-${project.replace(/[^a-zA-Z0-9]+/g, "-")}`;
  return (
    <div className="aw-panel">
      <label className="row">
        <input
          type="checkbox"
          checked={settings.enabled}
          disabled={saving}
          onChange={(e) => void save({ enabled: e.target.checked })}
        />
        <b>Auto-work neste projeto</b>
      </label>
      <p className="small muted aw-help">
        Ao fim de cada turno, varre PENDENCIAS.md, TODO/FIXME e build/testes, e dispara novos turnos até tudo ficar [x].
      </p>
      <div className="aw-limits">
        <NumberField
          id={`${idBase}-it`}
          label="Máx. iterações"
          value={settings.max_iterations}
          min={1}
          disabled={saving}
          onCommit={(v) => v != null && void save({ max_iterations: Math.max(1, Math.round(v)) })}
        />
        <NumberField
          id={`${idBase}-tok`}
          label="Teto de tokens"
          value={settings.max_tokens}
          min={0}
          placeholder="sem teto"
          disabled={saving}
          onCommit={(v) => void save({ max_tokens: v == null ? null : Math.round(v) })}
        />
        <NumberField
          id={`${idBase}-usd`}
          label="Teto de custo (US$, equivalente em API)"
          value={settings.max_cost_usd}
          min={0}
          step={0.5}
          placeholder="sem teto"
          disabled={saving}
          onCommit={(v) => void save({ max_cost_usd: v })}
        />
      </div>
      <label className="row small">
        <input
          type="checkbox"
          checked={settings.run_checks}
          disabled={saving}
          onChange={(e) => void save({ run_checks: e.target.checked })}
        />
        Rodar build/testes na verificação (timeout {settings.check_timeout_secs}s)
      </label>
      {error && (
        <span className="small err" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}
