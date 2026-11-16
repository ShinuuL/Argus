import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Configuração do Vite para o Tauri: porta fixa (a mesma do `devUrl` em
// src-tauri/tauri.conf.json) e sem observar as pastas do Rust.
export default defineConfig({
  plugins: [react()],
  // "@/..." aponta para src/ (mesmo mapeamento de `paths` no tsconfig.json).
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  clearScreen: false,
  server: {
    // ARGUS_VITE_PORT troca a porta (ex.: duas instâncias de dev ao mesmo
    // tempo); passe o mesmo valor em `devUrl` com `tauri dev --config`.
    port: Number(process.env.ARGUS_VITE_PORT) || 1420,
    strictPort: true,
    host: "127.0.0.1",
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] },
  },
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
    // Monaco e xterm são grandes por natureza; o app é local, sem download.
    chunkSizeWarningLimit: 4000,
  },
  worker: { format: "es" },
});
