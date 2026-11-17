/**
 * Uso de CPU, memória e GPU de um dono (sessão/terminal): soma do processo do agente,
 * filhos e servidores vinculados. Atualiza a cada 3 s só com a janela visível.
 */
import { useEffect, useState } from "react";
import { api, sameOwner, type Owner, type OwnerUsage } from "@/api";

function fmtMem(b: number): string {
  if (b >= 1 << 30) return `${(b / (1 << 30)).toFixed(1)} GB`;
  return `${Math.round(b / (1 << 20))} MB`;
}

/** Amostra periódica de todos os donos; pausa com a aba/janela oculta. */
export function useOwnersUsage(everyMs = 3000): OwnerUsage[] {
  const [list, setList] = useState<OwnerUsage[]>([]);
  useEffect(() => {
    let alive = true;
    let busy = false;
    const tick = () => {
      if (busy || document.visibilityState !== "visible") return;
      busy = true;
      api
        .ownersUsage(true)
        .then((v) => alive && setList(v))
        .catch(() => {})
        .finally(() => (busy = false));
    };
    tick();
    const t = window.setInterval(tick, everyMs);
    document.addEventListener("visibilitychange", tick);
    return () => {
      alive = false;
      window.clearInterval(t);
      document.removeEventListener("visibilitychange", tick);
    };
  }, [everyMs]);
  return list;
}

export function UsageLabel({ usage }: { usage: OwnerUsage | undefined }) {
  if (!usage || usage.processes === 0) return null;
  const title = `${usage.processes} processo(s): agente, filhos e servidores`;
  return (
    <span className="srv-usage" title={title}>
      CPU {usage.cpu.toFixed(0)}% · {fmtMem(usage.memory)}
      {usage.gpu !== null ? ` · GPU ${usage.gpu.toFixed(0)}%` : ""} · {usage.processes} proc.
    </span>
  );
}

/** Componente autônomo (ex.: cabeçalho da sessão). Cada instância faz o próprio poll. */
export function SessionUsage({ owner }: { owner: Owner }) {
  const list = useOwnersUsage();
  return <UsageLabel usage={list.find((u) => sameOwner(u.owner, owner))} />;
}
