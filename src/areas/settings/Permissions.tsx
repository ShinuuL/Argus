/**
 * Permissões dos agentes como tabela de ações (tela 12). A política unificada ainda
 * não existe no backend: as linhas mapeiam o que existe hoje (modo Auto/Restrito,
 * git local/remoto, MCP do celular); o resto aparece desabilitado com "em breve".
 */
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, errorMessage, samePath, type Path, type PermissionMode, type PermissionsInfo, type WorkMode, type WorkModeInfo } from "@/api";
import { useApp } from "@/store";
import { Btn, Chip, POLICY_OPTIONS, Seg, type PolicyValue, type SegOption, AreaStatus } from "@/ui";
import { AllowedCommands } from "@/views/servers";
import { Select } from "@/shell/Select";
import { PageBody } from "./PageBody";

type Row = {
  id: string;
  label: string;
  /** null = sem suporte no backend (linha desabilitada). */
  value: PolicyValue | null;
  options?: SegOption<PolicyValue>[];
  onChange?: (v: PolicyValue) => void;
  /** Valor herdado do padrão (sem escolha própria neste escopo). */
  inherited?: boolean;
  onReset?: () => void;
  note: ReactNode;
};

const TWO = POLICY_OPTIONS.filter((o) => o.value !== "ask");

function useData(project: Path | null) {
  const [perms, setPerms] = useState<PermissionsInfo | null>(null);
  const [mode, setMode] = useState<WorkModeInfo | null>(null);
  const [mcp, setMcp] = useState<boolean | null>(null);
  const [allowed, setAllowed] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      const [p, m] = await Promise.all([api.permissionsGet(project), api.workmodeGet(project)]);
      setPerms(p);
      setMode(m);
      setAllowed(project ? await api.allowedCommandsGet(project) : []);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
    api.devicesStatus().then((s) => setMcp(s.agentsMcp), () => setMcp(null));
  }, [project]);
  useEffect(() => {
    void load();
  }, [load]);
  const run = async (...actions: Promise<unknown>[]) => {
    try {
      await Promise.all(actions);
    } catch (e) {
      setError(errorMessage(e));
    }
    await load();
  };
  return { perms, mode, mcp, allowed, error, run };
}

const MODES: { id: string; label: string; hint: string; mode: PermissionMode; local: boolean; remote: boolean }[] = [
  { id: "leitura", label: "Leitura", hint: "Restrito, sem git local nem remoto", mode: "restricted", local: false, remote: false },
  { id: "interativo", label: "Interativo", hint: "Auto, git local ligado, sem push", mode: "auto", local: true, remote: false },
  { id: "total", label: "Total", hint: "Auto, git local e remoto ligados", mode: "auto", local: true, remote: true },
];

export default function PermissionsPage() {
  const projects = useApp((s) => s.projects);
  const selected = useApp((s) => s.selected);
  const [scope, setScope] = useState<Path | "global">(() => selected ?? "global");
  const projectObj = scope === "global" ? null : (projects.find((p) => samePath(p.path, scope)) ?? null);
  const project = projectObj?.path ?? null;
  const { perms, mode, mcp, allowed, error, run } = useData(project);
  const [editCmds, setEditCmds] = useState(false);

  // Tabs: padrão global + projeto selecionado + mais alguns (o mockup mostra poucos).
  const tabProjects = projects.slice(0, 4);
  if (projectObj && !tabProjects.some((p) => samePath(p.path, projectObj.path))) tabProjects.push(projectObj);

  const own = <T,>(proj: T | null | undefined, glob: T | null | undefined) => (project ? (proj ?? glob) : glob);
  const permMode: PermissionMode = own(perms?.project_mode, perms?.global_mode) ?? "auto";
  const workMode: WorkMode = (project ? mode?.project : null) ?? mode?.global ?? "Isolated";
  const localSet = own(perms?.project_local, perms?.global_local);
  const remoteSet = own(perms?.project_remote, perms?.global_remote);
  const buildSet = own(perms?.project_build, perms?.global_build);
  const localOn = localSet ?? workMode === "Isolated";
  const remoteOn = remoteSet ?? false;
  const buildOn = buildSet ?? true;

  const setOnOff = (level: "local" | "remote" | "build") => (v: PolicyValue) => void run(api.permissionsSet(project, level, v === "allow"));
  const inherit = (projVal: unknown) => (project ? projVal == null : false);

  const rows: Row[] = [
    { id: "read", label: "Ler arquivos do projeto", value: null, note: "Hoje o agente sempre pode ler." },
    { id: "edit", label: "Editar arquivos", value: null, note: workMode === "Isolated" ? "Modo Isolado: edita uma cópia e você aplica depois." : "Modo Direto: edita a pasta do projeto." },
    {
      id: "exec",
      label: "Executar comandos",
      value: perms ? (permMode === "auto" ? "allow" : "deny") : null,
      options: TWO,
      onChange: (v) => void run(api.permissionModeSet(project, v === "allow" ? "auto" : "restricted")),
      inherited: inherit(perms?.project_mode),
      onReset: () => void run(api.permissionModeSet(project, null)),
      note:
        permMode === "auto" ? (
          <span className="ui-mut">só os nocivos são bloqueados (formatar disco, push forçado, reset --hard…)</span>
        ) : (
          <span className="sa-chips">
            {(project ? allowed : (perms?.defaults.git_read ?? []).slice(0, 3)).map((c) => (
              <Chip key={c}>{c.replace(/:\*$/, " *")}</Chip>
            ))}
            {project ? (
              <button type="button" className="sa-link" onClick={() => setEditCmds((v) => !v)}>
                {allowed.length ? "editar" : "liberar comandos…"}
              </button>
            ) : (
              <span className="ui-mut">e a lista padrão de inspeção</span>
            )}
          </span>
        ),
    },
    {
      id: "build",
      label: "Compilar e testar (build, testes, tipos)",
      value: perms ? (buildOn ? "allow" : "deny") : null,
      options: TWO,
      onChange: setOnOff("build"),
      inherited: project ? perms?.project_build == null : perms?.global_build == null,
      onReset: () => void run(api.permissionsSet(project, "build", null)),
      note: (
        <span className="ui-mut">
          ligado por padrão: npm run, cargo test, npx tsc, pytest… · instalar dependência e publicar continuam de fora
        </span>
      ),
    },
    { id: "net", label: "Acessar a internet", value: null, note: "Sem regra própria ainda." },
    {
      id: "phone",
      label: "Controlar o celular",
      value: mcp == null ? null : mcp ? "allow" : "deny",
      options: TWO,
      onChange: (v) => void run(api.devicesSetAgents(v === "allow")),
      note: <span className="ui-mut">MCP do Screen-Mirror nas conversas{project ? " · vale para todos os projetos" : ""}</span>,
    },
    { id: "admin", label: "Firewall, serviços e processos de outros usuários", value: null, note: "Sempre bloqueado pelo modo Auto." },
    {
      id: "gitlocal",
      label: "Git local (commit, checkout, merge)",
      value: perms ? (localOn ? "allow" : "deny") : null,
      options: TWO,
      onChange: setOnOff("local"),
      inherited: project ? perms?.project_local == null : perms?.global_local == null,
      onReset: () => void run(api.permissionsSet(project, "local", null)),
      note: <span className="ui-mut">padrão: ligado no Isolado, desligado no Direto</span>,
    },
    {
      id: "push",
      label: "Git push e abrir PR",
      value: perms ? (remoteOn ? "allow" : "deny") : null,
      options: TWO,
      onChange: setOnOff("remote"),
      inherited: project ? perms?.project_remote == null : perms?.global_remote == null,
      onReset: () => void run(api.permissionsSet(project, "remote", null)),
      note: <span className="ui-mut">push forçado: sempre bloqueado</span>,
    },
  ];

  const applyMode = (m: (typeof MODES)[number]) =>
    void run(api.permissionModeSet(project, m.mode), api.permissionsSet(project, "local", m.local), api.permissionsSet(project, "remote", m.remote));
  const activeMode = MODES.find((m) => m.mode === permMode && m.local === localOn && m.remote === remoteOn)?.id;
  const restore = () =>
    void run(
      api.permissionModeSet(project, null),
      api.permissionsSet(project, "local", null),
      api.permissionsSet(project, "remote", null),
      api.permissionsSet(project, "build", null),
    );

  return (
    <>
      <div className="ui-area-main sa-main">
        <PageBody
          title="Permissões dos agentes"
          lead="O que um agente pode fazer sem te perguntar. Vale para Claude, Codex e o MCP do celular."
          right={
            <div className="sa-tabs" role="tablist" aria-label="Escopo das regras">
              <button type="button" role="tab" aria-selected={scope === "global"} className={scope === "global" ? "ui-tab on" : "ui-tab"} onClick={() => setScope("global")}>
                Padrão global
              </button>
              {tabProjects.map((p) => {
                const on = project != null && samePath(p.path, project);
                return (
                  <button key={p.path} type="button" role="tab" aria-selected={on} className={on ? "ui-tab on" : "ui-tab"} onClick={() => setScope(p.path)}>
                    {p.name}
                  </button>
                );
              })}
            </div>
          }
        >
          {error && (
            <div className="err small" role="alert">
              {error}
            </div>
          )}
          <table className="ui-t sa-perm">
            <thead>
              <tr>
                <th>Ação</th>
                <th style={{ width: 250 }}>Regra</th>
                <th>Exceções</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.id} className={r.value == null && !r.onChange ? "sa-off" : undefined}>
                  <td>{r.label}</td>
                  <td>
                    {r.onChange && r.value ? (
                      <span className="sa-rule">
                        <Seg label={r.label} value={r.value} options={r.options ?? POLICY_OPTIONS} onChange={r.onChange} />
                        {r.inherited ? (
                          <span className="ui-mut ui-sm" title={project ? "Segue o padrão global" : "Segue o padrão do Argus"}>
                            padrão
                          </span>
                        ) : (
                          r.onReset && (
                            <button type="button" className="sa-link" title={project ? "Voltar a seguir o padrão global" : "Voltar ao padrão do Argus"} onClick={r.onReset}>
                              {project ? "seguir global" : "padrão"}
                            </button>
                          )
                        )}
                      </span>
                    ) : r.onChange ? (
                      <span className="ui-mut ui-sm">carregando…</span>
                    ) : (
                      <span className="sa-rule" title="A política por tipo de ação ainda não existe no backend.">
                        <span className="sa-disabled">
                          <Seg label={r.label} value={"" as PolicyValue} options={POLICY_OPTIONS} />
                        </span>
                        <Chip>em breve</Chip>
                      </span>
                    )}
                  </td>
                  <td className="ui-sm">{r.note}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {project && editCmds && (
            <div className="ui-card sa-card">
              <AllowedCommands key={project} project={project} />
            </div>
          )}
          <div className="ui-card sa-card sa-modes">
            <span className="ui-sm sa-grow">
              Modos prontos:{" "}
              {MODES.map((m, i) => (
                <span key={m.id}>
                  {i > 0 && " · "}
                  <button type="button" className={activeMode === m.id ? "sa-mode on" : "sa-mode"} title={m.hint} onClick={() => applyMode(m)}>
                    {m.label}
                  </button>
                </span>
              ))}{" "}
              preenchem a tabela de uma vez. Depois você ajusta linha por linha.
            </span>
            <Btn variant="ghost" small onClick={restore}>
              {project ? "Seguir o global" : "Restaurar padrão"}
            </Btn>
          </div>
          <div className="ui-card sa-card sa-modes">
            <span className="ui-sm sa-grow">Modo de trabalho {project ? "deste projeto" : "padrão"}</span>
            <Select
              ariaLabel="Modo de trabalho"
              disabled={project != null && mode != null && !mode.is_git}
              value={(project ? (mode?.project ?? "") : (mode?.global ?? "Isolated")) as string}
              options={[
                ...(project ? [{ value: "", label: "Seguir o geral" }] : []),
                { value: "Isolated", label: "Isolado (cópia do projeto)", hint: "revisa e aplica depois" },
                { value: "Direct", label: "Direto na pasta", hint: "vale na hora" },
              ]}
              onChange={(v) => void run(api.workmodeSet(project, (v || null) as WorkMode | null))}
            />
          </div>
        </PageBody>
        <AreaStatus>
          <span>salvo automaticamente</span>
          <span className="sa-grow" />
          <span>{project ? projectObj?.name : "padrão global"}</span>
        </AreaStatus>
      </div>
      <aside className="sa-panel" aria-label="Últimas decisões">
        <div className="sa-panel-h">
          <b>Últimas decisões</b>
          <div className="ui-mut ui-sm">audit.log</div>
        </div>
        <div className="ui-empty ui-sm sa-panel-empty">
          O Argus ainda não grava as decisões de permissão dos agentes. Quando o registro existir, elas aparecem aqui e um clique cria uma regra a partir delas.
        </div>
      </aside>
    </>
  );
}
