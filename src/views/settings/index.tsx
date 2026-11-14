/**
 * Configurações (aba): tema de cores, auto-work por projeto (liga/desliga e
 * limites) e os comandos que o agente pode executar em cada projeto.
 */
import { Select } from "@/shell/Select";
import { useCallback, useEffect, useState } from "react";
import { api, errorMessage, samePath, type Path, type PermissionsInfo, type WorkMode, type WorkModeInfo } from "@/api";
import { THEMES, useApp, useTheme } from "@/store";
import AutoWorkPanel from "@/views/autowork";
import { AllowedCommands } from "@/views/servers";
import "./settings.css";

function ThemePicker() {
  const theme = useTheme((s) => s.theme);
  const setTheme = useTheme((s) => s.setTheme);
  return (
    <div className="set-themes" role="radiogroup" aria-label="Tema de cores">
      {THEMES.map((t) => (
        <button
          key={t.id}
          role="radio"
          aria-checked={theme === t.id}
          className={theme === t.id ? "set-theme on" : "set-theme"}
          title={`Usar o tema ${t.label}`}
          onClick={() => setTheme(t.id)}
        >
          {/* A amostra usa os tokens do próprio tema (data-theme no elemento). */}
          <span className="set-swatch" data-theme={t.id} aria-hidden="true">
            <span className="set-sw-side" />
            <span className="set-sw-main">
              <span className="set-sw-line" />
              <span className="set-sw-line short" />
              <span className="set-sw-btn" />
            </span>
          </span>
          <span className="set-theme-name">
            <b>{t.label}</b>
            <span className="small">{t.hint}</span>
          </span>
        </button>
      ))}
    </div>
  );
}

const MODE_OPTIONS = [
  { value: "Isolated", label: "Isolado (cópia do projeto)", hint: "revisa e aplica depois" },
  { value: "Direct", label: "Direto na pasta", hint: "vale na hora" },
];

/** Carrega o modo de trabalho e as permissões (geral e do projeto) e devolve como gravar. */
function useAgentSettings(project: Path | null) {
  const [mode, setMode] = useState<WorkModeInfo | null>(null);
  const [perms, setPerms] = useState<PermissionsInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      const [m, p] = await Promise.all([api.workmodeGet(project), api.permissionsGet(project)]);
      setMode(m);
      setPerms(p);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [project]);
  useEffect(() => void load(), [load]);
  const save = async (action: Promise<void>) => {
    try {
      await action;
    } catch (e) {
      setError(errorMessage(e));
    }
    await load();
  };
  return { mode, perms, error, save };
}

const onOff = (v: boolean | null): string => (v == null ? "" : v ? "on" : "off");
const parseOnOff = (v: string): boolean | null => (v === "" ? null : v === "on");

function Patterns({ title, items }: { title: string; items: string[] }) {
  return (
    <div className="set-perm-row">
      <span className="set-perm-title">{title}</span>
      <span className="set-perm-list">
        {items.map((p) => (
          <code key={p}>{p.replace(/:\*$/, " …")}</code>
        ))}
      </span>
    </div>
  );
}

/** Seção geral: modo de trabalho padrão, níveis de git e o conjunto padrão de comandos. */
function AgentDefaults() {
  const { mode, perms, error, save } = useAgentSettings(null);
  const d = perms?.defaults;
  return (
    <section className="set-section">
      <h3>Agentes</h3>
      <p className="page-lead">
        Como as conversas novas começam e o que o agente pode executar sem pedir. Vale para todos os projetos; cada projeto pode
        ter a sua escolha, mais abaixo.
      </p>
      {error && (
        <div className="err small" role="alert">
          {error}
        </div>
      )}
      <div className="set-grid">
        <label className="set-field">
          <span className="small">Modo de trabalho padrão</span>
          <Select
            ariaLabel="Modo de trabalho padrão"
            title="Isolado: o agente trabalha numa cópia do projeto (só o que está commitado) e você aplica depois. Direto na pasta: ele trabalha na pasta do projeto, enxerga tudo, e as alterações valem na hora."
            value={mode?.global ?? "Isolated"}
            options={MODE_OPTIONS}
            onChange={(v) => void save(api.workmodeSet(null, v as WorkMode))}
          />
        </label>
        <label className="set-field">
          <span className="small">Git local (add, commit, checkout, merge…)</span>
          <Select
            ariaLabel="Git local"
            title="Comandos de git que alteram o repositório sem falar com o servidor."
            value={onOff(perms?.global_local ?? null)}
            options={[
              { value: "", label: "Padrão", hint: "ligado no Isolado, desligado no Direto" },
              { value: "on", label: "Ligado" },
              { value: "off", label: "Desligado" },
            ]}
            onChange={(v) => void save(api.permissionsSet(null, "local", parseOnOff(v)))}
          />
        </label>
        <label className="set-field">
          <span className="small">Git remoto (push, pull, fetch, clone)</span>
          <Select
            ariaLabel="Git remoto"
            title="Comandos de git que falam com o servidor. O push forçado continua negado."
            value={onOff(perms?.global_remote ?? null)}
            options={[
              { value: "", label: "Padrão", hint: "desligado" },
              { value: "on", label: "Ligado" },
              { value: "off", label: "Desligado" },
            ]}
            onChange={(v) => void save(api.permissionsSet(null, "remote", parseOnOff(v)))}
          />
        </label>
      </div>
      {d && (
        <div className="set-card set-perms">
          <b>Comandos liberados ao agente por padrão</b>
          <div className="small muted">
            Valem em todo projeto, sem configurar nada (Claude Code, nas ferramentas Bash e PowerShell). O resto é negado, a não
            ser o que estiver em "Comandos permitidos ao agente" do projeto. "…" = aceita argumentos.
          </div>
          <Patterns title="Git, só leitura" items={d.git_read} />
          <Patterns title="Inspeção" items={[...d.inspect, ...d.inspect_powershell]} />
          <Patterns title="Com o Git local ligado" items={d.git_local} />
          <Patterns title="Com o Git remoto ligado" items={d.git_remote} />
          <Patterns title="Nunca por padrão" items={d.never} />
        </div>
      )}
    </section>
  );
}

/** Escolhas do projeto: seguem as gerais até serem trocadas aqui. */
function ProjectAgentSettings({ project }: { project: Path }) {
  const { mode, perms, error, save } = useAgentSettings(project);
  const follow = { value: "", label: "Seguir o geral" };
  return (
    <div className="set-card">
      <b>Agentes neste projeto</b>
      {error && (
        <div className="err small" role="alert">
          {error}
        </div>
      )}
      <div className="set-grid">
        <label className="set-field">
          <span className="small">Modo de trabalho</span>
          <Select
            ariaLabel="Modo de trabalho do projeto"
            disabled={mode != null && !mode.is_git}
            title={
              mode != null && !mode.is_git
                ? "Este projeto não é um repositório git: só aceita Direto na pasta."
                : "Modo em que as conversas novas deste projeto começam."
            }
            value={mode != null && !mode.is_git ? "Direct" : (mode?.project ?? "")}
            options={[follow, ...MODE_OPTIONS]}
            onChange={(v) => void save(api.workmodeSet(project, (v || null) as WorkMode | null))}
          />
        </label>
        <label className="set-field">
          <span className="small">Git local</span>
          <Select
            ariaLabel="Git local do projeto"
            value={onOff(perms?.project_local ?? null)}
            options={[follow, { value: "on", label: "Ligado" }, { value: "off", label: "Desligado" }]}
            onChange={(v) => void save(api.permissionsSet(project, "local", parseOnOff(v)))}
          />
        </label>
        <label className="set-field">
          <span className="small">Git remoto</span>
          <Select
            ariaLabel="Git remoto do projeto"
            value={onOff(perms?.project_remote ?? null)}
            options={[follow, { value: "on", label: "Ligado" }, { value: "off", label: "Desligado" }]}
            onChange={(v) => void save(api.permissionsSet(project, "remote", parseOnOff(v)))}
          />
        </label>
      </div>
    </div>
  );
}

export default function SettingsView() {
  const projects = useApp((s) => s.projects);
  const selected = useApp((s) => s.selected);
  const [chosen, setChosen] = useState<Path | null>(null);
  const project = projects.find((p) => samePath(p.path, chosen ?? selected)) ?? projects[0] ?? null;

  return (
    <div className="set-view">
      <section className="set-section">
        <h3>Tema</h3>
        <p className="page-lead">Vale para toda a interface, o editor e os terminais. A escolha fica lembrada.</p>
        <ThemePicker />
      </section>

      <AgentDefaults />

      <section className="set-section">
        <h3>Por projeto</h3>
        {project ? (
          <>
            <label className="set-field">
              <span className="small">Projeto</span>
              <Select
                ariaLabel="Projeto"
                value={project.path}
                options={projects.map((p) => ({ value: p.path, label: p.name }))}
                onChange={setChosen}
              />
            </label>
            <ProjectAgentSettings key={project.path} project={project.path} />
            <div className="set-card">
              <AutoWorkPanel key={project.path} project={project.path} />
            </div>
            <div className="set-card">
              <AllowedCommands key={project.path} project={project.path} />
            </div>
          </>
        ) : (
          <p className="muted small">Abra uma pasta raiz para configurar os projetos.</p>
        )}
      </section>
    </div>
  );
}
