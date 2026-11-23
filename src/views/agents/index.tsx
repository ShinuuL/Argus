/**
 * Provedores (seção de Configurações): login e status das CLIs dos agentes.
 *
 * A IDE só executa as CLIs oficiais; o login é feito pela própria CLI, num
 * terminal embutido. Nenhum token é lido ou guardado aqui.
 */
import { useCallback, useEffect, useState } from "react";
import { api, errorMessage, loginStatusLabel, onAnyPtyExit, type LoginStatus, type ProviderInfo } from "@/api";
import { defaultCwd, useApp, useTerminals } from "@/store";
import { Icon } from "@/shell/icons";
import "./agents.css";

let providersCache: Promise<ProviderInfo[]> | null = null;

/** Provedores disponíveis (Claude Code, OpenCode, Codex, Gemini): uma consulta por execução do app. */
export function useProviders(): ProviderInfo[] {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  useEffect(() => {
    let alive = true;
    providersCache ??= api.agentsProviders();
    providersCache.then(
      (list) => alive && setProviders(list),
      () => {
        providersCache = null;
      },
    );
    return () => {
      alive = false;
    };
  }, []);
  return providers;
}

export default function ProvidersView() {
  const providers = useProviders();
  /** `undefined` = verificando. */
  const [login, setLogin] = useState<Record<string, LoginStatus | undefined>>({});
  const [error, setError] = useState<string | null>(null);
  const openTerminal = useTerminals((s) => s.open);
  const requestInstall = useApp((s) => s.requestInstall);

  /** "Instalar...": só abre a confirmação do fluxo de Dependências (comando exato, terminal). */
  const doInstall = async (p: ProviderInfo) => {
    try {
      const plan = await api.depsInstallPlan(p.id);
      if (!plan) {
        setError(`Não há instalação conhecida para a CLI ${p.id}.`);
        return;
      }
      requestInstall({
        title: `Instalar ${p.display_name}?`,
        reason: `A CLI ${p.id} não foi encontrada no PATH. Depois de instalar, faça o login por aqui e atualize o status.`,
        plan,
      });
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const refresh = useCallback(() => {
    setLogin({});
    for (const p of providers) {
      api.agentsLoginStatus(p.id).then(
        (st) => setLogin((cur) => ({ ...cur, [p.id]: st })),
        () => setLogin((cur) => ({ ...cur, [p.id]: "Unknown" })),
      );
    }
  }, [providers]);

  // Na abertura e sempre que um terminal termina (ex.: a CLI concluiu o login).
  useEffect(() => {
    refresh();
    return onAnyPtyExit(() => refresh());
  }, [refresh]);

  const doLogin = async (p: ProviderInfo) => {
    try {
      const command = await api.agentsLoginCommand(p.id);
      openTerminal({ title: `login ${p.id}`, cwd: command.cwd ?? defaultCwd(), command });
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <div className="ag-view">
      <p className="page-lead">
        Os agentes são as CLIs oficiais, com o login da sua assinatura. O login acontece na própria CLI, num terminal
        da IDE; o Argus não lê nem guarda credenciais.
      </p>
      <div className="ag-list">
        {providers.length === 0 && <span className="muted small">Nenhum provedor disponível.</span>}
        {providers.map((p) => {
          const st = login[p.id];
          const { label, tone } = loginStatusLabel(st);
          return (
            <div key={p.id} className="ag-card">
              <Icon name="bot" size={22} />
              <div className="ag-card-main">
                <b>{p.display_name}</b>
                <span className="small muted">CLI: {p.id}</span>
              </div>
              <span className={`ag-status ${tone}`}>
                <span className="ag-status-dot" aria-hidden="true" />
                {label}
              </span>
              {p.id === "claude" && st !== "NotInstalled" && st !== undefined && (
                <button
                  title="Roda `claude update` num terminal: confere se há versão nova e instala. Turnos em andamento seguem com a versão atual."
                  onClick={() => openTerminal({ title: "Atualizar a CLI do Claude", cwd: defaultCwd(), command: { program: "claude", args: ["update"] } })}
                >
                  Atualizar CLI
                </button>
              )}
              {st === "NotInstalled" ? (
                <button title="Mostra o comando de instalação e pede confirmação antes de executar" onClick={() => void doInstall(p)}>
                  Instalar...
                </button>
              ) : st === "LoggedIn" ? (
                <button className="ag-relogin" title={`Abre o login da CLI ${p.id} num terminal para trocar de conta`} onClick={() => void doLogin(p)}>
                  Trocar de conta
                </button>
              ) : (
                <button
                  className="primary"
                  disabled={st === undefined}
                  title={`Abre o login da CLI ${p.id} num terminal`}
                  onClick={() => void doLogin(p)}
                >
                  Fazer login
                </button>
              )}
            </div>
          );
        })}
      </div>
      <div className="row">
        <button onClick={refresh}>
          <Icon name="refresh" size={13} /> Atualizar status
        </button>
      </div>
      {error && (
        <div className="err small" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
