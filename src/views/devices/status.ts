/** Estado do Screen-Mirror, compartilhado entre a área Dispositivos e a aba de espelhamento. */
import type { MirrorStatus } from "@/api";

export const STATE_LABEL: Record<string, string> = {
  device: "conectado",
  unauthorized: "autorize a depuração no celular",
  offline: "offline",
};

/** O que falta para o Screen-Mirror poder iniciar (null = pronto). */
export function missing(s: MirrorStatus): string | null {
  if (!s.dir) return "Screen-Mirror não encontrado. Informe a pasta do projeto abaixo.";
  if (!s.node) return "Node.js não encontrado no PATH. O Screen-Mirror precisa do Node 22.18 ou mais novo.";
  if (s.nodeTooOld) return `Node.js ${s.node} é antigo demais. Instale o 22.18 ou mais novo.`;
  if (s.needsInstall) return `Dependências não instaladas. Rode \`npm install\` em ${s.dir}.`;
  return null;
}
