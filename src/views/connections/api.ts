import { invoke } from "@tauri-apps/api/core";
import type { CommandSpec, Path } from "@/api";

/** Estado de uma CLI de serviço externo (nunca traz token). */
export interface Connection {
  id: "gh" | "gcloud" | "firebase" | "vercel" | string;
  label: string;
  installed: boolean;
  logged_in: boolean;
  account: string | null;
  error: string | null;
  /** Comando de login, para rodar no terminal embutido. */
  login: CommandSpec;
}

export interface RepoInfo {
  name_with_owner: string;
  url: string;
  default_branch: string | null;
  pull_requests: { number: number; title: string }[];
}

export const connectionsStatus = (): Promise<Connection[]> => invoke("connections_status");

/** Repositório GitHub do projeto (null se não for repo do GitHub). */
export const connectionsRepo = (project: Path): Promise<RepoInfo | null> =>
  invoke("connections_repo", { project });
