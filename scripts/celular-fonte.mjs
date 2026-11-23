/*
 * Controle para conferir a Geist no aparelho: mostra a mesma frase da web app
 * em `system-ui` (a fonte do fallback). Se a tela do Argus desenhar diferente
 * desta, a Geist carregou de verdade no celular.
 *
 * Uso: node scripts/celular-fonte.mjs [porta]
 */
import { createServer } from "node:http";

const porta = Number(process.argv[2] ?? 8767);
const frase = "conferindo a web app no celular";

const pagina = `<!doctype html>
<html lang="pt-BR"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>fallback</title>
<style>
  body { background:#0d1b2a; color:#e0e1dd; margin:0; padding:24px;
         font:15px/1.45 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }
  p.g { font-size:21px; font-weight:650; }
</style></head>
<body>
<p class="g">${frase}</p>
<p>projeto · Roteiro · modelo padrão · direto</p>
<p>175,2 mil tokens · US$ 0,51 equivalente em API</p>
</body></html>`;

createServer((_req, res) => {
  res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" });
  res.end(pagina);
}).listen(porta, "127.0.0.1", () => console.log(`fallback em http://localhost:${porta}/`));
