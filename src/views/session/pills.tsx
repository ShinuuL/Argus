/**
 * Pílulas e avisos da caixa de composição que dependem da conversa: modelo,
 * modo de trabalho (isolado ou direto na pasta) e o aviso que segura o envio
 * quando a cópia isolada não teria o que o pedido precisa.
 */
import { useCallback, useEffect, useState } from "react";
import { api, errorMessage, type IsolationReport, type ModelInfo, type Path, type TaskId, type WorkMode, type WorkModeInfo } from "@/api";
import { Icon } from "@/shell/icons";
import { Select, type SelectOption } from "@/shell/Select";

// ---------------------------------------------------------------------- modelo

/** Lista de modelos por provedor, guardada enquanto o app está aberto. */
const modelsCache = new Map<string, ModelInfo[]>();

/** Modelos do provedor, carregados sob demanda (`load`), com cache em memória. */
export function useModels(provider: string) {
  const [models, setModels] = useState<ModelInfo[]>(() => modelsCache.get(provider) ?? []);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setModels(modelsCache.get(provider) ?? []);
    setError(null);
    setLoading(false);
  }, [provider]);

  const load = useCallback(async (): Promise<ModelInfo[]> => {
    const cached = modelsCache.get(provider);
    if (cached?.length) return cached;
    if (!provider) return [];
    setLoading(true);
    setError(null);
    try {
      const list = await api.agentsModels(provider);
      if (list.length) modelsCache.set(provider, list);
      setModels(list);
      return list;
    } catch (e) {
      setError(errorMessage(e));
      return [];
    } finally {
      setLoading(false);
    }
  }, [provider]);

  return { models, loading, error, load };
}

export interface ModelPillProps {
  provider: string;
  /** Modelo escolhido; "" = padrão da CLI. */
  value: string;
  onChange(model: string): void;
  disabled?: boolean;
  title?: string;
}

/** Pílula de modelo: "Padrão da CLI" mais os modelos que a CLI do provedor informa. */
export function ModelPill({ provider, value, onChange, disabled, title }: ModelPillProps) {
  const { models, loading, error, load } = useModels(provider);
  const options: SelectOption[] = [{ value: "", label: "Padrão da CLI", hint: "o modelo que a CLI escolher" }];
  for (const m of models) options.push({ value: m.value, label: m.label, hint: m.hint || undefined });
  // O modelo em uso aparece mesmo antes de a lista ser consultada.
  if (value && !models.some((m) => m.value === value)) options.push({ value, label: value });
  return (
    <Select
      variant="pill"
      ariaLabel="Modelo"
      title={title ?? "Modelo que o agente usa (vale a partir do próximo turno)"}
      icon={<Icon name="box" size={12} />}
      value={value}
      options={options}
      disabled={disabled}
      popWidth={340}
      onOpen={() => void load()}
      note={loading ? "Consultando os modelos na CLI…" : error ? `Não foi possível listar os modelos: ${error}` : null}
      onChange={onChange}
    />
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

/** Pílula do modo de trabalho da conversa nova. Sem git, só existe "Direto na pasta". */
export function ModePill(props: { value: WorkMode; isGit: boolean; onChange(mode: WorkMode): void }) {
  const { value, isGit, onChange } = props;
  const options: SelectOption[] = isGit
    ? [
        { value: "Isolated", label: WORK_MODE_LABEL.Isolated, hint: "revisa e aplica depois" },
        { value: "Direct", label: WORK_MODE_LABEL.Direct, hint: "vale na hora" },
      ]
    : [{ value: "Direct", label: WORK_MODE_LABEL.Direct, hint: "vale na hora" }];
  return (
    <Select
      variant="pill"
      ariaLabel="Modo de trabalho"
      title={isGit ? WORK_MODE_HELP[value] : "Este projeto não é um repositório git: só dá para trabalhar direto na pasta."}
      icon={<Icon name={value === "Isolated" ? "box" : "folderOpen"} size={12} />}
      value={value}
      options={options}
      popWidth={330}
      onChange={(v) => onChange(v as WorkMode)}
    />
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
