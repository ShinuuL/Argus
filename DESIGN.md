---
name: Argus
description: IDE desktop densa e escura para despachar, acompanhar e revisar vários agentes de IA em paralelo.
colors:
  ink-deep: "#0d1b2a"
  page-abyss: "color-mix(in srgb, #0d1b2a 80%, #000000)"
  slate-surface: "#1b263b"
  slate-surface-raised: "color-mix(in srgb, #1b263b 70%, #415a77)"
  hairline: "color-mix(in srgb, #e0e1dd 10%, #1b263b)"
  steel-border: "#415a77"
  bone-text: "#e0e1dd"
  mist-text: "color-mix(in srgb, #778da9 72%, #e0e1dd)"
  harbor-blue: "#778da9"
  harbor-blue-hover: "color-mix(in srgb, #778da9 65%, #e0e1dd)"
  harbor-blue-text: "color-mix(in srgb, #778da9 55%, #e0e1dd)"
  on-harbor: "#0d1b2a"
  tide-selection: "color-mix(in srgb, #1b263b 35%, #415a77)"
  focus-ring: "color-mix(in srgb, #778da9 50%, #e0e1dd)"
  state-ok: "#5fd38d"
  state-warn: "#e8b04a"
  state-err: "#ff8f7a"
  state-run: "#6cb6ff"
  ardosia-accent: "#2bb3b1"
  ardosia-accent-text: "#5fd4d1"
  grafite-accent: "#e0a23b"
  grafite-accent-text: "#f0bd66"
  noite-accent: "#8b7cf6"
  noite-accent-text: "#b0a6ff"
typography:
  display:
    fontFamily: "Geist Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "21px"
    fontWeight: 500
    lineHeight: 1.3
  headline:
    fontFamily: "Geist Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "18px"
    fontWeight: 600
    lineHeight: 1.3
  title:
    fontFamily: "Geist Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 600
    lineHeight: 1.3
  body:
    fontFamily: "Geist Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.35
  reading:
    fontFamily: "Geist Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "13.5px"
    fontWeight: 400
    lineHeight: 1.62
  dense:
    fontFamily: "Geist Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "12.5px"
    fontWeight: 400
    lineHeight: 1.35
  label:
    fontFamily: "Geist Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "10.5px"
    fontWeight: 500
    letterSpacing: "0.08em"
  mono:
    fontFamily: "Cascadia Mono, Cascadia Code, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.5
rounded:
  sm: "4px"
  md: "6px"
  control: "7px"
  lg: "10px"
  composer: "12px"
  pill: "999px"
spacing:
  "1": "4px"
  "2": "8px"
  "3": "12px"
  "4": "16px"
  "5": "20px"
  "6": "24px"
components:
  button-primary:
    backgroundColor: "{colors.harbor-blue}"
    textColor: "{colors.on-harbor}"
    rounded: "{rounded.control}"
    padding: "4px 12px"
    typography: "{typography.dense}"
  button-primary-hover:
    backgroundColor: "{colors.harbor-blue-hover}"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.bone-text}"
    rounded: "{rounded.control}"
    padding: "4px 12px"
  button-ghost-hover:
    backgroundColor: "{colors.slate-surface-raised}"
  button-danger:
    backgroundColor: "transparent"
    textColor: "{colors.state-err}"
    rounded: "{rounded.control}"
    padding: "4px 12px"
  chip:
    backgroundColor: "transparent"
    textColor: "{colors.mist-text}"
    rounded: "{rounded.pill}"
    padding: "1px 8px"
  tab:
    backgroundColor: "transparent"
    textColor: "{colors.mist-text}"
    rounded: "{rounded.md}"
    padding: "3px 9px"
    typography: "{typography.dense}"
  tab-active:
    backgroundColor: "{colors.tide-selection}"
    textColor: "{colors.bone-text}"
  rail-item:
    backgroundColor: "transparent"
    textColor: "{colors.mist-text}"
    rounded: "8px"
    size: "30px"
  rail-item-active:
    backgroundColor: "{colors.tide-selection}"
    textColor: "{colors.bone-text}"
  card:
    backgroundColor: "{colors.slate-surface}"
    textColor: "{colors.bone-text}"
    rounded: "{rounded.lg}"
    padding: "12px"
  input:
    backgroundColor: "{colors.slate-surface}"
    textColor: "{colors.bone-text}"
    rounded: "{rounded.md}"
    padding: "6px"
  composer:
    backgroundColor: "{colors.slate-surface}"
    textColor: "{colors.bone-text}"
    rounded: "{rounded.composer}"
    padding: "10px 12px 2px"
  status-bar:
    backgroundColor: "{colors.ink-deep}"
    textColor: "{colors.mist-text}"
    height: "22px"
    padding: "0 10px"
---

# Design System: Argus

## Overview

**Creative North Star: "O Vigia de Muitos Olhos"**

O Argus é uma sala de vigília escura onde vários agentes trabalham ao mesmo tempo e o usuário precisa saber, de relance e de longe, quem está rodando, quem terminou, quem falhou e quem espera resposta. O sistema visual existe para isso: um fundo de tinta funda e quase sem cor, superfícies que sobem por tom (fundo, superfície, superfície elevada) separadas por linhas finas, e toda a informação viva concentrada em pequenos sinais de estado — bolinhas, halos, chips e ícones — que mantêm o mesmo significado em qualquer tema.

A densidade é de IDE, não de site: texto de interface em 13 px, rótulos de 10,5 a 12,5 px, linhas de ferramenta recolhidas, tabelas com números tabulares e nenhum espaço gasto explicando o que o usuário já sabe. A única área que respira é a coluna de leitura da conversa (~688 px, texto a 13,5 px com entrelinha 1,62), porque é onde a resposta do agente é lida e revisada. O resto da tela é instrumento.

A cor de destaque de cada tema é rara e funcional: ação primária, envio, pílula ligada, preenchimento de barra, marca de soltura. A seleção usa um tom próprio e mais escuro, não o destaque. Os estados (ok, aviso, erro, rodando) ficam fora das paletas, iguais nos quatro temas, para continuarem distinguíveis mesmo quando o destaque do tema é âmbar. Nada de cor fica fora de `src/styles/tokens.css`.

**Key Characteristics:**
- Escuro sempre (`color-scheme: dark`); quatro temas trocam só a paleta, nunca a estrutura.
- Profundidade por tom e linha fina de 1 px; sombra só em camada flutuante.
- Estado lido por forma + cor (bolinha, quadrado, anel, halo, ícone), nunca só por cor.
- Tipografia única (Geist) em escala curta; monoespaçada para caminho, comando e diff.
- Barra de ícones à esquerda, áreas em tela cheia, inspetor opcional à direita, paleta Ctrl+K.

## Colors

Paleta fria e dessaturada de tinta, ardósia e osso, com um único destaque por tema e uma família de estados neutra entre temas. Os valores do frontmatter são os do tema padrão, **Oceano**; os demais temas redefinem exatamente os mesmos papéis (ver `.impeccable/design.json`, `extensions.themes`).

### Primary
- **Azul Porto** (harbor-blue): o destaque do Oceano. Fundo do botão primário, botão Enviar, pílula ligada, preenchimento de barras de progresso e de contexto, marca de soltura de aba, o quadrado do logotipo. Em texto ele não é usado puro (contraste 4,4:1 sobre a superfície): para link, ícone ativo e item "novo" usa-se o **Azul Porto Claro** (harbor-blue-text), misturado com o osso.
- **Tinta sobre Porto** (on-harbor): texto sobre o destaque; no Oceano é o próprio fundo escuro.

### Destaques dos outros temas
O Oceano continua o padrão. Os outros três temas trocam só o destaque (e o tom claro dele, usado em texto, ícone ativo e foco); os papéis são os mesmos do Azul Porto e do Azul Porto Claro.
- **Destaque Ardósia** (ardosia-accent, #2bb3b1) e **Ardósia Claro** (ardosia-accent-text, #5fd4d1): tema Ardósia (`data-theme="ciano"`).
- **Destaque Grafite** (grafite-accent, #e0a23b) e **Grafite Claro** (grafite-accent-text, #f0bd66): tema Grafite (`data-theme="ambar"`).
- **Destaque Noite** (noite-accent, #8b7cf6) e **Noite Claro** (noite-accent-text, #b0a6ff): tema Noite (`data-theme="violeta"`).

### Neutral
- **Tinta Funda** (ink-deep): fundo do app, da barra superior, da barra de ícones, da barra de status e da coluna de conversa.
- **Abismo** (page-abyss): fundo ainda mais escuro, atrás de tudo, onde o tema não define o próprio.
- **Ardósia** (slate-surface): superfícies — cartões, inspetor, barras laterais de área, caixa de composição, menus e paleta.
- **Ardósia Elevada** (slate-surface-raised): controles e hover de botão fantasma; o degrau acima da superfície.
- **Linha Fina** (hairline): todas as divisórias e bordas de cartão; um sopro do texto sobre a superfície.
- **Borda de Aço** (steel-border): borda de controle, de chip, de segmentado, da paleta e de popovers; a borda que precisa ser vista.
- **Osso** (bone-text): texto principal e títulos.
- **Névoa** (mist-text): texto secundário, ícones em repouso, rótulos e metadados (8,6:1 sobre o fundo).
- **Maré** (tide-selection): seleção — aba ativa, item de lista ativo, item da barra de ícones ativo, linha selecionada de tabela, item marcado da paleta. Em hover, a mesma cor a 45–50% sobre transparente.
- **Anel de Foco** (focus-ring): contorno de foco de 2 px, sash de redimensionar e borda da composição em foco.

### Tertiary
- **Estado Ok** (state-ok), **Aviso** (state-warn), **Erro** (state-err), **Rodando** (state-run): os estados do agente e do diff. Fundos de estado são a cor a 16% sobre a superfície; bordas de chip, a cor a 45% sobre transparente.

### Named Rules
**A Regra do Cofre de Tokens.** Toda cor da interface sai de `src/styles/tokens.css`. Componente nenhum escreve hex; tom intermediário é `color-mix` de dois tokens. O Monaco e o xterm montam o tema dos mesmos tokens.

**A Regra do Estado Fora da Paleta.** Ok, aviso, erro e rodando têm os mesmos valores nos quatro temas e nunca são substituídos pelo destaque. O erro sempre vem com ícone ou rótulo, nunca só com a cor.

**A Regra do Destaque Raro.** O destaque marca ação e progresso, não seleção nem decoração. Seleção é Maré; hover é Maré diluída; foco é o Anel.

## Typography

**Display Font:** Geist Variable (com Segoe UI, system-ui)
**Body Font:** Geist Variable (com Segoe UI, system-ui)
**Label/Mono Font:** Cascadia Mono (com Cascadia Code, Consolas)

**Character:** Uma sans geométrica e neutra, empacotada localmente (sem CDN), em escala curta e densa; a monoespaçada do sistema entra para tudo que é literal — caminho, comando, diff, entrada e saída de ferramenta, atalho de teclado.

### Hierarchy
- **Display** (500, 21px): só o título de boas-vindas de uma conversa nova, com o nome do projeto em 700.
- **Headline** (600, 18px): título das telas de apoio abertas como aba (Configurações, Remoto).
- **Title** (600, 16px): cabeçalho das áreas em tela cheia; nos cabeçalhos de cartão, 600 a 12,5px.
- **Body** (400, 13px, 1.35): a fonte base da interface (`--font-ui`).
- **Reading** (400, 13.5px, 1.62): resposta do agente em Markdown, na coluna de ~688px; texto de apoio limitado a 52–62ch.
- **Dense** (400, 12.5px): itens de lista, abas, paleta, tabelas (12px), avisos de conversa.
- **Label** (500, 10.5px, 0.06–0.08em, maiúsculas): cabeçalho de coluna de tabela, grupo da barra lateral de área e grupo da paleta. Só como rótulo de estrutura, nunca acima de um título.
- **Mono** (400, 12px, 1.5): diff, IN/OUT de ferramenta, código, `kbd` a 10px.

### Named Rules
**A Regra dos Números Tabulares.** Contagem, custo, tokens, porcentagem de contexto e coluna numérica usam `font-variant-numeric: tabular-nums`.

**A Regra da Escala Curta.** Nada passa de 21px. Hierarquia vem de peso (500/600) e da Névoa contra o Osso, não de tamanho.

## Layout

Janela de ~1100px para cima. Estrutura fixa de cima para baixo: barra superior de 34px (logotipo, migalha, busca que abre a paleta, botões da janela), corpo e barra de status de 22px. O corpo tem a **barra de ícones** de 48px à esquerda, a área ativa em tela cheia no centro e, na conversa, o **inspetor** opcional de 320px à direita (280px abaixo de 1240px, quando os rótulos da barra superior também somem).

O centro da conversa é um workbench com até 3 grupos divididos em linha ou coluna, abas de 34px e um sash de 5px que acende no Anel de Foco. A transcrição fica numa coluna central de 688px com 24px de margem lateral (12px quando o grupo fica abaixo de 460px); turnos separados por 22px, blocos da linha do tempo por 14px. As áreas em tela cheia usam barra lateral de 230px, cabeçalho com 18px × 22px de respiro e grade com 12px de vão.

O ritmo de espaçamento é o passo de 4px (`spacing` 1–6). A responsividade é por **container query**, não por viewport: a barra de pílulas da composição, o cabeçalho da conversa e o diff escondem rótulos secundários em 680, 620, 560, 520, 460 e 440px de largura do contêiner.

## Elevation & Depth

O sistema é plano. Profundidade vem de três degraus de tom (Tinta Funda → Ardósia → Ardósia Elevada) e de bordas de 1px; cartões, abas, inspetor e caixa de composição não têm sombra em repouso. Sombra aparece só no que flutua sobre a tela — paleta de comandos, menu de contexto, sugestões da composição, dica de contexto, popover de conversa — sempre com a mesma cor de sombra preta a 45% e véu de 60% atrás dos modais.

### Shadow Vocabulary
- **Modal** (`box-shadow: 0 30px 80px -20px rgba(0,0,0,0.45)`): paleta de comandos.
- **Menu** (`box-shadow: 0 8px 24px rgba(0,0,0,0.45)`): menu de contexto das abas.
- **Popover** (`box-shadow: 0 6px 18px rgba(0,0,0,0.45)`): dica de contexto e popover de conversa.
- **Sugestão** (`box-shadow: 0 -6px 18px rgba(0,0,0,0.45)`): lista de `@` e `/` que abre para cima da composição.
- **Halo de ponto** (`box-shadow: 0 0 0 3px var(--bg)`): recorte do ponto da linha do tempo sobre o traço vertical; não é elevação.

### Named Rules
**A Regra do Plano em Repouso.** Superfície parada não tem sombra. Se precisa se destacar, sobe um degrau de tom ou ganha a Borda de Aço.

## Shapes

Cantos suaves e pequenos, crescendo com o tamanho do objeto: 4px em controles miúdos, código inline e linhas de árvore; 6px em campos, abas e itens; 7px nos botões e segmentados do redesign; 10px em cartões, avisos e blocos de diff; 12px na caixa de composição e na paleta; pílula cheia (`--radius-pill`, 999px) em chips, pílulas da composição e indicador de estado com rótulo. Botões de envio e parar são círculos de 30px. Bordas são sempre de 1px, exceto a faixa lateral de 2–3px que marca um aviso (falha, permissão negada, modo direto) e o traço de 2px das abas ativas. A forma também carrega estado: o ponto de erro é quadrado (raio 2px), o de interrompido é um anel vazado.

## Components

### Buttons
Contidos e pequenos; cabem numa linha de ferramenta.
- **Shape:** cantos de 7px (`rounded.control`); 4px × 12px, 12px peso 500; variante pequena 2px × 8px a 11,5px.
- **Primary:** Azul Porto com texto Tinta sobre Porto; hover clareia para harbor-blue-hover.
- **Ghost:** transparente, texto Osso, Borda de Aço; hover sobe para Ardósia Elevada.
- **Danger:** transparente, texto e borda em Erro (borda a 55%); hover com Erro a 14%.
- **Ícone:** 26px quadrados, sem fundo, Névoa; hover com `bg-hover` e Osso.
- **Enviar / Parar:** círculos de 30px. Enviar usa o destaque; **Parar é neutro** — Osso sobre Tinta, em qualquer tema.
- **Focus:** contorno de 2px no Anel de Foco, deslocado 1px (−2px dentro de faixas).

### Chips
- **Style:** pílula transparente com Borda de Aço, 11px, Névoa.
- **State:** variantes ok/aviso/erro/rodando pintam texto na cor do estado e a borda na mesma cor a 45%. Pílulas da composição (20px de altura) ficam ligadas com borda no destaque e fundo do destaque a 22%.

### Cards / Containers
- **Corner Style:** 10px.
- **Background:** Ardósia sobre Tinta Funda.
- **Shadow Strategy:** nenhuma (A Regra do Plano em Repouso).
- **Border:** 1px Linha Fina; avisos ganham faixa esquerda de 2–3px na cor do estado.
- **Internal Padding:** 12px; cabeçalho do cartão a 12,5px peso 600.

### Inputs / Fields
- **Style:** fundo de painel, borda de controle (Borda de Aço), 6px de raio, 6px de respiro, placeholder em Névoa a 80%.
- **Focus:** contorno de 2px no Anel de Foco; campos embutidos (composição, busca do popover, nota de diff) trocam a borda para o Anel em vez de contornar.
- **Composer:** caixa de 12px de raio em Ardósia com Borda de Aço, texto a 13px, barra inferior de pílulas numa linha só que esconde rótulos por container query.

### Navigation
- **Barra de ícones:** 48px, fundo Tinta Funda, botões de 30px com 8px de raio; repouso em Névoa, hover com Maré a 50%, ativo em Maré com Osso.
- **Abas de área (`ui-tab`):** 12,5px, 6px de raio; ativa em Maré. **Abas do workbench:** 34px de altura, separadas por Linha Fina; a ativa ganha fundo do editor e traço superior no Anel de Foco quando o grupo está focado.
- **Barra de status:** 22px, 11px em Névoa; botões com hover da própria cor a 16%.
- **Paleta de comandos:** 560px, 12px de raio, Borda de Aço, grupos com rótulo em maiúsculas, item marcado em Maré, rodapé de atalhos.

### Linha do tempo do agente
A assinatura do produto. Cada bloco da resposta é uma linha de grade (14px de marca + conteúdo) ligada por um traço vertical de 1px em Névoa a 38%. O ponto de 7px diz o estado: Névoa (neutro), Osso a 75% (texto), Ok, Erro, Aviso, ou Rodando com um halo que abre de 0,6× a 2,1× em 1,4s. Ferramentas viram uma linha compacta de 25px (nome em 600, resumo em mono) que abre IN/OUT ou diff numa caixa de Ardósia com raio de 6px; o chevron só aparece no hover ou quando aberto. Itens entram com 180ms de fade e 4px de subida.

### Indicador de estado do agente
Ícone + cor (+ rótulo em pílula quando há espaço): trabalhando em Rodando com ícone girando, atenção em Aviso com fundo de aviso, falhou em Erro com fundo de erro, concluído/aplicado em Ok, ocioso/na fila/interrompido em Névoa. O rótulo some abaixo de 520px de contêiner, o ícone fica.

## Do's and Don'ts

### Do:
- **Do** tirar toda cor de `src/styles/tokens.css`, misturando tokens com `color-mix(in srgb, …)` quando precisar de tom intermediário.
- **Do** acompanhar todo erro e estado de um ícone, forma ou rótulo, além da cor (ponto quadrado para erro, anel para interrompido, ícone no aviso).
- **Do** usar Maré para seleção e Maré a 45–50% para hover de lista, aba e item da barra de ícones.
- **Do** usar números tabulares em contagem, custo, tokens e porcentagens.
- **Do** desligar toda animação contínua (pulso, onda, halo, giro, brilho) sob `prefers-reduced-motion: reduce`.
- **Do** esconder rótulos secundários por container query antes de deixar uma barra quebrar linha.
- **Do** manter estados fora das paletas: um tema novo redefine só os papéis de base (bg, surface, border, text, accent, selection, focus, status).

### Don't:
- **Don't** escrever hex ou `rgb()` direto num componente.
- **Don't** usar o destaque do tema para Parar, erro ou seleção; Parar é Osso sobre Tinta.
- **Don't** dar sombra a cartão, aba, inspetor ou caixa de composição em repouso.
- **Don't** passar de 21px em texto de interface nem usar uma segunda família sans.
- **Don't** criar um raio novo fora de 4, 6, 7, 10, 12 px e pílula.
- **Don't** mostrar marca, logotipo ou nome de empresa de terceiros em tela.
