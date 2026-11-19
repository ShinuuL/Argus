/**
 * Configurações › Subagentes: lista e edita os agentes gravados em arquivo
 * (`.claude/agents/<nome>.md`), no escopo global ou do projeto.
 *
 * O formato é o que as CLIs já leem, então o que é editado aqui vale para
 * Claude, Codex e companhia sem nenhuma conversão. Esta tela é a parte de
 * leitura/escrita do editor visual de fluxo, que vem depois e grava os mesmos
 * arquivos.
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, errorMessage, type AgentDef, type AgentScope, type Path } from "@/api";
import { Select } from "@/shell/Select";
import { useApp } from "@/store";
import { Chip } from "@/ui";
import { PageBody } from "./PageBody";

/** Modelos que as CLIs aceitam no frontmatter. "" = o que a CLI decidir. */
const MODELS: { value: string; label: string }[] = [
  { value: "", label: "Padrão da CLI" },
  { value: "inherit", label: "Herdar da conversa" },
  { value: "opus", label: "Opus" },
  { value: "sonnet", label: "Sonnet" },
  { value: "haiku", label: "Haiku" },
];

const SCOPES: { value: AgentScope; label: string }[] = [
  { value: "project", label: "Este projeto" },
  { value: "global", label: "Global" },
];

function emptyDef(scope: AgentScope): AgentDef {
  return {
    name: "",
    description: "",
    model: null,
    tools: [],
    color: null,
    body: "",
    scope,
    path: "",
    shadowed: false,
    read_only: false,
    extra: {},
  };
}

const scopeLabel = (s: AgentScope) => (s === "global" ? "Global" : "Projeto");

export default function AgentDefsPage() {
  const project = useApp((s) => s.selected);
  const [list, setList] = useState<AgentDef[] | null>(null);
  const [editing, setEditing] = useState<AgentDef | null>(null);
  /** Nome do agente aberto, para saber se a gravação renomeia (e apaga o antigo). */
  const [original, setOriginal] = useState<{ name: string; scope: AgentScope } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api.agentDefsList(project).then(
      (l) => {
        setList(l);
        setError(null);
      },
      (e) => setError(errorMessage(e)),
    );
  }, [project]);

  useEffect(load, [load]);

  const open = (def: AgentDef) => {
    setEditing({ ...def, tools: [...def.tools] });
    setOriginal({ name: def.name, scope: def.scope });
    setError(null);
  };

  const novo = () => {
    setEditing(emptyDef(project ? "project" : "global"));
    setOriginal(null);
    setError(null);
  };

  const save = async () => {
    if (!editing) return;
    setBusy(true);
    try {
      await api.agentDefSave(editing, project);
      // Renomeou ou mudou de escopo: o arquivo antigo não serve mais.
      if (original && (original.name !== editing.name || original.scope !== editing.scope)) {
        await api.agentDefDelete(original.name, original.scope, project).catch(() => undefined);
      }
      setEditing(null);
      setOriginal(null);
      load();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (def: AgentDef) => {
    const ok = await api.systemConfirm("Apagar agente", `Apagar ${def.name} (${scopeLabel(def.scope)})? O arquivo ${def.path} será removido.`);
    if (!ok) return;
    try {
      await api.agentDefDelete(def.name, def.scope, project);
      if (editing?.name === def.name) setEditing(null);
      load();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const lead = project
    ? "Agentes que as CLIs leem de .claude/agents. O do projeto tem prioridade sobre o global de mesmo nome."
    : "Escolha um projeto para ver e gravar agentes do projeto. Sem projeto, só o escopo global.";

  return (
    <PageBody
      title="Subagentes"
      lead={lead}
      right={
        <button className="primary" onClick={novo}>
          Novo agente
        </button>
      }
    >
      {error && <div className="err small">{error}</div>}

      {editing ? (
        <AgentForm
          def={editing}
          project={project}
          busy={busy}
          onChange={setEditing}
          onSave={() => void save()}
          onCancel={() => {
            setEditing(null);
            setOriginal(null);
          }}
        />
      ) : list == null ? (
        <p className="ui-mut ui-sm">Lendo os agentes…</p>
      ) : list.length === 0 ? (
        <p className="ui-mut ui-sm">
          Nenhum agente ainda. "Novo agente" grava um arquivo .md com frontmatter que qualquer CLI lê.
        </p>
      ) : (
        <table className="ui-t sa-fixed">
          <thead>
            <tr>
              <th style={{ width: 180 }}>Nome</th>
              <th>Quando é acionado</th>
              <th style={{ width: 110 }}>Modelo</th>
              <th style={{ width: 110 }}>Escopo</th>
              <th style={{ width: 150 }} />
            </tr>
          </thead>
          <tbody>
            {list.map((d) => (
              <tr key={`${d.scope}-${d.name}`} className={d.shadowed ? "ui-dim" : undefined}>
                <td>
                  <b>{d.name}</b>
                </td>
                <td title={d.description}>
                  <span className="ellipsis">{d.description || <span className="ui-mut">(sem descrição)</span>}</span>
                </td>
                <td>{d.model === "inherit" ? "herda" : d.model ?? <span className="ui-mut">padrão</span>}</td>
                <td>
                  <Chip tone={d.scope === "project" ? "ok" : undefined}>{scopeLabel(d.scope)}</Chip>
                  {d.shadowed && (
                    <span className="ui-mut ui-sm" title="Existe um agente de mesmo nome no projeto; é ele que a CLI usa.">
                      {" "}
                      substituído
                    </span>
                  )}
                </td>
                <td>
                  {d.read_only ? (
                    <span className="ui-mut ui-sm" title={`O frontmatter usa YAML que o Argus não regrava sem perder conteúdo, ou o name difere do arquivo. Edite direto: ${d.path}`}>
                      só leitura
                    </span>
                  ) : (
                    <>
                      <button onClick={() => open(d)}>Editar</button>{" "}
                      <button onClick={() => void remove(d)}>Apagar</button>
                    </>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </PageBody>
  );
}

function AgentForm({
  def,
  project,
  busy,
  onChange,
  onSave,
  onCancel,
}: {
  def: AgentDef;
  project: Path | null;
  busy: boolean;
  onChange(def: AgentDef): void;
  onSave(): void;
  onCancel(): void;
}) {
  const set = <K extends keyof AgentDef>(k: K, v: AgentDef[K]) => onChange({ ...def, [k]: v });
  const nameOk = /^[a-z0-9_-]+$/.test(def.name) && !def.name.startsWith("-");
  const canSave = nameOk && def.description.trim().length > 0 && !busy && (def.scope === "global" || project != null);
  const preview = useMemo(() => frontPreview(def), [def]);

  return (
    <div className="sa-form">
      <label className="sa-field">
        <span className="ui-sm">Nome</span>
        <input
          value={def.name}
          placeholder="revisor-fiscal"
          onChange={(e) => set("name", e.target.value)}
          onBlur={(e) => {
            // Título digitado com espaços/acentos vira slug ao sair do campo.
            const v = e.target.value;
            if (v && !/^[a-z0-9_-]+$/.test(v)) void api.agentDefSlug(v).then((s) => s && set("name", s));
          }}
        />
        <span className="ui-mut ui-sm">
          {nameOk || !def.name
            ? "Vira o nome do arquivo: .claude/agents/<nome>.md"
            : "Só letras minúsculas, números e hífen (sai do campo e o Argus ajusta)."}
        </span>
      </label>

      <label className="sa-field">
        <span className="ui-sm">Quando acionar (description)</span>
        <textarea
          rows={2}
          value={def.description}
          placeholder="Revisa emissão de NF-e e CF-e. Use quando a mudança tocar impostos, SEFAZ ou TEF."
          onChange={(e) => set("description", e.target.value)}
        />
        <span className="ui-mut ui-sm">
          É por esta frase que a CLI decide chamar o agente. Diga o que ele faz e em que situação usá-lo.
        </span>
      </label>

      <div className="sa-row">
        <label className="sa-field">
          <span className="ui-sm">Modelo</span>
          <Select
            value={def.model ?? ""}
            options={MODELS}
            ariaLabel="Modelo do agente"
            onChange={(v) => set("model", v ? v : null)}
          />
        </label>
        <label className="sa-field">
          <span className="ui-sm">Escopo</span>
          <Select
            value={def.scope}
            // Sem projeto aberto só dá para gravar no global.
            options={SCOPES.filter((s) => s.value === "global" || project != null)}
            ariaLabel="Onde gravar o agente"
            onChange={(v) => set("scope", v as AgentScope)}
          />
          <span className="ui-mut ui-sm">
            {def.scope === "global" ? "~/.claude/agents — vale em qualquer pasta." : "No projeto, versionado com o git."}
          </span>
        </label>
      </div>

      <label className="sa-field">
        <span className="ui-sm">Ferramentas (vazio = todas)</span>
        <input
          value={def.tools.join(", ")}
          placeholder="Read, Grep, Glob, Bash"
          onChange={(e) => set("tools", e.target.value.split(",").map((t) => t.trim()).filter(Boolean))}
        />
      </label>

      <label className="sa-field">
        <span className="ui-sm">Prompt do agente</span>
        <textarea
          rows={10}
          className="ui-mono"
          value={def.body}
          placeholder="Você revisa alterações fiscais do ERP…"
          onChange={(e) => set("body", e.target.value)}
        />
      </label>

      <details className="sa-field">
        <summary className="ui-sm">Como o arquivo vai ficar</summary>
        <pre className="ui-mono ui-sm">{preview}</pre>
      </details>

      <div className="sa-row">
        <button className="primary" disabled={!canSave} onClick={onSave}>
          {busy ? "Gravando…" : "Gravar"}
        </button>
        <button onClick={onCancel}>Cancelar</button>
      </div>
    </div>
  );
}

/** Prévia do arquivo. Espelha `render()` do core; serve só para o usuário conferir. */
function frontPreview(def: AgentDef): string {
  const lines = ["---", `name: ${def.name || "<nome>"}`, `description: ${def.description || "<quando acionar>"}`];
  if (def.model) lines.push(`model: ${def.model}`);
  if (def.tools.length) lines.push(`tools: ${def.tools.join(", ")}`);
  for (const [k, v] of Object.entries(def.extra)) lines.push(`${k}: ${v}`);
  lines.push("---", "", def.body || "<prompt do agente>");
  return lines.join("\n");
}
