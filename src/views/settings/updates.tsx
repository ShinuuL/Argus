/**
 * Atualização do Argus pelo GitHub Releases (Tauri updater plugin).
 *
 * A checagem e a instalação só acontecem quando o usuário pede. A chave pública
 * fica no tauri.conf.json; a privada fica fora do repo (GitHub Secret).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { api, errorMessage } from "@/api";

const RELEASES_URL = "https://github.com/ShinuuL/Argus/releases/latest";

type Phase = "idle" | "checking" | "available" | "none" | "downloading" | "installing" | "ready" | "error";

function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

/** Conteúdo reaproveitado pela área de Configurações e pela view antiga. */
export function UpdatesContent() {
  const [current, setCurrent] = useState<string>("");
  const [phase, setPhase] = useState<Phase>("idle");
  const [update, setUpdate] = useState<Update | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [downloaded, setDownloaded] = useState(0);
  const [total, setTotal] = useState<number | null>(null);

  const autoChecked = useRef(false);
  useEffect(() => {
    void getVersion().then(setCurrent).catch(() => undefined);
  }, []);

  const reset = useCallback(() => {
    setPhase("idle");
    setUpdate(null);
    setError(null);
    setDownloaded(0);
    setTotal(null);
  }, []);

  const checkNow = useCallback(async () => {
    setPhase("checking");
    setError(null);
    setUpdate(null);
    setDownloaded(0);
    setTotal(null);
    try {
      const found = await check();
      if (found) {
        setUpdate(found);
        setPhase("available");
      } else {
        setPhase("none");
      }
    } catch (e) {
      setError(errorMessage(e));
      setPhase("error");
    }
  }, []);

  useEffect(() => {
    if (autoChecked.current) return;
    autoChecked.current = true;
    void checkNow();
  }, [checkNow]);

  const installNow = useCallback(async () => {
    if (!update) return;
    setError(null);
    setDownloaded(0);
    setTotal(null);
    setPhase("downloading");
    try {
      await update.downloadAndInstall((event: DownloadEvent) => {
        if (event.event === "Started") {
          setTotal(event.data.contentLength ?? null);
          setPhase("downloading");
        } else if (event.event === "Progress") {
          setDownloaded((n) => n + event.data.chunkLength);
        } else if (event.event === "Finished") {
          setPhase("installing");
        }
      });
      setPhase("ready");
      await relaunch();
    } catch (e) {
      setError(errorMessage(e));
      setPhase("error");
    }
  }, [update]);

  const openReleases = () => {
    void api.systemOpenUrl(RELEASES_URL).catch(() => undefined);
  };

  const progress = total && total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : null;

  return (
    <>
      <div className="set-card">
        <b>Versão instalada</b>
        <p className="small muted">
          Atual: <code>{current || "…"}</code>
        </p>
        <p className="small muted">
          O Argus consulta o <code>latest.json</code> publicado nas releases do GitHub. A atualização só é instalada se a
          assinatura bater com a chave pública configurada no app.
        </p>
        <div className="set-img-actions">
          <button type="button" className="primary" disabled={phase === "checking" || phase === "downloading" || phase === "installing"} onClick={() => void checkNow()}>
            {phase === "checking" ? "Verificando…" : "Verificar atualização"}
          </button>
          <button type="button" onClick={openReleases}>
            Abrir releases
          </button>
          {(phase === "none" || phase === "error" || phase === "available") && (
            <button type="button" onClick={reset}>
              Limpar estado
            </button>
          )}
        </div>

        {phase === "none" && <p className="small">Você já está na versão mais recente.</p>}

        {phase === "available" && update && (
          <div className="small">
            <p>
              Nova versão disponível: <b>{update.version}</b>
              {update.date ? ` (${update.date})` : ""}
            </p>
            {update.body && (
              <pre style={{ whiteSpace: "pre-wrap", margin: "8px 0", font: "inherit" }}>{update.body}</pre>
            )}
            <div className="set-img-actions">
              <button type="button" className="primary" onClick={() => void installNow()}>
                Baixar e instalar
              </button>
            </div>
            <p className="muted">
              Ao instalar, o instalador NSIS será aberto/executado e o Argus vai reiniciar ao final.
            </p>
          </div>
        )}

        {(phase === "downloading" || phase === "installing") && (
          <p className="small">
            {phase === "installing"
              ? "Instalando atualização…"
              : progress != null
                ? `Baixando atualização… ${progress}% (${bytes(downloaded)}${total ? ` / ${bytes(total)}` : ""})`
                : `Baixando atualização… ${bytes(downloaded)}`}
          </p>
        )}

        {phase === "ready" && <p className="small">Atualização instalada. Reiniciando…</p>}

        {phase === "error" && (
          <div className="small err" role="alert">
            <p>Não foi possível atualizar: {error}</p>
            <p>
              Você pode baixar manualmente pela página de releases. Se o erro mencionar assinatura, endpoint ou
              <code>latest.json</code>, confira a configuração do updater e a release publicada.
            </p>
          </div>
        )}
      </div>
    </>
  );
}

/** View antiga do workbench (ainda usada por abas restauradas). */
export default function UpdatesPanel() {
  return (
    <section className="set-section">
      <h3>Atualizações</h3>
      <p className="page-lead">
        Verifica novas versões publicadas no GitHub Releases e instala a atualização com assinatura verificada.
      </p>
      <UpdatesContent />
    </section>
  );
}
