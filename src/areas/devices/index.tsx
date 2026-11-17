/** Área "Dispositivos" (tela 08 do mockup): aparelhos à esquerda, espelho na moldura e abas de ferramentas. */
import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage, type Device, type DeviceList, type MirrorStatus } from "@/api";
import { AreaHeader, Btn, Chip, SideHeading, SideItem, SideList, StatusDot, Tab, Tabs, AreaStatus } from "@/ui";
import { ApkBar } from "@/views/devices/ApkBar";
import "./devices-area.css";

const STATE_LABEL: Record<string, string> = {
  device: "conectado",
  unauthorized: "autorize a depuração no celular",
  offline: "offline",
};

type ToolTab = "logcat" | "apps" | "files" | "net" | "info";
const TABS: { id: ToolTab; label: string }[] = [
  { id: "logcat", label: "Logcat" },
  { id: "apps", label: "Apps" },
  { id: "files", label: "Arquivos" },
  { id: "net", label: "Rede (AllSeenEye)" },
  { id: "info", label: "Info" },
];

/** O que falta para o Screen-Mirror poder iniciar (null = pronto). */
function missing(s: MirrorStatus): string | null {
  if (!s.dir) return "Screen-Mirror não encontrado. Informe a pasta do projeto na aba Info.";
  if (!s.node) return "Node.js não encontrado no PATH. O Screen-Mirror precisa do Node 22.18 ou mais novo.";
  if (s.nodeTooOld) return `Node.js ${s.node} é antigo demais. Instale o 22.18 ou mais novo.`;
  if (s.needsInstall) return `Dependências não instaladas. Rode \`npm install\` em ${s.dir}.`;
  return null;
}

const portOf = (url: string | null) => url?.match(/:(\d+)/)?.[1] ?? "?";
const devName = (d: Device) => d.model ?? d.serial;

export default function DevicesArea() {
  const [status, setStatus] = useState<MirrorStatus | null>(null);
  const [list, setList] = useState<DeviceList | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [dirInput, setDirInput] = useState("");
  const [frameKey, setFrameKey] = useState(0);
  const [sel, setSel] = useState<string | null>(null);
  const [tab, setTab] = useState<ToolTab>("logcat");
  const autoStarted = useRef(false);

  const run = useCallback(async (action: () => Promise<MirrorStatus>) => {
    setBusy(true);
    try {
      setStatus(await action());
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
      setStatus(await api.devicesStatus().catch(() => null));
    } finally {
      setBusy(false);
    }
  }, []);

  // Ao abrir: lê o estado e, se tudo estiver pronto, inicia uma vez sozinho.
  useEffect(() => {
    let alive = true;
    api.devicesStatus().then(
      (s) => {
        if (!alive) return;
        setStatus(s);
        if (s.dir) setDirInput(s.dir);
        if (!s.url && !missing(s) && !autoStarted.current) {
          autoStarted.current = true;
          void run(api.devicesStart);
        }
      },
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [run]);

  // Aparelhos: atualiza a cada 3 s enquanto o sidecar estiver no ar.
  const url = status?.url ?? null;
  useEffect(() => {
    if (!url) {
      setList(null);
      return;
    }
    let alive = true;
    const tick = () =>
      api.devicesList().then(
        (l) => alive && setList(l),
        () => alive && api.devicesStatus().then((s) => alive && setStatus(s), () => {}),
      );
    void tick();
    const id = window.setInterval(tick, 3000);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, [url]);

  const devices = list?.devices ?? [];
  const current = devices.find((d) => d.serial === sel) ?? devices[0];
  const problem = status ? missing(status) : null;
  const saveDir = () => {
    const d = dirInput.trim();
    if (d) void run(() => api.devicesSetDir(d));
  };
  const stop = () =>
    void run(async () => {
      await api.devicesStop();
      return api.devicesStatus();
    });

  return (
    <div className="ui-area">
      <SideList width={200} label="Aparelhos">
        <SideHeading>Aparelhos</SideHeading>
        {devices.map((d) => (
          <SideItem
            key={d.serial}
            on={current?.serial === d.serial}
            onClick={() => setSel(d.serial)}
            icon={<StatusDot tone={d.state === "device" ? "ok" : "warn"} />}
            meta={d.state !== "device" ? STATE_LABEL[d.state] ?? d.state : d.connection === "wifi" ? "Wi-Fi" : undefined}
            title={d.serial}
          >
            {devName(d)}
          </SideItem>
        ))}
        {url && list && devices.length === 0 && <div className="dva-none ui-mut ui-sm">Nenhum celular conectado por ADB.</div>}
        {!url && <div className="dva-none ui-mut ui-sm">Sidecar parado.</div>}
        <SideItem
          title="O pareamento por Wi-Fi ainda é feito pelo painel do Screen-Mirror."
          onClick={() => setTab("logcat")}
        >
          <span className="ui-mut">+ Parear por Wi-Fi</span>
        </SideItem>
      </SideList>

      <div className="ui-area-main">
        <AreaHeader
          title={
            current ? (
              <>
                Dispositivos <span className="ui-mut">/</span> {devName(current)}
              </>
            ) : (
              "Dispositivos"
            )
          }
          actions={
            <>
              {url && <ApkBar devices={devices} />}
              {url ? (
                <Btn variant="ghost" small onClick={stop} disabled={busy}>
                  Parar
                </Btn>
              ) : (
                <Btn small onClick={() => void run(api.devicesStart)} disabled={busy || !status || !!problem}>
                  {busy ? "Iniciando…" : "Iniciar"}
                </Btn>
              )}
            </>
          }
        >
          {current && (
            <Chip tone={current.state === "device" ? "ok" : "warn"}>{current.connection === "wifi" ? "Wi-Fi" : "USB"}</Chip>
          )}
        </AreaHeader>

        {error && <div className="dva-err">{error}</div>}

        <div className="dva-center">
          <div className="dva-phone">
            {url ? (
              <iframe
                key={frameKey}
                src={url}
                title="Painel do Screen-Mirror"
                allow="clipboard-read; clipboard-write; fullscreen"
              />
            ) : (
              <div className="dva-phone-empty ui-mut">{problem ?? (busy ? "iniciando…" : "espelho parado")}</div>
            )}
          </div>

          <div className="dva-tools">
            <Tabs label="Ferramentas do aparelho">
              {TABS.map((t) => (
                <Tab key={t.id} on={tab === t.id} onClick={() => setTab(t.id)}>
                  {t.label}
                </Tab>
              ))}
            </Tabs>

            <div className="dva-pane">
              {tab === "info" ? (
                <div className="dva-info">
                  {current ? (
                    <dl>
                      <dt>Modelo</dt>
                      <dd>{current.model ?? "—"}</dd>
                      <dt>Serial</dt>
                      <dd className="ui-mono">{current.serial}</dd>
                      <dt>Estado</dt>
                      <dd>{STATE_LABEL[current.state] ?? current.state}</dd>
                      <dt>Conexão</dt>
                      <dd>{current.connection === "wifi" ? "Wi-Fi" : "USB"}</dd>
                      <dt>Espelhando</dt>
                      <dd>{list?.mirroring.includes(current.serial) ? "sim" : "não"}</dd>
                    </dl>
                  ) : (
                    <p className="ui-mut">Nenhum aparelho selecionado.</p>
                  )}
                  <label className="dva-field">
                    <span className="ui-sm ui-mut">Pasta do projeto Screen-Mirror</span>
                    <div className="dva-row">
                      <input
                        value={dirInput}
                        placeholder="D:\…\Screen-Mirror"
                        onChange={(e) => setDirInput(e.target.value)}
                        onKeyDown={(e) => e.key === "Enter" && saveDir()}
                      />
                      <Btn variant="ghost" small onClick={saveDir} disabled={busy || !dirInput.trim()}>
                        Salvar
                      </Btn>
                    </div>
                  </label>
                  {url && (
                    <div className="dva-row">
                      <Btn variant="ghost" small onClick={() => setFrameKey((k) => k + 1)}>
                        Recarregar espelho
                      </Btn>
                      <Btn variant="ghost" small onClick={() => void api.systemOpenUrl(url)}>
                        Abrir no navegador
                      </Btn>
                    </div>
                  )}
                  <p className="ui-sm ui-mut">
                    O Screen-Mirror roda só em 127.0.0.1, numa porta livre escolhida pelo Argus, e é encerrado junto
                    com a janela.
                  </p>
                </div>
              ) : tab === "net" ? (
                <div className="ui-empty">A aba de rede do AllSeenEye ainda não foi integrada.</div>
              ) : (
                <div className="ui-empty">
                  {url
                    ? `${TABS.find((t) => t.id === tab)?.label} ainda não tem tela nativa. Por enquanto, use o painel do Screen-Mirror na moldura ao lado.`
                    : "Inicie o sidecar do Screen-Mirror para ver os dados do aparelho."}
                </div>
              )}
            </div>

            <div className="dva-chips">
              <span className="dva-sp" />
              {status?.dir && (
                <Chip
                  tone={status.agentsMcp ? "ok" : undefined}
                  onClick={busy ? undefined : () => void run(() => api.devicesSetAgents(!status.agentsMcp))}
                  title="Claude e Codex recebem as ferramentas do Screen-Mirror (mcp__screen-mirror__*). Clique para alternar."
                >
                  {status.agentsMcp ? "MCP liberado nas conversas" : "MCP desligado nas conversas"}
                </Chip>
              )}
            </div>
          </div>
        </div>

        <AreaStatus>
          <span>
            {url ? `sidecar Screen-Mirror ok · :${portOf(url)}` : `sidecar Screen-Mirror ${busy ? "iniciando" : "parado"}`}
          </span>
          <span className="dva-sp" />
          {status?.pid != null && <span>PID {status.pid}</span>}
        </AreaStatus>
      </div>
    </div>
  );
}
