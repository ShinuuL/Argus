# Crítica das telas novas (2026-11-23)

Substitui a crítica antiga, escrita sobre os prints de 2026-11-21. Aqui a evidência são as nove telas
refotografadas em `docs/telas/` e os 27 estados em `docs/estados/`, depois das correções dos sete
problemas de sistema e das quatro telas do passo 3 de `PROXIMOS_PASSOS.md`.

## Como foi feita

- **Avaliação de design (A)**: leitura das nove telas e dos 27 estados, contra `PRODUCT.md`,
  `DESIGN.md`, `tokens.css` e a rubrica de crítica. Modo de superfície **Operate** (IDE de trabalho),
  então os dez heurísticos se aplicam por inteiro, sem `n/a`.
- **Detector automático (B)**: `impeccable.cmd detect --cdp 9222 --target Argus --json`, contra a
  janela real por CDP. Saída: `exit 2`, **1 finding** (`tiny-text`). O CDP caiu logo depois; não houve
  screenshot por essa via.
- **Tema das fotos: Noite**, não Oceano. O card "Noite — noite e violeta" aparece selecionado em
  `09-configuracoes.png` e todo botão primário é violeta (`#8b7cf6`). Os cenários `metricas-ciano`,
  `metricas-ambar` e `metricas-violeta` são variações de tema da galeria, não o tema das nove telas.
- **Limitações**: `inicio-cheio.png` saiu com uma notificação do WhatsApp sobreposta e não deve ser
  usada como evidência pública sem refazer. Indicador de foco e contraste exato dependem da fonte,
  não de print. O grão `div.chat-hint` foi localizado no código, não medido na janela.

## Veredito

**Especificidade: sim, com uma exceção.** Dá para reconhecer o Argus por qualquer tela: a copy ensina
o modelo do produto (*"As tarefas só rodam com o Argus aberto — ele não deixa serviço no sistema"*),
e o vocabulário é próprio (custo "equivalente em API", "janela do plano · semanal", contadores de
agente/servidor). A exceção é `05-dispositivos.png`: o painel do Screen-Mirror chega com moldura azul,
tabs e botões próprios e poderia pertencer a qualquer ferramenta ADB.

**Nielsen: 28/40 — faixa "Good" (28–35).** Base sólida; os pontos fracos são endereçáveis e quase
todos são de estado, não de desenho.

**Carga cognitiva: moderada.** Cinco dos oito itens do checklist passaram. Falharam foco único,
chunking e número de escolhas visíveis.

A olho nu as telas estão calmas e coerentes. O que derruba a nota não é estética: é **erro que mente**
sobre o estado real e **informação crítica codificada só por cor**.

## 1. Correções prioritárias

A avaliação A marcou os erros abaixo como P1. Eles sobem para P0 aqui porque todos apresentam um
estado contrário ao que aconteceu — o usuário toma uma decisão com informação falsa.

### P0 — Erro apresentado como estado vazio

`servidores-erro` mostra o banner "não consegui listar as portas: acesso negado" e, logo abaixo,
"Nenhum servidor em execução" com a CTA "Abrir pasta raiz". `agendadas-erro` faz pior: "não consegui
ler as tarefas agendadas" seguido de "Nenhuma tarefa agendada" e "Nova tarefa agendada". As duas
frases se contradizem. O usuário lê que perdeu as tarefas ou que pode criar uma duplicata.

**Correção**: com erro, não renderizar o `empty`. Mostrar só o estado de erro, com "Tentar novamente"
e uma frase de segurança ("suas tarefas continuam salvas; só não consegui ler agora").

### P0 — Métricas sem histórico exibindo zero como dado

`metricas-vazio` ("Métricas · sem histórico") mostra o aviso "Histórico desligado: o orquestrador está
sem banco de dados, então o uso não é registrado" e, na mesma tela, o cartão "Turnos em 7 dias" com
`0`, acompanhado de "média 0 por dia · 0 tokens · US$ 0,00 eq. API". Zero é um número; histórico
desligado é ausência de número. São duas mensagens diferentes na mesma tela
(`src/areas/metrics/index.tsx:99-128`, `src/dev/cenarios.ts:348`).

**Correção**: quando `has_db` é falso, trocar o `0` por travessão ou ocultar o total. Manter a frase
de explicação e a ação para ligar o histórico.

### P0 — Métricas em erro: loading morto e erro cru

`metricas-erro` mostra "banco de histórico corrompido (database disk image is malformed)", os cards
presos em "— carregando…" e o gráfico vazio. O estado final nunca chega, e o texto técnico em inglês
vaza para a interface.

**Correção**: traduzir para "o histórico local está corrompido", oferecer "Tentar novamente" e um
"ver detalhes" com o texto original. Trocar "carregando…" por "sem dados" quando o erro é final.

### P1 — Status da timeline codificado só por cor

Em `conversas-cheio`, cada passo da ferramenta é um dot verde, âmbar ou vermelho. Se a fonte não
tiver ícone ou texto anunciando "falhou", quem usa leitor de tela não recebe a informação. A cor é
o único sinal.

**Correção**: ícone por estado (check, alerta, x) mais `aria-label` com o estado por extenso.

### P2 — "Abrir pasta raiz" onde abrir a pasta não resolve

O mesmo CTA aparece em Servidores, Git, Conversas e Início. Em Servidores a promessa é listar
servidores; abrir a pasta não muda isso. O clique leva a uma ação sem relação com o objetivo da tela.

**Correção**: em Servidores, CTA alinhado ao objetivo (pedir ao agente para subir um servidor, ou
instrução equivalente). Guardar "Abrir pasta raiz" para quando a raiz realmente falta.

### P2 — Dispositivos: dois produtos costurados

Painel azul do Screen-Mirror dentro do app violeta; 13 botões de hardware em uma fileira e 3 de
navegação, todos sem rótulo. É o maior vilão de carga cognitiva e quebra consistência visual.

**Correção**: aplicar os tokens do Argus ao painel embutido; mover teclas raras para "mais teclas" e
manter 4–5 visíveis com tooltip.

### P2 — CTA duplicado e botão de cabeçalho sem regra

`07-terminal` repete "Novo terminal" no header e no corpo. O botão superior direito muda de
"Atualizar" (Git) para "Parar" (Dispositivos) para três botões (Terminal) sem regra aparente.

**Correção**: um CTA primário por tela, no corpo do estado vazio; no header só ações secundárias.
Documentar a regra em `DESIGN.md`.

### P2 — Texto de 10,5 px no hint do composer

Único finding do detector B: `tiny-text`, seletor `div.chat-hint`, `10.5px body text`, em
`src/views/chat/Composer.tsx:786` e `src/views/chat/chat.css:251`. A dica fica abaixo do mínimo
legível e usa `--text-2` sobre superfície escura.

**Correção**: subir para 12 px e conferir contraste de 4,5:1 no tema Noite.

### P2 — Gráfico de Métricas desperdiça altura

Quatro barras em ~750 px quase vazios. Uma barra de 2,5 mi domina; as de 15/11 e 21/11 ficam
invisíveis e o eixo x nomeia só três datas. Com poucos pontos, uma lista diria mais.

### P2 — Seis estados vazios repetidos

Sem pasta raiz aberta, Conversas, Início, Git, Servidores, Terminal e Agendadas mostram quase a
mesma tela. Uma vez é onboarding; seis vezes parece app quebrado.

## 2. Nielsen — 10 heurísticas (modo Operate)

| # | Heurística | Nota | Problema-chave |
|---|---|---|---|
| 1 | Visibilidade do estado do sistema | 3 | Bom no geral; em `metricas-erro` os cards ficam em "carregando…" depois do erro |
| 2 | Correspondência sistema/mundo real | 2 | Erros crus em inglês: "database disk image is malformed", "adb: device unauthorized" |
| 3 | Controle e liberdade | 3 | Parar, fechar banner, voltar; falta undo/confirmação em ações destrutivas |
| 4 | Consistência e padrões | 2 | Mesmo CTA em telas diferentes, "Novo terminal" duplicado, painel Screen-Mirror à parte |
| 5 | Prevenção a erros | 3 | "salvo automaticamente", aviso de tarefas só com o app aberto, botões desabilitados |
| 6 | Reconhecer em vez de memorizar | 3 | Breadcrumb e paleta bons; rail de 8 ícones sem rótulo |
| 7 | Flexibilidade e eficiência | 4 | Ctrl K, Ctrl+J, Ctrl+., fila, ↑, filtros, 7/30 dias, Dividir — caminho de expert real |
| 8 | Estética e minimalista | 3 | Calma e hierárquica; ruído em Dispositivos e no gráfico de Métricas |
| 9 | Diagnosticar/recuperar de erros | 2 | Ask-quebrado e dispositivos-erro são bons; metricas-erro e agendadas-erro não oferecem saída |
| 10 | Ajuda e documentação | 3 | Empty states que ensinam, atalhos no rodapé; sem tooltip no rail |

**Total: 28/40.**

## 3. Carga cognitiva — checklist de 8 itens

| Item | Resultado | Observação |
|---|---|---|
| Foco único | falhou | Dispositivos disputa espelho, config, tabs e chips ao mesmo tempo |
| Chunking (≤4 por grupo) | falhou | Configurações tem 17 destinos; o grupo AGENTES sozinho tem 9 |
| Agrupamento | passou | cards com borda, proximidade e títulos consistentes |
| Hierarquia visual | passou | título > cards > status bar; secundário em cinza |
| Uma coisa por vez | passou | na conversa; falharia só junto do item 1 em Dispositivos |
| Escolhas mínimas (≤4) | falhou | 16 teclas em Dispositivos; 8 controles na linha do composer |
| Memória de trabalho | passou | CTA repetido onde precisa; estado vazio de Agendadas diz o que fazer |
| Divulgação progressiva | passou | segmented, tabs e busca de Configurações |

Pontos de decisão com mais de quatro opções visíveis: Dispositivos (16 teclas), Configurações
(17 destinos), composer de Conversas (8 controles), rodapé (~8 pontos clicáveis) e Início cheio
(12+ linhas, salvo pelo agrupamento em cards).

## 4. Jornada emocional

- **Pico**: `conversas-cheio.png` — timeline com progresso, correção aplicada, custo por turno e
  "escrevendo a resposta…". É a tela que faz o produto parecer competente.
- **Fim**: Configurações termina em "salvo automaticamente" no rodapé. Zero ansiedade de salvar.
- **Vales**: `metricas-erro` (parece que quebrou de vez), `agendadas-erro` (parece perda de dados),
  `servidores-erro` (duas verdades opostas) e a repetição dos seis estados vazios.
- **Segurança**: custo é o ponto mais forte — sempre rotulado, com detalhe por turno. Erro é
  inconstante: metade dos casos dá caminho, metade não. Essa inconstância é o que gera insegurança.
  Não há confirmação/undo visível para apagar sessão, terminal ou tarefa.

## 5. Três forças (manter)

1. **Linha de custo/tokens em Conversas** — "18,4 mil entrada · 3,1 mil saída · cache 212 mil lido ·
   14,9 mil gravado · US$ 0,25 equivalente em API", repetida no cabeçalho. Transparência rara.
2. **Estados vazios de Agendadas/Git/Terminal** — título + explicação do conceito + um CTA. Fazem
   onboarding em duas linhas, sem tutorial.
3. **Cards de tema com mini-prévia + "salvo automaticamente"** — escolha autoexplicativa e fim do
   medo de perder configuração.

## 6. Personas

- **Alex (power user impaciente)**: atalhos e fila aprovados. Red flags: não há atalho visível para
  "Abrir pasta raiz" (a ação nº 1), o rail sem rótulo força mouse, e as 16 teclas sem legenda em
  Dispositivos são o tipo de fileira que ele erra por pressa.
- **Sam (acessibilidade)**: breadcrumb, item ativo e erros com texto são pontos fortes. Red flags:
  dot colorido na timeline, densidade de 13 px com cinzas apagados no tema Noite e foco não
  verificável em print.
- **Jordan (primeira vez)**: passa nos primeiros cinco segundos ("Comece abrindo seus projetos" +
  CTA + placeholder apontando o botão). Depois trava no ícone sem rótulo de Métricas e no jargão sem
  glossário ("sidecar", "MCP", "Verbose", "cache 212 mil lido") e no erro em inglês.

## 7. O que ainda não foi verificado

- Indicador de foco e contraste real dos cinzas no tema Noite (precisa da fonte, não do print).
- `inicio-cheio.png` contaminada por notificação; refazer antes de usar em relatório público.
- "64,8 mil tokens hoje (US$ 0,00)" com valor não-zero e custo zero — conferir arredondamento.
- Janela estreita e temas além de Noite fora dos cenários dedicados da galeria.
- `01-conversas` vazio: a barra lateral anuncia "21 sessões anteriores continuam na paleta (Ctrl+K)"
  com a lista vazia. É honesto, mas pede confiança numa paleta que a pessoa ainda não abriu.
- Rodapé com ~8 pontos clicáveis, metade deles controles de player de música, disputando espaço com
  o estado do app.
- Início cheio tem 12+ linhas clicáveis; passa só porque estão agrupadas em cards.

## 9. Perguntas que a crítica deixa

1. Se "Abrir pasta raiz" é a porta de entrada, por que ela aparece onde abrir a pasta não resolve?
   O estado vazio deveria prometer pasta raiz ou coisa rodando?
2. O painel do Screen-Mirror é um app dentro do outro. Deve seguir os tokens do Argus ou é
   intencionalmente outro produto? A costura azul/violeta já confundiu quem olhou as fotos.
3. Seis estados vazios iguais antes de qualquer valor real: quantos a pessoa aceita antes de
   concluir que o app não funciona?

## 8. Evidências

- `docs/telas/01…09-*.png` — nove telas, 1400×900, tema Noite.
- `docs/estados/*.png` — 27 cenários (vazio, carregando, erro, cheio, 900 px, temas).
- `docs/estados/validacao.json` — 27/27 cenários passaram.
- `docs/telas/comparacao.md` — SSIM médio 0,9980 contra `docs/telas/base/`; 0 de 9 telas mudaram
  de verdade (o diff é relógio e player).
- Detector B: `impeccable.cmd detect --cdp 9222 --target Argus --json` → `exit 2`, 1 finding
  `tiny-text` em `div.chat-hint`, `http://localhost:1420/`.
