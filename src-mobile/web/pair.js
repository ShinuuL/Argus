/**
 * Tela de pareamento do Argus Celular.
 *
 * O trabalho dela é um só: guardar o endereço de pareamento e mandar a webview
 * para o servidor do Argus no computador. A partir daí quem desenha tudo é a
 * interface servida por ele (`crates/core/assets/remote`).
 *
 * A navegação usa `location.assign` de propósito: a tela de pareamento continua
 * no histórico, e é para cá que o botão "Trocar de servidor" dos Ajustes volta.
 */

const KEY = "argus.celular.url";
/** Marca de "esta tela já mandou a webview para o computador" (ver `voltouDoServidor`). */
const SAIU = "argus.celular.saiu";
const form = document.getElementById("form");
const campo = document.getElementById("link");
const erro = document.getElementById("erro");
const ok = document.getElementById("ok");
const ultimo = document.getElementById("ultimo");
const voltar = document.getElementById("voltar");
const esquecer = document.getElementById("esquecer");

function mostraErro(texto) {
  erro.textContent = texto;
  erro.hidden = !texto;
}

/**
 * Normaliza o que a pessoa colou e devolve a URL de pareamento, ou `null` com o
 * motivo. Aceita o link inteiro do QR code ou só "endereço token".
 */
function pairUrl(texto) {
  const bruto = String(texto || "").trim();
  if (!bruto) return { erro: "Cole o link que aparece embaixo do QR code." };
  // "192.168.0.10:8787 abcdef" (endereço e token separados por espaço)
  const partes = bruto.split(/\s+/);
  let alvo = partes[0];
  const tokenSolto = partes[1] || "";
  if (!/^https?:\/\//i.test(alvo)) alvo = "http://" + alvo;
  let url;
  try {
    url = new URL(alvo);
  } catch (_) {
    return { erro: "Endereço inválido. Confira o que foi colado." };
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return { erro: "O endereço precisa começar com http:// ou https://." };
  }
  const token = url.searchParams.get("token") || tokenSolto;
  if (!token) return { erro: "Falta o token no link. Use o endereço completo do QR code." };
  const saida = new URL("/pair", url.origin);
  saida.searchParams.set("token", token);
  // Marca que quem pareou foi o aplicativo: os Ajustes mostram "Trocar de servidor".
  saida.searchParams.set("app", "1");
  return { url: saida.toString() };
}

/** Confere se o computador responde antes de sair desta tela. */
async function alcancavel(url) {
  const origem = new URL(url).origin;
  const corta = new AbortController();
  const prazo = setTimeout(() => corta.abort(), 6000);
  try {
    // Resposta opaca (outra origem): só interessa saber que a porta respondeu.
    await fetch(origem + "/icon.svg", { mode: "no-cors", cache: "no-store", signal: corta.signal });
    return true;
  } catch (_) {
    return false;
  } finally {
    clearTimeout(prazo);
  }
}

async function conectar(entrada, guardar) {
  const { url, erro: motivo } = pairUrl(entrada);
  if (!url) {
    mostraErro(motivo);
    return;
  }
  mostraErro("");
  ok.disabled = true;
  ok.textContent = "Conectando…";
  if (!(await alcancavel(url))) {
    ok.disabled = false;
    ok.textContent = "Conectar";
    mostraErro("O computador não respondeu. Confira se o servidor do Argus está ligado e se o aparelho está na mesma rede.");
    return;
  }
  if (guardar) {
    try {
      localStorage.setItem(KEY, url);
    } catch (_) {
      /* sem armazenamento: vale só nesta abertura */
    }
  }
  // Marca a saída: se a pessoa voltar para cá ("Trocar de servidor"), a tela
  // fica parada em vez de reconectar sozinha e prender o aparelho num laço.
  try {
    sessionStorage.setItem(SAIU, "1");
  } catch (_) {
    /* sem armazenamento: só perde a proteção do laço */
  }
  location.assign(url);
}

/** Voltou do servidor para cá: não reconecta sozinho desta vez. */
function voltouDoServidor() {
  try {
    if (sessionStorage.getItem(SAIU) !== "1") return false;
    sessionStorage.removeItem(SAIU);
    return true;
  } catch (_) {
    return false;
  }
}

form.addEventListener("submit", (e) => {
  e.preventDefault();
  conectar(campo.value, true);
});
campo.addEventListener("input", () => mostraErro(""));

const guardado = (() => {
  try {
    return localStorage.getItem(KEY) || "";
  } catch (_) {
    return "";
  }
})();

if (guardado) {
  ultimo.hidden = false;
  voltar.textContent = new URL(guardado).host;
  voltar.addEventListener("click", () => conectar(guardado, false));
  esquecer.addEventListener("click", () => {
    try {
      localStorage.removeItem(KEY);
    } catch (_) {
      /* nada a fazer */
    }
    ultimo.hidden = true;
    campo.focus();
  });
  // Abertura normal do aplicativo: volta direto para o computador já pareado.
  // Se a pessoa veio do próprio Argus ("Trocar de servidor"), fica aqui.
  if (voltouDoServidor()) campo.focus();
  else conectar(guardado, false);
} else {
  campo.focus();
}
