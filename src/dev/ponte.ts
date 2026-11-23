/**
 * Ponte de validação, lado da janela (o lado Rust fica em src-tauri/src/ponte.rs).
 *
 * Existe porque, de fora, ninguém consegue operar o Argus: o WebView2 ignora
 * tanto `SendKeys` quanto clique sintético do Win32. Então a ponte age de
 * dentro — um `elemento.click()` daqui passa pelos mesmos ouvintes de um clique
 * humano, e o React não sabe a diferença.
 *
 * Só é montada quando `ponte_ligada()` diz que sim, isto é, quando o Argus
 * subiu com `ARGUS_PONTE`. Sem a variável este arquivo não registra nada.
 *
 * Ações (ver `scripts/ui.mjs`):
 *
 * | ação      | argumentos            | devolve                                   |
 * |-----------|-----------------------|-------------------------------------------|
 * | `ver`     | —                     | título, área, texto visível e o que é clicável |
 * | `clicar`  | `texto` ou `seletor`  | o que foi clicado                          |
 * | `digitar` | `texto`, `seletor?`   | o valor que ficou no campo                 |
 * | `tecla`   | `tecla`, `ctrl`…      | —                                          |
 * | `esperar` | `texto`, `ms?`        | quanto tempo levou                         |
 * | `janela`  | `largura?`, `altura?` | traz à frente (e redimensiona) para fotografar |
 * | `cenario` | `cenario` (ou vazio)  | monta um estado fabricado e recarrega      |
 * | `cenarios`| —                     | a lista de cenários da galeria             |
 * | `erros`   | —                     | erros de console desde que a janela abriu  |
 * | `js`      | `codigo`              | o valor da expressão                       |
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { aplicarCenario } from "./galeria";
import { CENARIOS } from "./cenarios";

const EVENTO = "argus://ponte";
/** Teto do texto devolvido por `ver`: o resto é rolagem que ninguém lê. */
const TEXTO_MAX = 4000;
/** Teto da lista de clicáveis. */
const CLICAVEIS_MAX = 80;
const ESPERA_PADRAO_MS = 10_000;

/** Erros de console desde que a janela abriu (a ação `erros` lê daqui). */
const erros: { quando: number; texto: string }[] = [];

type Pedido = { id: number; acao?: string; [k: string]: unknown };

/** Elemento está à vista? `offsetParent` some com `display:none` e com os pais escondidos. */
function visivel(el: Element): boolean {
  const e = el as HTMLElement;
  return e.getClientRects().length > 0 && getComputedStyle(e).visibility !== "hidden";
}

/** Rótulo de um elemento, na ordem em que uma pessoa o leria. */
function rotulo(el: Element): string {
  const e = el as HTMLElement;
  const texto = e.getAttribute("aria-label") || e.getAttribute("title") || e.innerText || (e as HTMLInputElement).placeholder || "";
  return texto.replace(/\s+/g, " ").trim();
}

const SELETOR_CLICAVEL = 'button, a[href], summary, input, textarea, select, [role="button"], [role="tab"], [role="menuitem"], [role="option"], [data-ponte]';

function clicaveis(): { rotulo: string; tag: string; marca: string | null }[] {
  return Array.from(document.querySelectorAll(SELETOR_CLICAVEL))
    .filter(visivel)
    .map((el) => ({ rotulo: rotulo(el), tag: el.tagName.toLowerCase(), marca: el.getAttribute("data-ponte") }))
    .filter((c) => c.rotulo || c.marca)
    .slice(0, CLICAVEIS_MAX);
}

/**
 * Acha o alvo: `data-ponte` exato primeiro, depois rótulo exato, depois pedaço.
 *
 * A ordem importa. Procurar por pedaço primeiro faz "Salvar" cair no botão
 * "Salvar e fechar" que estiver mais acima na página — e o teste passa clicando
 * no lugar errado, que é pior do que falhar.
 */
function achar(texto: string): Element | null {
  const alvos = Array.from(document.querySelectorAll(SELETOR_CLICAVEL)).filter(visivel);
  const alvo = texto.trim().toLowerCase();
  return (
    alvos.find((el) => el.getAttribute("data-ponte")?.toLowerCase() === alvo) ??
    alvos.find((el) => rotulo(el).toLowerCase() === alvo) ??
    alvos.find((el) => rotulo(el).toLowerCase().includes(alvo)) ??
    null
  );
}

/** Escreve num campo controlado pelo React (o setter nativo + o evento que ele escuta). */
function escrever(campo: HTMLInputElement | HTMLTextAreaElement, texto: string): void {
  const proto = campo instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  Object.getOwnPropertyDescriptor(proto, "value")?.set?.call(campo, texto);
  campo.dispatchEvent(new Event("input", { bubbles: true }));
  campo.dispatchEvent(new Event("change", { bubbles: true }));
}

const dormir = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function executar(p: Pedido): Promise<unknown> {
  switch (p.acao) {
    case "ver":
      return {
        titulo: document.title,
        area: document.querySelector("[data-area]")?.getAttribute("data-area") ?? null,
        texto: (document.body.innerText || "").replace(/\n{3,}/g, "\n\n").slice(0, TEXTO_MAX),
        clicaveis: clicaveis(),
      };

    case "clicar": {
      const el = p.seletor ? document.querySelector(String(p.seletor)) : achar(String(p.texto ?? ""));
      if (!el) throw new Error(`não achei "${p.seletor ?? p.texto}" na tela`);
      (el as HTMLElement).scrollIntoView({ block: "center" });
      (el as HTMLElement).click();
      // Um quadro para o React responder antes de a resposta voltar.
      await dormir(50);
      return { clicou: rotulo(el) || String(p.seletor), tag: el.tagName.toLowerCase() };
    }

    case "digitar": {
      const el = p.seletor ? document.querySelector(String(p.seletor)) : document.activeElement;
      if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) {
        throw new Error("o alvo não é um campo de texto");
      }
      el.focus();
      escrever(el, String(p.texto ?? ""));
      await dormir(50);
      return { valor: el.value };
    }

    case "tecla": {
      const init: KeyboardEventInit = {
        key: String(p.tecla ?? ""),
        ctrlKey: !!p.ctrl,
        shiftKey: !!p.shift,
        altKey: !!p.alt,
        bubbles: true,
      };
      const alvo = document.activeElement ?? document.body;
      alvo.dispatchEvent(new KeyboardEvent("keydown", init));
      alvo.dispatchEvent(new KeyboardEvent("keyup", init));
      await dormir(50);
      return { tecla: init.key };
    }

    case "esperar": {
      const alvo = String(p.texto ?? "").toLowerCase();
      const limite = Number(p.ms ?? ESPERA_PADRAO_MS);
      const comeco = Date.now();
      while (Date.now() - comeco < limite) {
        if ((document.body.innerText || "").toLowerCase().includes(alvo)) return { ms: Date.now() - comeco };
        await dormir(120);
      }
      throw new Error(`"${p.texto}" não apareceu em ${limite} ms`);
    }

    /**
     * Traz a janela para a frente, com o tamanho pedido.
     *
     * É para fotografar: o `gdigrab` captura a *região da tela* onde a janela
     * está, então qualquer coisa por cima entra no retrato. Pedir de fora não
     * funciona (o Windows barra quem não está em primeiro plano), mas o
     * aplicativo pode trazer a si mesmo — e esse caminho o WebView2 respeita.
     */
    case "janela": {
      const w = getCurrentWindow();
      // Cada passo por conta própria: num perfil sem a permissão da janela o
      // que vale é saber qual faltou, não perder a ação inteira.
      const feito: Record<string, string> = {};
      const tentar = async (nome: string, fn: () => Promise<unknown>) => {
        try {
          await fn();
          feito[nome] = "ok";
        } catch (e) {
          feito[nome] = String(e instanceof Error ? e.message : e);
        }
      };
      if (p.largura && p.altura) {
        await tentar("tamanho", () => w.setSize(new LogicalSize(Number(p.largura), Number(p.altura))));
      }
      await tentar("restaurar", () => w.unminimize());
      await tentar("frente", () => w.setFocus());
      // `setFocus` não basta para fotografar: o `gdigrab` copia o que está
      // composto na tela, e uma janela coberta devolve o último quadro que teve
      // — foi assim que saíram dez retratos da mesma tela com o DOM já noutra.
      // "Sempre à frente" é a única forma que o Windows aceita de um aplicativo
      // se pôr por cima sem o usuário clicar. `fixar: false` desfaz.
      await tentar("topo", () => w.setAlwaysOnTop(p.fixar !== false));
      await dormir(250);
      // Onde a janela está, em pixels de verdade: quem fotografa precisa
      // recortar a tela inteira nessa moldura (ver scripts/telas-print.mjs).
      let moldura: Record<string, number> | null = null;
      await tentar("moldura", async () => {
        const pos = await w.outerPosition();
        const fora = await w.outerSize();
        const dentro = await w.innerSize();
        // O retângulo de fora inclui a borda invisível do Windows 11 (a faixa
        // de redimensionar, uns 8 px de cada lado): recortar por ele deixava
        // entrar uma tira da janela de trás. A área de dentro é o que se vê.
        const borda = Math.max(0, Math.round((fora.width - dentro.width) / 2));
        moldura = { x: pos.x + borda, y: pos.y, largura: dentro.width, altura: dentro.height };
      });
      return { ...feito, moldura };
    }

    /**
     * Troca o cenário da galeria de estados (ver `src/dev/galeria.ts`).
     *
     * O argumento é `cenario`, não `id`: `id` já é o número do pedido da ponte.
     * Vazio desliga e volta ao backend de verdade. A página recarrega logo
     * depois desta resposta, então quem chamou precisa esperar a janela voltar
     * antes do próximo pedido — `scripts/galeria.mjs` faz isso.
     */
    case "cenario": {
      const alvo = p.cenario == null || p.cenario === "" ? null : String(p.cenario);
      if (!aplicarCenario(alvo)) throw new Error(`cenário desconhecido: ${p.cenario}`);
      return { cenario: alvo, recarregando: true };
    }

    case "cenarios":
      return CENARIOS.map((c) => ({ id: c.id, rotulo: c.rotulo, area: c.area, tema: c.tema ?? null, largura: c.largura ?? null, esperar: c.esperar ?? [] }));

    case "erros":
      return erros.slice(-50);

    case "js":
      // eslint-disable-next-line no-new-func
      return await new Function(`return (${String(p.codigo ?? "null")})`)();

    default:
      throw new Error(`ação desconhecida: ${p.acao ?? "(nenhuma)"}`);
  }
}

/**
 * Liga a ponte, se o backend disser que ela existe. Devolve a função que desliga.
 *
 * Erro de uma ação nunca sobe: ele volta como `{ erro }` no corpo da resposta.
 * Se subisse, o servidor ficaria esperando os 30 s do tempo limite para dizer a
 * mesma coisa com menos informação.
 */
export function startPonte(): () => void {
  let off: (() => void) | null = null;
  let vivo = true;
  void (async () => {
    if (!(await invoke<boolean>("ponte_ligada").catch(() => false)) || !vivo) return;
    const guardaErro = (texto: string) => erros.push({ quando: Date.now(), texto: texto.slice(0, 500) });
    const onErro = (e: ErrorEvent) => guardaErro(`${e.message} (${e.filename}:${e.lineno})`);
    const onRejeicao = (e: PromiseRejectionEvent) => guardaErro(`promessa rejeitada: ${String(e.reason)}`);
    window.addEventListener("error", onErro);
    window.addEventListener("unhandledrejection", onRejeicao);
    const consoleErro = console.error;
    console.error = (...args: unknown[]) => {
      guardaErro(args.map(String).join(" "));
      consoleErro(...args);
    };
    const parar = await listen<Pedido>(EVENTO, (e) => {
      const p = e.payload;
      void executar(p)
        .then((resultado) => invoke("ponte_resposta", { id: p.id, resultado: { ok: true, resultado } }))
        .catch((err: unknown) => invoke("ponte_resposta", { id: p.id, resultado: { erro: String(err instanceof Error ? err.message : err) } }));
    });
    console.info("ponte de validação ligada");
    off = () => {
      parar();
      window.removeEventListener("error", onErro);
      window.removeEventListener("unhandledrejection", onRejeicao);
      console.error = consoleErro;
    };
    if (!vivo) off();
  })();
  return () => {
    vivo = false;
    off?.();
  };
}
