# Argus — Estado atual e próximos passos

_Atualizado em 2026-11-26. Base: 2026-11-14, depois da migração da interface de Dioxus para Tauri 2, do redesenho do layout (chat no centro, temas) e de uma rodada de validação na janela com tarefas reais._

O Argus é uma IDE desktop que gerencia vários projetos de uma pasta raiz e orquestra agentes de IA (Claude Code, OpenCode, Codex e Gemini). A lógica fica em Rust (`argus-core`); a interface é React + TypeScript dentro de uma janela Tauri 2. Os agentes usam o login das próprias CLIs oficiais; a IDE nunca lê nem guarda tokens.

**Estado em uma frase:** tudo compila, os testes passam e quase tudo foi exercitado na janela em 2026-11-14 com tarefas reais do claude e do opencode (conversa nova, fila, anexos, diff, aplicar, descartar, auto-work, servidores, terminais, arrasto de abas). A coluna "Estado" da tabela abaixo diz o que foi conferido de verdade e o que ainda falta.

A conferência na janela foi feita pelo protocolo de depuração do WebView2 (cliques e teclas enviados à página, sem usar o mouse real). Para repetir: abra o executável com `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333` e conecte em `http://127.0.0.1:9333`.

- **Arrastar** (abas, arquivo da aba Arquivos): o arrasto começa de verdade na página (botão pressionado e movido dispara o `dragstart`, com o `DataTransfer` real) e é interceptado no ponto em que o navegador o entregaria ao sistema (`Input.setInterceptDrags`); os `dragenter`, `dragover` e `drop` seguintes entram por `Input.dispatchDragEvent`. Limite do protocolo: o laço de arrasto do Windows não roda, então um arquivo vindo do Explorer do Windows não era coberto. Conferido depois pelo usuário com o Explorer real em 2026-11-24.
- **Colar imagem:** evento `paste` sintético com um PNG no `DataTransfer`. Limite do protocolo: a área de transferência real do sistema (Ctrl+V depois de um print) não era coberta. Conferido depois pelo usuário com um print real em 2026-11-24.
- **Caixas nativas de confirmação** (Aplicar, Descartar, "Parar e enviar"): a página fica parada até a resposta; o teste respondeu mandando a mensagem de clique ao botão "Sim" da caixa.

Uso no dia a dia como o Claude Code no VS Code (equivalências e diferenças): [docs/USO_COMO_CLAUDE_CODE.md](docs/USO_COMO_CLAUDE_CODE.md).

## P5 — redesenhar as telas com evidência (proposto em 2026-11-21)

Até ontem uma crítica de design do Argus era chute: eu não via a tela. Com a ponte de validação (que navega) junto do `gdigrab` (que fotografa), passou a ser possível pegar os pixels de cada área sem ninguém clicando:

```powershell
$env:ARGUS_PONTE = "1"; npm run app:dev
node scripts/telas-print.mjs            # docs/telas/01-conversas.png … 10-agentes.png
```

A partir daí a crítica tem evidência, e aí sim vale rodar `/impeccable` ou `/design-critic` tela a tela. **A crítica das nove telas está pronta** em [docs/critica-telas.md](docs/critica-telas.md), e o resultado dela manda no resto deste plano: das 36 observações, **27 aparecem em quatro telas ou mais**. Não há dez redesenhos a fazer — há três componentes e uma paleta. O passo 2 da ordem abaixo deixou de ser aposta e virou o trabalho principal.

**Duas armadilhas que custaram caro para descobrir, e que o roteiro já evita:**

- **`gdigrab -i "title=Argus Dev"` devolve quadro velho com WebView2.** Saíram nove retratos da tela de Conversas com o DOM comprovadamente três telas adiante. O desenho da página não passa pelo DC da janela, então o que o `gdigrab` copia de lá está congelado no primeiro quadro. A saída é fotografar a **tela inteira** (`-i desktop`) e recortar a moldura da janela, que a ponte informa na ação `janela`. Essa ação também põe a janela "sempre à frente" durante as fotos (e desfaz no fim): janela coberta não aparece na tela composta.
- **Conferir texto não é conferir navegação.** O roteiro dava `✔ Agentes` para um clique que não tinha navegado para lugar nenhum — o rótulo bateu com um botão de outra tela e texto, claro, existia. Agora a `Shell` expõe `data-area` e o roteiro compara com a área esperada. (A área `agents` saiu das listas: ela não tem ícone na barra, abre pelo inspetor ou pela paleta.)

**A referência já existe e não deve ser reinventada.** Em 2026-11-17 foi aprovado o redesign de `docs/mockups/argus-telas.html` (12 telas, barra de ícones à esquerda, inspetor opcional, Ctrl+K para o que é raro) mais os três temas novos. O trabalho aqui é *aproximar o app do mockup aprovado*, com os ajustes que a prática mostrar — não abrir uma nova direção de design.

**Ordem**, porque redesenhar dez áreas de uma vez é como não redesenhar nenhuma:

1. ~~Fotografar as áreas e escrever uma crítica por tela.~~ **Feito**: `docs/critica-telas.md`, nove telas.
2. ~~Corrigir os sete problemas de sistema.~~ **Feitos os sete**, em 2026-11-21. O que mudou, e onde:

   1. **`<Empty icon titulo filhos acao>`** (`src/ui/components.tsx`), um desenho para as nove telas. Aplicado em Git, Servidores, Terminal, Agendadas e Início. *Os dois defeitos de fato foram junto*: Git e Servidores agora oferecem "Abrir pasta raiz", e o Terminal parou de mandar clicar num `+` que, na área em tela cheia, não existe — a ação virou o próprio botão.
   2. **Tabela vazia não desenha cabeçalho.** `DataTable` com zero linhas devolve só o `empty`, no lugar dela inteira — passe um `<Empty>` ali e ele ocupa o espaço todo. Antes o estado vazio era espremido dentro de um `<td colSpan>`, debaixo de sete rótulos anunciando nada.
   3. **Token de texto secundário separado do de link.** `--text-2` era a matiz do acento clareada, nos quatro temas; virou neutro (texto misturado ao fundo, 5,0:1 sobre a superfície). O azul ficou para o que se clica. Isso sozinho conserta as nove telas, porque era uma variável, não nove enganos.
   4. **`<AreaHeader>` em todas as áreas**, com o título pequeno: a trilha do topo já nomeia a área, então o que carrega informação é a contagem. Git e Servidores, que começavam direto na fileira de abas, ganharam cabeçalho; o padding de topo passou a ser um só (era 51, 62 e 63 px).
   5. **O vazio de meia tela**, no caso que mais aparecia: o `<Empty>` cresce para ocupar a altura (`flex: 1`), e os contêineres de rolagem de Agendadas e Servidores viraram coluna para deixar. Tabela **cheia** que não estica continua como estava — é o passo 3.
   6. **Barra de estado.** A metade direita virou **bandeja**, separada por uma linha e sem poder espremer o estado do app; o "PID" saiu de Dispositivos e virou dica do próprio estado do sidecar; e "roda só com o Argus aberto" ganhou sujeito tanto na barra quanto no estado vazio de Agendadas, que é onde a frase muda uma decisão.
   7. **Desligado legível**: `.ui-btn:disabled` com borda tracejada e texto no contraste do secundário. A borda é que diz "desligado" — o texto não precisa sumir para isso.

   **Conferido na janela em 2026-11-21**, pela galeria: 22 de 22 cenários fotografados em `docs/estados/`. A conferência achou quatro coisas, todas corrigidas antes desta linha ser escrita:

   - **A galeria nunca tinha funcionado.** O Tauri 2 injeta `window.__TAURI_INTERNALS__` e o `invoke` dentro dele como propriedades **não graváveis e não configuráveis**: os dois `defineProperty` jogavam `TypeError`, a galeria caía no próprio `catch` e o app subia normal. O sintoma era mudo — a tela certa, com os dados de verdade e sem erro no console. A troca passou para o `invoke` importado em `src/api/commands.ts` (e o `listen` em `events.ts`): continua sendo um ponto só, continua cobrindo as nove áreas, e o custo é uma linha em cada arquivo.
   - **Servidores derrubava a área** quando não havia filtro de sessão: o `empty` da tabela é montado sempre, mesmo com linhas, e chamava `ownerLabel(filter!)` com `filter` nulo. Defeito meu, do passo 2, que só a galeria mostraria — é preciso ter o que listar *e* nenhum filtro.
   - **Os dois cenários de "carregando" nunca montavam.** Eles penduram um comando da carga inicial de propósito, e a ida para a área esperava justamente pelo `ready` que esse comando impede. Agora navega na montagem *e* a cada mudança de `ready`.
   - **A barra de Servidores transbordava a 900 px** (busca cortada pela metade, TCP/UDP/Escutando fora da tela, rolagem horizontal na área inteira): passou a quebrar linha. E o instante fixo das fixtures, escrito há um mês, já mostrava "No ar há 8775h13" — virou `Date.now()`.

3. ~~Tela a tela, do que mais se usa para o que menos se usa.~~ **Feitas as quatro**, em 2026-11-21, cada uma conferida na foto antes e depois:

   - **Conversas.** O cabeçalho da sessão repetia a barra de abas: mesmo título da conversa (que já aparece na aba *e* na trilha do topo) e um `+` idêntico ao de 36 px acima. O título saiu e o botão passou a dizer o que faz — "Conversas anteriores", que é o que ele tem de próprio, a lista com busca. O `+` saiu (a ação continua na barra de abas e dentro da lista). E os dois centros pararam de competir: sem nenhum projeto, a caixa mandava escolher um na pílula vazia enquanto o centro mandava abrir a pasta raiz — agora a caixa repete o centro, e o botão de enviar deixa de ser o único elemento aceso numa tela que não envia nada.
   - **Dispositivos.** Quatro das cinco abas existiam só para dizer que não existem: cada uma abria um painel inteiro cuja única frase mandava olhar a moldura ao lado. Com duas fileiras de aba na mesma tela, "Logcat" e "Arquivos" apareciam duas vezes cada, apontando para coisas diferentes. A fileira do Argus saiu; sobrou o que é real (dados do aparelho e configuração do sidecar), e o roteiro do que falta virou uma linha. A moldura, que lia como um aplicativo estranho mal encaixado, ganhou legenda: é um aplicativo estranho, e agora diz isso, com "abrir fora".
   - **Métricas.** O cartão "Limite da janela atual" dizia "o Argus ainda não lê o limite do plano" — texto que ficou velho na 0.1.3, quando o anel do compositor passou a mostrar exatamente esse número. Virou o dado (`rate_limits`), e some quando a CLI não informa janela nenhuma. O gráfico tinha altura fixa de 170 px dentro de um cartão que ia até o rodapé (daí os ~450 px de nada) e uma escala sem degrau entre 5 e 10, que desenhava um pico de 6,5 mi contra um eixo de 10 mi. Agora o gráfico ocupa o cartão, o eixo tem 3, 4, 6 e 8 no meio, e a barra acompanha a casa em vez de um teto de 28 px.
   - **Configurações.** "Autowork" estava em **Extras**, ao lado do mascote e do tocador de música, com nome em inglês. Foi para **Agentes**, como "Trabalho automático".

4. **Comparar o antes e o depois, com massa de teste e crítica de novo** (começado em 2026-11-21). Mudança de layout que ninguém conferiu depois é como teste que ninguém rodou. Pendências:

   - [x] 4.1 Congelar o baseline: copiar as nove fotos atuais de `docs/telas/` para `docs/telas/base/` antes de refotografar. Sem o "antes" guardado, "comparar com o print anterior" é chute — as fotos anteriores foram sobrescritas a cada rodada.
   - [x] 4.2 Ferramenta de comparação (`scripts/comparar-telas.mjs`): SSIM por tela contra o baseline, imagem de diferença quando houver mudança e relatório em `docs/telas/comparacao.md`. O número sozinho não diz se a mudança foi para melhor; a imagem de diferença diz onde olhar. Rodado: SSIM médio 0,9980 nas nove telas — as mudanças são o relógio e o player, não layout.
   - [x] 4.3 Massa de validação (`scripts/validar-galeria.mjs`): percorrer os 27 cenários e conferir, por cenário, área correta, galeria instalada, texto/seletor declarado em `esperar` e console sem erro. Saída em `docs/estados/validacao.json`. Hoje `galeria.mjs` só fotografa e confere a área — uma fixture que monta com o console cheio de erro passa. Rodado: **27/27 passaram**.
   - [x] 4.4 Marcas `data-ponte` nos elementos que a validação opera: `nav-<área>` na barra de ícones e `vazio` no `<Empty>` (vale para as nove áreas). A ponte procura primeiro pela marca; sem ela, trocar um rótulo quebra o roteiro silenciosamente.
   - [x] 4.5 Subir o app com a ponte, rodar validação e fotos, corrigir o que aparecer e rodar de novo até limpo. Rodado em 2026-11-22: validação 27/27, nove telas e 27 estados fotografados, `tsc` limpo. Nenhum defeito novo apareceu nesta rodada.
   - [x] 4.6 Rodar `/impeccable critique` nas telas novas (avaliação de design + detector no WebView2 por CDP) e registrar o resultado em `docs/critica-telas.md`, substituindo a crítica escrita sobre os prints antigos. **Feito em 2026-11-23**: 9 telas e 27 estados revisitados, tema Noite. Nielsen 28/40, carga cognitiva moderada (5/8 itens), 1 finding do detector (`tiny-text` em `div.chat-hint`). Correções priorizadas no próprio documento; ainda não aplicadas.

5. ~~Cenários de Conversas e Dispositivos.~~ **Feitos**, em 2026-11-21. Eram as duas únicas áreas sem cenário, e por isso as duas únicas que só tinham sido vistas **vazias** — justamente as que mais se usam. Cinco cenários novos: conversa com histórico, conversa sem pasta raiz, pergunta com opções ilegível, três aparelhos conectados e sidecar parado. Um cenário agora pode abrir uma aba de conversa (`sessao` em `cenarios.ts`), porque a área "chat" sozinha só sabe mostrar a conversa nova e vazia.

   Fotografar o cheio achou duas coisas, as duas corrigidas:

   - **O nome do item sumia atrás do recado** nas listas laterais de 200 px: o `meta` quebrava em duas linhas, sua largura mínima virava a da maior palavra e o rótulo era espremido até zero — "autorize a depuração no celular" ocupava a linha inteira sem dizer de qual aparelho era. O `meta` ganhou corte com reticências (vale para as nove áreas), e na lista de aparelhos o estado vai curto, com o recado inteiro no `title`.
   - **A moldura do Screen-Mirror fica branca quando o painel não sobe** — o sidecar anuncia uma URL, a página não responde e quem pinta o quadro é a página de erro do WebView2: 620 px de branco num app escuro. `color-scheme: dark` no iframe **não** resolve (testado). Resolver pede sondar a URL antes de montar o iframe. **Não feito**, anotado aqui.

   **Pendência do bloco `argus-ask`, corrigida junto**: quando o agente erra o JSON da pergunta com opções, o bloco voltava inteiro pelo caminho do markdown e o usuário levava mil caracteres de JSON cru, sem pergunta e sem aviso de que havia uma. Aconteceu de verdade nesta sessão. Agora o bloco sai do texto de qualquer jeito e vira uma linha: "o agente mandou uma pergunta com opções que não deu para ler". O cenário `conversas-ask-quebrado` fotografa isso.

**Ferramenta: a galeria de estados está construída** (`src/dev/cenarios.ts`, `src/dev/galeria.ts`, `scripts/galeria.mjs`, ação `cenario` da ponte). Era a peça que mais rendia e resolve três dos cinco buracos da lista antiga: estados, tema e largura.

Como funciona, e por que foi feita assim: um cenário declara **o que o backend responde** a cada comando — `{ valor }`, `"demora"` (nunca responde → carregando) ou `{ erro }` (rejeita). A interceptação é no `invoke` que `src/api/commands.ts` usa — o único ponto por onde a interface fala com o Rust (o `__TAURI_INTERNALS__`, que seria o ponto mais baixo, vem lacrado pelo Tauri 2) —, então as nove áreas ficam cobertas sem que nenhuma saiba que a galeria existe: nenhum componente ganhou prop de teste, nenhuma tela tem caminho só de desenvolvimento. O que é fotografado é o código que vai para o usuário. Comando não declarado passa direto para o backend real, por isso a janela, o tema e a própria ponte continuam vivos. São 27 cenários cobrindo vazio, carregando, erro, cheio, janela de 900 px e os três temas que nunca foram vistos.

```powershell
$env:ARGUS_PONTE = "1"; npm run app:dev
node scripts/galeria.mjs                 # docs/estados/<id>.png
node scripts/galeria.mjs metricas        # só os que casam
```

Três coisas aprendidas construindo, que valem para quem mexer nisso:

- **Fabricar a resposta do comando não basta.** O backend continua empurrando a realidade por evento, e segundos depois do boot um `servers-changed` com a lista verdadeira apaga a fixture — a primeira foto de "Servidores · cheio" saiu com a tela certa e zero linhas. Por isso o cenário também **cala eventos**, na mesma interceptação (`plugin:event|listen`).
- **Navegar na montagem não adianta.** Quando a carga inicial termina, a Shell abre uma conversa nova e `openNewChat` leva a área para "chat". O cenário só pode navegar depois de `ready`.
- **`alvo.invoke = meu` joga TypeError** num módulo ES se a propriedade não for gravável, e como isso roda no topo do `main.tsx` derrubava o boot inteiro — janela em branco, ponte junto. É `defineProperty`, e a instalação inteira é `try`/`catch`: galeria é ferramenta de desenvolvimento, não pode derrubar o app.

**Conferida na janela** em 2026-11-21: os 27 cenários saíram em `docs/estados/`, com os dados fabricados na tela. (O parágrafo que ficava aqui dizia que o embrulho no `__TAURI_INTERNALS__` ainda não tinha sido exercitado — não funcionava mesmo, e por isso a interceptação mudou de lugar; ver o item 2 acima.) Para checar de fora: `{"acao":"js","codigo":"window.__ARGUS_GALERIA__"}` devolve o id do cenário ativo.

**Aviso de segurança sobre fotografar**: `scripts/galeria.mjs` e `telas-print.mjs` capturam a **tela inteira** e recortam. Se a janela do Argus perder o "sempre à frente" — o que acontece quando alguém está usando o computador —, o recorte sai com o que estiver por cima. Aconteceu: uma captura pegou o Argus do usuário com uma conversa real dentro, e foi apagada. **Não rodar sessão de fotos com a máquina em uso.**

**O que ainda falta de ferramenta:**

- **Marcas `data-ponte`** nos elementos que importam. Hoje a ponte acha pelo rótulo visível, o que quebra quando o texto muda — e um rótulo ambíguo clica no lugar errado, que é pior do que falhar.
- **Comparar duas capturas.** Para pegar regressão visual sem reler tudo.
- **Cenários para Conversas e Dispositivos**, as duas telas mais densas e as únicas sem fixture — Conversas porque a transcrição vem por evento (`orch`) e não por comando, Dispositivos porque depende do sidecar.

## Argus como servidor MCP (proposto em 2026-11-21)

Ideia do usuário: em vez de cada projeto reaprender onde fica a memória, o Argus se oferece como servidor MCP e qualquer agente — de qualquer projeto — consulta as fontes dele. Vale a pena, e boa parte da encanação já existe.

**O que o Argus tem para servir que hoje ninguém alcança:**

- `memoria_buscar(consulta)` — as notas de `_Argus/Memoria` e a nota de cada projeto. Hoje o agente tem que lembrar do índice e abrir arquivo por arquivo; metade do valor da memória se perde aí.
- `memoria_gravar(titulo, texto, tipo, projetos)` — grava já com o frontmatter certo e a linha no índice. Hoje isso é prosa repetida em todo prompt de sistema, e cada agente cumpre um pedaço.
- `sessoes_buscar(texto)` — o banco guarda todos os eventos de todas as conversas. "O que a gente decidiu sobre X" viraria uma consulta em vez de arqueologia no histórico.
- `projetos_listar` / `projeto_status` — a raiz aberta, o status git, os servidores de dev no ar.
- `mural_ver` — o mural de arquivos legível sob demanda, não só injetado no começo do turno.
- `fontes_projeto` — `PRODUCT.md`, `DESIGN.md`, `PROXIMOS_PASSOS.md`, checklist.

**Como:** o `remote.rs` já é um servidor axum com token; a rota `/mcp` (HTTP streamable) entra nele e aproveita a mesma autenticação. O registro na CLI o Argus já sabe fazer sozinho — `mcp.rs` tem o `add`, então é um botão em Configurações, não um passo manual.

**Três coisas para decidir antes de escrever código**, porque todas mudam o desenho:

1. **Escopo de leitura.** A memória é pessoal e atravessa projetos. Um agente trabalhando no projeto A consultando uma nota do projeto B copia aquele conteúdo para o histórico de A. O frontmatter já tem `projetos:` — o filtro natural é servir por projeto e exigir pedido explícito para sair dele. Credencial nenhuma passa, em hipótese alguma.
2. **Recursão.** Um agente *do Argus* consultando o Argus no meio de um turno do Argus. O `loop_guard` existe, mas foi feito para outro problema; vale conferir antes, não depois.
3. **Quando o servidor liga.** O remoto hoje sobe sob demanda e com Funnel; o MCP precisa estar de pé sempre que houver agente rodando. A resposta compatível com o que já foi decidido é: `127.0.0.1`, sem Funnel, sem prompt de firewall.

## Pendências por prioridade (2026-11-19)

Fila de trabalho. P1 a P3 em andamento; P4 é conferência que o usuário faz em casa (aparelho, sistema, chave).

### P1 — uso e custo do dia a dia (feito em 2026-11-19)
- [x] Números em pt-BR na conversa (total, tooltip, linha do tempo, medidor de contexto).
- [x] `cd <pasta>; git status` negado: é proteção da própria CLI (nega `cd` + `git` no mesmo comando; `cd` sozinho passa). O aviso de sistema orienta `git -C pasta`.
- [x] Aviso do modo Isolado: conferido com haiku e sonnet, nenhuma busca antes de responder.
- [x] OpenCode: o `run` passava pelo serviço em segundo plano, que ignorava o `OPENCODE_CONFIG_CONTENT` e aprovava tudo. Agora `--standalone` e, no Restrito, política real (shell só nos padrões liberados, resto negado). Conferido: negado/liberado, edição, sessão retomada.
- [x] Worktree do OpenCode sai limpo logo depois do turno (servidor próprio termina junto).
- [x] Turno cancelado: uso gravado; a saída é estimada pelo texto recebido (917 tokens em vez de 4).

### P2 — funções novas (conferidas na janela em 2026-11-19)
- [x] Tarefas agendadas: criar, "Executar agora" (conversa + "Abrir conversa"), disparo no horário, horário perdido com o app fechado roda ao abrir.
- [x] Planejar/Bypass: o Planejar do claude não usa mais `--permission-mode plan` (a CLI trocava haiku por sonnet: US$ 0,81 → 0,07); negativas do Planejar não viram alerta. "Executar o plano" e selos de Bypass conferidos.
- [x] Extensão VS Code: Alt+K mostra uma conversa e insere uma vez; chips "em foco" e "erro do editor"; rejeitar trecho em CRLF; "Liberar e repetir".
- [x] Inspetor › Pendências: atualização ao vivo e quatro abas no painel.
- [x] Auto-work: Pausar, Retomar, Parar. Agora usa o mesmo arquivo do inspetor (PROXIMOS_PASSOS.md, PENDENCIAS.md ou TODO.md) e a configuração não depende de `/` ou `\`.

### P3 — melhorias (feito em 2026-11-19)
- [x] "Voltar a este ponto" em cada pedido (`crates/core/src/checkpoints.rs`, refs `refs/argus/t<id>/<n>`).
- [x] Alt+K sem conversa à vista.
- [x] Aviso de custo do cache na pílula de modelo da conversa nova.
- [x] Servidor do agente: no claude 2.1.220 ele sobreviveu ao turno; quando cai, aviso com "Iniciar pelo Argus".
- [x] Aparência: tamanho da interface (zoom nativo) e fonte do editor/terminais; ícones na paleta.
- [x] Um só laço de "rodando" (o anel); pontos, ícone e pendência em andamento ficaram fixos.

### P4 — o usuário confere em casa
- [x] Celular: pareamento pelo QR (cookie `Lax`), fonte Geist, divisória de troca de agente / comando negado / subtítulo na web app. **Conferido em 2026-11-21 num Galaxy M55 (Android 16, Chrome)**, com o servidor só em `127.0.0.1` e o aparelho chegando por `adb reverse` — sem Firewall e sem Tailscale. O roteiro está no teste `remote::tests::live_mobile_web_app` (`--ignored`) e em `scripts/celular-camera.mjs`, que faz o papel do leitor de QR servindo o link numa origem diferente (`localhost` → `127.0.0.1`), que é a navegação de fora do site onde o cookie `Strict` não ia. O que se viu: pareou e caiu na lista "ao vivo"; o cookie do token antigo deu "Não autorizado" depois de revogar; a Geist desenha diferente do fallback `system-ui` (`scripts/celular-fonte.mjs`); a divisória "Agente trocado: claude → opencode", o cartão "Comando negado · git status" e o selo "contexto 87% (acima de 80%)" aparecem como projetados.
  - Achado e corrigido na conferência: no celular o subtítulo da conversa era uma linha só com reticências, e o selo de contexto — justamente o aviso de janela enchendo — ficava espremido fora da tela. Agora quem corta é o nome do projeto (`.bar-sub` virou flex, `.bar-sub-main` trunca, `.ctx` fica inteiro).
- [ ] Melhorar pedido: uma reescrita de verdade com a CLI (custo real no haiku), o modo "em todo envio" e o Desfazer.
- [ ] Métricas: virada do "Hoje" depois das 21h.
- [x] OpenCode real: modelo `-free` com custo 0, uso do último passo. **Conferido em 2026-11-21** na tarefa #25 (`opencode/beam-free`): `US$ 0,00 equivalente em API` na tela e `61 entrada · 76 saída · cache 13,4 mil lido` no último passo.
- [ ] Selo de auto-work sem "Reiniciar" em sessão aplicada/descartada.
- [x] Arrastar do Explorer, Ctrl+V de print, login do claude no terminal. **Conferido pelo usuário em 2026-11-24.**
- [x] Player tocando música. **Conferido pelo usuário em 2026-11-24.**
- [ ] "Iniciar com o Windows" no app instalado (reiniciar e ver abrir minimizado).
- [x] APK assinado: keystore do usuário criado (PKCS12, RSA 4096) e `app-universal-release.apk` gerado em 2026-11-21. `apksigner verify` confirma — um assinante, esquema v2, RSA 4096. O `key.properties` e o `.jks` ficam fora do repositório. O `gerar-apk.ps1` agora repõe o bloco de assinatura no `build.gradle.kts` toda vez que acha o `key.properties`: `gen/android` é regenerável, e sem o bloco o release saía **sem assinatura** mesmo com as chaves no lugar.
- [ ] Codex e gemini: instalar e logar as CLIs; conferir `--add-dir`/`--include-directories` na pasta `_Argus` e o Planejar/Bypass deles.
- [x] Toast do Windows das tarefas agendadas (o evento sai; a notificação do sistema não foi vista). **Conferido pelo usuário em 2026-11-24.**
- [x] Funnel e app nativo `dev.argus.celular`. **Conferido no aparelho em 2026-11-24:** `remote_start` com `Exposure::Funnel` em `desktop-sie986u.taile13b10.ts.net:443` → `127.0.0.1:8765`; `funnel-check.mjs` deu 200 em `/`, `/app.js`, `/style.css`, `/geist.woff2` e `/api/tasks`, com `Set-Cookie HttpOnly; SameSite=Lax`, e 401 sem cookie. O Chrome Android abriu o domínio com `Argus / ao vivo`, Geist carregada e service worker presente. No app nativo o campo de pareamento recebeu o link HTTPS, apareceu o toast `Aparelho pareado.`, o indicador `ao vivo` e, em Ajustes, `Computador: desktop-sie986u.taile13b10.ts.net` (antes `127.0.0.1:8765`). Nenhum token foi registrado.
- [x] Remoção em larga escala da referência a Mastersys em `D:\Dev\Desenvolvimento\Projetos`, autorizada em 2026-11-24. Regra aplicada: texto visível e documentação passam a usar o nome do projeto (ou “Suporte”, quando o citado é um sistema externo); identificadores técnicos passam a usar o slug do projeto. Os detalhes e o que ficou pendente estão em “Ação em larga escala”, abaixo.
- [x] Reconstruir os artefatos gerados que ainda guardavam o identificador antigo: `AllSeenEye\app\build\**` e `SwitchOS\hub\src-tauri\target-agent\**`. **Feito em 2026-11-24:** `gradlew clean assembleDebug --no-build-cache --rerun-tasks` no AllSeenEye (o primeiro build reaproveitou o cache e ainda deixou cinco arquivos Kotlin com a string; sem cache, ficou em zero); no SwitchOS o `target-agent` foi removido e recriado com `CARGO_TARGET_DIR` apontando para ele. Os arquivos do hub (`fingerprint`, `build/output` e `.d`) ficaram sem a string; sobrou um falso positivo num `.rmeta` de crate de terceiros.
- [x] Gerar de novo o APK `Screen-Mirror\apks\switchos-debug.apk` com a nova identidade `br.com.switchos.app` e substituir o arquivo antigo. **Feito em 2026-11-24:** `gradlew :app:assembleDebug --no-build-cache --rerun-tasks` em `SwitchOS\android\android`, depois `Copy-Item` para `apks\`. O `aapt2 dump badging` confirma `package: name='br.com.switchos.app'`; o arquivo passou de 301 MB para 278 MB e a varredura de bytes deu zero ocorrências.
- [ ] Conferir o impacto da troca de `applicationId`/`identifier`: celular com o app antigo instalado precisa desinstalar antes de instalar o novo (Android e Tauri tratam identificadores diferentes como apps distintos).
- [ ] Migrar as pastas de dados pessoais que usavam o identificador antigo (`%APPDATA%\br.com.mastersys.digitama` e `%LOCALAPPDATA%\br.com.mastersys.digitama\bin` em `Digitama.md`): o nome novo é `br.com.digitama.app`; copiar os dados antes de apagar os antigos.
- [x] Rodar os gates dos repositórios alterados (AllSeenEye, Controle de gastos, Scale, Screen-Mirror, SwitchOS e volume-mixer), além dos gates do Argus. **Feito em 2026-11-24, todos passaram.** Argus: `npm run typecheck`, `cargo test --workspace` (276 no core + 23 no desktop, 0 falhas), `cargo check -p argus-desktop` (só o aviso `sanitize_name is never used`), `npm run teste:celular` (18/18), `npm run build` e `node --check crates/core/assets/remote/app.js`. Controle de gastos: `typecheck`, `lint`, 296 testes e `build`. Screen-Mirror: `typecheck` e 11 testes. Scale: `lint`, `build` e `docker compose config`. portal-geral: `apps.json` com 5 itens e `masternote.html`. SwitchOS: `tauri.conf.json`, `games.toml`, `cargo check --manifest-path hub/src-tauri/Cargo.toml` e `./gradlew :app:compileDebugKotlin`. AllSeenEye: `./gradlew compileDebugKotlin`. volume-mixer: conferido pelos READMEs alterados.

## Ação em larga escala — remoção da referência a Mastersys (2026-11-24)

Pedido do usuário: remover a referência em toda a pasta raiz aberta no Argus,
deixando só registros com os nomes dos projetos, itens indicados a ele e as
configurações dele. A regra aplicada foi esta:

| Tipo de ocorrência | Substituição |
|---|---|
| Texto visível e documentação de um projeto | nome do projeto (por exemplo `SwitchOS`, `Scale`, `AllSeenEye`) |
| Sistema externo citado como “Mastersys Suporte” | `Suporte` |
| Pacote Android / identificador Tauri | slug do projeto, mantido o prefixo (`br.com.switchos.app`, `br.com.switchos.hub`, `com.allseeeye.app`, `br.com.pdv.app`, `br.com.digitama.app`) |
| Fixture de extrato bancário | `SISPAG PIX LANCAMENTO ...` (o valor e o teste continuam iguais) |
| Caminho de código-fonte de referência em `PDV.md` | `C:\MASTERERP-SRC\server-weld` |

Arquivos de código e documentação alterados: `AllSeenEye` (pacote Kotlin e
`build.gradle.kts`, com o diretório movido para `com/allseeeye/app`), `Argus`
(`docs/mockups/argus-telas.html`), `Controle de gastos` (duas fixtures),
`Scale` (`docs/lgpd-moderacao.md` e a cópia no worktree), `Screen-Mirror`
(README e teste de política), `SwitchOS` (PRODUCT, pendência, `games.toml`,
`build.gradle.kts`, `tauri.conf.json`, `Cargo.toml`, dois `strings.xml` e um
comentário Kotlin), `volume-mixer` (dois README), os docs soltos `AllSeenEye.md`,
`Digitama.md`, `MasterDesk.md` e `PDV.md`, e `portal-geral` (`apps.json` e
`apps/masternote.html`).

Varredura final em arquivos-fonte, documentação e artefatos gerados: **zero
ocorrências** na cadeia de build, com exceção desta seção, que registra o pedido.
Antes da reconstrução havia 150 arquivos em `AllSeenEye\app\build`, cinco
arquivos de texto em `SwitchOS\hub\src-tauri\target-agent` e a identidade antiga
no manifesto interno do APK de 301 MB. Depois da reconstrução em 2026-11-24:
`AllSeenEye\app\build` e o APK novo têm zero; no `target-agent` os arquivos do
hub estão limpos e o único match é um `.rmeta` de crate externa. Nenhum artefato
foi editado à mão — todos saíram de `clean` + rebuild.

Uma segunda varredura, depois da reconstrução, achou 12 arquivos esquecidos nos
worktrees antigos do Scale (`.claude\worktrees\agent-*`, `desktop-r2-voz`): eram
o Electron removido, parados desde 23/10, com `Mastersys`, `Copyright (c)
Mastersys` e `br.com.mastersys.scale` em `electron-builder.yml`, `package.json`,
`main.ts` e `menu.ts`. Aplicada a mesma regra em 2026-11-24: texto virou `Scale` e
identificador virou `br.com.scale.app`. Os 12 arquivos foram conferidos um a um e
agora têm zero ocorrências. Nenhum desses worktrees foi recriado nem teve outro
arquivo tocado.

Também foi criado `AGENTS.md` na raiz do Argus, no formato usado pelo Claude
(regras de escopo, gates, arquitetura, armadilhas e validação no aparelho). O
padrão de ponteiro é `CLAUDE.md` com `@AGENTS.md`; não foi criado um `CLAUDE.md`
porque o pedido cobria só o `AGENTS.md`.

## Como rodar

Requisitos: Rust (stable, MSVC), Node.js 20 ou mais novo e o WebView2 (já vem no Windows 11).

```powershell
npm install                                   # uma vez, ou quando o package.json mudar
npm run tauri dev                             # abre a IDE com recarga automática da interface
npm run tauri build -- --debug --no-bundle    # gera target\debug\argus-desktop.exe sem instalador
```

Testes e verificações:

```powershell
npx tsc --noEmit            # tipos da interface
cargo test --workspace      # 141 testes do core + 18 do desktop
node --check crates/core/assets/remote/app.js   # sintaxe da web app do celular
```

A interface ainda não tem runner de testes próprio; a lógica pura das telas foi conferida por scripts avulsos durante a migração.

Variáveis úteis para testes:

| Variável | O que faz |
|---|---|
| `ARGUS_ROOT` | abre essa pasta raiz automaticamente |
| `ARGUS_DB` | usa outro banco SQLite |
| `ARGUS_ALLOWED_TOOLS` | usa outro arquivo de comandos permitidos |
| `ARGUS_OPENCODE_MODEL` | só o modelo INICIAL das conversas novas do OpenCode (`provedor/modelo`, como em `opencode models`); o de cada conversa é escolhido na pílula de modelo e fica gravado na tarefa |

Os dados ficam em `%APPDATA%\argus\`: `argus.db`, `pricing.json`, `allowed_tools.json` e a pasta `anexos\` (anexos do chat, apagados depois de 1 dia).

## Estrutura

```
package.json, vite.config.ts, index.html     front-end (vite)
src/                                         interface em React 18 + TypeScript + zustand
  api/        uma função tipada por comando Tauri, eventos e tipos que espelham o serde
  store/      useApp, useLayout, useWorkbench, useTerminals, useTheme, bootstrap (liga eventos ao estado)
  shell/      Shell (layout), TopBar, RightPanel, BottomPanel, StatusBar, ContextMenu, icons (Lucide, mesma API Icon/IconName/toolIcon),
              useActiveContext (sessão e projeto da aba em foco), useGlobalShortcuts
  styles/     tokens.css (TODAS as cores por tema, mais fonte, espaçamento --space-1..6 e raios --radius-sm/--radius/--radius-lg/--radius-xl), components.css (classes portadas), shell.css (layout)
  views/
    projects/   barra esquerda: projetos, sessões e o rodapé que abre as telas de apoio
    session/    a conversa (aba principal): index (ChatTab), Transcript, panels (Alterações e Subagentes
                do painel direito), SubagentTree, events (funções puras), useTaskEvents, actions
    chat/       componentes da conversa: Composer, MessageView/Markdown, fila, anexos, menções, comandos de barra
    workbench/  grupos de abas, split, Monaco (tema gerado dos tokens), Painel de projetos
    explorer/   árvore de arquivos do projeto ativo (aba Arquivos do painel direito)
    servers/    aba Servidores do painel direito, selos e "Comandos permitidos"
    agents/     aba Provedores (login e status das CLIs)
    settings/   aba Configurações (tema, auto-work por projeto, comandos permitidos)
    autowork/, deps/, metrics/, remote/, terminal/, player/, pet/
src-tauri/    crate argus-desktop (Tauri 2, identificador dev.argus.app)
  src/commands/  workspace, monitor, fs, pty, agents, autowork, servers, deps, metrics,
                 remote, media, system, extra_agents, extra_chat, schedules (tarefas agendadas,
                 notificação do sistema e "Iniciar com o Windows")
  src/events.rs  repassa os eventos do orquestrador e dos servidores para a interface
src-mobile/   crate argus-celular (Tauri 2 Android, dev.argus.celular): casca do aplicativo do
              celular. Só a tela de pareamento (web/); depois disso a webview abre a interface
              servida pelo computador. Fora do workspace; gerar com `gerar-apk.ps1` (ver README)
crates/core   (argus-core): workspace, monitor, pty, procs, deps, metrics, db, remote, autowork, schedules,
              isolation (o que a cópia isolada não teria), permissions (comandos liberados ao agente),
              agents/{provider, claude, opencode, codex, gemini, orchestrator, handoff (resumo na troca de agente)}
              assets/remote (web app do celular: index.html, app.js, style.css, unauthorized.html,
              manifest.webmanifest, icon.svg; embutidos no executável por include_str)
```

Monaco e xterm entram por npm e são empacotados pelo vite; nada vem de CDN.

## Layout

Inspirado na Orca (projetos e sessões à esquerda, abas no centro, arquivos à direita) e no chat do Claude Code no VS Code. Não há mais activity bar.

| Região | O que tem |
|---|---|
| Barra superior (32 px) | alternar a barra esquerda, projeto e sessão em foco, alternar terminal e painel direito |
| Barra esquerda | "Abrir pasta raiz", Painel, filtro de encerradas, recolher tudo e busca. Cada projeto (nome, branch, ±alterados) expande nas sessões: ponto de status, título (primeiro pedido), selos de auto-work e servidores, provedor. Sessão com atividade não vista fica em negrito. "+" no hover cria conversa no projeto; clique direito: Abrir, Ver diff, Cancelar o turno, Descartar, Excluir sessão (também no seletor de conversas do cabeçalho do chat); no projeto: Nova conversa e Excluir encerradas. A lista se atualiza sozinha (watcher do primeiro nível da raiz com debounce de 500 ms, foco da janela e o botão Atualizar); projeto apagado do disco com sessões fica como "pasta ausente". Excluir apaga tarefa e eventos, mas mantém as linhas de uso nas Métricas (órfãs, com o id original; ids nunca são reaproveitados). Rodapé: Provedores, Dependências, Métricas, Remoto e Configurações abrem como abas |
| Centro | grupos de abas com split (até 3). A aba de conversa é a principal: "Nova conversa" (saudação, sugestões, pílulas de projeto e provedor) vira a sessão da tarefa no primeiro envio, na mesma aba. Não existem mais as abas "Chat" e "Sessão" separadas |
| Conversa | cabeçalho com seletor de conversas (busca), "+", estado, "provedor · modelo" (some em aba estreita), total de tokens e custo equivalente em API. Coluna central de 820 px: pedido em caixa, resposta em Markdown, ferramentas em linhas recolhidas, "N passos" no turno terminado, divisória "Agente trocado: X → Y" e caixa "Comando negado" com o atalho "Liberar este comando neste projeto". Caixa de composição arredondada, numa linha só: anexar, "/", "@", pílula de projeto (só na conversa nova), pílula "provedor · modelo" (menu com agente e modelo; estreita, mostra só o modelo), botão de opções (ícone de ajustes, com contagem do que foge do padrão: modo de trabalho, modo de permissão e auto-work, cada um com explicação) e Enviar, que vira Parar durante o turno. Seta para cima recupera os pedidos anteriores |
| Pílulas da conversa | **Conversa nova:** projeto, provedor, modelo ("Padrão da CLI" + os que a CLI informa) e modo de trabalho ("Isolado (cópia do projeto)" ou "Direto na pasta"; sem git, só o direto). **Sessão existente:** o provedor pode ser trocado com o turno parado (desabilitado durante o turno, com dica), o modelo pode ser trocado a qualquer hora (vale no próximo turno) e o modo de trabalho é fixo. `/modelo` abre a lista; `/modelo nome` troca direto. A lista é pedida à CLI assim que a caixa aparece; enquanto não chega mostra "Carregando modelos…", em erro mostra a mensagem e "Tentar de novo" (erro e lista vazia nunca vão para o cache) e sempre dá para digitar o nome de um modelo |
| Aviso antes do envio | No modo Isolado, o primeiro envio compara a pasta do projeto com o que o worktree teria. Havendo algo de fora (não versionados, ignorados relevantes, alterações não commitadas), o pedido fica segurado num aviso acima da caixa, com o caminho citado em destaque e os botões "Usar Direto na pasta" e "Continuar isolado" (✕ devolve o texto à caixa). Em "Direto na pasta", avisa antes de iniciar uma segunda sessão direta no mesmo projeto |
| Painel direito (retrátil) | Arquivos (projeto ativo), Alterações (diff da sessão, Aplicar e Descartar; numa sessão "Direto na pasta", no lugar dos botões vem o aviso de que não há o que aplicar e o diff da pasta em relação ao último commit), Subagentes (árvore; abre o chat somente leitura), Servidores |
| Configurações | Tema; **Agentes** (modo de trabalho padrão, "Git local", "Git remoto" e a lista do que está liberado por padrão); **Por projeto** (as mesmas três escolhas, com "Seguir o geral", auto-work e comandos permitidos) |
| Painel inferior | terminais (oculto por padrão; abre pelo botão Terminal, Ctrl+J ou ao criar um terminal). "+" abre outro terminal no painel; "⧉" abre um terminal como aba do editor, que pode ser arrastada entre os grupos |
| Status bar | "N agentes rodando" (clique foca a sessão), servidores, uso de hoje; player, direção do split e pet |

Atalhos novos: Ctrl+N (nova conversa), Ctrl+B (barra esquerda), Ctrl+J (terminal); não valem com o foco num terminal.

"Projeto ativo" (`useApp.selected`) acompanha a aba em foco (`shell/useActiveContext.ts`) e pode ser trocado clicando num projeto na barra esquerda. É ele que a aba Arquivos, os terminais novos e as telas por projeto usam.

### Temas

Três temas, escolhidos em Configurações e lembrados no localStorage (`argus.tema`): **Oceano** (padrão), **Nebulosa** e **Brasa**. Toda cor vem de `src/styles/tokens.css`: cada tema define os tokens de base (`--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-2`, `--accent`, `--accent-hover`, `--on-accent`, `--accent-text`, `--selection`, `--focus`, `--status-bg`, `--on-status`) só com cores da própria paleta; os demais são derivados. As cores de estado (`--ok`, `--warn`, `--err`, `--run`) são iguais nos três. Nenhum componente usa hex direto. O Monaco (`views/workbench/monaco.ts`) e o xterm (`views/terminal/session.ts`) montam o tema a partir dos tokens e trocam junto.

Desde 2026-11-16 (fases 1 a 3 de `docs/PLANO_UI_ORCA.md`): fonte **Geist** variável (OFL-1.1) embutida pelo pacote npm `@fontsource-variable/geist`, sem CDN, em `--font-sans`; mono Cascadia em `--font-mono-family`. A hierarquia vem do tom das superfícies: `--border` é só ~7–10% do texto sobre a superfície (a `--border-strong` continua com a cor da paleta, nos controles e na caixa de composição) e a status bar é neutra (`--status-bg` = fundo do app, `--on-status` = texto secundário, com uma borda no topo). Ícones: `lucide-react` (ISC) com versão fixada; `src/shell/icons.tsx` mapeia os nomes antigos para os componentes Lucide (traço 1,75).

A web app do celular (`crates/core/assets/remote/style.css`) repete os mesmos tokens e os mesmos três temas; o seletor fica na tela Ajustes dela e a escolha é guardada no aparelho (mesma chave `argus.tema`). O `theme-color`, o manifest e o ícone acompanham o tema (`/manifest.webmanifest?tema=...`).

### Acesso remoto (web app do celular)

Servidor axum em `crates/core/src/remote.rs`, desligado por padrão; liga na aba Remoto, que mostra o QR code. A web app é HTML, CSS e JavaScript puros (sem framework e sem CDN) e foi refeita em 2026-11-14 para acompanhar o desktop.

| Tela | O que tem |
|---|---|
| Pareamento | `/pair?token=...` grava o cookie (HttpOnly, SameSite=Strict) e redireciona para `/`, tirando o token do endereço. Token ausente, errado ou revogado dá a mesma página 401 "Não autorizado" |
| Sessões (início) | projetos com as suas sessões: ponto de status (erro vira ícone de alerta com o rótulo "Erro"), título, provedor, selos de auto-work e de servidores, negrito e ponto azul na sessão com novidade, busca, "Encerradas", "+" geral e por projeto |
| Conversa | pedido em caixa, resposta em Markdown (tabela com rolagem lateral, código com rótulo e Copiar), ferramentas e "N passos" recolhidos, stream pelo WebSocket, total de tokens e custo equivalente em API, caixa de composição no rodapé (acompanha o teclado), Enviar que vira Parar. O subtítulo mostra projeto, provedor e modelo atuais (e "direto na pasta"); a divisória "Agente trocado: X → Y" e a linha "Comando negado" aparecem. Trocar de agente, de modelo e de modo de trabalho é só no desktop (conversa criada pelo celular usa os padrões do projeto) |
| Subagentes | lista em árvore e o chat de cada um (prompt recebido, ferramentas, resultado), somente leitura, sem caixa de resposta |
| Nova conversa | projeto (só os da pasta raiz aberta na IDE), provedor e pedido; cai na conversa criada |
| Auto-work | liga e desliga por projeto, progresso, Pausar, Retomar e Parar (com confirmação) |
| Servidores | por sessão ou terminal, com Abrir e Derrubar (com confirmação) |
| Ajustes | tema, estado da conexão, como revogar e como instalar |

Rotas (todas exigem o token, por cookie ou `Authorization: Bearer`; sem ele, 401, inclusive no WebSocket):

| Rota | O que faz |
|---|---|
| `GET /pair?token=` | pareamento (única rota sem token prévio) |
| `GET /`, `/app.js`, `/style.css`, `/icon.svg?tema=`, `/manifest.webmanifest?tema=` | a web app |
| `GET /api/tasks`, `POST /api/tasks` | lista as sessões; cria uma (só em projeto conhecido da IDE) |
| `GET /api/tasks/{id}` | a sessão e os eventos (inclui `SubagentStart`, `SubagentEnd` e `Scoped`) |
| `POST /api/tasks/{id}/reply`, `/cancel` | responde; para o turno |
| `GET /api/projects` | `[{ name, path }]`: projetos da pasta raiz aberta mais os das sessões |
| `GET /api/providers` | `[{ id, name }]` |
| `GET /api/pricing` | tabela de preços do custo equivalente em API |
| `GET /api/autowork`, `POST /api/autowork`, `POST /api/tasks/{id}/autowork/{start,pause,resume,stop}` | auto-work |
| `GET /api/servers`, `POST /api/servers/{pid}/kill` | servidores rastreados; derruba um (só PID rastreado) |
| `GET /api/stream` | WebSocket: `state`, `agent`, `stderr`, `autowork`, `resync` e `ping` a cada 5 s |

Cuidados já no código: o token não aparece em log nem em corpo de resposta; a página não carrega nada de fora (Content-Security-Policy `default-src 'self'`); o aviso da CLI "Warning: claude.ai MCP servers blocked" é filtrado no servidor e não chega ao celular; desligar o servidor na aba Remoto derruba os WebSockets abertos (antes eles continuavam recebendo eventos); a web app reconecta sozinha, avisa "Conexão perdida" e recarrega os dados ao voltar.

Limites conhecidos:

- **Instalação como aplicativo:** por `http://` na rede local o navegador só oferece "Adicionar à tela inicial" (atalho). A instalação completa de PWA e o service worker exigem HTTPS; não há service worker.
- **Anexos:** aparecem como chips com o nome do arquivo; a miniatura das imagens não é enviada ao celular, e o celular não anexa arquivos.
- **Fila de mensagens:** não existe no remoto; com o agente trabalhando, a caixa fica desabilitada e o botão vira Parar.
- **Negrito de novidade:** vale para o que chega com a web app aberta (fica guardado no aparelho); o servidor não guarda o que cada aparelho já viu.
- **"Abrir" de um servidor** usa o IP do computador; servidor que só escuta em `localhost` não abre pelo celular.
- **Iniciar o auto-work numa sessão** não tem botão na web app (a rota `start` existe); ele começa pelas regras do desktop.

## Modelo, agente, modo de trabalho e permissões (2026-11-14)

**Modelo por conversa.** Fica na tarefa (coluna `model`). claude: `--model` em todo turno, junto com `--resume`; opencode: `-m`. A lista vem da própria CLI, sob demanda, com cache em memória: no claude, da resposta ao pedido de controle `initialize` do stream-json (`response.response.models`; nenhum token gasto, o campo de conta não é lido); no opencode, de `opencode models`. Nomes são validados antes de virar argumento (nada com espaço ou começando por `-`).

**Troca de agente.** `Orchestrator::set_provider`: a tarefa, a pasta e o histórico continuam; a sessão e o modelo do provedor que sai ficam em `provider_state` (JSON na tarefa) e voltam se o usuário retornar a ele (aí o `--resume`/`--session` é retomado e ele só recebe o que perdeu). O próximo turno leva na frente do pedido um resumo montado pelo Argus (`agents/handoff.rs`): pedidos do usuário, respostas em texto do agente principal e a lista de arquivos alterados (`git status` da pasta de trabalho), sem saída de ferramentas, até 24 mil caracteres (corta o meio e avisa). O histórico grava só a mensagem do usuário e o evento `ProviderSwitch` (a divisória). O uso continua na mesma tarefa, com provedor e modelo em cada linha.

**Modo de trabalho por tarefa** (coluna `mode`; o booleano global de worktrees saiu do `Orchestrator`). Isolado = git worktree, como antes. Direto = a pasta do projeto: sem Aplicar/Descartar, diff contra o último commit (sem tocar no índice do git). Padrão: Isolado; preferência geral e por projeto na tabela `settings` do banco. Pasta sem `.git` só aceita o direto. No Isolado o claude recebe `--append-system-prompt` dizendo que está numa cópia, onde fica a pasta original e que arquivos fora do git não existem ali (o opencode não tem essa opção: o aviso vai na frente do primeiro pedido). Nunca entra `--add-dir` da pasta original.

**Comandos liberados ao agente** (`crates/core/src/permissions.rs`). No Windows o claude usa a ferramenta `PowerShell`, não a `Bash`: por isso nem `git status` funcionava, e a lista do projeto (que virava só `Bash(...)`) não ajudava. Agora todo turno do claude leva `--allowedTools` com, para `Bash` e `PowerShell`:

- Git só leitura: `git status`, `git diff`, `git log`, `git show`, `git ls-files`, `git rev-parse`, `git blame`, `git grep` (com argumentos); `git branch` nas formas `git branch`, `--list`, `--show-current`, `-a`, `-r`, `-v`, `-vv`; `git remote` e `git remote -v`; `git stash list`.
- Inspeção: `cd`, `ls`, `dir`, `pwd`, `cat`, `type`, `head`, `tail`, `wc`, `which`, `where`; só no PowerShell: `Set-Location`, `Get-ChildItem`, `Get-Content`, `Get-Location`, `Get-Command`, `Test-Path`.
- Nível "Git local" (ligado por padrão no Isolado, desligado no Direto): `git add`, `commit`, `restore`, `checkout`, `switch`, `branch`, `merge`, `rebase`, `stash`, `tag`.
- Nível "Git remoto" (desligado por padrão nos dois): `git push`, `pull`, `fetch`, `clone`; com ele ligado, o push forçado segue negado por `--disallowedTools`.
- Nunca por padrão: `git push --force`, `git reset --hard`, `git clean`, shell genérico.

**Modo de permissão (2026-11-16)**: "Auto (recomendado)" é o padrão; "Restrito" é o comportamento acima. Geral e por projeto em Configurações (chave `perm_mode`). No Auto só os nocivos (`crates/core/src/harmful.rs`: apagar raiz/home/disco, formatar, desligar, registro/serviços/usuários, push forçado, reset --hard, clean, baixar e executar, credenciais, chmod/icacls amplos, matar processo por nome) ficam bloqueados: claude `--permission-mode auto` + nocivos em `--disallowedTools`; codex rede liberada no sandbox; gemini `yolo` + `--admin-policy` com `deny`; opencode `OPENCODE_CONFIG_CONTENT`. ⚠ Conferido só a montagem (testes) e o claude aceitando os argumentos (Haiku); gemini/codex não instalados aqui, opencode não rodado com a política.

A lista "Comandos permitidos ao agente" do projeto soma a isso (linha simples vira `Bash(...)`; para o PowerShell escreva `PowerShell(comando)`). Comando negado vira o evento `Denied` (lido de `permission_denials` do `result`) e aparece no chat com o atalho "Liberar este comando neste projeto". O OpenCode não foi alterado: as permissões dele vêm da configuração de agentes do próprio usuário.

## Funcionalidades

Legenda: **na janela** = visto funcionando na janela do Tauri; **compila** = implementado, passa em `tsc`/build, nunca exercitado na janela; **core testado** = a lógica em Rust tem teste, a tela não foi exercitada.

| Recurso | Estado na interface nova |
|---|---|
| Janela, barra de projetos e sessões, painel direito, barra superior, status bar | ✓ na janela (layout novo, de 1100 px para cima) |
| Temas Oceano, Nebulosa e Brasa (interface, Monaco e terminal) | ✓ na janela, inclusive o terminal: com um terminal aberto, o fundo e o texto do xterm trocam junto com o tema |
| Explorador: árvore por projeto, status git por arquivo (M/U), abrir arquivo | ✓ na janela |
| Editor Monaco com cores (.rs, .toml, .md), abas, split, Ctrl+Tab/W/\\, Ctrl+S e indicador de modificado | ✓ na janela, inclusive arrastar abas: para a borda (cria grupo), para a faixa de outro grupo e para cima de uma aba (entra antes dela) |
| Terminais embutidos (ConPTY + xterm) | ✓ na janela: várias abas no painel inferior, cada uma com o seu processo; terminal como aba do editor arrastado de um grupo para outro sem matar o processo |
| Provedores claude/opencode e status | ✓ na janela ("conectado" nos dois). "Trocar de conta" do OpenCode abre o `opencode auth login` num terminal (visto; fechado sem concluir). O do claude não foi aberto (o login dele abre o navegador) |
| Modelo por conversa (pílula, `/modelo`, cabeçalho, banco) | ✓ na janela com tarefas reais: lista do claude (opus[1m], claude-fable-5[1m], sonnet, haiku) e do opencode (146, com busca); sessão trocada de haiku para sonnet e a linha de uso seguinte veio como `claude-sonnet-5`. ⚠ a troca de modelo no claude regrava o cache inteiro da conversa (visto: 107 mil tokens de cache gravados, US$ 0,64 equivalentes em API num turno de uma linha) |
| Troca de agente na sessão (Claude Code ↔ OpenCode) | ✓ na janela com tarefa real: claude → OpenCode, divisória no chat, sessão nova do opencode respondeu certo qual tinha sido o primeiro pedido e o arquivo alterado (veio do resumo). Voltar ao provedor anterior retomando a sessão guardada: só teste automático |
| Modo de trabalho (Isolado / Direto na pasta) e aviso antes do envio | ✓ na janela com tarefas reais: pasta fora do git + Isolado mostra o aviso antes de enviar, com o caminho citado em destaque; "Continuar isolado" roda no worktree e o agente responde em 2 passos que a pasta não existe na cópia; "Usar Direto na pasta" roda na pasta, acha e edita o arquivo, e a aba Alterações mostra o diff sem Aplicar/Descartar. ⚠ o aviso de segunda sessão direta e as preferências em Configurações gravando: lógica com teste automático, não exercitados com duas sessões reais |
| Git e inspeção liberados por padrão; "Comando negado" no chat | ✓ na janela com tarefa real (claude): `git status` e `git log -1` rodaram, `git push` foi negado e apareceu a caixa "Comando negado" com o atalho. ⚠ níveis "Git local"/"Git remoto" ligados e o clique no atalho: montagem com teste automático, não vistos numa tarefa real. ✓ OpenCode 2.3.0 rodou `git status` na tarefa #25 (ferramenta `shell`, exit 0); permissões dele não foram alteradas |
| Tarefas em worktree isolado, stream, diff, responder (`--resume`) | ✓ na janela com tarefas reais do claude: Aplicar leva a alteração ao projeto (commit, merge, worktree e branch removidos) e Descartar remove o worktree e a branch. ✓ Aplicar (#25) e Descartar (#26) conferidos com o opencode 2.3.0; worktree removido nos dois casos |
| Tarefa pelo OpenCode | ✓ na janela com `ARGUS_OPENCODE_MODEL=openai/gpt-6-luna`: stream, ferramenta, tokens e custo equivalente. ✓ 2.3.0 emite o `step_finish` final com tokens (uso registrado); edição de arquivos pela ferramenta `write` conferida; `opencode/beam-free` mostra US$ 0,00 equivalente em API |
| Histórico SQLite persistente | ✓ na janela (a tarefa e o chat voltam depois de fechar e abrir) |
| Subagentes: árvore, cartão "→ subagente", chat somente leitura sem caixa de resposta | ✓ na janela com um subagente real do claude |
| Subagentes do opencode e subagente em segundo plano do claude | ⚠ escritos sem conferir contra saída real. A linha real de ferramenta do opencode 2.0.22 foi conferida e virou teste (o id da chamada vem em `part.id`), mas nenhuma execução trouxe a ferramenta `task` |
| Métricas de tokens e custo equivalente em API, gráfico por dia | ✓ na janela (atualiza ao fim do turno). "Turnos" conta turnos, não linhas de uso (visto: 8 turnos com 11 linhas). Os dias seguem o fuso do computador (teste automático com fuso fixo; a virada do dia não foi vista na janela) |
| Dependências: detecção, confirmação antes de instalar, comando inexistente no terminal | ✓ na janela (só o pedido; nenhuma instalação foi executada) |
| Acesso remoto pela LAN (QR + token) | ✓ na janela: liga, QR, todas as rotas e o WebSocket sem token = 401, desliga, fecha a porta e derruba quem estava conectado |
| Web app do celular (refeita: projetos e sessões, conversa, subagentes, nova conversa, auto-work, servidores, temas) | ✓ num navegador emulando celular (390, 360 e 430 px, toque), contra o servidor real: lista, histórico da sessão #1, subagente sem caixa de resposta, `/context` enviado com a resposta chegando pelo WebSocket, reconexão, três temas, Derrubar num servidor real. ⚠ auto-work: ligar e desligar conferidos de verdade; progresso, Pausar, Retomar e Parar só com uma execução simulada na tela. ⚠ "Enviar" de Nova conversa não foi tocado (gastaria tokens). **Nunca aberta num celular de verdade** |
| Auto-work (PENDENCIAS.md, TODO/FIXME, build/testes, travas) | ✓ na janela: um ciclo real com 2 itens, da pílula ligada até "auto concluído 2/2", com o selo, a barra de progresso e a etiqueta "auto" na barra esquerda. Pausar, Retomar, Parar, as travas e a verificação de build/testes conferidos pelo usuário em 2026-11-24 |
| Player de música (mídia do Windows + arquivos locais) | ✓ na janela: a mídia do sistema é detectada (título e autor do que estava tocando apareceram e mudaram sozinhos na status bar); tocar música conferido pelo usuário em 2026-11-24. Os arquivos locais seguem sem conferência registada |
| Pet de uso | ✓ na janela (dormindo e trabalhando, balão com os números) |
| Servidores + painel lateral | ✓ na janela com um terminal da IDE e com uma sessão de agente: com `python -m http.server:*` em Comandos permitidos, o servidor aberto pelo claude aparece no selo da sessão, na status bar e na aba Servidores ("Sessão #N"); "Derrubar" mata o processo e fecha a porta. Limite: o `claude -p` encerra os processos em segundo plano quando o turno termina, então o servidor só vive durante o turno |
| Conversa (aba única: conversa nova + sessão) | ✓ na janela com tarefas reais: o primeiro envio cria a tarefa na mesma aba, com stream ao vivo, Enviar virando Parar, fila, "1 agente rodando", ponto pulsando e negrito quando a novidade chega com a aba fora de foco |
| Caixa de composição numa linha (pílula "provedor · modelo", opções, pílula de projeto) | ✓ na janela em 2026-11-16: lista de modelos do claude (Padrão, opus[1m], claude-fable-5[1m], sonnet, haiku) e do opencode (busca, "Usar “nome”" para modelo digitado); botão de opções com explicações, Esc fecha e devolve o foco, contagem "1" com Direto na pasta num projeto git; pílula de projeto só na conversa nova. Corrigido na mesma rodada: a pílula de agente e modelo era cortada ("Claude Code · pad…") e, em aba estreita (split), a barra quebrava em duas linhas. "Carregando modelos…" não foi visto: a lista já estava pronta quando a pílula foi aberta |
| Melhorar pedido (✨ / Ctrl+I / `/melhorar`) e entrevista de briefing (`/briefing` e o convite na primeira conversa do projeto) | compila + core testado (`crates/core/src/polish.rs`, 9 testes: quando pular, montagem do meta-pedido, leitura da resposta, regra contra injeção; comandos `chat_polish_prompt`, `polish_mode_get/set`, `chat_briefing_prompt/missing`). **Falta ver na janela**: a reescrita com a CLI real (gasta tokens), o modo "em todo envio", o Desfazer e o convite de briefing num projeto sem `BRIEFING.md` |
| Configurações com navegação lateral (Tema, Agentes, Provedores, Por projeto) | ✓ na janela em 2026-11-16: os três temas trocam a interface inteira; Provedores mostra claude e opencode "conectado"; o rodapé não tem mais Provedores. Corrigido: num grupo estreito (split) a navegação não empilhava e os campos estouravam o cartão (agora pela largura do grupo) |
| Pasta raiz que se atualiza sozinha | ✓ na janela em 2026-11-16: criar, renomear e apagar uma subpasta de projeto aparece na lista em 440–565 ms; botão Atualizar redetecta também os comandos de build; projeto apagado com sessão vira "pasta ausente" (riscado) e some depois de excluir a última sessão |
| Excluir sessão e "Excluir encerradas" | ✓ na janela em 2026-11-16: pelo menu da barra esquerda e pelo clique direito no seletor de conversas do cabeçalho (caixa nativa com o texto certo para modo Direto), aba da sessão fecha; "Excluir encerradas (1)" depois de descartar uma sessão isolada do OpenCode (worktree e branch removidos). Métricas iguais antes e depois (US$ 0,18 → 0,18; no fim, US$ 0,79 com 5 turnos, incluindo os das sessões excluídas) |
| Janela de contexto (% no cabeçalho) | ✓ na janela em 2026-11-16 com haiku: 19% e dica "38.297 de 200.000 tokens"; opus[1m]: 4% de 1 M; OpenCode sem tamanho de janela conhecido mostra só os tokens |
| Mapa de agentes | ✓ na janela em 2026-11-16 com um subagente real (Task, haiku): aberto pelo cabeçalho e por "Abrir mapa" da aba Subagentes; dois nós ligados por linha, "2 agentes"; Enter no nó do subagente abre o chat somente leitura (prompt recebido, resposta, resultado devolvido, sem caixa de resposta) |
| Builds (tela no rodapé) | ✓ na janela em 2026-11-16: comandos detectados de um projeto Node, de um Cargo e de uma cópia dos manifestos do Argus (app:dev :1420, dev :5173, preview :4173, app:instalador, build, cargo build/--release); `npm run build` trivial: histórico "concluído · 1s · botão", artefato `dist 1 B` com Abrir pasta, aviso "Build concluído em …" na status bar. Builds pesados do Argus não foram rodados |
| Servidor de dev com um clique | ✓ na janela em 2026-11-16 (script `vite` falso): ▶ na tela Builds e no hover do projeto, "pronto" com a porta respondendo, Abrir no navegador, Parar fecha a porta; com a 5173 ocupada, "A porta 5173 está ocupada; usando a 5174." e o servidor sobe na 5174. Corrigido: o botão do comando mostrava ":5173 (rodando)" com o servidor na 5174 |
| Anexos por aba de conversa nova | ✓ na janela em 2026-11-16: duas conversas novas (uma por grupo do split) gravam em `anexos\novo\<id da aba>\` diferentes. Observação: o "+" da barra esquerda e o "+" da faixa de abas reaproveitam a aba de conversa nova já aberta no grupo |
| Inspetor › Pendências (2026-11-19): check-list do PROXIMOS_PASSOS.md, PENDENCIAS.md ou TODO.md da raiz (do worktree da sessão, se houver), feito/total e barra por seção, tarefas TodoWrite do agente no turno com o `in_progress` destacado, item que o agente editou no arquivo destacado, quadro "Trabalhando agora" no topo, clique num item aberto põe "Trabalhe na pendência: …" na caixa | compila + core testado (parser em `crates/core/src/checklist.rs`, comando `project_checklist`; tela em `src/views/session/ChecklistPanel.tsx` e `checklist.ts`). Falta ver na janela: atualização ao vivo pelo watcher, destaque durante um turno real e quatro abas cabendo em 340 px |
| Passada rápida (2026-11-16) | ✓ terminal (eco no PowerShell), explorador e editor Monaco, Métricas, Remoto (liga na 8787, QR, `/` sem token = 401, desliga e a porta fecha), Dependências ("Tudo em ordem"). Nenhum erro nem exceção no console ao recarregar a página e percorrer as telas. Corrigido: com muitos terminais, a faixa de abas do painel inferior vazava por cima do painel direito |

O pedido de instalação por comando inexistente só aparece para ferramentas conhecidas (tabela em `crates/core/src/deps.rs`, por exemplo `gradle`, `pnpm`, `go`); um comando qualquer não abre nada.

## O que veio do chat do Digitama

Tudo em `src/views/chat`, com os comandos Rust em `src-tauri/src/commands/extra_chat.rs`. A aba de conversa (`src/views/session`) usa esses componentes. Os estados abaixo são de antes do redesenho; a caixa foi reorganizada (botões e pílulas) e não foi reexercitada com um agente rodando.

| Item | O que faz | Estado |
|---|---|---|
| 1.1 Markdown | títulos, listas, tabelas, código com rótulo da linguagem, copiar código e copiar mensagem | ✓ na janela (título, lista, código com rótulo); os botões de copiar aparecem, o clique não foi conferido |
| 1.2 Fila de mensagens | com o agente ocupado, Enter põe na fila; a fila sai quando o turno termina; Ctrl+Enter para o turno e envia | ✓ na janela com agente real: Enter, saída da fila e Ctrl+Enter (pergunta, para o turno e manda a mensagem) |
| 1.3 Menções | `@arquivo` (busca no projeto) e `@agente` (arquivos de `.claude/agents` e `.opencode/agents`) | ✓ na janela |
| 1.4 e 1.5 Anexos | clipe, arrastar e soltar, colar print, texto colado grande vira anexo; miniaturas na mensagem | ✓ na janela: clipe (.txt e .png), texto grande colado, miniatura, imagem colada (evento sintético) e lida pelo claude numa tarefa real, arquivo arrastado da aba Arquivos (vira `@caminho`). Arquivo arrastado do Explorer do Windows e Ctrl+V de um print de verdade conferidos pelo usuário em 2026-11-24 |
| 1.6 Comandos de barra | `/nova`, `/parar`, `/diff`, `/aplicar`, `/descartar`, `/sessoes`, `/modelo`, `/ajuda` e os comandos da CLI do claude | ✓ na janela: menu com os do Argus e os da CLI, `/ajuda`, `/diff`; `/modelo` abre a lista de modelos da pílula (visto); `/nova` abre uma conversa nova e `/diff` abre a aba Alterações do painel direito (não reconferidos depois do redesenho). Desde 2026-11-18, a barra abre o menu **em qualquer ponto da mensagem** (começo, depois de um espaço ou de um parêntese), como o `@`, e a escolha troca só o trecho sob o cursor; os comandos do Argus continuam executando apenas quando a barra é a primeira coisa escrita (no meio da frase, o texto vai inteiro ao agente). Não conferido na janela |
| 1.7 Chamada de ferramenta escrita como texto | aviso com o botão "Executar de verdade" | ⚠ compila; detecção testada por script |
| 1.8 Uso do dia | pílula "hoje: X tokens · ≈ US$ Y (equivalente em API)" | ✓ na janela (status bar; atualiza a cada turno) |
| 2.4 (atalho) Detecção de loop | cancela o turno quando o agente repete a mesma chamada; só vale com a conversa aberta numa aba | ✓ na janela com agente real: o turno foi cancelado no terceiro `Read` igual, com o aviso "Parei: o agente repetiu Read 3 vezes seguidas" |

Ficou pendente do Digitama:

- **1.9 Conexões e cartão do repositório:** não feito (é o passo 9 abaixo).
- **Imagens anexadas com o claude:** resolvido. Os anexos ficam em `%APPDATA%\argus\anexos\<tarefa>\` (ou `novo\`, antes de a tarefa existir), fora da pasta de trabalho do agente; o core passa `--add-dir` com a pasta da tarefa e com as pastas dos anexos citados no pedido (`crates/core/src/attachments.rs`). Conferido numa tarefa real: o claude leu a imagem colada. Conversas novas usam `novo\<rascunho>\`, uma subpasta por aba, e o `--add-dir` libera só ela (conferido no código em 2026-11-16).
- **Comandos de barra da CLI:** na primeira vez que você digita `/` numa pasta, o Argus sobe um `claude -p --no-session-persistence`, manda `/context`, lê a linha de inicialização e mata o processo. Conferido contra a CLI real (claude 2.1.220), deixando o comando terminar: a resposta vem da própria CLI, com 0 tokens e custo 0. Para desligar mesmo assim, tire `chat_agent_capabilities` do `invoke_handler` em `src-tauri/src/main.rs`.
- **Comando de barra que o Argus não conhece** (por exemplo `/context`) continua indo para a CLI como mensagem, e a resposta aparece na conversa. Os que a CLI resolve sozinha (nenhum modelo chamado, tudo zerado) não geram linha de uso: não entram nas métricas nem na contagem de turnos da tarefa. Conferido com `/context` numa tarefa real.
- **"Turnos" nas métricas:** resolvido. Cada linha de uso leva o turno que a gerou (coluna `turn`), e as métricas contam turnos distintos.
- **`/modelo`:** resolvido. Abre a pílula de modelo; `/modelo nome` troca direto se a CLI informar esse nome.
- **`@agente`** vai como texto; os agentes embutidos de cada CLI não entram na lista.
- **Fila e anexos** vivem só na memória da janela: não sobrevivem a reinício nem aparecem no remoto.
- **Realce de sintaxe:** feito (2026-11-16, `src/views/chat/highlight.ts`, sem dependência nova; cores das variáveis existentes). A função foi conferida dentro da janela; um bloco de código numa conversa real ainda não foi visto.
- **Detecção de loop no core:** feita (2026-11-16, `crates/core/src/agents/loop_guard.rs`): 3 chamadas iguais seguidas (nome + entrada) encerram o turno com "Parei: o agente repetiu X 3 vezes seguidas", com a aba fechada e no remoto. Só testes unitários. **Pendente:** o `RepeatDetector` do frontend (`src/views/chat/toolcall.ts`, usado em `src/views/session`) continua ligado e pode dar aviso duplicado; tirar a parte de "seguidas" dele.
- **Ctrl+C no Explorer + Ctrl+V** anexando pelo caminho real.
- **Licença:** a lógica de menções e fila veio da versão reescrita do Digitama, que cita um projeto MIT de terceiros. Conferir antes de distribuir.

## Pendências de validação

Marcado com `[x]` o que foi visto funcionando na janela em 2026-11-14.

**Por que tanta coisa trava em "falta ver na janela" (apurado em 2026-11-21).** Dá para **fotografar** a janela sem ajuda: `ffmpeg -f gdigrab -framerate 10 -i "title=Argus"` grava em `.mkv` (formato que aguenta ser interrompido no meio), e `-frames:v 1` tira um quadro só. Foi assim que saíram os dois defeitos de hoje (a linha `(evento RateLimit)` e a compactação muda). O que **não** dá é **mexer** nela: numa instância `Argus Dev` subida à parte, nem `SendKeys` nem clique sintético (`mouse_event`) chegam ao WebView2 — o cursor até acende o estado de passagem do mouse sobre a caixa de busca, provando que o movimento chega, mas o clique e o `Ctrl+K` não abrem a paleta, com a janela comprovadamente em primeiro plano. Ou seja: **ver, sim; navegar, não.** Vale também notar que a instância `Argus Dev` tem identificador próprio mas **compartilha o banco** com a normal, então não é um ambiente limpo e não convém deixar as duas abertas juntas.

**A saída: a ponte de validação (feita em 2026-11-21).** Se o sistema operacional não deixa entrar de fora, entra-se por dentro. Com a variável `ARGUS_PONTE` definida, o Argus sobe um servidor minúsculo em `127.0.0.1:8787` que repassa pedidos para a própria página; o `src/dev/ponte.ts` executa lá dentro e devolve a resposta. Um `elemento.click()` dali passa pelos mesmos ouvintes de um clique humano — o React não sabe a diferença.

```powershell
$env:ARGUS_PONTE = "1"; npm run app:dev
```

```bash
node scripts/ui.mjs ver                 # título, texto visível e o que é clicável
node scripts/ui.mjs clicar "Métricas"   # pelo rótulo, ou --seletor <css>
node scripts/ui.mjs tecla k --ctrl      # abre a paleta
node scripts/ui.mjs esperar "Tudo certo" --ms 15000
node scripts/ui.mjs erros               # erros de console desde que a janela abriu
node scripts/ui.mjs js "document.querySelectorAll('.ap-mural-item').length"
```

Sai 1 quando a ação falha, então dá para encadear um roteiro e parar no primeiro erro. Três travas: só existe com a variável definida, só escuta em `127.0.0.1`, e não toca em arquivo nem no banco — tudo o que ela faz, a janela já fazia.

O limite que sobra é outro, e é honesto: a ponte opera a interface, não o mundo. Item que depende de um celular ligado, de uma conta com limite de plano de verdade ou de um turno caro continua dependendo de você — mas a parte que era só "alguém precisa clicar" deixou de depender.

- [x] Primeiro envio numa conversa nova: criar a tarefa e a mesma aba virar a sessão (com stream ao vivo, botão Parar e fila).
- [x] "N agentes rodando" na status bar e o ponto pulsando na barra esquerda com um agente de verdade; negrito de "não lida" com a aba fora de foco.
- [x] Pílula de auto-work ligando e desligando o projeto; seta para cima recuperando o pedido anterior.
- [x] Arrastar um arquivo da aba Arquivos para a caixa de composição (vira `@caminho`).
- [x] Terminal com as cores do tema.
- [x] Arrastar abas entre grupos e para a borda; colar imagem na caixa (evento sintético).
- [x] Arrastar um arquivo do Explorer do Windows para a conversa e Ctrl+V de um print de verdade (dependem do sistema; o protocolo de depuração não cobre). **Conferido pelo usuário em 2026-11-24.**
- [x] Aplicar e descartar uma tarefa com alterações (claude).
- [x] Aplicar e descartar uma sessão do OpenCode (ver o defeito do worktree preso, abaixo). **Conferido em 2026-11-21**: Aplicar na #25 e Descartar na #26, ambos sem worktree preso.
- [x] Auto-work rodando um ciclo na interface nova.
- [x] Auto-work: Pausar, Retomar, Parar, travas e verificação de build/testes na janela. **Conferido pelo usuário em 2026-11-24.**
- [x] Player tocando música (só a detecção da mídia do sistema foi vista). **Conferido pelo usuário em 2026-11-24.**
- [x] Login pela CLI num terminal (OpenCode; aberto e fechado sem concluir).
- [x] Login do claude num terminal (abre o navegador; não foi aberto). **Conferido pelo usuário em 2026-11-24.**
- [x] Tarefa pelo OpenCode.
- [x] Web app aberta num celular de verdade (2026-11-19, SM-M556B, Android 16, APK debug 0.1.2): pareamento pelo Funnel (`https://<pc>.ts.net/pair?token=…`), lista de sessões, histórico de uma sessão, aba Servidores e indicador "ao vivo". Ao desligar, `tailscale funnel status` ficou sem configuração. Corrigidos no teste: biblioteca nativa sem alinhamento de 16 KB (aviso do Android 16; `src-mobile/build.rs`), "Último computador usado" vazio aparecendo sem nada salvo (`[hidden]` vencido pelo `section { display:flex }`), texto do rodapé que dizia que nada ia para a internet, números em formato inglês na web app.
- [x] App do celular: a linha `FORTIFY: pthread_mutex_lock called on a destroyed mutex` no buffer de crash só apareceu num `am force-stop`; `src-mobile/src/lib.rs` só monta o Tauri (sem thread nem mutex do Argus), então é do encerramento forçado do runtime, não do app. Fechado como externo.
- [x] Servidor vinculado a uma sessão de agente.

## Defeitos conhecidos

Corrigidos (`[x]` = visto funcionando; `[ ]` = corrigido no código, falta ver):

- [x] As dez áreas abrem e o console fica limpo ao passar por todas (`npm run teste:telas`, pela ponte de validação — foi a primeira vez que isso pôde ser conferido sem alguém clicando).
- [ ] Dois agentes editavam o mesmo arquivo sem saber um do outro, e o conflito só aparecia no diff. Agora o Argus anota as ferramentas de arquivo que passam (`crates/core/src/mural.rs`) e avisa no começo do turno seguinte; falta ver o painel "Arquivos em uso" com duas sessões reais mexendo na mesma pasta.
- [x] "Enviar" em Nova tarefa não abria a aba da sessão. A conversa nova cria a tarefa e a mesma aba vira a sessão (visto com tarefas reais).
- [x] As métricas contavam 2 turnos quando um turno usava dois modelos (teste automático; na janela, 8 turnos para 11 linhas de uso).
- [x] O botão "Fazer login" aparecia mesmo com o provedor já conectado. Agora, conectado, aparece só "Trocar de conta" (visto na aba Provedores).
- [x] Imagens anexadas não eram lidas pelo claude, por ficarem fora da pasta do projeto: `--add-dir` com as pastas de anexos (visto numa tarefa real).
- [x] O aviso da CLI do claude sobre servidores MCP bloqueados mostrava o nome da organização do usuário no chat. As linhas que começam com "Warning: claude.ai MCP servers blocked" não são mais renderizadas (`isHiddenNotice` em `views/session/events.ts`). O texto continua gravado no banco; no acesso remoto ele é filtrado no servidor e não chega ao celular.
- [x] Um comando de barra resolvido pela própria CLI contava como turno e entrava nas métricas (visto com `/context`).
- [ ] "Hoje" nas métricas virava às 21h de Brasília. Corrigido no código (os dias seguem o fuso do computador) e coberto por teste automático com fuso fixo; **a virada do dia ainda não foi vista na janela** (confira depois das 21h).
- [x] Uso contado em dobro: quando um comando em segundo plano termina depois da resposta, o `claude -p` acorda e imprime um segundo `result` com o total ACUMULADO do processo. O parser agora desconta o que o `result` anterior já informou (teste com os números da execução real).
- [x] O OpenCode trabalhava na pasta errada quando o Argus era aberto por um shell (Git Bash, `npm run tauri dev`): a CLI confia na variável `PWD`, herdada de quem abriu o Argus, em vez do diretório real. O Argus agora apaga `PWD` ao iniciar e a acerta em cada processo filho (visto: antes "README.md não encontrado", depois a resposta certa).
- [x] "Trocar de conta" do OpenCode falhava com "não é um aplicativo Win32 válido (erro 193)": o terminal executava o script sem extensão do npm. O terminal agora resolve o nome (`.exe` antes de `.cmd`) e roda `.cmd`/`.bat` por `cmd /c` (visto).
- [x] A aba Alterações mostrava "O nome do diretório é inválido (erro 267)" depois de aplicar ou descartar. Sessão encerrada não calcula mais o diff (visto).
- [x] "hoje: … US$ -0,00" na status bar sem uso no dia (visto).
- [x] A aba ativa ficava meio escondida debaixo dos botões da direita quando a faixa de abas transbordava (visto).
- [ ] O selo de auto-work oferecia "Reiniciar auto-work" numa sessão já aplicada ou descartada. Corrigido no código, **ainda não revisto na janela** (depois de reiniciar o app o selo some, e o teste foi feito assim).
- [x] Não havia como abrir um terminal como aba do editor (o tipo existia, nenhum botão o criava): botão "⧉" na barra de terminais (visto, com a aba mudando de grupo).

- [x] O agente respondia "não achei a pasta" depois de vários passos: o worktree só tem o que está commitado e nada avisava. Agora há o aviso antes do envio, o modo "Direto na pasta" e o agente isolado sabe onde está (visto com tarefas reais: de 8 passos e US$ 1,82 para 2 passos e US$ 0,15 com o modelo haiku).
- [x] O agente não conseguia rodar `git` nem comando nenhum: no Windows o claude usa a ferramenta `PowerShell`, e só `Bash(...)` da lista do projeto era liberado. Conjunto padrão para as duas ferramentas (visto com tarefa real).
- [x] A lista de escolha (`Select`) com busca não recebia o foco ao abrir: digitar para filtrar e as setas não funcionavam (o foco era pedido antes de a lista existir). Corrigido e visto.
- [x] A lista de escolha fechava quando a janela perdia o foco, inclusive no meio da consulta de modelos à CLI. Não fecha mais por perda de foco.

Em aberto:

- [x] (2026-11-21) Sem a saída bruta do OpenCode, os defeitos dele ficavam sem diagnóstico. Agora todo `opencode run` leva `--print-logs --log-level debug` (com `--standalone`, os logs do servidor privado também saem) e o Argus grava stdout e stderr em `%APPDATA%\argus\logs\opencode-<tarefa>-<turno>.log`, sempre (`crates/core/src/agentlog.rs`). Linhas `timestamp=... level=...` vão só para o arquivo, não para o chat; o resto do stderr continua virando evento `Stderr`. Valores de credenciais são mascarados antes da gravação. **Conferido na janela em 2026-11-21**: tarefa #25 (`opencode/beam-free`) gerou `%APPDATA%\argus-dev\logs\opencode-25-1795281349852.log` (71 978 bytes) com `meta`, `cmd`, `out` e `err`, incluindo os logs `timestamp=... level=...` da CLI fora do chat. Gates passaram.
- [ ] (2026-11-21) A pílula "Trabalhando" nunca girou: o ícone `LoaderCircle` só recebia cor, sem `animation` (a única rotação do projeto era `.pj-spin`, de outra tela). Medido num vídeo da janela: zero repinturas em 4 s a 60 fps. Corrigido em `session.css`; falta ver na janela.
- [ ] (2026-11-21) Nada avisava que a CLI tinha emudecido: `idle_secs` só é lido **antes** do turno (para compactar), então uma sessão parada há 5 minutos ficava igual a uma respondendo. Agora a pílula `StalledPill` mostra o tempo sem resposta a partir de 1 min, com "Interromper". Falta ver na janela.
- [ ] (2026-11-21) Cada `TextDelta` redesenhava a conversa inteira (um `setState` por token; `MessageView` era `memo` mas inerte, porque todo call site escreve `event={{ … }}` inline). Agora os deltas saem num `setState` por quadro (`requestAnimationFrame`), `MessageView` compara a variante do evento e `ToolRow`/`ThinkingRow` são `memo`. Falta medir com um turno longo de verdade.
- [ ] (2026-11-21) Limites de uso do plano: o popover era meio inventado. Três defeitos. (a) O orquestrador gravava o evento inteiro (`{"RateLimit": …}`) e a interface esperava o conteúdo, então **nada guardado voltava** depois de reiniciar — só valia o evento ao vivo do turno. (b) `rate_limits` lia só `ratelimit.claude.*`, mas a chave gravada é `ratelimit.<provedor>.<janela>`. (c) A interface inventava as linhas `five_hour` e `seven_day` com "ainda não informado" quando a CLI não as informava, e mantinha a porcentagem de um ciclo já vencido. Agora grava só o conteúdo, lê todos os provedores, descarta janela com `resets_at` no passado e não inventa linha nenhuma. Falta ver com turnos reais do claude.

- [ ] (2026-11-24) Janelas de 5 horas do plugin `usage-window` do OpenCode na bolinha de uso: leitura do `state.json`, cálculo do percentual e ajustes `set_limit`/`reset_samples` implementados em `crates/core/src/usage_window.rs`, comandos Tauri e `UsageRing.tsx`. Gates passaram (typecheck, 282 testes do core, `cargo check`, `teste:celular`, build e `node --check`). Falta ver na janela com turnos reais de modelo gratuito, inclusive a escrita no arquivo do plugin e a atualização a cada 30 s.

- [ ] (2026-11-21) **Compactação sem aviso nenhum.** Quando a conversa passa do limite (ou ficou parada além da vida do cache), o Argus roda um turno `/compact` **antes** da mensagem. Esse turno pode levar minutos sem imprimir nada, e a tela ficava idêntica a uma CLI travada: "Trabalhando" parado e, desde a correção da pílula, ainda um "N min sem resposta" — um alarme falso, porque o silêncio estava explicado. Visto na gravação da janela de 2026-11-21 (183 mil → 11 mil tokens). Agora o orquestrador manda um `AgentEvent::Compacting { reason }` (só ao vivo, fora do histórico) antes do turno, e a pílula diz "Compactando a conversa · N min", em cor de "rodando" e não de aviso, com o motivo no tooltip (`size` ou `idle`) e o "Interromper". O fim vem do `Compacted`, com o `Result` como rede de segurança. Falta ver na janela.
- [x] (2026-11-21) A linha solta **"(evento RateLimit)"** aparecia no meio da conversa (vista na gravação da janela): o limite de uso chega ao vivo, não é conversa e não tinha tratamento, então caía no ramo "evento desconhecido" do `Transcript`. Agora ele sai antes de entrar na lista (`useTaskEvents`), e `Session`/`Context`/`RateLimit`/`Compacting` são ignorados também no `chatEntries`.
- [x] (2026-11-21) A seleção dos limites de uso (desembrulhar o formato antigo, exigir `window`, descartar ciclo vencido) saiu do comando Tauri e foi para `live_rate_limits`/`rate_limit_key` no `orchestrator.rs`, **ao lado de quem grava**, com teste. O formato já se separou duas vezes quando leitura e escrita moravam em arquivos diferentes; agora não dá para mudar um sem ver o outro.
- [x] (2026-11-21) **Defeito que eu mesmo criei na rodada anterior:** ao fazer o `rate_limits` devolver o conteúdo em vez do evento inteiro, o lado TypeScript continuou desembrulhando `.RateLimit` (`api/commands.ts`), o que transformava a lista inteira em `undefined`. Ou seja: o popover de limites continuaria vazio mesmo com as três correções do banco. Corrigido.

- [x] (2026-11-16) Formatos misturados. `formatIntPt`, `formatTokensPt` e `formatUsdPt` em `src/api/util.ts`. **2026-11-21:** `src/views/session` está inteiro nas versões `Pt` (`index.tsx`, `Transcript.tsx`, `ContextMeter.tsx`) e `formatTokens`/`formatUsd` viraram apelidos das versões `Pt`, sem nenhuma chamada restante — conferido por busca no `src/`. Falta só rever na janela.
- [x] (2026-11-16) Modelo gratuito do OpenCode aparecia com custo "equivalente em API". Corrigido no core: nomes terminados em `-free`, `:free` ou `/free` custam 0 (`metrics.rs`, com teste). **2026-11-21:** o custo continuava errado na janela (US$ 0,0053 para `opencode/beam-free`, embora a CLI reportasse `cost: 0`) porque o frontend recalculava com `priceFor` sem a regra de modelo gratuito. Corrigido em `src/views/session/events.ts` (mesma regra de `is_free_model` do core). Conferido na tarefa #25: cabeçalho e linha de uso passaram a mostrar `US$ 0,00 equivalente em API`.
- [x] (2026-11-16) Web app do celular passou a usar Geist, embutida no binário e servida em `/geist.woff2` (atrás da mesma autenticação). A escala tipográfica não mudou (não há escala nos tokens além de `--font-ui`). **2026-11-21:** teste `geist_is_served_locally_and_referenced_by_css` confere a rota (200, `font/woff2`, `nosniff`), que o arquivo embutido começa com a assinatura `wOF2` e tem tamanho de fonte de verdade, e que a CSS declara `@font-face` apontando para `/geist.woff2` sem nada de fora. **2026-11-23:** conferido no SM-M556B (Chrome Android, página autenticada servida por `127.0.0.1:8765`): `getComputedStyle(document.body).fontFamily` = `"Geist Variable", system-ui, …`, `document.fonts.check('16px Geist')` = `true`, `document.fonts` mostra `Geist Variable:loaded` e `performance.getEntriesByType('resource')` registra `geist.woff2` (21 ms). Não caiu no fallback.
- [ ] (2026-11-16) A primeira tarefa haiku de um projeto gravou 73 mil tokens de cache (US$ 0,16 equivalentes para listar 3 arquivos); as seguintes leram o cache. Custo da CLI, não do Argus, mas vale avisar na pílula de modelo.
- [ ] Trocar o modelo de uma sessão do claude regrava o cache da conversa inteira no modelo novo: o primeiro turno depois da troca sai caro (visto: US$ 0,64 equivalentes em API). **2026-11-16:** a lista de modelos de uma sessão do claude com histórico mostra o aviso; falta ver na janela.
- [ ] `cd <pasta>; git status` escrito pelo agente no PowerShell foi negado numa tarefa real (o `cd` não estava liberado). `cd` e `Set-Location` entraram no padrão, mas a correção não foi vista funcionando: no teste seguinte o agente se recusou sozinho a encadear o `cd`.
- [ ] OpenCode: permissões de comando seguem a configuração de agentes do usuário (com regras "ask" e "deny" próprias); o Argus não as altera. Num `opencode run` sem interação, o que cai em "ask" não tem quem aprove. **2026-11-21:** comando git real conferido na tarefa #25 — `tool_use` com `tool: "shell"`, `git status`, `status: completed`, `exit 0` no log. As regras "ask"/"deny" da config do usuário continuam sem conferir.
- [ ] O aviso de ambiente do modo Isolado não impede o agente de procurar: no teste ele ainda fez 2 buscas antes de responder.
- [x] Web app do celular: a divisória de troca de agente, a linha "Comando negado" e o provedor/modelo no subtítulo foram escritos e passam no `node --check`, mas não foram vistos numa tela. **2026-11-21:** `npm run teste:celular` (`scripts/teste-webapp.mjs`) carrega o `app.js` de verdade num DOM mínimo e monta as três coisas a partir de eventos: a divisória sai de um `ProviderSwitch` com os dois agentes, trocas seguidas viram uma só, ida e volta não deixa divisória, a divisória fica sozinha entre os turnos (fora de "N passos"); `Denied` vira a linha com ferramenta e comando (e sem comando mantém o nome); o subtítulo some sem dado, mostra tokens em pt-BR sem janela conhecida e a porcentagem com os níveis em 80% e 95%. **2026-11-23:** visto no SM-M556B (viewport 384 px): tarefa #17 mostrou `Comando negado`, o comando `python -c print(7*6)` e o botão `Liberar neste projeto` (188×40 px, toque confortável); tarefa #9 mostrou `Ferramenta negada: Write`. O subtítulo trouxe `demo · Claude Code · haiku · direto na pasta · #17`, com reticências (`overflow: hidden; text-overflow: ellipsis`) e o selo `contexto 19%` intacto; sem estouro horizontal (`body.scrollWidth` = 384 = `innerWidth`). A divisória foi desenhada na própria página do aparelho com o markup de `.switch-line` (356×18 px, texto central, sem quebra ou estouro); a montagem a partir do evento `ProviderSwitch` continua coberta pelo teste automático.
- [x] Turno cancelado não registra uso. **2026-11-16:** corrigido no código: soma o `usage` das mensagens `assistant` vistas depois do último `result` e grava uma linha parcial (custo estimado pela tabela). **2026-11-24:** conferido com um turno real do OpenCode na janela (tarefa #34, `opencode/beam-free`, modo isolado). O prompt pedia a leitura de quatro arquivos e a escrita de `relatorio.md`; depois de um `step_finish` (`input: 12587`, `output: 55`, `reasoning: 87`) cliquei em "Parar o turno". A sessão ficou "Interrompida" e a linha de uso mostrou `12,6 mil entrada · 142 saída · cache 0 lido · 0 gravado · US$ 0,00 equivalente em API`. O banco gravou `usage` id 46 (`task_id 34`, `12587/142`, `cost 0`) e a linha permaneceu depois de excluir a sessão. Log: `%APPDATA%\argus-dev\logs\opencode-34-1795551521876.log`. Nota: o número 142 saída é 55 `output` + 87 `reasoning`, do passo que terminou antes do cancelamento; nenhum número foi inventado para o passo interrompido.
- [x] OpenCode 2.0.22 não imprime o `step_finish` do último passo no `run --format json` (numa tarefa real: 10 959 de 11 547 tokens de entrada registrados). **2026-11-16:** o resultado agora é sintetizado no fim do processo; o uso do último passo só entra se outra linha trouxer os tokens (nenhum número inventado). **2026-11-21:** conferido com o opencode 2.3.0 na tarefa #25: o último `step_finish` (`reason: "stop"`) veio com `input: 61`, `output: 45`, `reasoning: 31`, `cache.read: 13376`, e a tela mostrou `61 entrada · 76 saída · cache 13,4 mil lido`. A CLI atual já emite o passo final; a síntese do core continua como rede de segurança. **2026-11-24:** reconferido já sem `--standalone` (serviço compartilhado da Fase 2) nas sessões #32 e #33: o log bruto trouxe `step_finish` com `reason: "stop"` e a tela mostrou `73 saída` e `97 saída` (soma de `output` e `reasoning`), com `cache 12,5 mil lido` na #33. Nenhum número foi preenchido pelo cliente.
- [ ] `which_bin` agora segue a ordem do PATH e, em cada pasta, a da PATHEXT (antes: `.exe` sempre antes de `.cmd`). Coberto por testes com pastas falsas. **2026-11-21:** na máquina há uma instalação real só (a do npm: `opencode.ps1`, `opencode.cmd` e o arquivo sem extensão na mesma pasta); o `opencode run` disparado pelo Argus resolveu para `...\@opencode\cli\bin\opencode.exe`. Não há dois executáveis distintos para comparar a preferência.
- [x] Worktree preso por processo da sessão: Aplicar e Descartar agora encerram os processos ligados ao worktree (árvore da sessão, cwd ou linha de comando dentro da pasta), tentam a remoção 4 vezes com espera curta e, por fim, `--force --force` + `prune`; se a pasta continuar, dão erro e mantêm a branch e o estado (dá para repetir). Seleção dos processos coberta por teste. **2026-11-21:** conferido com o serviço real do opencode (`--standalone`): Aplicar na tarefa #25 levou `NOTA.md` ao projeto e removeu `.argus-worktrees/proj-25`; Descartar na tarefa #26 removeu `.argus-worktrees/proj-26` sem levar `DESCARTAR.md` ao projeto (estado `Discarded`). Nenhum processo ficou prendendo o worktree. **2026-11-24:** reconferido já sem `--standalone`: Aplicar na #32 e Descartar na #33 removeram `proj-32` e `proj-33`, com confirmação Sim/Não nas caixas nativas (a ponte não as vê). O `index.lock` que apareceu numa leitura foi transitório, criado por um `git status` paralelo; "Recarregar o diff" resolveu e não houve worktree preso.
- [ ] (2026-11-21) Ao calcular o diff da tarefa #26, o painel mostrou `git add -A -N: fatal: Unable to create '.../.git/worktrees/proj-26/index.lock': File exists`. Não havia processo git e o lock desapareceu sozinho segundos depois — disputa entre o cálculo do diff e outro comando git no mesmo worktree. **2026-11-26:** corrigido em `crates/core/src/agents/orchestrator.rs`: `git_index_safe` repete até 4 vezes só quando a falha contém `index.lock` e `File exists`, com espera de 120/240/360 ms; qualquer outro erro continua indo para a interface. Usado em `diff()` no modo isolado. Teste `git_index_safe_waits_for_a_transient_lock` cria um `.git/index.lock`, remove depois de 180 ms e confirma que o comando passa. Falta ver com dois git paralelos de verdade na janela.
- [ ] O servidor aberto pelo claude numa sessão morre quando o turno termina (a CLI encerra os processos em segundo plano). Para um servidor que sobreviva ao turno, o agente precisa iniciá-lo desvinculado, o que pede outro comando permitido. **2026-11-26:** com o host persistente da Fase 3 o processo do agente e seus filhos passam a sobreviver ao turno; continua sem conferência com sessão real porque `claude auth status` devolve `loggedIn: false` nesta máquina. No OpenCode o servidor em segundo plano já foi visto vivo depois do `run` (sonda na seção de 2026-11-26).
- [x] `anexos\novo\` era compartilhada por todas as conversas novas: o `--add-dir` dessa pasta deixava o agente de uma conversa ler os anexos recentes das outras. **2026-11-16:** cada aba de conversa nova tem a sua subpasta `anexos\novo\<id>\` (visto na janela com duas abas).
- [x] Edição de arquivos pelo OpenCode v2 não conferida: sem `--auto`, o `run` pode negar a permissão de escrita. **2026-11-21:** conferido na tarefa #25 — `tool_use` com `tool: "write"`, `path: "NOTA.md"`, `status: completed`, saída "Created file successfully: NOTA.md"; o arquivo apareceu no worktree com o conteúdo `ok` e no diff do inspetor (`+1`). A escrita foi permitida sem `--auto`.
- [ ] Um turno retomado gravou um evento de resultado vazio (ok, custo 0) logo no começo. **2026-11-16:** `result` de sucesso sem texto, sem uso novo e custo 0 não vira mais evento. Causa deduzida (não reproduzida): `result` de zero turnos do processo retomado ou repetido com o mesmo acumulado.
- [x] Pareamento pelo celular dava "Não autorizado" ao abrir o link do QR (visto pelo Tailscale). Causa provável: cookie `SameSite=Strict`, que o navegador do celular não envia quando o link vem da câmera ou de outro app. Corrigido no código (cookie `Lax` e página de pareamento que segue sozinha para a tela inicial). **2026-11-21:** teste `qr_cookie_authorizes_next_navigation` faz o caminho inteiro — pareia, pega o `Set-Cookie` e navega com ele — e confere o que antes não era conferido: o cookie **autoriza mesmo** `/`, `/app.js`, `/style.css`, `/geist.woff2` e `/api/tasks`, as mesmas rotas dão 401 sem ele, um cookie com outro valor não passa e revogar derruba o cookie já entregue. Também fixa `HttpOnly`, `Path=/` e `SameSite=Lax`. **2026-11-23:** fechado no SM-M556B. Com um token recém-gerado, um intent `VIEW` externo abriu `http://127.0.0.1:8765/pair?token=…` no Chrome e a página entrou direto em `Argus / ao vivo`, com a lista de sessões (OCR do screenshot e CDP: `document.body.innerText` começa com `Argus\nao vivo`). O cookie `Lax` autorizou a navegação de primeiro nível. Sem cookie válido a mesma origem mostra `Erro 401 / Não autorizado`, conferido na aba antiga depois da revogação do token.

## Codex e Gemini (2026-11-16)

**Estado: compila e os testes do parser passam; não exercitado na janela** (nenhuma das duas CLIs estava instalada na máquina de desenvolvimento).

| | Codex (`@openai/codex`, `codex`) | Gemini (`@google/gemini-cli`, `gemini`) |
|---|---|---|
| Código | `crates/core/src/agents/codex.rs` | `crates/core/src/agents/gemini.rs` |
| Login | `codex login` (conta ChatGPT); status por `codex login status` | `gemini` interativo ("Login with Google" ou `/auth`); sem comando de status: lê só o tipo escolhido em `~/.gemini/settings.json` e a existência de `oauth_creds.json`/`GEMINI_API_KEY`, nunca o conteúdo |
| Turno | `codex exec --json --sandbox workspace-write -c approval_policy=never --skip-git-repo-check -C <pasta> [-m modelo] [resume <thread_id>] -` | `gemini --output-format stream-json --approval-mode auto_edit --skip-trust [-m modelo] [--resume <session_id>]` |
| Pedido | entrada padrão (`-`) | entrada padrão (modo sem interface porque a entrada não é terminal) |
| Sem aprovação | edição e comandos de shell DENTRO do sandbox (escrita só na pasta de trabalho, sem rede); o que o sandbox barra falha (vira `Denied`) | só edição de arquivos na pasta de trabalho; shell e o resto que pediria aprovação é negado pela CLI |
| Modelos | lista fixa (gpt-6.1-sol, gpt-6.1-terra, gpt-6-astra, gpt-6-luna) + digitação | `gemini --list-models --output-format json`; se falhar, lista fixa |
| Contexto | só em turno sem ferramentas (o `usage` é soma dos pedidos); janela conhecida só do gpt-6.1-sol (500 mil) | idem; janela de 1 milhão |
| Custo | tabela oficial da OpenAI; o codex não informa o modelo, então "Padrão da CLI" cai no fallback | tabela oficial do AI Studio, uma linha de uso por modelo |

Isolado: os dois recebem o aviso da cópia na frente do primeiro pedido (como o opencode). Anexos do chat ficam fora da pasta de trabalho e não são liberados para eles (`--add-dir` é só do claude).

Falta:
- instalar (botão "Instalar..." em Provedores, que usa o fluxo de Dependências), fazer login e rodar uma conversa de cada na janela: conferir o stream real, retomada, troca de agente e custo;
- conferir as suposições: total acumulado do `turn.completed` do codex numa retomada (o Argus desconta o último total visto da thread; depois de reiniciar o app, o primeiro turno retomado pode contar o total inteiro), raciocínio dentro de `output_tokens`, `--skip-trust` aceito pela versão instalada do gemini, `gemini` sem `-p` entrando no modo sem interface;
- sandbox do codex no Windows (é experimental na CLI): verificar se `workspace-write` realmente bloqueia escrita fora da pasta;
- "Liberar este comando neste projeto" não vale para o codex (a lista de comandos permitidos é do claude).

## Tarefas agendadas e "Iniciar com o Windows" (2026-11-19)

Branch `feat/tarefas-agendadas`. **Estado: core testado; compila; a tela não foi exercitada na janela.**

- **Core** (`crates/core/src/schedules.rs`, tabela `schedules(id, data JSON)` em `db.rs`): nome, tipo (pedir a um agente ou só lembrar), projeto, provedor, modelo, modo de trabalho (ausente = padrão do projeto), pedido, recorrência (`Once` em data/hora, `Daily` HH:MM, `Weekly` dias + HH:MM, `EveryHours` contadas da criação, `Cron` de 5 campos com `*`, listas, faixas e `/passo`), ligado, última e próxima execução, id da última tarefa, último resultado e "atrasada". Horários no fuso do computador (`chrono::Local`); hora inexistente na troca de horário de verão vira a primeira hora que existe depois.
- **Laço** (`Scheduler::start`, a cada 30 s, a primeira volta logo ao abrir): o que venceu dispara criando uma tarefa normal por `Orchestrator::submit_with` (mesmo caminho de "Nova conversa": histórico, uso e permissões iguais). Venceu com o Argus fechado: roda UMA vez, marcada "atrasada", e a próxima conta a partir de agora (nada se acumula). "Uma vez" desliga depois de rodar. Lembrete puro só mostra a notificação.
- **Notificação do sistema** (`src-tauri/src/commands/schedules.rs`, toast do Windows pelo crate `windows`, sem dependência nova): lembrete, tarefa agendada concluída, falhou, cancelada ou que nem iniciou. O app instalado usa o AUMID `dev.argus.app`; a cópia de desenvolvimento usa o do PowerShell (como o plugin oficial faz). O fim de turno das conversas comuns continua sem notificação do sistema.
- **Tela**: área "Tarefas" no rail (ícone de calendário com relógio): lista (nome, projeto · agente, recorrência em pt-BR, próxima, último resultado com "Abrir conversa"), ligar/desligar, "Executar agora", apagar (confirmação nativa) e o painel de criar/editar ao lado. Paleta: "Nova tarefa agendada". Evento `schedules-changed` atualiza a lista.
- **Iniciar com o Windows**: Configurações › Este computador › Inicialização. Grava `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` valor `Argus` = `"<exe>" --minimizado` pelo `reg.exe` (sem plugin, sem elevação); com `--minimizado` a janela abre minimizada. No perfil dev (`argus-dev`, qualquer build de depuração) a opção fica desabilitada com o motivo e o comando recusa.
- **Falta ver na janela**: criar/editar cada tipo de recorrência, "Executar agora" criando a conversa, o link "Abrir conversa", recuperação ao abrir depois de um horário perdido, e o registro de verdade no app instalado (reiniciar o Windows e ver abrir minimizado). O toast das tarefas agendadas foi conferido pelo usuário em 2026-11-24.

## Funções da extensão do VS Code portadas (2026-11-19, branch `feat/extensao-vscode`)

Valem para qualquer provedor. Estado: **core testado** (Rust) e **compila** (tela; nada visto na janela).

- **Acesso por conversa: Planejar / Normal / Bypass** (`permissions::AccessMode`, coluna `access` da tarefa, `Orchestrator::set_access`, comando `task_set_access`). Escolha em "Opções da conversa" do composer, depois do primeiro envio. Argumentos por provedor (cada `task_command` lê `TurnOptions::access`):
  - Planejar: claude `--permission-mode plan` (se o `--help` tiver `"plan"`; senão fica `acceptEdits`), gemini `--approval-mode plan`, codex `--sandbox read-only`, opencode `--agent plan`. Ao fim do turno, "Executar o plano" volta ao Normal e manda "Execute o plano que você propôs acima.".
  - Bypass: claude `--permission-mode bypassPermissions` (ou `--dangerously-skip-permissions` se a CLI não listar `bypassPermissions`), codex `--dangerously-bypass-approvals-and-sandbox` (sem `--sandbox`), gemini `--approval-mode yolo`, opencode `OPENCODE_CONFIG_CONTENT={"permission":"allow"}`. A política de shell do Argus (`permissions::apply`) não entra (`apply_for`). Nunca padrão, só na conversa onde foi ligado, confirmação pelo diálogo do sistema, selo vermelho "Bypass" no cabeçalho, na aba e na barra de status.
  - ⚠ conferido só a montagem: o claude 2.1.x lista `plan`/`bypassPermissions` no `--help`; codex e gemini não estão instalados aqui (flags da documentação); `opencode run --agent plan` e a config `permission: allow` não rodados.
- **Editor no chat**: Alt+K (e menu de contexto do Monaco) "Adicionar à conversa" insere `@caminho#L10-20` e o trecho na conversa visível. Acima do composer, chip "arquivo em foco · L10-20" (ligado por padrão; clique desliga) anexa o arquivo/seleção ao pedido, e chip "N erros do editor" (só com markers do Monaco; desligado por padrão) anexa os erros/avisos. Estado em `src/views/workbench/editorContext.ts`.
- **Rejeitar no diff**: `Orchestrator::revert_file` (volta ao último commit com `git restore --worktree`, ou apaga o arquivo novo) e `revert_hunk` (patch do trecho tirado do `git diff HEAD`, aplicado com `git apply -R` só nos arquivos), no worktree ou na pasta (Direto). Na aba de alterações: "Rejeitar arquivo" e "Rejeitar trecho acima" (quando há mais de um trecho), com confirmação. Bloqueado com turno rodando.
- **"Liberar e repetir"** no cartão Comando negado: libera o comando exato e manda o turno curto "o comando X foi liberado; rode de novo".

Falta ver na janela: menu de opções com os três modos e o diálogo do Bypass; selos na aba e na barra de status; "Executar o plano" depois de um turno real em Planejar; Alt+K com seleção e o chip de foco atualizando ao trocar de arquivo/grupo; chip de erros num .ts com erro; rejeitar trecho num arquivo CRLF; "Liberar e repetir" numa negação real.

Feito em 2026-11-19: escolher Planejar/Bypass antes do primeiro envio (`SubmitOptions::access`, `task_create` com `access`). Codex e gemini leem a memória: `--add-dir` / `--include-directories` só com a pasta `_Argus` (flags da documentação, CLIs não instaladas aqui). O ponto de "rodando" não pulsa mais (anel fixo).

Ficou de fora: checkpoints por turno ("Voltar a este ponto", ref `refs/argus/t<id>/<n>` com `git stash create` antes do turno); Alt+K quando não há conversa aberta (o texto só entra se uma aba de conversa estiver visível).

## Próximos passos (em ordem)

1. ~~**Terminar o teste na janela.**~~ Feito em 2026-11-24: edição de arquivos pelo OpenCode conferida no log bruto (`tool_use` com `tool: "write"`, `status: completed`) e no diff do inspetor. "Aplicar tudo" na sessão #32 levou `argus-aplicar.md` ao projeto (`APLICAR OK`, commit `9ee06f6 Merge branch 'argus/32'`, git limpo e worktree removido); "Descartar" na sessão #33 removeu o worktree sem levar `argus-descartar.md` ao projeto (sessão "Descartada", git limpo). As caixas nativas de confirmação Sim/Não foram clicadas por UI Automation: a ponte enxerga a página, não os diálogos do Tauri.
   Arrasto do Explorer, Ctrl+V de print, login do claude num terminal, player tocando música e auto-work (Pausar, Retomar, Parar, travas e verificação de build/testes) foram conferidos pelo usuário em 2026-11-24 e marcados como feitos acima.
2. **Fechar os defeitos em aberto** (lista acima). Os três primeiros desta fila foram fechados em 2026-11-24: uso de turno cancelado (tarefa #34, `usage` id 46), worktree preso e uso do último passo, ambos reconferidos com o serviço compartilhado nas sessões #32/#33 (detalhes na lista de defeitos). Continuam em aberto os demais itens marcados `[ ]` da lista — pílula "Trabalhando", `StalledPill`, redesenho por token, limites de uso do plano, janela de 5 horas do `usage-window`, compactação sem aviso, `index.lock` transitório, servidor que morre com o turno, entre outros.
3. **Abrir a web app num celular de verdade.** Na primeira vez, o Windows pede permissão no Firewall: clique em **Permitir** (rede privada). A web app foi refeita e conferida num navegador emulando celular; falta o aparelho real (teclado de verdade, área segura, "Adicionar à tela inicial", toque). Conferir também o auto-work com uma execução real e o primeiro envio de Nova conversa.
4. **Subagentes: conferir o que foi suposto.** Uma saída real do opencode com a ferramenta `task`, e um subagente em segundo plano do claude.
5. **Pequenos ajustes conhecidos:**
   - [ ] **Divergência de `ARGUS_PONTE`:** `src-tauri/src/ponte.rs` entende `ARGUS_PONTE=1` como a porta padrão 8787, mas `scripts/ui.mjs` interpreta qualquer número como porta — com `1` ele tenta `http://127.0.0.1:1` e falha. Os roteiros documentados no plano usam `=1`; para operar hoje, use `=8787` (ou deixe a variável sem valor no cliente). Alinhar o script ao Rust (tratar `1` como padrão) para que os comandos copiados do plano funcionem.
   - O botão "Reiniciar" de um servidor roda sem saída visível. Seria melhor reabrir o processo num terminal da IDE.
   - Janela separada de servidores não foi portada (a permissão para janelas `aux-*` já existe).
   - Trocar o `netstat` por GetExtendedTcpTable, o que exige adicionar a crate `windows` ao core.
   - Espaçar as verificações de build e testes do auto-work em projetos grandes, ou torná-las opcionais.
   - O OpenCode não informa o modelo usado: a linha de uso leva o modelo escolhido na pílula; com "Padrão da CLI" o custo cai na tabela de fallback.
   - Sessão sem `session_id` (primeiro turno falhou antes de a CLI abrir a sessão) continua sem resposta possível; daria para reaproveitar o resumo da troca de agente para recomeçar.
   - O modo de trabalho de uma sessão existente é fixo; "continuar esta conversa direto na pasta" pediria copiar o worktree ou abrir outra tarefa com o resumo.
   - A tabela de preços não tem o modelo "fable": vale o custo informado pela CLI; sem ele, cairia no fallback.
   - Web app do celular: HTTPS (para instalar como PWA de verdade), miniaturas dos anexos e fila de mensagens.
   - A fila de mensagens e o rascunho da "conversa nova" são compartilhados entre todas as abas de conversa nova (chave `new`).
   - `OrchestratorEvent` não deriva `Serialize`; por isso existe o espelho `OrchMsg` em `src-tauri/src/events.rs`. Derivar no core elimina o espelho.
   - TypeScript/JavaScript no Monaco só valida sintaxe (a validação semântica foi desligada).
6. **Empacotamento:** `npm run tauri build` gera o instalador. O Tauri avisa que o identificador `dev.argus.app` termina em `.app`, o que conflita com bundles do macOS; no Windows não afeta.
7. ~~Apagar `crates/app`~~: feito (commit `14d5fa6`).
8. **Uso de CPU e GPU por sessão.**
   - Feito no core e no painel Servidores (2026-11-16): `crates/core/src/usage.rs` (sysinfo + WMI `GPUPerformanceCounters_GPUEngine`, sem janela), comando `owners_usage`, componente `src/views/servers/SessionUsage.tsx`. Comando conferido na janela (lista vazia sem sessão rodando); números com uma sessão real não vistos. **2026-11-26:** `<SessionUsage owner={{ kind: "Session", id: taskId }} />` passou para o cabeçalho da sessão em `src/views/session/index.tsx`, exportado por `src/views/servers/index.tsx`. Antes o painel só mostrava donos que tinham servidor. **Pendente:** ver os números na janela com uma sessão rodando.
   - Mostrar, em cada sessão, o consumo de CPU, memória e GPU dos processos dela (agente, subprocessos e servidores vinculados).
   - A árvore de processos por sessão já existe em `crates/core/src/procs.rs`; a CPU e a memória vêm do `sysinfo`.
   - GPU por processo no Windows: contadores de desempenho "GPU Engine" (PDH) ou `nvidia-smi` quando houver placa NVIDIA.
9. **Conexões com serviços de desenvolvimento.**
   - Feito (2026-11-16): `crates/core/src/connections.rs` (gh, gcloud, firebase, vercel: instalada e logada perguntando à própria CLI; linhas com "token" descartadas), comandos `connections_status` e `connections_repo`, tela `src/views/connections/` com "Fazer login" no terminal embutido e `RepoCard.tsx`. Botão "Conexões" no rodapé da barra esquerda; tela vista na janela (GitHub conectado, gcloud e firebase não instalados, Vercel com "Fazer login") e `connections_repo` conferido com o próprio repositório. **Pendente:** clicar em "Fazer login" (não foi clicado) e pôr o `RepoCard` no chat (`src/views/session`).
   - GitHub, Google Cloud, Firebase, Vercel e outros.
   - Seguir a mesma regra dos agentes: usar a CLI oficial de cada serviço (`gh`, `gcloud`, `firebase`, `vercel`), que faz o próprio login. A IDE não lê nem guarda tokens.
   - Tela "Conexões": detectar a CLI, mostrar o status de login, botão "Fazer login" no terminal embutido e ações por projeto (PRs e issues, deploys, logs).
   - Inclui o cartão do repositório do chat (item 1.9 do Digitama).
10. **Mapa de agentes (Agent map).**
    - Janela ou painel "Mapa de agentes" por sessão: à esquerda o nó da sessão principal (nome, modelo, tokens no contexto) e à direita, ligados por linhas, os subagentes que ela chamou.
    - Cada nó de subagente mostra o nome, um ponto de status (verde = rodando, cinza = concluído), a duração e os tokens gastos.
    - Cabeçalho com a contagem ("6 agentes · clique em um agente para ver detalhes"). Clicar num nó abre o chat somente leitura daquele subagente.
    - Embaixo, a lista de tarefas em segundo plano da sessão (comandos de shell em execução, com o tempo), clicável para ver a saída.
    - Usa os eventos `SubagentStart`/`SubagentEnd` do core (já prontos) e a árvore de processos de `procs.rs`.
11. **Janela de contexto e % de uso.**
    - Mostrar, em cada sessão, quanto da janela de contexto do modelo está ocupada: uma linha (barra fina) com a porcentagem, no cabeçalho do chat ou junto da caixa de composição.
    - Passar o mouse sobre a linha abre o detalhe dos tokens em contexto: entrada, saída, cache lido, cache gravado, total e o tamanho da janela do modelo; se a CLI informar, a divisão por categoria (prompt de sistema, ferramentas, mensagens, arquivos).
    - Fontes: o campo `usage` de cada turno no stream-json e o tamanho da janela por modelo; para o claude, o comando `/context` devolve a divisão por categoria com custo 0.
    - Avisar quando passar de um limite (por exemplo 80%).
12. **Monitor de builds e instaladores.**
    - Feito (grupo C, 2026-11-16): `crates/core/src/runs.rs` (detecção, portas, artefatos), `src-tauri/src/commands/runs.rs` (execuções, watcher, histórico em `build_runs`), tela Builds no rodapé. Builds de fora dos botões são reconhecidos pela linha de comando (sem código de saída).
    - Acompanhar, por projeto, a geração de builds e de instaladores (por exemplo `cargo build`, `npm run build`, `npm run tauri build`, `gradle assemble`, `flutter build`): em andamento, concluído ou falhou, com duração, saída e o caminho e tamanho do artefato gerado.
    - Detectar os comandos de build de cada projeto pelos arquivos dele (scripts do `package.json`, `Cargo.toml`, `tauri.conf.json`, Gradle e outros) e oferecer um botão para cada um.
    - Histórico dos builds por projeto e aviso (pet, status bar e celular) quando um build termina ou falha.
    - Vale para builds iniciados pelo usuário num terminal da IDE e pelos agentes.
13. **Servidores de dev com um clique.**
    - Feito (grupo C, 2026-11-16): seção "Servidores de dev" no painel Servidores, botão no hover do projeto e no menu dele; porta livre mais próxima quando o comando aceita porta; último comando em `settings` (`devserver:<projeto>`).
    - Detectar os comandos de servidor de desenvolvimento de cada projeto (`npm run dev`, `npm run tauri dev`, `cargo run`, `vite` e outros) e mostrar um botão Iniciar/Parar por projeto, na barra esquerda e no painel Servidores.
    - Saber quando dá para abrir: bloquear ou avisar enquanto um build do mesmo projeto estiver rodando ou a porta estiver ocupada; mostrar "pronto" quando a porta começar a responder, com o botão Abrir no navegador.
    - Se a porta estiver em uso, escolher a livre mais próxima quando o comando aceitar a porta por parâmetro.
    - Lembrar o último comando usado em cada projeto e permitir "iniciar todos" de um conjunto.
    - Aproveita o rastreamento de processos e portas de `crates/core/src/procs.rs`.
14. **Memória como vault do Obsidian.**
    - Tratar a memória do Argus (notas, contexto e decisões que os agentes leem e gravam) como um vault do Obsidian: arquivos Markdown simples numa pasta, com links `[[nota]]` e frontmatter, abríveis e editáveis direto no Obsidian.
    - Hoje o vault é a pasta que o Argus controla, acima de todos os projetos: `D:\Dev\Desenvolvimento` já tem `.obsidian/`, e os `.md` com o nome de cada projeto em `Projetos\` (ex.: `Argus.md`, `Digitama.md`) são notas do vault. O Argus ainda não lê nem grava nada nele.
    - Feito (2026-11-19): preferências globais e memória em `<raiz>/_Argus/` (não `Argus`: na raiz real essa é a pasta do repositório do Argus; `workspace::scan` não lista `_Argus` como projeto) (a raiz é a pasta aberta no Argus, nada fixo no código). `Preferencias.md` é o prompt que vale para todos os projetos e provedores; `Memoria.md` é o índice; `Memoria/*.md` são as notas, uma por fato, com frontmatter `tipo`, `projetos`, `atualizado` e links `[[nota]]`. Editável em Configurações › Preferências e memória (salvar cria a estrutura; lista das notas com busca, Abrir no editor e Abrir no Obsidian por `obsidian://open?path=`). O conteúdo não vai no prompt: o aviso de sistema (`crates/core/src/memory.rs`, `compose_note`) só aponta os arquivos, é estável entre turnos, sem aspas nem quebras, e a raiz inteira entra com `--add-dir` no claude (decisão do usuário, 2026-11-19: dá acesso à nota do projeto e às outras notas do vault). Se existir `<raiz>/<Projeto>.md`, o aviso aponta para ela também. O aviso só entra quando a pasta `_Argus` existe. A memória fica fora do git dos projetos.
    - Falta ver na tela e numa conversa real: a página nova; se o agente lê as preferências e grava notas; se o claude consegue ler `<raiz>/<Projeto>.md` (fora da pasta de trabalho e fora do `--add-dir`); se codex, gemini e opencode leem a pasta `Argus` fora do diretório de trabalho; o "Abrir no Obsidian" com o Obsidian instalado.
15. **Eficiência de tokens (2026-11-19).**
    - Feito: o claude roda com `--exclude-dynamic-system-prompt-sections` (o git status saía do prompt de sistema e regravava o cache da conversa inteira a cada turno; a sessão #9 regravou ~10,9 M tokens em 21 turnos) e com `--include-partial-messages` (o texto aparece enquanto é gerado, como `TextDelta`, que não é gravado no banco nem enviado ao celular).
    - Feito (2026-11-19): compactação automática. Antes de um turno de resposta do claude, se o contexto do último pedido passou do limite (Configurações › Contexto; padrão 200 mil, 0 desliga), o Argus roda `/compact` com `--resume` e só então o turno. Conferido na CLI 2.1.220: 39 252 → 2 663 tokens e a sessão lembrou o contexto. O `compact_boundary` vira a divisória "Conversa compactada" no chat. **Falta ver numa sessão longa real dentro do app.**
    - Pendente: as opções novas do claude exigem uma CLI recente (conferidas na 2.1.220); com uma antiga, todo turno falharia. Checar a versão antes de passar as opções.
    - Pendente: "Padrão da CLI" hoje resolve para o modelo de 1M; avaliar um padrão de janela menor para conversas comuns.
    - Pendente: medir de novo uma sessão longa com as duas opções e comparar `cache_write` por turno.
16. **Refazer o ícone do app (não crítico).** O olho atual é provisório; redesenhar o ícone do Argus (janela, barra de tarefas, instalador e web app do celular).
17. **Pendência visual do critique: aba Celular com outra linguagem visual.** O painel do Screen-Mirror (iframe) não aceita tema: `web/app.js` não lê query string nem escuta `postMessage`. A faixa do Argus em volta já usa os tokens; o miolo continua com o visual do Screen-Mirror. Para alinhar sem tocar no CSS dele por fora, o Screen-Mirror precisaria expor:
    - parâmetros de tema na URL (ex.: `?accent=%23778da9&bg=%230d1b2a&surface=%231b263b&text=%23e0e1dd&radius=6`) aplicados como variáveis CSS na raiz;
    - ou um `window.addEventListener("message")` que aceite `{ type: "theme", tokens: {...} }` só da origem do Argus, para trocar o tema sem recarregar quando o usuário muda de tema;
    - o `web/style.css` dele já tem variáveis na raiz (`--bg`, `--surface`, `--border`, `--text`, `--muted`, `--accent`, `--accent-text`), mas claras e sem raio; bastaria sobrescrevê-las pelos dois caminhos acima, somar `--radius` e ter um modo escuro.

## Notas

- Os worktrees dos agentes ficam em `<projeto>/../.argus-worktrees/`. Worktrees antigos em `.harness-worktrees` podem ser apagados com `git worktree prune` no projeto de origem.
- A pasta do projeto foi renomeada de `Harness` para `Argus`. As sessões antigas do Claude Code ficaram registradas sob o nome antigo.
- O `index.html` não pode ter `<style>` inline: o `vite build` falha quando o diretório atual vem com a letra do drive minúscula (`d:\`).
- App instalado x desenvolvimento: o instalado (`npm run app:instalador`, NSIS em `target\release\bundle\nsis\`) usa `%APPDATA%\argus\`; `npm run app:dev` abre o "Argus Dev" com `%APPDATA%\argus-dev\`. O app instalado foi instalado e conferido pelo usuário em 2026-11-16.

## Decisões fixas

- Os agentes são as CLIs oficiais, com o login da assinatura. Nada de chave de API, e a IDE não lê tokens.
- O claude roda com `--permission-mode acceptEdits`. O shell (Bash e PowerShell) fica bloqueado, a não ser o conjunto padrão (git de leitura e inspeção), os níveis "Git local" e "Git remoto" quando ligados e o que estiver em "Comandos permitidos" por projeto. Nunca entra shell genérico nem `--add-dir` da pasta original do projeto.
- Nenhuma marca ou nome de empresa no projeto. O identificador do app é `dev.argus.app`.
- O custo é sempre exibido como "equivalente em API".
- Textos da interface em português do Brasil. Windows primeiro.

## Integração de outros projetos

Os planos para unificar Screen-Mirror, Gerenciador de Portas e AllSeenEye no Argus estão em [PENDENCIAS.md](PENDENCIAS.md).

## Critique 3 e revisão de motion (2026-11-19)

Pendentes do critique 3 (27/40; snapshot em `.impeccable/critique/`):
- [x] Popover de opções do composer em texto corrido; "Modo de permissão" (só leitura) compete com "Acesso nesta conversa" (`src/views/session/pills.tsx`).
- [x] Pendências: recolher os itens feitos numa linha "N feitos" e mostrar os abertos primeiro (`src/views/session/ChecklistPanel.tsx`).
- [x] Início: "Próximas agendadas" e "Pendências abertas" no lugar do vazio; legenda das bolinhas.
- [x] (feito: rótulos de estrutura a 11px; nomes de projeto viraram conteúdo, 12,5px sem caixa-alta) Texto de 10,5px (nomes de projeto, cabeçalhos de tabela, Ctrl K) e `.meta` de item selecionado com 3,7:1.
- [x] Faixa de abas: botões cobrem o título da última aba com o inspetor aberto. (só as abas rolam, em `.wb-tabs-list`; os botões ficam fora)
- [x] Diálogo do Bypass com ícone de aviso em vez de informação. (`system_confirm` ganhou `warn`)

Motion (revisão só pelo código; falta captura quadro a quadro):
- [x] Chaves por posição na linha do tempo ao vivo (`Transcript.tsx`, `r${i}`/`s${i}`): itens trocam de conteúdo no lugar e a entrada roda de novo; usar a posição global do item.
- [x] (o rascunho agora entra no `.tl` do último turno, via `liveItem`) Rascunho (TextDelta) vira texto final com fade/salto: reaproveitar o nó.
- [x] Ao trocar de conversa todo o histórico anima: só animar o que chega depois do primeiro paint.
- [~] (parcial: sem o brilho no texto "trabalhando"; pet em rajadas de 3) 6–7 laços de "rodando" ao mesmo tempo; deixar o halo como único laço focal. Pet em laço infinito.
- [x] Tokens de duração e curva em `tokens.css`; entrada curta na paleta, menus e painel de Nova tarefa.

## Suporte às ferramentas da CLI (loop iniciado em 2026-11-19)

Do levantamento `claude` 2.1.220 x Argus. Uma volta por item: implementar, validar (`cargo test -p argus-core`, `cargo check` do src-tauri, `tsc`), corrigir e validar de novo.

- [x] 1. MCP completo: `crates/core/src/mcp.rs` (lê `claude mcp list`, add, remove) e Configurações › Ferramentas MCP com estado, Adicionar (comando local/HTTP/SSE, escopo, variáveis), Remover e Entrar (abre `claude mcp login` num terminal). Falta conferir na janela.
- [x] 2. Cartão "Aprovar plano" no fim do turno em Planejar ("Aprovar e executar" / "Pedir ajuste") e `ExitPlanMode` mostrado como plano (não JSON). O Planejar segue emulado (`plan_args`), não com `--permission-mode plan`: no 2.1.220 a CLI troca o haiku pelo sonnet sozinha nesse modo.
- [x] 3. Raciocínio ao vivo: `thinking_delta` vira `ThinkingDelta` (só ao vivo, como o `TextDelta`) e a conversa mostra "Pensando…" com as últimas linhas e a contagem de caracteres. O acesso remoto não recebe esses pedaços.
- [x] 4. Evento `Notice` (gravado no histórico): `api_retry` ("A API falhou; tentando de novo (2 de 10) em 4 s: 529"), `notification` (sem o "ctrl+o" do terminal) e hook com saída ≠ 0 ou `outcome` ≠ success. Hook ok e `status` continuam fora. O formato do `api_retry` não foi visto numa execução real: a leitura aceita os campos que vierem.
- [x] 5. `task_progress` vira `SubagentProgress` (só ao vivo; conferido na fixture real): o cartão do subagente no painel Agentes mostra o que ele está fazendo agora. Comandos e processos em segundo plano que estão rodando ganharam "Parar" (`server_kill`, com confirmação). Falta: andamento também no cartão do subagente dentro da conversa.
- [x] 6. WebSearch vira "Buscou na web “consulta”" e WebFetch "Leu a página host/caminho"; aberta, a linha lista os links do resultado (título e domínio, sem repetidos, até 12) que abrem no navegador. Conferido com saída de exemplo no formato `Links: [{"title","url"}]`; falta ver numa busca real.
- [x] 7. Teto de custo por turno (`--max-budget-usd`: sem teto, US$ 0,50 a 10) e modelo reserva (`--fallback-model`: Sonnet, Haiku, Opus) como pílulas do compositor, só no claude e na conversa já criada; guardados em `settings` (`task.limits.<id>`); a CLI só recebe os flags se o `--help` os listar. A barra de pílulas ficou cheia: rever o arranjo (talvez mover limites para o popover de opções).
- [x] 8. Ramificar conversa: `Orchestrator::fork` cria a tarefa "(ramo de #N)" com o histórico copiado, a mesma sessão, esforço/limites/lista; o 1º turno do ramo usa `--resume <s> --fork-session` e a marca (`task.fork.<id>`) some quando a sessão própria chega. Botão no cabeçalho da conversa e item no clique direito da lista de sessões. Só claude + "Direto na pasta" (isolada dividiria a cópia). Falta: conferir numa conversa real; ramo de conversa isolada (worktree próprio).
- [x] 9. Autocompletar sem os comandos que só servem no terminal interativo (`/mcp`, `/agents`, `/usage`, `/color`, `/heapdump`, `/permissions`…) nem os que o Argus faz por conta própria (`/model`, `/effort`, `/clear`, `/fast`); nomes `__interno` somem; comparação sem caixa. Lista em `src/views/chat/slash.ts` (`TERMINAL_ONLY`), conferida contra o `slash_commands` real do 2.1.220.
- [x] 10. Provedores: "Atualizar CLI" (abre `claude update` num terminal; o login já existia). Configurações › Ferramentas MCP ganhou "Plugins da CLI do Claude": lista (`plugin list --json`, conferido no 2.1.220) com versão, escopo e servidores MCP do plugin, e Ativar/Desativar. Instalar plugins segue pelo terminal.

Loop encerrado em 2026-11-19 com os 10 itens. Nada foi conferido com o app aberto: falta uma passada visual em cada tela nova.

Feito antes do loop, na mesma sessão: cartão de pergunta com abas, "Minhas tarefas", lembretes (com notificação do Windows), esforço, aba Servidores no inspetor, bolinha de uso do plano (`rate_limit_event`), Ctrl+K em qualquer frame e "Abas abertas" na paleta.

Consumo (análise de 2026-11-19, banco real: 162 turnos, US$ 299,71): opus[1m] = 95,5%; uma conversa = 62%; regravação de cache ≈ 54% do custo (50 turnos com escrita > 50 k); compactação automática nunca disparou. A fazer depois do loop:
- [x] Revisar `needs_compact` (2026-11-19). Causa de não ter disparado: a compactação automática só entrou no commit 1aae607 (18/11 22:46); todos os turnos da conversa #9 (17/11 21:18 a 18/11 21:50) rodaram antes dela existir. Depois disso disparou, ao que tudo indica (tarefa #15, compactação pedida pelo Argus: 215 k -> 8,7 k). Mudou: padrão 150 k (`AUTOCOMPACT_DEFAULT`); novo gatilho de pausa (`compact_reason`: parada > `CACHE_TTL_SECS` = 60 min e contexto >= `IDLE_COMPACT_MIN` = 60 k), com a pausa medida antes de gravar a mensagem nova (`Db::last_event_at`); o evento `Compacted` troca o contexto guardado pelo tamanho do resumo (antes ficava o valor antigo, acima do limite, até o próximo pedido). Configurações › Contexto: opção 150 mil como padrão e o texto explica o gatilho de pausa. Testes: `compact_by_size`, `compact_after_cache_expired`, `idle_and_compacted_update_context`.
- [x] Métricas (2026-11-19): `metrics::insights` (acerto do cache, `CostSplit` entrada/escrita/leitura/saída pela tabela, `by_task` com custo, turnos, custo/turno e maior contexto, turnos com regravação: escrita > 50 k ou > 30% da entrada do turno). Escrita de cache a 2x (`CACHE_WRITE_1H`). Relatório (`metrics_report`) leva `insights`; seção "Onde o custo vai" na tela de métricas. Testes: `split_and_hit_rate`, `ranking_and_rewrites`.
- [x] Aviso na conversa (2026-11-19): `src/views/chat/ContextAdvice.tsx` (contexto >= 150 k, ou pausa > 60 min com >= 60 k), comando `task_idle_secs`. **Falta encaixar** em `src/views/session/index.tsx` (acima do `<Composer`, perto do aviso de handoff), que estava com outra pessoa.
- [x] `SHELL_NOTE` (2026-11-19): frase fixa pedindo saída curta (head, tail, --quiet, git status -s, git log --oneline -n 20) e leitura por trechos (offset e limit).

## Processos da sessão que morriam no fim do turno (investigado em 2026-11-19)

- [x] Causa: a própria CLI do Claude. Em `-p`, ao sair, ela encerra os comandos que abriu com `run_in_background`. Reproduzido fora do Argus: `python -m http.server 4199` em segundo plano e, com o `claude -p` encerrado, a porta já estava fechada. O Argus não mata nada no fim normal do turno: `kill_tree` só roda em cancelar/laço (`orchestrator.rs`), e "parar servidores da sessão" só com a escolha do usuário ao cancelar/descartar.
- [x] Correção: o `SHELL_NOTE` (texto fixo, não quebra o cache) avisa o agente que o fim do turno encerra o segundo plano e pede servidor duradouro como processo independente (`Start-Process -WindowStyle Hidden`), com PID e como parar. Processos com a pasta de trabalho da conversa continuam associados à sessão (`procs::track_dir`) e aparecem em Servidores, com Parar.
- [~] Solução de fundo (maior): manter um processo da CLI vivo por conversa (`--input-format stream-json`), para o segundo plano sobreviver entre turnos. Muda o ciclo de vida do turno e o cancelamento. **Parcial em 2026-11-23:** a medição de latência e o pre-warm das CLIs estão no lugar (Fase 1) e o `AgentHost` do claude foi implementado (Fase 3, ver seção no fim). Falta a validação com o claude logado e o equivalente do OpenCode.
- [ ] TODO em crates/core/src/autowork.rs:2 — /FIXME no código, falhas de
- [ ] TODO em crates/core/src/autowork.rs:20 — .md`, nessa ordem). Sem nenhum,
- [ ] TODO em crates/core/src/autowork.rs:253 — `/`FIXME` (palavra inteira, depois de um marcador de comentário).
- [ ] TODO em crates/core/src/autowork.rs:280 — /FIXME em arquivos de texto, ignorando target/node_modules/.git.
- [ ] TODO em crates/core/src/autowork.rs:997 — tratar erro\nlet todos = 1; // FIXME(ana) - lento\nlet s = \"TODO\";\n").unwrap();
- [ ] TODO em crates/core/src/autowork.rs:998 — ignorar\n").unwrap();
- [ ] TODO em crates/core/src/checklist.rs:2 — .md na raiz) e separa os itens `- [ ]` / `- [x]`
- [ ] TODO em crates/core/src/checklist.rs:8 — .md"];
- [ ] TODO em dist/assets/index-DPb8qXlz.js:434 — /FIXME e build/testes, e dispara novos turnos até tudo ficar [x]."}),t.jsxs("div",{className:"aw-limits",children:[t.jsx
- [ ] TODO em dist/assets/index-DPb8qXlz.js:513 — .md na raiz do projeto."}),l&&y===0&&t.jsxs("p",{className:"rp-empty",children:[l.name," não tem itens no formato “- [ ]
- [ ] TODO em dist/assets/r-CdQndTaG.js:6 — ","@usage","@useDynLib"],constants:["NULL","FALSE","TRUE","NA","Inf","NaN","NA_integer_","NA_real_","NA_complex_","NA_ch
- [ ] TODO em src-mobile/gen/android/app/src/main/java/dev/argus/celular/generated/Ipc.kt:26 — call the handler on the UI thread once WebView methods called from IPC handlers no longer wait on it,
- [ ] TODO em src-mobile/gen/android/gradle.properties:24 — Remove this in v3
- [ ] TODO em src-tauri/src/commands/fs.rs:112 — .md na raiz).
- [ ] TODO em src/api/commands.ts:109 — .md); null se não houver.
- [ ] TODO em src/areas/home/index.tsx:61 — ) dos projetos recentes.
- [ ] TODO em src/views/session/ChecklistPanel.tsx:3 — .md), com feito/total por seção, a lista de tarefas

## Latência da primeira mensagem — Fase 1 (2026-11-23)

Diagnóstico: a primeira mensagem espera o custo de iniciar a CLI e o da configuração de MCP antes de emitir qualquer evento. A Fase 1 só mede e aquece; não muda o ciclo de vida do turno. Gates: `cargo test -p argus-core` (272 passados), `cargo check` do `src-tauri` e `npm run typecheck`, todos com exit 0.

**Feito na Fase 1:**

- `crates/core/src/agentlog.rs`: `TurnTimer` e `TurnLog::latency` gravam linhas `lat` no arquivo bruto do turno — nunca na conversa nem no banco. Dois testes novos.
- `crates/core/src/agents/orchestrator.rs`: `run_turn` registra `prepare_ms` (montagem), `mcp_ms` (Screen-Mirror), `spawn_ms`, `first_event_ms` (primeiro evento de stdout) e `result_ms`.
- `crates/core/src/agents/provider.rs`: `AgentProvider::warmup` com implementação no claude (`flags`, que já é `OnceLock`) e no opencode (`list_models`).
- `src-tauri/src/main.rs`: pre-warm de todos os provedores no setup, em `spawn_blocking`. Falha é silenciosa; o turno segue com o padrão da CLI.

**Decisões registradas:**

- Claude Agent SDK descartado: a documentação oficial proíbe login claude.ai/assinatura em produto de terceiro sem aprovação prévia e exige `ANTHROPIC_API_KEY` — contraria a decisão fixa de "zero API key". Segue a CLI oficial.
- Gemini SDK/ADC descartado nesta fase: exige projeto GCP, OAuth consent screen e `gcloud auth application-default login`, fluxo diferente do login da Gemini CLI.
- OpenCode `--standalone` mantido na Fase 1: trocar pelo serviço compartilhado obriga a migrar `OPENCODE_CONFIG_CONTENT` para rotas HTTP de permission, o que mudaria o comportamento de segurança sem validação.
- MCP do Screen-Mirror continua sempre injetado quando configurado: o core não sabe qual aba da UI está ativa. A medição (`mcp_ms`) foi adicionada para decidir com dados.
- `--bare` do claude não entra na Fase 1: remove MCP, memória, skills e `CLAUDE.md`, dos quais o Argus depende.

**Pendências da Fase 2:**

- [x] Ler as linhas `lat` de turnos reais (24/11, tarefa 29). O `mcp_ms=0` **não** prova que o MCP é barato: `apply_agent_mcp` só monta args (`devices.rs:398`), sem I/O; o custo real do servidor fica dentro de `first_event_ms`. A decisão sobre condicionar o MCP à aba Dispositivos continua em aberto.
- [x] Trocar o `--standalone` do OpenCode pelo serviço compartilhado (2026-11-24). As regras V2 (`{ action, resource, effect }`) vão na sessão via `opencode api` (`POST /api/session` no primeiro turno, `PATCH` nos seguintes); sem `--standalone` e sem `OPENCODE_CONFIG_CONTENT`. Falha ao gravar as regras aborta o turno (fail closed). Asserts de `opencode.rs` atualizados.
- [x] Implementar o host persistente (`AgentHost`) por conversa, com `--input-format stream-json` e cancelamento revisado (2026-11-23, Fase 3: claude; OpenCode segue com spawn por turno).
- [x] Decisão sobre `reqwest`: autorizado pelo desenvolvedor, mas não adicionado. A CLI oficial (`opencode api`) já autentica no serviço; HTTP direto obrigaria o Argus a ler o token, contrariando a regra de nunca guardar credenciais das CLIs.
- [x] Conferir o ganho na janela com tarefa real (24/11, tarefa 29; números na seção "Turno real de teste" abaixo).
- [x] Corrigir a falha "Linha de comando muito longa" ao preparar a sessão do OpenCode (24/11, janela; diagnóstico e correção na seção "Implementação da Fase 2").

### Levantamento feito ao iniciar a Fase 2 (2026-11-23)

- **Não há dados de latência para analisar ainda.** Os dois logs em `%APPDATA%\argus-dev\logs` são de 21/11, anteriores à Fase 1, e não contêm linhas `lat`. A decisão sobre injetar o MCP só com a aba Dispositivos ativa continua sem evidência.
- **Confirmado o diagnóstico do `--standalone`:** com o serviço compartilhado no ar (`opencode service status` → `http://127.0.0.1:28894`), defini `OPENCODE_CONFIG_CONTENT` com uma regra de negação e rodei `opencode api GET /api/config`. A lista de configurações saiu **idêntica** com e sem a variável: o serviço iniciado antes não a recebe. É por isso que a política atual só vale com `--standalone`.
- **Contrato oficial das permissões** (https://opencode.ai/docs/permissions/): a configuração V2 usa um array `permissions` com `{ action, resource, effect }`, `effect` em `allow`/`deny`/`ask`, última regra que casa vence; sem regra, o padrão é `ask`. Respostas possíveis: `once`, `always`, `reject`. A API expõe `GET /api/permission/request` e `POST /api/session/{sessionID}/permission/{requestID}/reply` (https://opencode.ai/docs/api).
- **Na ocasião, o código gerava config no formato V1** (`{"permission":{"bash":...,"edit":"allow"}}`), enquanto a documentação descreve o formato V2. Antes de migrar, conferir qual dos dois a CLI 2.3.0 instalada aceita; trocar às cegas mudaria a política de segurança. Conferido: o serviço compartilhado usa V2 na sessão; a config V1 ficou só como referência e nos testes das funções legadas.
- **Alternativa sem dependência nova descoberta na CLI local:** `opencode api <method> <path> --data <json>` (2.3.0) faz requisições HTTP ao serviço. Em tese substitui o `HttpClientHost` com `reqwest` para consultar e responder permissões, usando a CLI oficial — mas exigiria levar as regras `ask` até o serviço (config por projeto ou sessão persistente), o que ainda é decisão de desenho.
- **Resolvido em 2026-11-24:** a troca não dependia de `reqwest` nem de responder `ask` pela API. Num `opencode run` não-interativo a CLI auto-rejeita `ask`, então as regras do Argus viram `allow`/`deny` diretos na sessão. O `AgentHost` segue só no claude, como na Fase 3.

### Implementação da Fase 2 (2026-11-24)

- `crates/core/src/agents/opencode_session.rs` (novo): cria e atualiza a sessão com `opencode api`. A CLI oficial faz a autenticação; o Argus não lê token. `PATCH` sem corpo de resposta e erro devolvido como JSON são tratados; se o serviço não estiver no ar, `service start` é tentado uma vez.
- `crates/core/src/permissions.rs`: `opencode_session_rules` gera as regras V2 por modo (Auto, Restrito, Bypass, Planejar) e `opencode_shell_allow_patterns` reúne os padrões de shell. `apply` não injeta mais `OPENCODE_CONFIG_CONTENT` no opencode. Nenhuma regra `ask`: em `run` não-interativo ela é auto-rejeitada.
- `crates/core/src/harmful.rs`: `opencode_deny_patterns` extrai a lista de comandos nocivos para reuso nas regras V2 (a lista é a mesma da config V1); `opencode_session_deny_patterns` é a versão compacta usada na sessão.
- `crates/core/src/agents/orchestrator.rs`: `run_turn` chama `ensure_session` antes do spawn, anexa `--session <id>` quando a sessão é nova e registra `session_ms` no log bruto. Falha na preparação aborta o turno com linha `err` no log; seguir sem as regras seria fail open.
- **Validado na CLI 2.3.0**, com modelo `free` (sem custo): `POST /api/session` aceita `id`, `location.directory` e `permissions`; `PATCH` atualiza as regras; `GET /api/session/{id}` confirma. Com regra `ask`, um pedido para rodar `git status --short` foi auto-rejeitado ("cannot ask the user for permission"). Com `[{ "action": "*", "resource": "*", "effect": "allow" }]`, o mesmo comando executou e a saída apareceu no evento `tool_use` com `status: completed`. As sessões de teste foram removidas.
- **Gates:** `npm run typecheck`, `cargo test --workspace` (289 + 23), `cargo check -p argus-desktop`, `npm run teste:celular`, `npm run build` e `node --check crates/core/assets/remote/app.js`, todos com exit 0. Sem commit.
- **Defeito encontrado e corrigido na janela (2026-11-24):** as regras V2 com 375 padrões ocupavam 23.631 caracteres; `opencode api --data` estourava a linha de comando (`Linha de comando muito longa`, logs `opencode-30` e `opencode-31`) e o turno abortava antes do spawn. O limite vinha do shim `.cmd` instalado pelo npm, que passa pelo cmd.exe (~8 mil caracteres); chamando o `.exe` que o shim invoca, o mesmo corpo de 9.570 caracteres passa. Correção em duas partes: `opencode_session_deny_patterns` gera um padrão por prefixo (`{prefixo}*`, já que `*` casa zero ou mais caracteres) — 149 regras, 9.453 caracteres — e `api_bin()` em `opencode_session.rs` resolve o `.exe` do shim antes de chamar a API. A autenticação continua na CLI oficial; o Argus não lê token. Testes novos conferem que os padrões compactos cobrem os mesmos comandos e que o shim aponta para o executável real.
- **Ainda em aberto:** `host_ms` e a comparação de `first_event_ms` no claude seguem sem medição real (`claude auth status` → `loggedIn: false`); nenhum aparelho Android conectado para validar a web app no celular.

### Turno real de teste (2026-11-24)

- Feito pela ponte de validação, no Argus Dev: raiz isolada em `.tmp-anexos/raiz-teste/proj`, provedor `opencode`, modelo `opencode/beam-free` (login do OpenCode Console, `cost_usd=0`), prompt "Responda apenas: ok". Tarefa 29, sessão `ses_df4ce1bf3ffenqz36HzUqdkilP`.
- Resultado: resposta `ok`, `Result.ok=true`, `summary=stop`, console limpo.
- Linhas `lat` de `%APPDATA%\argus-dev\logs\opencode-29-1795484148027.log`:
  - `prepare_ms=3`
  - `mcp_ms=0`
  - `spawn_ms=13`
  - `first_event_ms=4938`
  - `result_ms=5494`
- **Leitura:** o pre-warm da Fase 1 deixa o spawn barato (13 ms), mas a latência percebida é dominada por `first_event_ms` (~4,9 s), que é o tempo até a CLI/modelo produzir o primeiro evento. Ou seja: o pre-warm sozinho não explica nem resolve o tempo até a primeira visível; é esse número que o `AgentHost` da Fase 3 precisa atacar.
- Duas tentativas anteriores falharam por conta do modelo, não do código: `opencode-go/beam-free` exige API key (fere a regra "zero API key") e `google/gemini-2.5-flash-lite` falhou porque o free tier do Gemini não está disponível neste projeto. Nenhum token foi gasto nelas.
- Não foi feita medição comparativa com o pre-warm desligado; o ganho do pre-warm não foi quantificado, apenas o custo absoluto do turno.

## Host persistente por conversa — Fase 3 (2026-11-23)

Escopo fechado: só o claude. `--input-format stream-json` existe na CLI do claude; o OpenCode instalado (2.3.0) só aceita um prompt por `opencode run` e continua com um processo por turno. A Fase 2 (troca do `--standalone`) ficou fora do caminho.

**Feito:**

- `crates/core/src/agents/provider.rs`: `AgentProvider::host_command` (opcional, `None` por padrão), `host_message` (linha JSON com `uuid` de correlação) e `host_turn_ended`. Quem não implementa host não muda de comportamento.
- `crates/core/src/agents/claude.rs`: `base_command(cwd, session, opts, stream_input)` concentra os flags; `host_command` usa o mesmo comando do turno e acrescenta `--input-format stream-json`, sem `stdin` de prompt; `host_turn_ended` reconhece a linha `result`. Dois testes de contrato.
- `crates/core/src/agents/orchestrator.rs`: `Inner.hosts`, `HostKey`, `HostLine`, `AgentHost`, `spawn_host`, `host_turn`, `host_message_id`, `kill_pid` e `discard_host`. Um host por conversa; leitura contínua de stdout/stderr desde o spawn (pipe sem leitor trava a CLI); processo registrado em `procs::tracker()`. `host_ms` entra na linha `lat` do log bruto.
- Reutilização: `HostKey::matches` aceita um host aberto sem sessão (`session: None`) quando o turno seguinte já conhece o `session_id` que ele criou. Host aberto com `--resume` só serve para aquela sessão.
- Descarte do host: mudança de modelo/esforço/limites/acesso/cwd/sessão, `/compact`, troca de provedor, `delete`, `apply`, `discard`, fim com estado diferente de `Completed`, cancelamento, laço de repetição, EOF ou erro ao escrever. O turno seguinte retoma com `--resume`.
- Cancelamento (até 2026-11-25): destrutivo, matava a árvore da CLI (`taskkill /T /F` no Windows). **2026-11-26:** passou a interromper só o comando do turno e preservar o host quando a CLI confirma (`command_lifecycle` `cancelled` do mesmo `command_uuid`, ou `result` correlacionado); sem confirmação em até 5 s continua destrutivo. Detalhes na seção abaixo.
- Testes: `host_is_reused_between_turns` e `changing_model_discards_host`, com um provider falso que grava `host N` / `turno N` num arquivo. O id `N` vem da contagem de linhas do próprio arquivo, para dois hosts não receberem o mesmo id.
- **Correlação do `result` (2026-11-24):** `result_message_ids` e `AgentProvider::host_result_message_ids` em `provider.rs`; `host_turn` em `orchestrator.rs` compara os ids com o `message_id` enviado. `result` de outro prompt vira `out_stale` e não fecha o turno; sem os ids (ou com array vazio), o turno termina, mas `keep` exige `correlated` e o host é descartado (`out_no_correlation` no log). Testes novos: `result_message_ids_reads_claude_correlation`, `stale_result_does_not_end_turn` e `host_without_correlation_is_discarded`.

**Gates (raiz do repo):**

- Gates de 2026-11-23: `cargo test -p argus-core` → 276 passados, 0 falhas, 2 ignorados; `cargo check -p argus-desktop` → exit 0 (1 aviso antigo de `sanitize_name` não usado); `npm run typecheck` → exit 0.
- Gates de 2026-11-24, depois da correlação, na ordem do `AGENTS.md`: `npm run typecheck` → exit 0; `cargo test --workspace` → 285 + 23 passados, 0 falhas, 4 ignorados; `cargo check -p argus-desktop` → exit 0 (só o aviso antigo de `sanitize_name`); `npm run teste:celular` → tudo certo; `npm run build` → exit 0; `node --check crates/core/assets/remote/app.js` → exit 0.
- Gates de 2026-11-24, depois da correlação por `command_lifecycle`, na ordem do `AGENTS.md`: `npm run typecheck` → exit 0; `cargo test --workspace` → 294 + 23 passados, 0 falhas, 5 ignorados; `cargo check -p argus-desktop` → exit 0 (só o aviso antigo de `sanitize_name`); `npm run teste:celular` → tudo certo; `npm run build` → exit 0; `node --check crates/core/assets/remote/app.js` → exit 0. Testes novos: `command_lifecycle_id_reads_the_acknowledged_command`, `lifecycle_ack_keeps_host_between_turns`, `result_without_current_ack_is_ignored`.

**Validação de protocolo (fora do app):**

- `node .tmp-anexos/teste-stream.mjs`: dois `result` no mesmo processo, segundo turno em 109 ms, processo vivo ao fim.
- `node .tmp-anexos/teste-stream-resume.mjs`: `session_id` `974678be-c1d6-4074-b6f3-42206b3cacf7` estável nos dois turnos e no `--resume`. Porém a CLI respondeu `Not logged in · Please run /login`; `claude auth status` devolveu `"loggedIn": false` e exit 1. **Não houve turno real do claude**, só validação do protocolo.

**Decisões e limitações:**

- Até 2026-11-25: nenhuma dependência nova, sem alteração de `Cargo.toml` ou `package.json`. **2026-11-26:** dependência `windows` (v0.61, feature `Win32_System_Console`) adicionada só em `crates/core/Cargo.toml`, sob `[target.'cfg(windows)'.dependencies]`, com autorização explícita do usuário; compilou como `windows v0.61.3`. `package.json` segue sem mudança.
- Claude Agent SDK e Gemini SDK/ADC continuam descartados (Fase 1): exigem chave de API ou fluxo OAuth diferente do login da CLI.
- `--bare` não entra: remove MCP, memória, skills e `CLAUDE.md`.
- A CLI 2.1.220 não devolve `user_message_uuids`, embora a mensagem enviada leve `uuid`: dois turnos no mesmo processo produziram `result` com `uuid` próprio e `num_turns=1`, sem o campo de correlação. Depois do `result`, o core drena linhas extras (`out_extra` no log). Decisão antiga (2026-11-24): aceitar o `result` mas descartar o host, cada turno abrindo o seu. **Decisão revisada no mesmo dia:** a sonda com `--input-format stream-json` mostrou que a CLI devolve o `uuid` enviado em `command_lifecycle.command_uuid` (`queued`, `started`, `cancelled`). `command_lifecycle_id` em `provider.rs` lê esse campo; `host_turn` só mantém o host quando o `result` veio com `user_message_uuids` do turno ou depois do lifecycle que confirmou o comando (`out_acknowledged` no log). Result sem confirmação, quando a CLI já falou lifecycle, vira `out_stale_no_ack` e não fecha o turno. CLI antiga sem lifecycle continua no fallback: aceita o `result` mas descarta o host. Motivo da mudança: o usuário reportou que a CLI do Claude fecha a cada resposta no instalador NSIS, matando subagentes e processos em segundo plano junto com o turno.
- Um processo pode emitir mais de um `result` (trabalho em segundo plano que acorda a sessão); o host é descartado se o processo terminar antes do `result` esperado. Com `user_message_uuids`, um `result` cujos ids não incluem a mensagem do turno é registrado como `out_stale` e não encerra o turno.

**Pendências da Fase 3:**

- [ ] Fazer login do claude (`claude auth login`) e rodar dois turnos reais no mesmo host; conferir `host_ms` e a queda de `first_event_ms` no log bruto.
- [ ] Conferir o ganho na interface com o app aberto (não foi feito nesta fase).
- [x] Decidir como correlacionar `result` tardio na 2.1.220 (2026-11-24): usar `user_message_uuids` quando houver; sem ele, usar o `command_uuid` do `command_lifecycle`. Implementado e nos gates.
- [x] (2026-11-26) Gerar novo instalador NSIS com esta correção: `npm run app:instalador` produziu `target\release\bundle\nsis\Argus_0.1.4_x64-setup.exe` (9 019 929 bytes, SHA256 `AFC8B23CC9627312D9CA63E3EDBA7D6CF15E180FBEA430DA7D71DBA10FE70204`), publicado como release `v0.1.4` em https://github.com/ShinuuL/Argus/releases/tag/v0.1.4. Commit `544404d`, tag `v0.1.4`, `main` enviado para `origin`.
- [ ] Confirmar numa sessão real que o host do Claude continua vivo entre turnos (`claude auth status` segue sem login nesta máquina).
- [x] (2026-11-26) Preservar o host no cancelamento: usuário autorizou a crate `windows`. Implementado com `CTRL_BREAK_EVENT` (não `CTRL_C_EVENT`, que iria para todo o console, inclusive o Argus) em grupo de processo próprio. Ver a seção abaixo.
- [x] (2026-11-26) Decisão do OpenCode fechada em favor do serviço compartilhado (Fase 2). A CLI 2.3.0 não tem host persistente por conversa, mas os processos em segundo plano já sobrevivem ao `run` (sonda com HTTP 200 na porta 4199). Cliente ACP existe, mas exige handshake e `session/request_permission`; ficou como opção futura, não implementada nesta rodada.
- [ ] Validar em aparelho físico: não havia dispositivo Android conectado nesta sessão (`list_devices` → "Nenhum aparelho conectado").
- [ ] Confirmar o servidor em segundo plano numa sessão real do Claude (bloqueado por `claude auth status` → `loggedIn: false`, exit 1).
- [ ] Confirmar na janela que o cancelamento preserva o host e que o turno seguinte reusa o mesmo PID (`cancel confirmado: host preservado` no log bruto).

### Cancelamento com host preservado e OpenCode — 2026-11-26

**Cancelamento preservando o host (Claude):**

- `crates/core/src/agents/provider.rs`: `command_lifecycle_state` lê `queued`/`started`/`cancelled`; `AgentProvider::host_cancel_ack_id` devolve o `command_uuid` só quando o estado é `cancelled`. Padrão `None`: provedor sem lifecycle continua com cancelamento destrutivo.
- `crates/core/src/agents/orchestrator.rs`: `AgentHost.keep_on_cancel` e `take_keep_on_cancel`; `spawn_host` cria grupo de processo próprio no Windows (`creation_flags(0x200)`) e `process_group(0)` no Unix. `interrupt_host` chama `GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid)` no Windows e `kill -INT -<pid>` no Unix. `CTRL_C_EVENT` foi descartado porque iria para todos os processos anexados ao console, inclusive o Argus.
- No cancelamento, `host_turn` interrompe o comando e continua lendo por até 5 s à procura do ack. O log bruto registra `cancel: interrompendo o comando deste turno`, `cancel_acknowledged` / `cancel_ack_stale`, e `cancel confirmado: host preservado` ou `cancel sem ack da CLI: host sera descartado`. `finish` consulta `take_keep_on_cancel` antes de descartar: sem ack, o host continua sendo morto e o próximo turno retoma com `--resume`.
- Regra isolada em `host_survives_cancel`: preserva com lifecycle `cancelled` do mesmo comando ou `result` correlacionado; nunca preserva se o processo saiu (EOF). Testes novos: `command_lifecycle_state_reads_the_cancelled_ack` e `cancel_preserves_host_only_with_cli_ack`.

**Melhorias de sessão que não bloqueiam o host:**

- `src/views/session/index.tsx`: `<SessionUsage owner={{ kind: "Session", id: taskId }} />` no cabeçalho da sessão, ao lado do selo de servidores.
- `crates/core/src/agents/orchestrator.rs`: `git_index_safe` em `diff()` (modo isolado) repete só o erro transitório de `index.lock`. Teste `git_index_safe_waits_for_a_transient_lock`.

**OpenCode — sondas e decisão (sem código novo de host):**

- `opencode --help` e `opencode run --help` (CLI 2.3.0): `run` aceita um prompt por processo; `--session/-s` retoma a sessão, mas não há modo de enviar vários prompts para o mesmo processo `run`.
- `opencode service status` → `http://127.0.0.1:28894`; `opencode auth list` mostra as credenciais armazenadas. O Argus não lê nem guarda token.
- Sonda ACP: `opencode acp` respondeu `initialize` (com `loadSession`, `resume`, `fork`) e `session/new`; `session/setModel` devolveu "Method not found" e `session/prompt` terminou com `stopReason: "end_turn"`. O protocolo exige `session/request_permission`. Cliente ACP não implementado nesta rodada (seria outra arquitetura).
- Sonda de servidor em segundo plano: `opencode run --auto -m opencode/beam-free` iniciou `python -m http.server 4199` com `Start-Process`; com o `run` já encerrado, `curl.exe http://127.0.0.1:4199/` devolveu `200` e o `netstat` mostrou o PID 8388 escutando. O PID foi encerrado com `taskkill /PID 8388 /T /F`. O objetivo prático (processos vivos entre turnos) já é atendido pelo serviço compartilhado da Fase 2.

**Gates (2026-11-26, na ordem do `AGENTS.md`, raiz do repo):** `npm run typecheck` → exit 0; `cargo test --workspace` → 297 + 23 passados, 0 falhas, 6 ignorados (2 do core, 4 `live`); `cargo check -p argus-desktop` → exit 0 (só o aviso antigo de `sanitize_name`); `npm run teste:celular` → tudo certo; `npm run build` → exit 0; `node --check crates/core/assets/remote/app.js` → exit 0.

**Entregue depois dos gates:** commit `544404d`, push em `main`, tag `v0.1.4`, instalador NSIS e release `v0.1.4` no GitHub (link acima). **Ainda não feito:** turno real do Claude, teste em aparelho e cliente ACP.

---

## Correções do relatório de validação 0.1.4 — 26/11

Argus em produção com Claude logado confirmou: servidor em segundo plano vivo depois do turno, mesmo host nos turnos 1 e 2 e `SessionUsage` no cabeçalho. Falharam: terminal visível, `prepare_ms` de 24 a 206 s, cancelamento sem ack e log dos eventos do host no arquivo do primeiro turno.

- [x] **Cold start (C):** `checkpoints::snapshot` deixa de fazer `git add -A` no repositório inteiro. Agora lista só o que mudou (`git diff-index` + `git ls-files --others`) e reescreve esses caminhos no índice temporário. Testes `incremental_snapshot_matches_full_add` (compara a árvore com a do método antigo: editar, apagar, criar com espaço no nome, ignorar) e `snapshot_without_commit_still_captures_files` (pasta sem commit). `snapshot_ms` passou a ter linha própria no log do turno.
- [x] **Log do turno (A):** as leituras de stdout/stderr do host escrevem no log do turno corrente (`AgentHost::set_log`), não no da abertura do host.
- [x] **Terminal visível (B):** `spawn_host` passa `CREATE_NO_WINDOW` junto de `CREATE_NEW_PROCESS_GROUP`.
- [x] **Cancelamento (E):** no Windows o Argus anexa ao console do host antes de mandar `CTRL_BREAK_EVENT` e registra `cancel: sinal de interrupt nao entregue ao host` quando o sinal não chega. O botão "Parar" ganhou trava contra cliques repetidos enquanto a confirmação está aberta.
- [x] **Descarte sem registro (D):** troca de opções entre turnos agora grava `host descartado: opcoes mudaram entre turnos`. A troca no turno 3 da task 23 provavelmente veio do `/compact` automático (evento `Compacting` e `system init` novos); falta conferir o log do turno de compactação.
- [ ] Validar de novo em sessão real: `snapshot_ms`/`first_event_ms` no start, nenhuma janela `claude`, `cancel confirmado: host preservado` e linhas `out` no arquivo do turno corrente.
- [x] Perguntas do opencode: `opencode run` não interativo auto-rejeita regras `ask` ("cannot ask the user for permission"). Implementado o provedor `opencode-acp` (cliente ACP) — ver a seção abaixo. Falta validar ao vivo com uma regra `ask` de verdade.
- [x] Validar ao vivo uma pergunta ACP (`session/request_permission` e `elicitation/create`) com o usuário escolhendo no cartão e o turno continuando sem mensagem nova. Feito na janela de dev em 26/11; ver a seção "Validação ao vivo do ACP na janela".

**Gates (26/11, na ordem do `AGENTS.md`, raiz do repo):** `npm run typecheck` → exit 0; `cargo test --workspace` → 299 + 23 passados, 0 falhas, 2 ignorados no core e 4 `live` ignorados; `cargo check -p argus-desktop` → exit 0 (só o aviso antigo de `sanitize_name`); `npm run teste:celular` → tudo certo; `npm run build` → exit 0; `node --check crates/core/assets/remote/app.js` → exit 0.

---

## Sessão interativa ACP e caveman — 26/11

Pedido do usuário: resolver a pendência de sessão interativa com cliente ACP e integrar o repositório `caveman` ao Argus para escolher a saída recebida. Nada foi commitado nem enviado nesta rodada.

**Cliente ACP (implementado):**

- `crates/core/src/agents/acp.rs`: JSON-RPC 2.0 linha a linha sobre stdin/stdout do `opencode acp`. Faz `initialize` (com `elicitation.form`), `session/new` / `session/load`, `session/set_config_option` (`configId: "model"`), `session/prompt`, `session/cancel` e responde `session/request_permission` / `elicitation/create`. Nenhum token é lido pelo Argus; o login continua sendo da CLI.
- `opencode-acp` é um provedor **novo**, registrado em `src-tauri/src/state.rs`. O `opencode` atual (`opencode run`) segue intacto. Um formulário com várias perguntas vira um cartão só (`questions`), porque o ACP espera uma resposta com o conteúdo completo; a escolha volta por `task_acp_answer`, sem criar novo turno.
- `crates/core/src/agents/orchestrator.rs`: caminho próprio para `prov.acp()`, com host vivo por conversa. Num host já aberto não se repete `session/load` (isso repetiria o histórico na transcrição). `session/cancel` confirma por `stopReason: "cancelled"`; com confirmação o host fica vivo, sem confirmação ele é descartado. Uso: o total acumulado do ACP vira consumo do turno por subtração.
- `Transcript.tsx` / `AskCard.tsx` / `ask.ts` / `index.tsx`: cartão ACP com abas e botões, marcado como respondido na interface (não se inventa `ToolResult`; um resultado falso fecharia a pergunta errada quando há duas na fila). O botão "Parar" já tinha trava contra cliques repetidos.
- Testes novos em `acp.rs`: parse de requisições/respostas, parâmetros de sessão com `cwd` absoluto, `elicitation` com uma e várias perguntas, eventos de texto/ferramenta/uso e `stopReason: "cancelled"`.

**Caveman (implementado):**

- `crates/core/src/caveman.rs`: chama `caveman compress` de verdade (texto na stdin, comprimido na stdout, relatório JSON na stderr), com `DO_NOT_TRACK=1`. Sem `caveman-engine`, a CLI faz pass-through e o relatório diz `engine: "missing"`, `ratio: 0` — a interface mostra "sem compressão (pass-through)" e a nota do relatório, sem fingir compressão.
- Comandos `caveman_status` / `caveman_compress` e funções `cavemanStatus` / `cavemanCompress`. Em `Transcript.tsx`, selecionar texto sobre a conversa oferece "Comprimir com caveman" ao lado de "Responder"; o painel Mostra original e comprimido e o usuário escolhe "Usar original" ou "Usar comprimido". Nada substitui a resposta recebida automaticamente.

**Sonda ao vivo (26/11, Windows):** `opencode acp` (CLI 2.3.0) respondeu `initialize` com `loadSession`, `resume` e `fork`, e `session/list` respondeu normalmente. Mas `session/new` **não respondeu** — nem com `mcpServers: []`, nem com servidor stdio/http, nem com `cwd` diferente, mesmo após 60 s; sem `mcpServers` ele responde `-32602`. Isso é um bug upstream conhecido da CLI no Windows (issue `anomalyco/opencode#64686`: Bun `fetch` fica preso pela subscription SSE `/global/event`; afeta v1 e v2). Como efeito colateral da correção, `orchestrator.rs` agora tem limite de 20 s para a abertura da sessão ACP: em vez de travar para sempre, o turno falha com mensagem em pt-BR orientando a usar o provedor `opencode` padrão. Nenhuma mudança na CLI foi feita (fora do escopo do repo).

**Premissa a confirmar:** "selecionar a saída que recebo" foi implementado como escolher entre o trecho original e o comprimido pelo caveman. Se o pedido era outro (por exemplo, escolher entre respostas de modelos ou provedores), é preciso dizer antes de ampliar.

**Ainda não feito:**

- [x] Rodar um turno ACP real com login do `opencode` e uma regra `ask`, conferir no log bruto `initialize`, `session/new`, `ask`, a resposta do cartão e o turno continuando sem novo `User`. Feito em 26/11 na janela de dev: tarefa 36 (`elicitation/create` com Alpha/Beta) e tarefa 37 (`session/request_permission` para `echo oi`). O turno continuou sem mensagem nova do usuário.
- [x] Conferir `session/cancel` no ACP com `stopReason: "cancelled"` e o host preservado no turno seguinte. Feito em 26/11 na janela de dev, tarefa 38: `stopReason: cancelled`, `meta cancel ACP confirmado: host preservado` e o próximo turno terminou com `stopReason: end_turn` respondeu `ok`.
- [ ] Instalar `caveman` (`npm install -g @caveman-ai/cli` + `caveman setup`, com autorização) e validar a compressão real; hoje `Get-Command caveman` não encontra o CLI, então só os testes do relatório passaram.
- [ ] Confirmar com o usuário o significado de "selecionar a saída que recebo".
- [x] Validar na janela (ponte) o cartão ACP: feito em 26/11 com o dev em `ARGUS_PONTE=8788` e `devUrl=http://127.0.0.1:1421`; cartões de pergunta e de permissão clicados, turno cancelado e retomado. A instância principal (PID 26512) não foi fechada.
- [ ] Validar na janela o painel do caveman; a CLI ainda não está instalada (`Get-Command caveman` vazio). Também não há cenário na galeria para o cartão ACP.
- [ ] **Tipo de saída mostrado no print (não implementado).** O usuário mostrou uma tela com checklist de tarefas e estado de subagente; o OCR do anexo traz: "Abrir chamado e mover para em validacao", "Registrar na fila e perguntar sobre vídeo", "Preparo: jar e massa", "Executar A1/A2 (ok)", "B1/B3 com NFC-e em loopback", "1 subagente trabalhando", "Restaurar base, revisar e redigir", "Levar pacote do sim ao avaliador" e "Há 7 minutos". Saída desejada: itens com status (pendente/ok), contagem de subagentes em atividade e tempo desde a última mudança. Hoje o Argus mostra a lista de tarefas do `TodoWrite` (painel Agentes) e os cartões de subagente, mas não esse painel consolidado com tempo relativo. Fica como pendência até o usuário confirmar o formato.

**Gates (26/11, na ordem do `AGENTS.md`, raiz do repo):** `npm run typecheck` → exit 0; `cargo test --workspace` → 0 falhas (309 testes do core, 23 do desktop; `updates_become_stream_events` corrigido para ler `content` do update e params de `session/update`); `cargo check -p argus-desktop` → exit 0 (só o aviso antigo de `sanitize_name`); `npm run teste:celular` → tudo certo; `npm run build` → exit 0; `node --check crates/core/assets/remote/app.js` → exit 0.

---

## Correções das entregas ACP, caveman e fork — 26/11

Pedido do usuário: corrigir os bugs das duas entregas, gerar o fork que não saiu e registrar como pendência a saída mostrada no print. Nada foi commitado nem enviado nesta rodada.

- [x] **ACP — abertura da sessão:** resposta de erro (ou `session/new` sem `sessionId`) sai do loop na hora e grava o erro uma vez só. Antes o host esperava o timeout e o erro aparecia duplicado. Teste `acp_session_new_error_ends_the_turn_once`.
- [x] **ACP — cartão respondido:** novo evento `AcpAnswered { request }` fecha o cartão pelo id do pedido, no histórico e ao vivo. O estado local `answeredAcp` saiu: recarregar a janela não reabre uma pergunta já respondida. `acp_asks` agora remove só a pergunta respondida, não a fila inteira.
- [x] **Caveman:** o shim `.cmd` do npm passa a rodar por `cmd /c` no Windows (senão `CreateProcess` falha); `CavemanReport` usa `#[serde(default)]`, então um relatório com campos ausentes não vira "engine desconhecido".
- [x] **Fork:** `Orchestrator::fork` deixou de exigir Claude. No Claude segue `--fork-session`; nos outros provedores o Argus cria a cópia com histórico, sessão nova e resumo honesto no primeiro turno (`build_fork_context`). `canFork` acompanha a regra. `session/fork` do ACP não é usado: anuncia a capacidade, mas não responde no Windows (#64686).

**Gates (26/11, na ordem do `AGENTS.md`, raiz do repo):** `npm run typecheck` → exit 0; `cargo test --workspace` → 313 + 23 passados, 0 falhas, 6 ignorados; `cargo check -p argus-desktop` → exit 0 (só o aviso antigo de `sanitize_name`); `npm run teste:celular` → tudo certo; `npm run build` → exit 0; `node --check crates/core/assets/remote/app.js` → exit 0.

**Revalidação das pendências (26/11):** sonda ao vivo com `opencode acp` 2.3.0 confirmou `initialize`, `session/new` e `session/prompt` com `stopReason: end_turn`, e `session/cancel` com `stopReason: cancelled`. Não apareceu `session/request_permission` nem `elicitation/create` (capacidades `fs`/`terminal` desligadas no cliente e nenhum servidor MCP que elice). `caveman` segue sem instalar (`Get-Command caveman` vazio); Screen-Mirror não vê aparelho; a janela do Argus já estava aberta, então a ponte não pôde ser subida sem fechar a do usuário. Testes direcionados passaram: 8 de `agents::acp::tests`, 4 de `caveman::tests`, `cancel_preserves_host_only_with_cli_ack` e `acp_session_new_error_ends_the_turn_once`.

---

## Skills da máquina no menu `/` — 26/11

Pedido: subir o `tauri dev` para validar e fazer as skills instaladas na máquina aparecerem no `/`. Nada foi copiado nem regravado: o Argus lê os `SKILL.md` que as próprias CLIs já leem. Para ligar a ponte foi preciso encerrar a instância anterior do `argus-desktop` (PID 5848) e o `npm run tauri dev` órfão; a janela de dev segue aberta com `ARGUS_PONTE=1`.

- [x] `agentdefs::list_skills` passou a ler `~/.claude/skills`, `~/.agents/skills`, `~/.config/opencode/skills` e `~/.opencode/skills`, além de `<projeto>/.claude/skills`. Prioridade: projeto, Claude, Agentes, OpenCode. Nome repetido volta `shadowed`, então o menu não duplica. Teste `skills_de_varias_raizes_nao_duplicam_o_menu`.
- [x] `SkillDef` ganhou `source` (`Projeto`, `Claude`, `Agentes`, `OpenCode`) para a interface dizer de onde veio a skill. A gravação de skills continua escrevendo em `.claude`.
- [x] `allSlashItems` junta comandos locais, da CLI e skills do disco. Skill com o mesmo nome de comando da CLI usa a descrição do `SKILL.md`; comando local do Argus continua na frente. No menu aparece a etiqueta `skill`.
- [x] Escolher a sugestão insere `/<nome> ` no rascunho e o texto segue ao agente, como os outros comandos de barra.
- [x] Validado na janela pela ponte: com `/imp`, apareceu `/impeccable` com origem `Claude` **uma vez só**, embora o arquivo exista em três raízes; com `/context7`, apareceu `/context7-mcp` com origem `Agentes`. `Tab` inseriu `/impeccable ` no rascunho. `node scripts/ui.mjs erros` devolveu `[]`.

**Gates (26/11, na ordem do `AGENTS.md`, raiz do repo):** `npm run typecheck` → exit 0; `cargo test --workspace` → 314 + 23 passados, 0 falhas, 6 ignorados; `cargo check -p argus-desktop` → exit 0 (só o aviso antigo de `sanitize_name`); `npm run teste:celular` → tudo certo; `npm run build` → exit 0; `node --check crates/core/assets/remote/app.js` → exit 0.

- [ ] Mandar um turno real com `/<skill>` em cada provedor. O Claude lista as skills no `system/init` e resolve `/skill-name`; nos outros provedores o texto entra como pedido comum, sem garantia de que a CLI acione a skill. A validação de hoje cobre só o menu e a inserção no rascunho.
- [ ] Trazer para o menu as skills de plugins do Claude (`vercel:*`, no cache de plugins), que hoje continuam disponíveis apenas quando a CLI do Claude informa a própria lista.

---

## Configurações reorganizada (UI Bento) — 24/11

Coluna de conteúdo centrada em 1080px, lead de 760px, opções em cartões
(`.ui-card .sa-group`) com rótulo, Aparência com grade de cartões, Contexto e
Melhorar pedido com descrição por opção, Atalhos em dois cartões de altura igual
(`align-items: stretch`), alerta de erro como banner e alvos de rádio de 32px.
Crítica visual em três passadas (subagente `visual`): veredito "está bom" com a
página ancorada no topo (a centralização vertical foi testada e descartada — o
título pulava a cada troca de página).

Pendências:

- [ ] Conferir no app real (ponte Tauri) que o rádio marcado da página Contexto aparece selecionado: nas capturas no Vite nada vem marcado porque `invoke` não existe fora do runtime; `autocompact_get` sempre devolve valor (`autocompact_limit` com `AUTOCOMPACT_DEFAULT`), então deve ser artefato do ambiente.
- [ ] Se a página Atalhos continuar parecendo vazia (~490px livres em 900px), fundir os dois cartões num único com duas colunas internas.

---

## Sessões: ramo e arquivamento em lote — 24/11

Sessões fork do Claude Chat mostram o chip "ramo" (`fork_of` no banco, sem
backfill dos ramos antigos). Arquivamento persistido em SQLite (`archived`),
com "Arquivadas" no rodapé e seleção múltipla para arquivar/desarquivar várias
de uma vez. A paleta (Ctrl+K) continua mostrando tudo, inclusive arquivadas.

Pendência:

- [ ] Validar a gravação no app real (Tauri): sem runtime, o Vite simulou o
  `invoke` no navegador. A chamada em lote (`tasks_archive` com os 3 ids) foi
  conferida, mas a ida ao banco e o `refreshTasks()` com dados reais não.

---

## Validação ao vivo do ACP na janela — 26/11

Pedido: validar o cartão de pergunta do ACP na janela e testar permissão,
cancelamento e próximo turno. O dev subiu em portas livres (`Vite` 1421,
ponte 8788) sem mexer na instância principal (PID 26512). Nada foi commitado.

- [x] **Correção encontrada na validação:** o opencode manda `id` numérico
  (`0`) no JSON-RPC; o Argus devolvia `"0"` como string e a CLI descartava a
  resposta ("Got response to unknown request 0"). `Ask` agora guarda o valor
  original (`id_json`), `respond` preserva o tipo, e o teste
  `numeric_request_id_stays_numeric_in_the_answer` cobre número e string.
- [x] **Pergunta (tarefa 36):** cartão Alpha/Beta clicado na janela; o log
  `opencode-acp-36-1795720012532.log` mostra `User has answered your questions:
  "Qual opção escolher?"="Alpha"`. A conversa terminou com "Opção escolhida:
  Alpha" e `Concluída`, sem mensagem nova do usuário.
- [x] **Permissão (tarefa 37):** cartão "Permitir echo oi?" com `Allow once`;
  o comando rodou e a resposta na janela foi "Resultado: oi".
- [x] **Cancelamento (tarefa 38):** `Parar o turno` → `Só parar o turno`;
  log com `stopReason: "cancelled"` e `meta cancel ACP confirmado: host
  preservado`. Janela mostrou `Interrompida`.
- [x] **Próximo turno (tarefa 38):** `taskReply` voltou no mesmo host e
  terminou com `stopReason: "end_turn"`; a janela mostrou `ok` e `Concluída`.

**Gates (26/11, na ordem do `AGENTS.md`, raiz do repo):** `npm run typecheck` →
exit 0; `cargo test --workspace` → 315 + 23 passados, 0 falhas, 2 ignorados no
core e 4 `live` ignorados; `cargo check -p argus-desktop` → exit 0 (só o aviso
antigo de `sanitize_name`); `npm run teste:celular` → tudo certo;
`npm run build` → exit 0; `node --check crates/core/assets/remote/app.js` →
exit 0.

Pendência descoberta fora do escopo:

- [x] O texto do cancelamento pelo usuário dizia "Erro: o turno falhou.
  cancelled". `acp::prompt_events` agora recebe `user_cancelled` e não grava
  `Result { ok: false }` quando o `stopReason: "cancelled"` confirma um "Parar"
  do usuário; o estado da tarefa continua `Cancelled`. Uma interrupção sem pedido
  do usuário e um erro real da CLI continuam aparecendo como falha. Testes:
  `prompt_usage_is_a_delta_not_the_session_total` e
  `acp_cancel_confirmado_nao_vira_erro_na_conversa`.

---

## Fallback de imagem com Google AI Studio — 26/11

Pedido: fazer o Argus ler a chave do Google AI Studio para gerar e ver imagens
como fallback, sem mexer no login das CLIs dos agentes. Nada foi commitado nem
enviado nesta rodada.

**Implementado:**

- [x] `crates/core/src/imagem.rs` lê a chave só de `GEMINI_API_KEY` ou
  `GOOGLE_API_KEY` no ambiente do Rust. Nada é gravado, logado ou devolvido à
  interface: o status informa apenas o NOME da variável. A chave vai no header
  `x-goog-api-key` pela stdin do `curl.exe --config -`, nunca em argumento de
  processo.
- [x] Pedido no endpoint `https://generativelanguage.googleapis.com/v1beta/interactions`
  com `model: "gemini-nano-banana-2.1"` e `Api-Revision: 2026-05-20`, conforme a
  documentação consultada em 26/11. `ARGUS_IMAGEM_MODELO` e
  `ARGUS_IMAGEM_ENDPOINT` permitem trocar modelo/endpoint sem código. A leitura
  aceita tanto `output_image` quanto `inlineData` de `generateContent`.
- [x] Imagens salvas em `%APPDATA%/<perfil>/imagens`, com galeria, miniatura e
  Lightbox. A leitura do arquivo valida o caminho dentro dessa pasta; fora dela
  é recusada. Oito testes novos cobrem escape do config do curl, extensão por
  MIME, os dois formatos de resposta e remoção da chave de mensagens de erro.
- [x] Comandos Tauri `imagem_status`, `imagem_gerar`, `imagem_listar` e
  `imagem_ler`, com wrappers em `src/api/commands.ts` e tipos em `src/api/types.ts`.
  Nenhuma dependência nova foi adicionada.
- [x] Página "Imagens" em Configurações (área `src/areas/settings` e view antiga
  do workbench) e comando local `/imagem <pedido>` no chat, que gera a imagem e
  já a anexa à caixa de mensagem via `chat_attachment_save`.

**Premissas adotadas — confirmar se não eram as esperadas:**

1. Origem da chave: começou só em variável de ambiente lida no Rust e, a pedido
   do usuário, passou a aceitar também campo na interface, sempre em memória.
   Motivo: o Argus nunca lê nem guarda token das CLIs.
2. Gatilho: só ação explícita (botão "Gerar imagem" ou `/imagem`). Nada é gerado
   automaticamente durante um turno do agente.
3. Transporte: `curl.exe` do sistema, para não acrescentar dependência de TLS ao
   `argus-core`.
4. Destino: `%APPDATA%/<perfil>/imagens`, fora da pasta de trabalho do agente.
5. Aviso de custo: a documentação de 2026-11 marca os modelos de imagem do Gemini
   como "Free Tier: Not available". O painel e o comando avisam que é pago; os
   agentes continuam com o login das próprias CLIs e o que o plano free permite.

**Validação ao vivo (26/11):** janela de dev com `ARGUS_PONTE=8788`, Vite em
`127.0.0.1:1421`, instância principal (PID 26512) intacta. A página "Imagens"
abriu com "Não configurada. Modelo: gemini-nano-banana-2.1."; clicar em "Gerar
imagem" devolveu "Chave do Google AI Studio não encontrada. Defina
GEMINI_API_KEY no ambiente e reinicie o Argus." Após reload, `node scripts/ui.mjs
erros` devolveu `[]`. Evidências: `argus-imagens-ver4.json`, `argus-imagens-ver5.json`
e `argus-imagens-ver6.json` em `%TEMP%\opencode`.

**Gates (26/11, na ordem do `AGENTS.md`, raiz do repo):** `npm run typecheck` →
exit 0; `cargo test --workspace` → 322 + 23 passados, 0 falhas, 2 ignorados no
core e 4 `live` ignorados; `cargo check -p argus-desktop` → exit 0 (só o aviso
antigo de `sanitize_name`); `npm run teste:celular` → tudo certo; `npm run build`
→ exit 0; `node --check crates/core/assets/remote/app.js` → exit 0.

**Ainda não feito:**

- [ ] Com uma chave real, subir o dev com `GEMINI_API_KEY` no ambiente e validar
  geração, resposta da API, miniatura, Lightbox e anexo via `/imagem`. Sem chave,
  apenas o caminho de erro sem chave foi exercitado na janela.
- [ ] Confirmar se a origem da chave deve ser realmente a variável de ambiente e
  se modelo/endpoint padrão devem continuar `gemini-nano-banana-2.1` /
  `v1beta/interactions`.
- [ ] Nenhum aparelho Android conectado: sem validação por Screen Mirror nesta
  rodada.

---

## Chave da imagem pela tela — 26/11

Pedido do usuário: poder incluir a chave do Google AI Studio pela tela. A
ferramenta de perguntas não estava disponível nesta execução, então adotei a
opção segura: a chave vale só nesta sessão e nunca é gravada em disco. Nada foi
commitado nem enviado.

- [x] `crates/core/src/imagem.rs` ganhou `set_key_from_ui` e
  `clear_key_from_ui`. A chave colada fica num `Mutex<Option<String>>` em
  memória e tem precedência sobre `GEMINI_API_KEY` / `GOOGLE_API_KEY`.
  `status()` passou a devolver `from_ui`, nunca o valor da chave. Teste
  `ui_key_is_kept_in_memory_only_until_cleared`.
- [x] Comandos Tauri `imagem_chave_definir` / `imagem_chave_limpar` e wrappers
  `imagemChaveDefinir` / `imagemChaveLimpar`.
- [x] Campo `type="password"` em Configurações › Imagens, com botões "Guardar
  chave" e "Limpar chave da tela". O campo nunca é preenchido com a chave
  guardada; a interface diz que ela some ao reiniciar e não é logada.
- [x] `/imagem <pedido>` continua sendo comando local e passou a usar a chave da
  tela quando ela está ativa.

**Validação ao vivo (26/11):** o `tauri dev` reiniciou sozinho após a mudança em
Rust (PID 4396), sem fechar a instância principal (PID 26512). Na ponte 8788,
digitar uma chave falsa no campo e clicar "Guardar chave" devolveu
`configured: true`, `from_ui: true`, `source: "tela"`; a página mostrou
"Configurada nesta sessão (tela)." Depois de "Limpar chave da tela", voltou a
`configured: false`. O `/imagem` foi retestado e mostrou o novo aviso "Cole a
chave em Configurações › Imagens (só nesta sessão)…", sem criar tarefa.
`node scripts/ui.mjs erros` → `[]`.

**Gates (26/11, na ordem do `AGENTS.md`, raiz do repo):** `npm run typecheck` →
exit 0; `cargo test --workspace` → 323 + 23 passados, 0 falhas, 2 ignorados no
core e 4 `live` ignorados; `cargo check -p argus-desktop` → exit 0 (só o aviso
antigo de `sanitize_name`); `npm run teste:celular` → tudo certo; `npm run build`
→ exit 0; `node --check crates/core/assets/remote/app.js` → exit 0.

**Ainda não feito:**

- [ ] Se você quiser que a chave sobreviva ao reinício, é preciso autorizar
  gravar segredo em disco e escolher como proteger o arquivo no perfil.
- [ ] Com uma chave real, validar geração, miniatura, Lightbox e anexo via
  `/imagem` ponta a ponta.

---

## Autoatualização via GitHub Release (desenho aprovado em 26/11, Fase A iniciada)

Desenho aprovado pelo usuário: atualizar o Argus por release do GitHub, com
instalação automática abrindo o instalador NSIS, e deixar o instalador só com o
ícone do Argus. Se fizer sentido depois, dá para usar o gateway geral de
atualizações do portal-geral como fonte/canal alternativo.

### Fase A — preparar updater no Tauri (concluída em 26/11, base)

- [x] Adicionar `tauri-plugin-updater` e `tauri-plugin-process` (Rust + npm).
- [x] Registrar plugins no `Builder` do `src-tauri/src/main.rs`.
- [x] Configurar `tauri.conf.json`: `bundle.createUpdaterArtifacts: true`, `plugins.updater.pubkey` e `plugins.updater.endpoints` (GitHub Releases `latest.json`).
- [x] Ajustar capabilities para `updater:default` e `process:allow-restart`.
- [x] Gerar chave de assinatura do updater: pubkey no config; chave privada salva fora do repo em `C:\Users\kinga\.tauri\argus-updater.key` (guardar como segredo, ex.: GitHub Secret `TAURI_SIGNING_PRIVATE_KEY`; não commitar).
- [x] Gates da fase: `npm run typecheck` (ok) e `cargo check -p argus-desktop` (ok, warning antigo de `sanitize_name`).

**Notas da Fase A:**
- `bundle.windows.nsis.installMode` não foi colocado como `passive` porque o schema do Tauri 2 aceita `currentUser|perMachine|both` nesse campo; `plugins.updater.windows.installMode` também ficou de fora por enquanto para não chutar enum. O comportamento padrão do updater no Windows já abre/roda o instalador; validar o modo desejado na Fase B/C.
- O endpoint assume release pública em `ShinuuL/Argus`; se o canal mudar para o gateway do portal-geral, trocar `plugins.updater.endpoints`.

### Fase B — UI de atualização (implementada em 26/11, falta validação real)

- [x] Checar update em Configurações (auto-checagem ao abrir a página + botão "Verificar atualização"). Checagem global ao abrir o app ainda não foi ligada.
- [x] Mostrar versão atual/nova, notas e confirmação antes de instalar.
- [x] Baixar, instalar (`downloadAndInstall`) e `relaunch`; erro seguro com link manual para releases.
- [ ] Validar ponta a ponta com uma release de teste (app antigo detectando `latest.json`, assinatura, instalador abrindo e app reiniciando).

### Fase C — workflow de release no GitHub (pendente)

- [ ] Criar `.github/workflows/release.yml` disparado por tag `v*`.
- [ ] Publicar instalador NSIS, `.nsis.zip`, `.nsis.zip.sig` e `latest.json`.
- [ ] Garantir versão sincronizada (`package.json`, `tauri.conf.json`, `Cargo.toml/workspace`).

### Fase D — instalador só com ícone do Argus (pendente)

- [ ] Criar assets NSIS mínimos (sidebar/header) com só a marca/ícone do Argus.
- [ ] Configurar `bundle.windows.nsis.installerIcon/sidebarImage/headerImage/installMode`.
- [ ] Validar visual do instalador e desinstalador no Windows.
