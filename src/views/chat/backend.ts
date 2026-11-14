/**
 * Comandos Tauri próprios do chat (src-tauri/src/commands/extra_chat.rs).
 *
 * Eles ainda podem não estar registrados no backend: cada função aqui degrada
 * sem quebrar a tela (lista vazia, ou erro em português só onde o usuário pediu
 * a ação). Quando o integrador mover estas funções para `src/api`, este arquivo
 * vira um reexport.
 */
import { invoke } from "@tauri-apps/api/core";
import { errorMessage, isTauri, type Path, type TaskId } from "@/api";

export interface ChatAgent {
  name: string;
  description: string;
  /** "claude" ou "opencode". */
  provider: string;
  /** "projeto" ou "usuario". */
  scope: string;
}

export interface SavedAttachment {
  path: Path;
  size: number;
}

export interface AttachmentData {
  mime: string;
  /** Conteúdo em base64. */
  data: string;
}

export interface AgentCapabilities {
  /** Comandos de barra da CLI, sem a "/". */
  slash_commands: string[];
  mcp_servers: string[];
  model: string | null;
}

/** Comandos que o backend disse não conhecer (não adianta chamar de novo). */
const missing = new Set<string>();

function isMissing(e: unknown): boolean {
  return /not found|não encontrado|not allowed/i.test(errorMessage(e)) && /command/i.test(errorMessage(e));
}

/** Chama o comando; `null` quando não há backend ou o comando ainda não existe. */
async function call<T>(command: string, args: Record<string, unknown>): Promise<T | null> {
  if (!isTauri() || missing.has(command)) return null;
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    if (isMissing(e)) {
      missing.add(command);
      return null;
    }
    throw e;
  }
}

/** Arquivos de `workdir` que casam com a busca (caminhos relativos com "/"), já ranqueados. */
export async function chatProjectFiles(workdir: Path, query: string, limit = 30): Promise<string[]> {
  try {
    return (await call<string[]>("chat_project_files", { project: workdir, query, limit })) ?? [];
  } catch {
    return [];
  }
}

const agentsCache = new Map<string, Promise<ChatAgent[]>>();

/** Agentes definidos em arquivos do projeto e do usuário (uma consulta por pasta). */
export function chatAgents(project: Path | null): Promise<ChatAgent[]> {
  const key = project ?? "";
  let p = agentsCache.get(key);
  if (!p) {
    p = call<ChatAgent[]>("chat_agents", { project })
      .then((r) => r ?? [])
      .catch(() => []);
    agentsCache.set(key, p);
  }
  return p;
}

/** Grava um anexo na pasta de anexos do Argus. Rejeita com a mensagem do motivo. */
export async function chatAttachmentSave(task: TaskId | null, name: string, base64: string): Promise<SavedAttachment> {
  const saved = await call<SavedAttachment>("chat_attachment_save", { task, name, data: base64 });
  if (!saved) throw new Error("A gravação de anexos ainda não está disponível nesta versão.");
  return saved;
}

/** Lê uma imagem da pasta de anexos como data URL; `null` se não der. */
export async function chatAttachmentRead(path: Path): Promise<string | null> {
  try {
    const d = await call<AttachmentData>("chat_attachment_read", { path });
    return d ? `data:${d.mime};base64,${d.data}` : null;
  } catch {
    return null;
  }
}

let cleaned = false;
/** Apaga anexos com mais de 1 dia (uma vez por execução do app). */
export function chatAttachmentsCleanup(): void {
  if (cleaned) return;
  cleaned = true;
  void call<number>("chat_attachments_cleanup", {}).catch(() => undefined);
}

const capsCache = new Map<string, Promise<AgentCapabilities | null>>();

/**
 * Comandos de barra que a CLI oferece em `workdir`. Só o claude informa; a
 * consulta sobe a CLI por alguns segundos e não gasta tokens. Uma vez por pasta.
 */
export function chatAgentCapabilities(workdir: Path, provider: string): Promise<AgentCapabilities | null> {
  if (provider !== "claude") return Promise.resolve(null);
  let p = capsCache.get(workdir);
  if (!p) {
    p = call<AgentCapabilities>("chat_agent_capabilities", { project: workdir, provider }).catch(() => {
      // falhou (CLI ausente, tempo esgotado): deixa tentar de novo depois
      capsCache.delete(workdir);
      return null;
    });
    capsCache.set(workdir, p);
  }
  return p;
}
