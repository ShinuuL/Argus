/**
 * Configurações (aba): tema de cores, auto-work por projeto (liga/desliga e
 * limites) e os comandos que o agente pode executar em cada projeto.
 */
import { Select } from "@/shell/Select";
import { useState } from "react";
import { samePath, type Path } from "@/api";
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
