import { useCallback, useEffect, useState } from "react";
import { errorMessage } from "@/api";
import { useApp, useWorkbench } from "@/store";
import { connectionsStatus, type Connection } from "./api";
import { RepoCard } from "./RepoCard";
import "./connections.css";

export { RepoCard } from "./RepoCard";

/** Tela "Conexões": login das CLIs (GitHub, Google Cloud, Firebase, Vercel) e repositório do projeto ativo. */
export default function ConnectionsView() {
  const [list, setList] = useState<Connection[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const project = useApp((s) => s.selected);
  const openTerminal = useWorkbench((s) => s.openTerminal);

  const refresh = useCallback(() => {
    setList(null);
    setError(null);
    connectionsStatus()
      .then(setList)
      .catch((e) => setError(errorMessage(e)));
  }, []);

  useEffect(refresh, [refresh]);

  return (
    <div className="conn-view">
      <div className="conn-row">
        <h2>Conexões</h2>
        <button onClick={refresh} disabled={list === null && !error}>
          Atualizar
        </button>
      </div>
      {error && <div className="small muted">{error}</div>}
      {list === null && !error && <div className="small muted">Consultando as CLIs…</div>}
      {list?.map((c) => (
        <div key={c.id} className="conn-card conn-row">
          <b>{c.label}</b>
          <span className="small muted">
            {!c.installed
              ? `CLI ${c.id} não instalada`
              : c.error
                ? `Não foi possível verificar (${c.error})`
                : c.logged_in
                  ? `Conectado${c.account ? ` como ${c.account}` : ""}`
                  : "Não conectado"}
          </span>
          {c.installed && !c.logged_in && (
            <button onClick={() => openTerminal({ title: `login ${c.id}`, command: c.login })}>Fazer login</button>
          )}
        </div>
      ))}
      <h3>Repositório do projeto</h3>
      {project ? <RepoCard project={project} /> : <div className="small muted">Selecione um projeto.</div>}
    </div>
  );
}
