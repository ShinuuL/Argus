/**
 * Galeria de estados: mostra as telas de verdade com dados fabricados.
 *
 * O problema que ela resolve: toda foto de `docs/telas/` era da tela **vazia**,
 * janela larga, tema padrão — porque é o estado em que o app abre. Carregando,
 * erro, lista cheia, janela estreita e os outros três temas nunca foram vistos,
 * e é onde interface costuma quebrar.
 *
 * Como funciona: um cenário (ver `cenarios.ts`) diz o que o backend responde a
 * cada comando, e a galeria embrulha o `invoke` que `src/api/commands.ts` usa.
 * Comando não mencionado passa direto para o backend real.
 *
 * **Por que não no `__TAURI_INTERNALS__`.** Era ali que isto ficava, por ser o
 * ponto mais baixo. Não dá: o Tauri 2 injeta `window.__TAURI_INTERNALS__` e o
 * `invoke` dentro dele como propriedades **não graváveis e não configuráveis**
 * — tanto a atribuição quanto o `defineProperty` jogam `TypeError`, e a galeria
 * caía inteira no `catch` enquanto o app subia normal. O sintoma era mudo: a
 * tela certa, com os dados de verdade e sem erro nenhum no console.
 *
 * O ponto de troca passou a ser o `invoke` importado em `src/api/commands.ts`,
 * que é por onde *toda* chamada da interface sai — `api.serversList()` e as
 * outras acabam ali. Continua sendo um ponto só, continua cobrindo as nove
 * áreas, e nenhuma tela sabe que a galeria existe: o custo é uma linha no
 * `commands.ts` e outra no `events.ts`, não um caminho de teste nos componentes.
 *
 * Três travas, todas necessárias para isto não ser um modo de produção:
 *
 *  1. o cenário só entra pela ação `cenario` da ponte, que por sua vez só
 *     existe com `ARGUS_PONTE` ligada — não há caminho pela interface;
 *  2. ele vive no `sessionStorage`, que morre junto com a janela;
 *  3. nada é gravado em arquivo nem no banco — só respostas de leitura mudam.
 *
 * Trocar de cenário **recarrega a página**, de propósito: as telas carregam no
 * `useEffect` de montagem, então mudar as respostas com o app em pé deixaria
 * metade da tela com o dado antigo — uma foto que não corresponde a estado
 * nenhum, que é pior que não ter foto.
 */
import { acharCenario, type Cenario, type Resposta } from "./cenarios";

const CHAVE = "argus.cenario";

type Invoke = (cmd: string, ...resto: never[]) => Promise<unknown>;

/** Cenário escolhido para esta janela, se houver. */
export function cenarioAtivo(): Cenario | undefined {
  try {
    const id = sessionStorage.getItem(CHAVE);
    return id ? acharCenario(id) : undefined;
  } catch {
    return undefined;
  }
}

/**
 * Guarda o cenário e recarrega. Devolve false se o id não existe.
 *
 * A recarga é adiada um quadro para a resposta da ponte sair antes: recarregar
 * na mesma linha mata a página com o `ponte_resposta` ainda na fila, e quem
 * chamou fica esperando os 30 s do tempo limite por uma troca que deu certo.
 */
export function aplicarCenario(id: string | null): boolean {
  if (id && !acharCenario(id)) return false;
  try {
    if (id) sessionStorage.setItem(CHAVE, id);
    else sessionStorage.removeItem(CHAVE);
  } catch {
    return false;
  }
  setTimeout(() => location.reload(), 50);
  return true;
}

function responder(r: Resposta): Promise<unknown> {
  if (r === "demora") return new Promise(() => {});
  if ("erro" in r) return Promise.reject(r.erro);
  // Um quadro de atraso para o "carregando" aparecer antes do dado, como numa
  // chamada de verdade: resposta síncrona faz a tela pular direto para o fim e
  // esconde o estado intermediário que a galeria existe para mostrar.
  return new Promise((ok) => setTimeout(() => ok(r.valor), 30));
}

/**
 * O `invoke` que `src/api/commands.ts` deve usar.
 *
 * Sem cenário devolve o próprio `real` — nem embrulho, nem custo, nem desvio:
 * fora da galeria o caminho é literalmente o mesmo de antes.
 *
 * Roda na avaliação do módulo, ou seja antes de qualquer `useEffect` de
 * montagem disparar seu comando — que é a única garantia de tempo de que isto
 * precisa.
 */
export function invokeDaGaleria<T extends Invoke>(real: T): T {
  const cen = cenarioAtivo();
  if (!cen) return real;
  // O genérico devolve o *mesmo* tipo que entrou: sem isto, as ~180 funções de
  // `commands.ts` perderiam o tipo de retorno e virariam `Promise<unknown>`.
  return ((cmd: string, ...resto: never[]) => {
    const r = cen.respostas[cmd];
    return r ? responder(r) : real(cmd, ...resto);
  }) as unknown as T;
}

/**
 * O `listen` que `src/api/events.ts` deve usar — cala os eventos do cenário.
 *
 * Fabricar a resposta do comando não basta: o backend continua empurrando a
 * realidade por evento, e segundos depois do boot um `servers-changed` com a
 * lista verdadeira (vazia) apaga a fixture. A primeira foto de "Servidores ·
 * cheio" saiu assim — a tela certa, com zero linhas.
 *
 * Devolve um `unlisten` que não faz nada, porque inscrição nenhuma houve.
 */
export function listenDaGaleria<T extends (...a: never[]) => Promise<unknown>>(real: T): T {
  const cen = cenarioAtivo();
  if (!cen?.calar?.length) return real;
  const nada = () => {};
  return ((evento: unknown, ...resto: never[]) =>
    typeof evento === "string" && cen.calar!.includes(evento)
      ? Promise.resolve(nada)
      : real(evento as never, ...resto)) as unknown as T;
}

/**
 * Marca visível de fora (ação `js` da ponte): sem ela, uma galeria que não
 * instalou é indistinguível de uma fixture que não chegou à tela, e os dois
 * defeitos se investigam em lugares bem diferentes.
 */
export function instalarGaleria(): void {
  const cen = cenarioAtivo();
  if (!cen) return;
  (window as unknown as Record<string, unknown>).__ARGUS_GALERIA__ = cen.id;
  console.info(`galeria de estados: cenário "${cen.id}"`);
}

/**
 * Põe a janela no estado que o cenário pede: área à vista, tema e, nos cenários
 * de Conversas, a aba da conversa aberta.
 *
 * Roda depois do primeiro render (a largura fica por conta de quem fotografa,
 * pela ação `janela` da ponte, que é quem sabe falar com o sistema).
 *
 * `abrirSessao` é idempotente (foca a aba que já existe), e é por isso que dá
 * para chamar isto de novo a cada mudança de `ready` sem encher a barra de abas
 * repetidas.
 */
export function montarCenario(
  cen: Cenario,
  irPara: (area: Cenario["area"]) => void,
  trocarTema: (t: NonNullable<Cenario["tema"]>) => void,
  abrirSessao?: (task: number) => void,
): void {
  if (cen.tema) trocarTema(cen.tema);
  irPara(cen.area);
  if (cen.sessao != null) abrirSessao?.(cen.sessao);
}
