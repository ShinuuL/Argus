#!/usr/bin/env node
/**
 * Fotografa a galeria de estados: cada tela em vazio, carregando, erro e cheio.
 *
 * `telas-print.mjs` fotografa o app como ele está — o que, com nenhuma pasta
 * aberta, é sempre a tela vazia na janela larga e no tema padrão. Este aqui
 * percorre os cenários de `src/dev/cenarios.ts`, que fabricam as respostas do
 * backend e levam as telas de verdade aos estados que ninguém vê.
 *
 *   $env:ARGUS_PONTE = "1"; npm run app:dev      # numa janela
 *   node scripts/galeria.mjs                     # noutra
 *   node scripts/galeria.mjs metricas-cheio      # só os cenários que casam
 *
 * Sai em `docs/estados/<id>.png`. Ao terminar, desliga a galeria e devolve a
 * janela ao normal — mesmo se algum cenário falhar.
 *
 * Por que é lento (~6 s por cenário): trocar de cenário **recarrega a página**,
 * de propósito (ver src/dev/galeria.ts). Vinte e dois cenários dão uns dois
 * minutos. É o preço de fotografar a tela real em vez de uma imitação dela.
 */
import { execFile } from "node:child_process";
import { mkdir } from "node:fs/promises";
import { promisify } from "node:util";

const exec = promisify(execFile);
const PORTA = process.env.ARGUS_PONTE && /^\d+$/.test(process.env.ARGUS_PONTE) ? Number(process.env.ARGUS_PONTE) : 8787;
const SAIDA = "docs/estados";
const ALTURA = 900;
const LARGURA_PADRAO = 1400;
/** Teto da espera pela janela voltar depois do reload. */
const VOLTA_MS = 15_000;

const filtro = process.argv[2] ?? null;

/**
 * Um pedido à ponte, com tempo limite próprio.
 *
 * O `tempo` importa durante a recarga: a ponte só devolve "a janela não
 * respondeu" depois de **30 s**, então uma sondagem sem corte fica presa nesse
 * prazo e queima sozinha a janela de espera inteira. Com 2 s, a sondagem falha
 * rápido e tenta de novo — que é o comportamento certo para esperar algo subir.
 */
const ponte = async (corpo, tempo = 35_000) => {
  try {
    const r = await fetch(`http://127.0.0.1:${PORTA}/cmd`, {
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

/**
 * Espera a página voltar do reload.
 *
 * Enquanto o React não montou, a ponte não existe e o pedido falha — então a
 * falha aqui é o estado normal da espera, não um erro. Só depois de `ver`
 * responder é que dá para fotografar.
 */
async function esperarVolta() {
  // Antes de sondar, deixar a recarga começar: a página velha ainda responde
  // por uns instantes, e uma sondagem que a alcança devolve a tela anterior
  // como se fosse a nova.
  await dormir(1200);
  const fim = Date.now() + VOLTA_MS;
  while (Date.now() < fim) {
    const r = await ponte({ acao: "ver" }, 2000);
    if (!r.erro && r.resultado) return r.resultado;
    await dormir(400);
  }
  return null;
}

async function fotografar(arquivo, moldura) {
  const recorte = moldura ? ["-vf", `crop=${moldura.largura}:${moldura.altura}:${moldura.x}:${moldura.y}`] : [];
  await exec("ffmpeg", ["-y", "-loglevel", "error", "-f", "gdigrab", "-framerate", "1", "-i", "desktop", ...recorte, "-frames:v", "1", arquivo]);
}

await mkdir(SAIDA, { recursive: true });

const lista = await ponte({ acao: "cenarios" });
if (lista.erro) {
  console.error(lista.erro);
  console.error('suba o Argus com a ponte: $env:ARGUS_PONTE = "1"; npm run app:dev');
  process.exit(2);
}

const cenarios = lista.resultado.filter((c) => !filtro || c.id.includes(filtro));
if (cenarios.length === 0) {
  console.error(`nenhum cenário casa com "${filtro}". Disponíveis:`);
  for (const c of lista.resultado) console.error(`  ${c.id}`);
  process.exit(1);
}

let falhas = 0;
for (const cen of cenarios) {
  const trocou = await ponte({ acao: "cenario", cenario: cen.id });
  if (trocou.erro) {
    console.log(`✘ ${cen.id}: ${trocou.erro}`);
    falhas++;
    continue;
  }
  const visao = await esperarVolta();
  if (!visao) {
    console.log(`✘ ${cen.id}: a janela não voltou em ${VOLTA_MS / 1000} s`);
    falhas++;
    continue;
  }
  // Mesma conferência de `telas-print.mjs`, pelo mesmo motivo: sem ela um
  // cenário que não montou rende uma foto da tela anterior com nome de nova.
  if (visao.area !== cen.area) {
    console.log(`✘ ${cen.id}: esperava a área "${cen.area}", achei "${visao.area ?? "?"}"`);
    falhas++;
    continue;
  }
  const posto = await ponte({ acao: "janela", largura: cen.largura ?? LARGURA_PADRAO, altura: ALTURA });
  // O "carregando" de algumas telas só aparece depois de um quadro; e a troca
  // de área tem transição. 700 ms cobre as duas sem esperar à toa.
  await dormir(700);
  const arquivo = `${SAIDA}/${cen.id}.png`;
  try {
    await fotografar(arquivo, posto.resultado?.moldura);
    console.log(`✔ ${arquivo}  ${cen.rotulo}`);
  } catch (e) {
    console.log(`✘ ${cen.id}: ffmpeg não fotografou — ${e.message.split("\n")[0]}`);
    falhas++;
  }
}

// Desligar a galeria importa mais que as fotos: a janela ficaria com o backend
// fabricado até alguém fechá-la, e quem a encontrasse assim veria dados falsos
// sem nenhum aviso na tela.
await ponte({ acao: "cenario", cenario: "" });
await esperarVolta();
await ponte({ acao: "janela", fixar: false });

console.log(falhas === 0 ? `\n${cenarios.length} estados em ${SAIDA}/` : `\n${falhas} de ${cenarios.length} falharam`);
process.exit(falhas === 0 ? 0 : 1);
