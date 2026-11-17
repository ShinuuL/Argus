import { useCallback, useEffect, useState } from "react";
import { api, baseName, errorMessage, type Apk, type Device } from "@/api";
import { Icon } from "@/shell/icons";
import { Select } from "@/shell/Select";
import { useApp } from "@/store";

function ago(secs: number): string {
  const d = Math.max(0, Date.now() / 1000 - secs);
  if (d < 60) return "agora";
  if (d < 3600) return `há ${Math.floor(d / 60)} min`;
  if (d < 86400) return `há ${Math.floor(d / 3600)} h`;
  return `há ${Math.floor(d / 86400)} d`;
}

const mb = (n: number) => `${(n / 1024 / 1024).toFixed(1)} MB`;

/**
 * Faixa "Instalar APK": lista os APKs das pastas de build do projeto ativo (Gradle e
 * Flutter) e instala o escolhido no celular, com confirmação. Some quando o projeto
 * não tem APK gerado.
 */
export function ApkBar({ devices }: { devices: Device[] }) {
  const project = useApp((s) => s.selected);
  const [apks, setApks] = useState<Apk[]>([]);
  const [apk, setApk] = useState<string>("");
  const [serial, setSerial] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  const ready = devices.filter((d) => d.state === "device");

  const load = useCallback(() => {
    if (!project) {
      setApks([]);
      return;
    }
    api.devicesApks(project).then(
      (list) => {
        setApks(list);
        setApk((cur) => (list.some((a) => a.path === cur) ? cur : (list[0]?.path ?? "")));
      },
      () => setApks([]),
    );
  }, [project]);

  useEffect(() => {
    setMsg(null);
    load();
  }, [load]);

  useEffect(() => {
    if (!ready.some((d) => d.serial === serial)) setSerial(ready[0]?.serial ?? "");
  }, [ready, serial]);

  if (!project || apks.length === 0) return null;

  const chosen = apks.find((a) => a.path === apk);
  const device = ready.find((d) => d.serial === serial);

  const install = async () => {
    if (!chosen || !device) return;
    const ok = await api
      .systemConfirm("Instalar APK", `Instalar ${chosen.label} em ${device.model ?? device.serial}? Uma versão já instalada é atualizada.`)
      .catch(() => false);
    if (!ok) return;
    setBusy(true);
    setMsg({ ok: true, text: "Instalando…" });
    try {
      const out = await api.devicesInstall(project, chosen.path, device.serial);
      setMsg({ ok: true, text: out || "Instalado." });
    } catch (e) {
      setMsg({ ok: false, text: errorMessage(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="dv-apk">
      <Icon name="box" size={15} />
      <span className="small muted">APK de {baseName(project)}</span>
      <div className="dv-apk-pick">
        <Select
          value={apk}
          ariaLabel="APK"
          options={apks.map((a) => ({ value: a.path, label: a.label, hint: `${mb(a.size)} · ${ago(a.modified)}` }))}
          onChange={setApk}
          onOpen={load}
        />
      </div>
      {ready.length > 1 && (
        <Select
          value={serial}
          ariaLabel="Celular"
          options={ready.map((d) => ({ value: d.serial, label: d.model ?? d.serial, hint: d.serial }))}
          onChange={setSerial}
        />
      )}
      <button onClick={() => void install()} disabled={busy || !chosen || !device} title={device ? undefined : "Nenhum celular pronto"}>
        Instalar{device && ready.length === 1 ? ` em ${device.model ?? device.serial}` : ""}
      </button>
      {msg && <span className={`small ${msg.ok ? "muted" : "dv-apk-err"}`}>{msg.text}</span>}
    </div>
  );
}
