import { Select } from "@/shell/Select";
import { useEffect, useState } from "react";
import { api, errorMessage, type AddrKind, type RemoteExposure, type RemoteInfo } from "@/api";
import "./remote.css";

const KIND_LABEL: Record<AddrKind, string> = { Lan: "LAN", Tailscale: "Tailscale", Virtual: "VPN/virtual" };
const DEFAULT_PORT = "8787";

const MODES: { id: RemoteExposure; label: string; hint: string }[] = [
  { id: "Funnel", label: "Tailscale Funnel", hint: "HTTPS aberto na internet: o celular entra de qualquer rede." },
  { id: "Tailnet", label: "Só no meu tailnet", hint: "HTTPS só para aparelhos com o Tailscale na sua conta." },
  { id: "Lan", label: "Rede local", hint: "http pelo IP da LAN. Depende de liberar a porta no Firewall." },
];

/**
 * Remoto: liga/desliga o servidor, mostra o QR code de pareamento e o botão de
 * gerar novo token. O padrão é publicar por Tailscale Funnel — o servidor escuta
 * só em 127.0.0.1 e a publicação HTTPS existe apenas enquanto ele estiver ligado.
 *
 * `onRunningChange` avisa quem mostra o estado por fora (o chip do menu de
 * Configurações) sempre que o servidor liga ou desliga.
 */
export default function RemoteView({ onRunningChange }: { onRunningChange?: (running: boolean) => void } = {}) {
  const [info, setInfo] = useState<RemoteInfo | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [port, setPort] = useState(DEFAULT_PORT);
  const [mode, setMode] = useState<RemoteExposure>("Funnel");
  const [ts, setTs] = useState<{ installed: boolean; running: boolean; dns_name: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let alive = true;
    api.remoteInfo().then(
      (i) => {
        if (!alive) return;
        setInfo(i);
        if (i) setMode(i.exposure);
        setLoaded(true);
      },
      (e) => {
        if (!alive) return;
        setError(errorMessage(e));
        setLoaded(true);
      },
    );
    api.remoteTailscaleStatus().then((s) => alive && setTs(s), () => {});
    return () => {
      alive = false;
    };
  }, []);

  // Avisa o estado real do servidor: na carga e a cada ligar/desligar.
  const running = loaded ? info != null : null;
  useEffect(() => {
    if (running != null) onRunningChange?.(running);
    // `onRunningChange` costuma ser recriada a cada render de quem chama.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running]);

  /** Roda uma ação do servidor e aplica o resultado (ou mostra o erro). */
  const run = async (action: () => Promise<RemoteInfo | null>, prefix = "") => {
    setBusy(true);
    try {
      setInfo(await action());
      setError(null);
      setCopied(false);
    } catch (e) {
      setError(prefix + errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const start = () => {
    const p = Number(port.trim());
    if (!Number.isInteger(p) || p < 1 || p > 65535) {
      setError("Porta inválida. Use um número entre 1 e 65535.");
      return;
    }
    void run(() => api.remoteStart(p, mode), "Falha ao iniciar: ");
  };

  const stop = () =>
    void run(async () => {
      await api.remoteStop();
      return null;
    });

  const regenerate = async () => {
    const ok = await api
      .systemConfirm(
        "Gerar novo token",
        "Todos os aparelhos pareados perdem o acesso e precisam ler o novo QR code. Continuar?",
      )
      .catch(() => false);
    if (ok) void run(() => api.remoteRegenerateToken());
  };

  const copyUrl = async () => {
    if (!info) return;
    try {
      await navigator.clipboard.writeText(info.url);
      setCopied(true);
    } catch {
      setError("Não foi possível copiar. Selecione o endereço e copie manualmente.");
    }
  };

  const viaTailscale = mode !== "Lan";
  const tsBlocked = viaTailscale && ts != null && (!ts.installed || !ts.running);

  return (
    <div className="side-pad remote">
      <p className="small muted rx-p">
        Abre um servidor para ver as sessões e responder aos agentes pelo celular. Leia o QR code com a câmera.
      </p>

      {loaded && !info && (
        <>
          <label className="small" htmlFor="rx-mode">
            Como publicar
          </label>
          <Select
            id="rx-mode"
            ariaLabel="Como publicar o servidor"
            value={mode}
            disabled={busy}
            options={MODES.map((m) => ({ value: m.id, label: m.label, hint: m.hint }))}
            onChange={(v) => setMode((v as RemoteExposure) || "Funnel")}
          />
          {mode === "Funnel" && (
            <p className="small warn rx-p">
              O Funnel publica o Argus na internet: quem tiver a URL entra, mesmo sem Tailscale. Como a URL leva o
              token, trate-a como senha. Se o celular tem Tailscale, prefira "Só no meu tailnet".
            </p>
          )}
          {viaTailscale && (
            <p className="small muted rx-p">
              Nesse modo o Argus escuta só em 127.0.0.1 e quem publica é o Tailscale — o Firewall do Windows não
              pede liberação.
            </p>
          )}
          {tsBlocked && (
            <p className="small warn rx-p">
              {ts?.installed
                ? "O Tailscale está instalado, mas não conectado. Abra o Tailscale e entre na sua conta."
                : "Tailscale não encontrado neste computador. Instale-o ou escolha “Rede local”."}
            </p>
          )}
        </>
      )}

      {!loaded && <p className="small muted rx-p">Verificando o servidor...</p>}

      {loaded && info && (
        <>
          <p className="ok rx-p">
            {info.exposure === "Funnel"
              ? `Publicado na internet pelo Tailscale Funnel (porta local ${info.port})`
              : info.exposure === "Tailnet"
                ? `Publicado no seu tailnet pelo Tailscale (porta local ${info.port})`
                : `Servidor ligado na porta ${info.port}`}
          </p>
          {info.exposure === "Lan" && Number(port.trim()) > 0 && info.port !== Number(port.trim()) && (
            <p className="small rx-p">A porta {port.trim()} estava em uso; o Argus usou a livre mais próxima.</p>
          )}
          {/* O SVG do QR code é gerado pelo próprio backend do Argus. */}
          <div className="qr" role="img" aria-label="QR code de pareamento" dangerouslySetInnerHTML={{ __html: info.qr_svg }} />
          <p className="small rx-url">{info.url}</p>
          <div className="row">
            <button onClick={copyUrl}>{copied ? "Endereço copiado" : "Copiar endereço"}</button>
          </div>
          <p className="small muted rx-p">
            O endereço contém o token de acesso do Argus: quem tiver o endereço entra. Não o compartilhe.
          </p>

          {info.exposure === "Funnel" && (
            <p className="small warn rx-p">
              Enquanto estiver ligado, este endereço responde a qualquer um na internet. Desligue o servidor quando
              terminar — a publicação cai junto.
            </p>
          )}

          {info.exposure === "Lan" && info.hosts.length > 1 && (
            <>
              <label className="small" htmlFor="rx-host">
                Endereço IP
              </label>
              <Select
                id="rx-host"
                ariaLabel="Endereço IP"
                value={info.host}
                disabled={busy}
                options={info.hosts.map((h) => ({ value: h.ip, label: h.ip, hint: `${h.iface} · ${KIND_LABEL[h.kind] ?? h.kind}` }))}
                onChange={(v) => void run(() => api.remoteSetHost(v || null))}
              />
              <p className="small muted rx-p">
                Se o celular não abrir, escolha o IP da mesma rede Wi-Fi do celular. IPs VPN/virtuais só funcionam
                com o celular na mesma VPN.
              </p>
            </>
          )}
          {info.exposure === "Lan" && (
            <p className="small warn rx-p">
              Firewall do Windows: libere a porta {info.port} (TCP) para redes privadas, ou aceite o aviso do
              Firewall. Em rede marcada como Pública o acesso costuma ser bloqueado.
            </p>
          )}

          <div className="row">
            <button onClick={stop} disabled={busy}>
              Desligar
            </button>
            <button
              title="Revoga o acesso de todos os aparelhos pareados"
              onClick={() => void regenerate()}
              disabled={busy}
            >
              Gerar novo token
            </button>
          </div>
        </>
      )}

      {loaded && !info && (
        <div className="row">
          <label className="small" htmlFor="rx-port">
            Porta
          </label>
          <input
            id="rx-port"
            className="rx-port"
            inputMode="numeric"
            value={port}
            onChange={(e) => setPort(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && !busy && start()}
          />
          <button className="primary" disabled={busy} onClick={start}>
            {busy ? "Ligando..." : "Ligar servidor"}
          </button>
        </div>
      )}

      {error && (
        <p className="err rx-p" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
