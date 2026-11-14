/**
 * Dependências do projeto e modal de confirmação de instalação.
 *
 * Regra: NADA é instalado sem o usuário clicar em "Executar" no `InstallPrompt`,
 * que sempre mostra o comando exato. A execução acontece numa aba do terminal
 * embutido, onde o usuário acompanha (e pode interromper) a instalação.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage, samePath, type DepIssue, type DepKind, type Path } from "@/api";
import { useApp, useTerminals } from "@/store";
import "./deps.css";

const KIND_LABEL: Record<DepKind, string> = {
  MissingToolchain: "Ferramenta ausente",
  ProjectDepsMissing: "Dependências não instaladas",
  ProjectDepsOutdated: "Dependências desatualizadas",
};

/**
 * Modal global de confirmação de instalação, montado uma vez pelo shell.
 * Lê `useApp().pendingInstall`, mostra o comando exato e só executa (num
 * terminal) depois do clique em "Executar".
 */
export function InstallPrompt() {
  const pending = useApp((s) => s.pendingInstall);
  const clearInstall = useApp((s) => s.clearInstall);
  const selected = useApp((s) => s.selected);
  const root = useApp((s) => s.root);
  const cancelRef = useRef<HTMLButtonElement | null>(null);

  // O foco começa em "Cancelar": Enter por engano não executa nada.
  useEffect(() => {
    if (pending) cancelRef.current?.focus();
  }, [pending]);

  if (!pending) return null;
  const { title, reason, plan } = pending;
  const cwd = plan.command.cwd ?? selected ?? root;

  const run = () => {
    useTerminals.getState().open({ title, cwd, command: plan.command });
    clearInstall();
  };

  return (
    <div
      className="dx-backdrop"
      onMouseDown={(e) => e.target === e.currentTarget && clearInstall()}
      onKeyDown={(e) => e.key === "Escape" && clearInstall()}
    >
      <div className="dx-modal" role="dialog" aria-modal="true" aria-labelledby="dx-title">
        <h3 id="dx-title">{title}</h3>
        <p>{reason}</p>
        <p>O comando abaixo será executado em uma nova aba do terminal:</p>
        <pre className="dx-cmd">{plan.display}</pre>
        <p className="muted small">Pasta: {cwd ?? "pasta atual do aplicativo"}</p>
        {plan.needs_admin && <p className="warn">Este comando requer privilégios de administrador.</p>}
        <div className="dx-actions">
          <button ref={cancelRef} onClick={clearInstall}>
            Cancelar
          </button>
          <button className="primary" onClick={run}>
            Executar
          </button>
        </div>
      </div>
    </div>
  );
}

/** Pendências de um projeto, em cartões. "Instalar..." só abre a confirmação. */
function DepsPanel({ project }: { project: Path }) {
  const requestInstall = useApp((s) => s.requestInstall);
  const [issues, setIssues] = useState<DepIssue[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rev, setRev] = useState(0);

  useEffect(() => {
    let alive = true;
    setIssues(null);
    setError(null);
    api.depsAnalyze(project).then(
      (list) => alive && setIssues(list),
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [project, rev]);

  const recheck = useCallback(() => setRev((r) => r + 1), []);

  return (
    <div className="dx-panel">
      <div className="dx-top">
        <span className="small muted">
          {issues == null ? "" : issues.length === 0 ? "Tudo em ordem" : `${issues.length} pendência(s)`}
        </span>
        <button onClick={recheck} disabled={issues == null && !error}>
          Verificar novamente
        </button>
      </div>
      {error && <p className="err small">{error}</p>}
      {!error && issues == null && <p className="muted small">Verificando dependências...</p>}
      {issues != null && issues.length === 0 && <p className="muted small">Nenhuma dependência pendente.</p>}
      {issues != null && issues.length > 0 && (
        <div className="deps-list">
          {issues.map((issue, i) => (
            <div key={`${issue.tool}-${i}`} className="dep-item">
              <span className="warn small">{KIND_LABEL[issue.kind] ?? issue.kind}</span>
              <div className="dep-head">
                <b className="ellipsis">{issue.tool}</b>
                <button
                  title="Mostra o comando e pede confirmação antes de executar"
                  onClick={() =>
                    requestInstall({
                      title: `Instalar ${issue.tool}?`,
                      reason: issue.description,
                      plan: issue.fix,
                    })
                  }
                >
                  Instalar...
                </button>
              </div>
              <div className="small">{issue.description}</div>
              <code className="dep-cmd small" title={issue.fix.display}>
                {issue.fix.display}
              </code>
              {issue.fix.needs_admin && <span className="small muted">Requer administrador.</span>}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Dependências (barra lateral): escolhe o projeto e lista o que falta
 * (ferramentas ausentes, pacotes não instalados ou desatualizados).
 */
export default function DepsView() {
  const projects = useApp((s) => s.projects);
  const selected = useApp((s) => s.selected);
  const select = useApp((s) => s.select);
  const current = projects.find((p) => samePath(p.path, selected)) ?? null;

  return (
    <div className="side-pad">
      <select
        aria-label="Projeto"
        value={current?.path ?? ""}
        onChange={(e) => select(e.target.value || null)}
      >
        <option value="">Escolha um projeto...</option>
        {projects.map((p) => (
          <option key={p.path} value={p.path}>
            {p.name}
          </option>
        ))}
      </select>
      {current ? (
        <DepsPanel key={current.path} project={current.path} />
      ) : (
        <p className="muted small">Selecione um projeto (abra uma pasta raiz na barra de projetos).</p>
      )}
    </div>
  );
}
