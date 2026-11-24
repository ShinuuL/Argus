# Changelog

O Argus segue versionamento semântico. Enquanto o app estiver na série 0.1.x, cada release
corrige ou reorganiza a interface sem mudar o formato dos dados salvos. A versão fica em cinco
lugares, que precisam andar juntos: `package.json`, `src-tauri/tauri.conf.json`,
`[workspace.package]` no `Cargo.toml` e, para o app do celular,
`src-mobile/tauri.conf.json` e `src-mobile/Cargo.toml`. Cada release tem uma tag `vX.Y.Z` e o instalador
`Argus_X.Y.Z_x64-setup.exe` anexado no GitHub.

## 0.1.4 (2026-11-24)

Configurações em cartões e sessões mais fáceis de organizar. As duas colunas novas
em `tasks` (`archived` e `fork_of`) entram com migração e valores padrão: o
formato dos dados salvos continua o mesmo da série 0.1.x.

- **Configurações reorganizada (UI Bento).** Coluna centrada em 1080 px, texto de
  abertura mais curto e opções em cartões com rótulo e descrição. Aparência virou
  grade de temas, Contexto e Melhorar pedido mostram o que cada escolha faz, e
  Atalhos ficou em dois cartões de altura igual. Alvos de rádio de 32 px e banner
  de erro com mais destaque. A centralização vertical foi testada e descartada:
  o título pulava a cada troca de página.
- **Sessões: ramo e arquivamento em lote.** Forks do Claude Chat ganham o chip
  "ramo" na lista; sessões antigos não recebem marca retroativa. O rodapé da
  lista tem "Selecionar" e "Arquivadas": dá para marcar várias sessões e arquivar
  ou desarquivar todas de uma vez, além do item no menu de contexto. O estado
  fica no SQLite e a paleta Ctrl+K continua mostrando tudo, inclusive arquivadas.

- **Janelas de uso de 5 horas do OpenCode na bolinha do compositor.** O Argus lê o `state.json` do plugin `usage-window` e mostra cada `provider/modelo` ativo com tokens, requisições, limite estimado (manual ou mediana das amostras de rate limit), percentual e horário de reinício. Na própria linha dá para ajustar tokens e/ou requisições (`set_limit`) e apagar amostras e limite manual (`reset_samples`), com confirmação. A lista é relida ao abrir e a cada 30 segundos; janela vencida não aparece e sem calibração o percentual mostra "—". É a estimativa local do plugin, não a cota oficial do provider.
- **Comparação e validação das telas.** `scripts/comparar-telas.mjs` compara as nove fotos com um
  baseline congelado em `docs/telas/base/`, calcula SSIM e grava imagens de diferença; o relatório
  fica em `docs/telas/comparacao.md`. `scripts/validar-galeria.mjs` percorre os 27 cenários e exige
  área correta, galeria instalada, o conteúdo declarado em `esperar` e console sem erro — antes,
  a galeria só provava que a foto saiu. Rodado: 27/27 cenários passaram e nenhuma das nove telas
  mudou de verdade contra o baseline (SSIM médio 0,9980; o resto é ruído de relógio e player).
  Marcas `data-ponte` na barra de áreas e no estado vazio para o roteiro não depender de texto.
- **Pergunta com opções ilegível não vira JSON na tela.** O bloco `argus-ask` é lido com
  `JSON.parse`; quando o agente erra o JSON (uma chave faltando basta), o bloco voltava inteiro
  pelo caminho do markdown — mil caracteres de JSON cru, sem pergunta nenhuma e sem dizer que
  havia uma. Agora o bloco sai do texto de qualquer jeito e o lugar dele recebe uma linha:
  "o agente mandou uma pergunta com opções que não deu para ler".
- **Galeria de estados: Conversas e Dispositivos.** Eram as duas únicas áreas sem cenário, então
  só tinham sido vistas vazias. Entraram cinco: conversa com histórico (pedido, raciocínio,
  ferramenta que passou, ferramenta que falhou, aviso, resposta e a linha de uso), conversa sem
  pasta raiz, a pergunta com opções ilegível, três aparelhos conectados (um por Wi-Fi, um sem
  autorização) e o sidecar parado. Um cenário pode abrir uma aba de conversa (`sessao` em
  `cenarios.ts`): sem isso a área "chat" só sabia mostrar a conversa nova e vazia.
- **Nome do item não some mais atrás do recado**, nas listas laterais de 200 px. Um `meta` longo
  ("autorize a depuração no celular") quebrava em duas linhas e espremia o rótulo até zero: a
  linha ficava só com o aviso, sem dizer de qual aparelho era. O `meta` passou a ter corte com
  reticências, e na lista de aparelhos o estado vai curto ("não autorizado"), com o recado
  inteiro no `title`.
- **Melhorar pedido** (✨ ou Ctrl+I na caixa): reescreve o rascunho antes de ele ir ao agente —
  objetivo, o que muda, onde, o que não mexer e como saber que ficou pronto —, e o que estiver
  ambíguo vira uma lista "Em aberto" em vez de palpite. O texto volta para a **caixa**, com
  "Desfazer": nada é enviado sem você ver. Em Configurações › Agentes › Melhorar pedido dá para
  deixar desligado, só no botão (padrão) ou em todo envio. Roda um `claude -p` no modelo barato,
  fora da pasta do projeto e sem nenhuma ferramenta, então texto colado de fora não vira ação.
  As preferências do `Preferencias.md` entram na reescrita.
- **Briefing (entrevista) de projeto novo**: na primeira conversa de um projeto, a tela oferece a
  entrevista — o agente lê o que já existe, pergunta em blocos de quatro, não altera nada e no fim
  grava o `BRIEFING.md` com escopo, fora do escopo, restrições, critério de pronto e o que ficou em
  aberto. `/briefing` chama a qualquer momento.
- **Mural de arquivos**: o Argus anota sozinho em que arquivo cada agente e subagente está
  mexendo (ele já vê as ferramentas passarem) e conta isso aos outros no começo do turno
  seguinte — é assim que duas sessões na mesma pasta ficam sabendo uma da outra antes do
  diff, e não depois. Painel "Arquivos em uso" na aba Agentes, com destaque para o arquivo
  que tem mais de uma sessão dentro. É recado, não tranca: ninguém é impedido de escrever.
- **Ponte de validação** (`ARGUS_PONTE`): servidor de teste em `127.0.0.1` que deixa um
  script operar a janela de dentro, já que o WebView2 ignora clique e tecla sintéticos do
  Windows. `node scripts/ui.mjs clicar "Métricas"`, e `npm run teste:telas` passa pelas dez
  áreas conferindo que cada uma abre e que o console fica limpo. `node scripts/telas-print.mjs`
  navega e fotografa as nove áreas em `docs/telas/`, que é o que faltava para uma crítica de
  design ter evidência em vez de palpite.
- **Telas mais parecidas umas com as outras**, a partir da crítica dos nove prints: um só desenho de
  estado vazio (ícone, o que a tela faz e **o botão que resolve** — Git e Servidores diagnosticavam
  sem oferecer "Abrir pasta raiz"); tabela sem linha não desenha mais o cabeçalho de colunas sobre o
  nada; o texto secundário deixou de ter cor de link nos quatro temas; cabeçalho de área igual em
  todas, com o título pequeno porque a trilha do topo já diz onde você está; barra de estado com os
  acessórios numa bandeja à direita, sem número de processo; e botão desligado que ainda se lê.
  O texto do Terminal parou de mandar clicar num "+" que não existe.
- **Quatro telas, uma a uma.** *Conversas*: o cabeçalho da sessão não repete mais o nome da conversa
  (que já está na aba e na trilha) nem o "+" da barra de abas — o botão agora diz o que tem de
  próprio, "Conversas anteriores", com busca; e sem projeto nenhum a caixa deixou de mandar escolher
  um projeto enquanto o centro mandava abrir a pasta. *Dispositivos*: as abas Logcat, Apps, Arquivos
  e Rede existiam só para mandar você olhar a moldura ao lado — com isso "Logcat" e "Arquivos"
  apareciam duas vezes na mesma tela. Saíram; o painel mostra o aparelho e a configuração, e a
  moldura passou a dizer que é o painel do Screen-Mirror, com "abrir fora". *Métricas*: o cartão da
  janela do plano mostra o número de verdade (ele dizia que o Argus não sabia ler, o que deixou de
  ser verdade na 0.1.3), e o gráfico ocupa o cartão inteiro com uma escala que não desperdiça um
  terço da altura. *Configurações*: "Autowork" virou "Trabalho automático" e saiu de Extras — não é
  parente do mascote nem do tocador de música.
- **Responder a um trecho**: marcar um pedaço da conversa faz aparecer um "Responder" junto da
  seleção; clicar leva o trecho para a caixa como citação, com o cursor na linha de baixo. Perguntar
  sobre *aquele* parágrafo deixa de ser copiar, colar e marcar o `>` na mão.
- **Galeria de estados** (desenvolvimento): um cenário declara o que o backend responde a cada
  comando — valor, demora ou erro — e as telas de verdade aparecem vazias, carregando, com falha
  ou cheias, em quatro temas e em janela estreita. A interceptação é no `invoke` do Tauri, então
  nenhuma tela precisa saber que a galeria existe: o que é fotografado é o código que vai para o
  usuário. `node scripts/galeria.mjs`. Só entra pela ponte, só vive enquanto a janela estiver
  aberta e não grava nada. A troca passou do `__TAURI_INTERNALS__` (que o Tauri 2 injeta
  lacrado, e por isso a galeria caía calada) para o `invoke` de `src/api/commands.ts`.

## 0.1.3 (2026-11-19)

Suporte às ferramentas da CLI do Claude, menos consumo e listas pessoais.

- Ferramentas MCP: servidores da CLI com estado, adicionar, remover e entrar (OAuth num
  terminal); plugins com ativar/desativar. "Atualizar CLI" em Provedores.
- Conversa: cartão de pergunta com abas e "Outra"; "Aprovar plano" no fim do Planejar;
  raciocínio ao vivo; avisos da CLI (nova tentativa na API, hook com falha); buscas na web com
  links; ramificar conversa; esforço, teto de custo por turno e modelo reserva no menu de opções.
- Painel Agentes: "Minhas tarefas" (o agente vê a lista e marca o que concluir), lembretes com
  notificação do Windows e andamento dos subagentes; "Parar" em processos em segundo plano.
  Aba Servidores no inspetor.
- Bolinha de uso do plano (janela de 5 horas e semanal, vindas da CLI).
- Ctrl+K funciona com o foco no espelhamento do celular; a paleta lista as abas abertas.
- Consumo: compactação automática a partir de 150 mil tokens e depois de pausa maior que a vida
  do cache; aviso de contexto grande na conversa; Métricas com acerto do cache, custo por tipo de
  token, conversas mais caras e turnos que regravaram o cache; preço de escrita do cache de 1 h.
- O agente é avisado de que o fim do turno encerra os comandos em segundo plano e sobe servidores
  duradouros como processo independente.
- Autocompletar sem os comandos de barra que só funcionam no terminal da CLI.

## 0.1.2 (2026-11-19)

Uso como o Claude Code no VS Code: ver [docs/USO_COMO_CLAUDE_CODE.md](docs/USO_COMO_CLAUDE_CODE.md).

- Planejar / Normal / Bypass por conversa, escolhidos já antes do primeiro envio. O Planejar do
  claude não usa mais o `--permission-mode plan` (a CLI trocava haiku por sonnet).
- "Voltar a este ponto" em cada pedido (`refs/argus/t<id>/<n>`, sem mexer em índice nem branch).
- Funções da extensão do VS Code: Alt+K, chips de arquivo em foco e de erros, rejeitar trecho,
  "Liberar e repetir"; Alt+K mostra uma conversa quando nenhuma está à vista.
- Tarefas agendadas e "Iniciar com o Windows"; preferências e memória em `_Argus/` no vault.
- Auto-work usa o mesmo arquivo de pendências do inspetor; configuração por projeto sem
  depender de `/` ou `\`.
- OpenCode com `--standalone`: a política do modo Restrito passa a valer.
- Compactação automática, texto ao vivo, uso estimado de turno cancelado.
- Aparência: tamanho da interface e fonte do código; paleta com ícones; aviso quando o servidor
  que o agente subiu cai, com "Iniciar pelo Argus".

- Builds: uma execução não é mais dada como encerrada quando um elo da corrente de processos
  termina. Um build é `npm run …` chamando `cargo`, que chama um `rustc` por alvo; cada processo
  que acaba fazia a tela dizer "encerrado" com o build ainda rodando (aconteceu com o APK, que
  compila um alvo do Android por vez). Agora a execução adota os descendentes e só termina
  quando a corrente inteira some.
- Servidores: a lista agora enxerga servidor que roda na pasta do projeto da sessão mesmo fora
  da árvore de processos dela — é o caso do opencode, que executa os comandos por um daemon
  próprio, anterior ao Argus (por isso o dev server do projeto não aparecia nem no computador
  nem no celular). Esses entram marcados como **"pela pasta"**, para não se confundirem com os
  que descendem do processo lançado pelo Argus.
- Túneis: `cloudflared`, `ngrok` e `tailscale funnel` são reconhecidos pela linha de comando e
  viram o selo **"exposto"** no servidor da porta publicada, com o endereço público lido da API
  local da própria ferramenta (nada sai da máquina). Dá para copiar o endereço, abrir e
  **fechar só o túnel**, sem derrubar o servidor. Vale no computador e no celular.
- Permissões: nível novo **"Compilar e testar"**, ligado por padrão — `npm run`, `pnpm build`,
  `cargo test`, `npx tsc`, `pytest`, `go test`, `dotnet build`, `make`, `node <script>` e
  companhia deixam de ser negados ao agente, inclusive no auto-work. Instalar dependência e
  publicar continuam de fora. Dá para desligar no geral ou por projeto (Configurações ›
  Permissões).
- Inspeção: `grep`, `rg`, `findstr`, `Select-String`, `sort`, `diff`, `file` e afins entram no
  conjunto padrão. Procurar texto no projeto era negado, e o agente parava no meio do trabalho
  (visto em tarefas reais). `find` do unix e `sed` ficam de fora: apagam e escrevem arquivo.
- Celular: comando negado agora tem o botão **"Liberar neste projeto"**, como no computador
  (`POST /api/permissions/allow`); antes só dizia para liberar no computador.
- Aplicativo do celular (APK) em Tauri 2, em `src-mobile/`: a casca nativa traz só a tela de
  pareamento e abre a interface servida pelo computador, então o aplicativo acompanha as
  atualizações do Argus sem gerar outro APK. Ajustes ganham "Trocar de servidor" quando o
  acesso vem do aplicativo. Como gerar: `src-mobile/README.md`.
- Celular: as linhas de ferramenta abrem como no computador — comando rodado, resultado,
  estado (rodando, concluída, falhou), lista de tarefas e o raciocínio do modelo. Antes só
  mostravam a entrada em JSON e nunca a saída.
- Celular: o app agora anexa imagens da galeria (ou da câmera) ao pedido, tanto numa conversa
  nova quanto na resposta a uma em andamento. A foto é reduzida no aparelho, gravada na pasta
  de anexos do Argus por `POST /api/uploads` e vai ao agente pelo caminho, como no computador.
- Conversa: enquanto o turno roda, só os últimos passos ficam à vista; os anteriores recolhem
  em "N passos antes". Caminhos compridos nas linhas de ferramenta são encurtados pelo meio
  (o caminho inteiro continua no tooltip).
- Animações: a linha "trabalhando" mostra o passo atual (ferramenta, raciocínio ou resposta)
  com pontos em onda; cada passo novo entra com um deslize curto e a bolinha de "rodando" ganha
  um halo. Tudo desligado com `prefers-reduced-motion`.

## 0.1.1 (2026-11-17)

- Interface reorganizada pelo mockup `docs/mockups/argus-telas.html`: barra de ícones com áreas
  em tela cheia, paleta de comandos (Ctrl+K) e inspetor da conversa (Ctrl+.).
- Telas novas ou refeitas: Início, Agentes (linha do tempo), Git (PRs e revisão de alterações),
  Servidores, Dispositivos, Métricas, Terminais e Configurações (permissões em tabela).
- Temas novos: Ardósia (ciano), Grafite (âmbar) e Noite (violeta).
- Uma barra de status só; "Ver alterações" abre a revisão na área Git.
- Erros de interface mostram uma tela com a mensagem em vez de deixar a janela em branco.

## 0.1.0

- Primeira versão em Tauri.
