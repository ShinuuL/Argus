#!/usr/bin/env node
/**
 * Compara as fotos atuais das nove áreas contra o baseline congelado.
 *
 *   node scripts/comparar-telas.mjs
 *
 * Lê `docs/telas/base/<n>-<area>.png` (o "antes") e `docs/telas/<n>-<area>.png`
 * (o "depois"). Para cada par calcula SSIM e, quando mudou, grava uma imagem de
 * diferença em `docs/telas/diff/`. O relatório sai em `docs/telas/comparacao.md`.
 *
 * SSIM mede semelhança estrutural, não qualidade: 1,0 quer dizer "idêntico ao
 * pixel", 0,97 quer dizer "mudou". Ele não diz se a mudança ficou melhor — para
 * isso existe a foto de diferença, que mostra onde olhar, e a crítica de design.
 */
import { execFile } from "node:child_process";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { promisify } from "node:util";

const exec = promisify(execFile);
const BASE = "docs/telas/base";
const ATUAL = "docs/telas";
const DIF = "docs/telas/diff";
/**
 * Duas faixas, porque um SSIM de 0,998 não quer dizer "a tela mudou": relógio,
 * player e cursor ocupam poucos pixels e derrubam o índice sem tocar no layout.
 * Abaixo de 0,9995 é ruído; abaixo de 0,995 é mudança que vale uma foto de diff.
 */
const LIMIAR_RUIDO = 0.9995;
const LIMIAR_MUDOU = 0.995;

async function ssim(antes, depois) {
  try {
    const { stderr } = await exec("ffmpeg", ["-hide_banner", "-i", antes, "-i", depois, "-lavfi", "ssim", "-f", "null", "-"]);
    const valor = stderr.match(/All:([0-9.]+)/)?.[1];
    return valor ? Number(valor) : null;
  } catch (e) {
    return { erro: String(e.stderr || e.message).split("\n").slice(-2).join(" ").trim() };
  }
}

async function imagemDiff(antes, depois, destino) {
  await exec("ffmpeg", [
    "-y",
    "-loglevel",
    "error",
    "-i",
    antes,
    "-i",
    depois,
    "-filter_complex",
    "[0:v][1:v]blend=all_mode=difference,eq=contrast=2:brightness=0.05[out]",
    "-map",
    "[out]",
    "-frames:v",
    "1",
    destino,
  ]);
}

await mkdir(DIF, { recursive: true });
const arquivos = (await readdir(BASE)).filter((f) => f.endsWith(".png")).sort();
if (arquivos.length === 0) {
  console.error(`sem baseline em ${BASE}/ — rode a galeria de fotos antes de comparar`);
  process.exit(2);
}

const linhas = [];
let mudou = 0;
let falhou = 0;
const valores = [];

for (const arquivo of arquivos) {
  const antes = join(BASE, arquivo);
  const depois = join(ATUAL, arquivo);
  let atual;
  try {
    await readFile(depois);
    atual = true;
  } catch {
    atual = false;
  }
  if (!atual) {
    linhas.push(`| ${basename(arquivo, ".png")} | — | sem foto nova | — |`);
    falhou++;
    continue;
  }

  const resultado = await ssim(antes, depois);
  if (typeof resultado === "object") {
    linhas.push(`| ${basename(arquivo, ".png")} | — | erro: ${resultado.erro.replaceAll("|", "\\|")} | — |`);
    falhou++;
    continue;
  }
  valores.push(resultado);
  const estado = resultado >= LIMIAR_RUIDO ? "igual" : resultado >= LIMIAR_MUDOU ? "ruído" : "mudou";
  let diff = "—";
  if (estado === "mudou") {
    const alvo = join(DIF, arquivo);
    try {
      await imagemDiff(antes, depois, alvo);
      diff = `[diff](diff/${arquivo})`;
      mudou++;
    } catch (e) {
      diff = `falhou: ${String(e.message).split("\n")[0]}`;
      falhou++;
    }
  }
  linhas.push(`| ${basename(arquivo, ".png")} | ${resultado.toFixed(4)} | ${estado} | ${diff} |`);
}

const media = valores.length ? valores.reduce((a, b) => a + b, 0) / valores.length : 0;
const cabecalho = [
  "# Comparação das telas",
  "",
  `Baseline: \`${BASE}/\` · Atual: \`${ATUAL}/\` · gerado em ${new Date().toISOString().slice(0, 10)}.`,
  "",
  `SSIM médio: **${media.toFixed(4)}**. ${mudou} de ${arquivos.length} telas mudaram de verdade`,
  `(abaixo de ${LIMIAR_MUDOU}); o resto é igual ou ruído de relógio/player.`,
  "",
  "O número diz *se* houve mudança; a imagem de diferença diz *onde*. Um SSIM baixo não é",
  "nota: trocar um tema ou abrir uma pasta muda muito a foto sem ser regressão.",
  "",
  "| Tela | SSIM | Resultado | Diferença |",
  "|---|---:|---|---|",
  ...linhas,
  "",
];
await writeFile("docs/telas/comparacao.md", cabecalho.join("\n"));

console.log(cabecalho.slice(0, 6).join("\n"));
console.log(`\nrelatório: docs/telas/comparacao.md`);
process.exit(falhou > 0 ? 1 : 0);
