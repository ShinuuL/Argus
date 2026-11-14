/**
 * Contrato da interface com o backend do Argus.
 *
 *   import { api, onOrch, type Task } from "@/api";
 *   const tasks = await api.tasksList();
 *
 * - `commands.ts`: uma função por comando Tauri (também agrupadas em `api`).
 * - `events.ts`:   assinatura dos eventos (`onOrch`, `onPtyOutput`, `onProjectChanged`...).
 * - `types.ts`:    tipos que espelham o serde do Rust.
 * - `util.ts`:     rótulos, formatação e leitura de enums.
 */
import * as commands from "./commands";

export * from "./types";
export * from "./commands";
export * from "./events";
export * from "./util";
export { isTauri } from "@tauri-apps/api/core";

/** Todos os comandos num objeto só (`api.taskCreate(...)`). */
export const api = commands;
