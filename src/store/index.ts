/**
 * Stores (zustand) compartilhados pelas telas:
 *
 * - `useApp`:        raiz, projetos, status git, tarefas, auto-work, servidores, uso, instalação pendente.
 * - `useLayout`:     barra esquerda, painel direito (abas internas) e painel inferior.
 * - `useWorkbench`:  grupos e abas da área central (conversa, arquivo, terminal, painel e telas do rodapé).
 * - `useTerminals`:  abas do painel inferior de terminais.
 * - `useTheme`:      tema de cores (Oceano, Nebulosa, Brasa).
 *
 * Fora de componentes, use `useApp.getState()` / `useWorkbench.getState()`.
 */
export * from "./app";
export * from "./layout";
export * from "./workbench";
export * from "./terminals";
export * from "./theme";
export { forgetSession, startApp } from "./bootstrap";
