#!/usr/bin/env node
/**
 * Valida a galeria de estados, não só fotografa.
 *
 *   $env:ARGUS_PONTE = "1"; npm run app:dev   # numa janela
 *   node scripts/validar-galeria.mjs          # noutra
 *
 * `galeria.mjs` prova que a foto saiu e que a área trocou. Ele não prova que a
 * fixture chegou à tela, que a galeria instalou ou que a página subiu sem erro:
 * um cenário que renderiza vazio, com o console cheio de exceção, passaria.
 * Aqui cada cenário carrega quatro assertivas:
 *
 *   1. a área da Shell é a declarada no cenário;
 *   2. `window.__ARGUS_GALERIA__` é o id do cenário (o embrulho do invoke instalou);
 *   3. cada item de `esperar` aparece na tela — texto em `innerText`, ou seletor
 *      CSS quando começa com `@` (ver `src/dev/cenarios.ts`);
 *   4. a página não registrou erro de console desde o reload.
 *
 * Saída: `docs/estados/validacao.json`. Código 1 se qualquer cenário falhar.
 */
import { mkdir, writeFile } from "node:fs/promises";

const PORTA = process.env.ARGUS_PONTE && /^\d+$/.test(process.env.ARGUS_PONTE) ? Number(process.env.ARGUS_PONTE) : 8787;
const BASE = `http://127.0.0.1:${PORTA}/cmd`;
const SAIDA = "docs/estados/validacao.json";
const VOLTA_MS = 15_000;

const ponte = async (corpo, tempo = 35_000) => {
  try {
    const r = await fetch(BASE, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(corpo),
      signal: AbortSignal.timeout(tempo),
    });
    return await r.json();
  } catch (e) {
    return { erro: `a ponte não respondeu (${e.message})` };
  }
};

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

/** Espera a página voltar do reload provocado pela troca de cenário. */
async function esperarVolta() {
  await dormir(1000);
  const fim = Date.now() + VOLTA_MS;
  while (Date.now() < fim) {
    const r = await ponte({ acao: "ver" }, 2000);
    if (!r.erro && r.resultado) return r.resultado;
    await dormir(350);
  }
  return null;
}

/**
 * Uma expressão única para todos os itens de `esperar`: menos idas e vindas à
 * ponte e, principalmente, um único instante de leitura da tela.
 */
function expressao(itens) {
  const lista = JSON.stringify(itens);
  return `(() => { const t = document.body.innerText || ''; return ${lista}.map(x => ({ item: x, ok: x.startsWith('@') ? !!document.querySelector(x.slice(1)) : t.includes(x) })); })()`;
}

const lista = await ponte({ acao: "cenarios" });
if (lista.erro) {
  console.error(lista.erro);
  console.error('suba o Argus com a ponte: $env:ARGUS_PONTE = "1"; npm run app:dev');
  process.exit(2);
}

const cenarios = lista.resultado;
const resultados = [];
let falhas = 0;

for (const cen of cenarios) {
  const inicio = Date.now();
  const problemas = [];
  const trocou = await ponte({ acao: "cenario", cenario: cen.id });
  if (trocou.erro) {
    problemas.push(`não aplicou: ${trocou.erro}`);
    resultados.push({ id: cen.id, ok: false, problemas });
    falhas++;
    console.log(`✘ ${cen.id}: ${problemas[0]}`);
    continue;
  }

  const visao = await esperarVolta();
  if (!visao) {
    problemas.push("a janela não voltou do reload");
    resultados.push({ id: cen.id, ok: false, problemas });
    falhas++;
    console.log(`✘ ${cen.id}: ${problemas[0]}`);
    continue;
  }
  if (visao.area !== cen.area) problemas.push(`área: esperava "${cen.area}", viu "${visao.area ?? "?"}"`);

  const ativo = await ponte({ acao: "js", codigo: "window.__ARGUS_GALERIA__ ?? null" });
  if (ativo.erro) problemas.push(`não leu a marca da galeria: ${ativo.erro}`);
  else if (ativo.resultado !== cen.id) problemas.push(`galeria: esperava "${cen.id}", viu ${JSON.stringify(ativo.resultado)}`);

  const erros = await ponte({ acao: "erros" });
  if (erros.erro) problemas.push(`não leu o console: ${erros.erro}`);
  else if ((erros.resultado ?? []).length) {
    for (const e of erros.resultado.slice(0, 3)) problemas.push(`console: ${e.texto}`);
  }

  const checagem = await ponte({ acao: "js", codigo: expressao(cen.esperar ?? []) });
  if (checagem.erro) problemas.push(`não checou o conteúdo: ${checagem.erro}`);
  else {
    for (const r of checagem.resultado ?? []) if (!r.ok) problemas.push(`faltou na tela: ${r.item}`);
  }

  const item = { id: cen.id, rotulo: cen.rotulo, area: visao.area, ok: problemas.length === 0, problemas, ms: Date.now() - inicio };
  resultados.push(item);
  if (item.ok) console.log(`✔ ${cen.id}  (${item.ms} ms)`);
  else {
    falhas++;
    console.log(`✘ ${cen.id}: ${problemas.join(" · ")}`);
  }
}

// Deixar a janela com dados fabricados é pior do que não ter validado nada.
await ponte({ acao: "cenario", cenario: "" });
await esperarVolta();

await mkdir("docs/estados", { recursive: true });
await writeFile(SAIDA, JSON.stringify({ gerado: new Date().toISOString(), total: cenarios.length, falhas, resultados }, null, 2) + "\n");
console.log(`\n${cenarios.length - falhas}/${cenarios.length} cenários passaram · ${SAIDA}`);
process.exit(falhas === 0 ? 0 : 1);
