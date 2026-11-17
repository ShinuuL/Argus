import { Select } from "@/shell/Select";
import { useEffect, useState } from "react";
import { api, errorMessage, type AddrKind, type RemoteInfo } from "@/api";
import "./remote.css";

const KIND_LABEL: Record<AddrKind, string> = { Lan: "LAN", Tailscale: "Tailscale", Virtual: "VPN/virtual" };
const DEFAULT_PORT = "8787";

/**
 * Remoto (barra lateral): liga/desliga o servidor da rede local, mostra o QR
 * code de pareamento, a escolha de IP e o botão de gerar novo token.
 */
export default function RemoteView() {
  const [info, setInfo] = useState<RemoteInfo | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [port, setPort] = useState(DEFAULT_PORT);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let alive = true;
    api.remoteInfo().then(
      (i) => {
        if (!alive) return;
        setInfo(i);
        setLoaded(true);
      },
      (e) => {
        if (!alive) return;
        setError(errorMessage(e));
        setLoaded(true);
      },
    );
    return () => {
      alive = false;
    };
  }, []);

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
    void run(() => api.remoteStart(p), "Falha ao iniciar: ");
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

  return (
    <div className="side-pad remote">
      <p className="small muted rx-p">
        Abre um servidor na rede local para ver as sessões e responder aos agentes pelo celular. Leia o QR code com
        a câmera. O tráfego não é criptografado: use só em redes confiáveis (ou via VPN como Tailscale).
      </p>
      {!loaded && <p className="small muted rx-p">Verificando o servidor...</p>}
      {loaded && info && (
        <>
          <p className="ok rx-p">Servidor ligado na porta {info.port}</p>
          {Number(port.trim()) > 0 && info.port !== Number(port.trim()) && (
            <p className="small rx-p">
              A porta {port.trim()} estava em uso; o Argus usou a livre mais próxima.
            </p>
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
          {info.hosts.length > 1 && (
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
          <p className="small warn rx-p">
            Firewall do Windows: libere a porta {info.port} (TCP) para redes privadas, ou aceite o aviso do
            Firewall. Em rede marcada como Pública o acesso costuma ser bloqueado.
          </p>
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
        <>
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
          <p className="small warn rx-p">
            O Firewall do Windows pode pedir permissão ao ligar: permita em redes privadas. Se o celular não
            conectar, verifique se a rede Wi-Fi está como Privada.
          </p>
        </>
      )}
      {error && (
        <p className="err rx-p" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
