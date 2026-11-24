# Plano: redesign Bento — todas as telas

_2026-11-23. Referência: `docs/referencia-bento/referencia.jpg` + especificação em `docs/referencia-bento/spec.md` (anexos do pedido, copiados para dentro do repo). Complementa o P5 do `PROXIMOS_PASSOS.md`: o mockup aprovado de 2026-11-17 (`docs/mockups/argus-telas.html`) continua valendo para a **estrutura** (rail de ícones, inspetor, Ctrl+K) — o Bento muda a pele, não o esqueleto._

## 1. Decisões (confirmadas pelo desenvolvedor em 2026-11-23)

1. **Paleta**: novo tema **Bento** em `src/styles/tokens.css` com os hexes exatos da spec, como tema padrão; os 4 atuais (Oceano, Ardósia, Grafite, Noite) ficam intactos e selecionáveis em Configurações → Tema. **Confirmado.**
2. **Microfone**: botão aparece na pílula conforme a referência, **só visual** — tooltip "voz ainda não disponível"; sem Web Speech API nesta rodada. **Confirmado.**
3. **Ctrl+K abre qualquer tela como aba**: fora do escopo desta rodada (pedido separado); fica como pendência em `PENDENCIAS.md`.

## 2. Design tokens (Fase A)

Mapeamento da spec para os nomes que já existem em `tokens.css` — nenhum hex direto em componente:

| spec | token Argus |
|---|---|
| `--bg-canvas` `#0d0d10` | `--page` (canvas entre os painéis) |
| `--bg-panel` `#151519` | `--surface` (painéis) |
| `--bg-surface-elevated` `#1e1e24` | `--surface-2` (pílula, cards, código) |
| `--bg-surface-hover` `#26262e` | `--bg-hover` (sobrescrita do tema) |
| `--border-subtle` `rgba(255,255,255,.07)` | `--border` |
| `--border-input` `rgba(255,255,255,.11)` | `--border-strong` |
| `--text-primary` `#ececf1` | `--text` |
| `--text-secondary` `#9898a6` | `--text-2` |
| `--accent-primary` `#7c6df2` | `--accent` |
| `--status-working` `#60a5fa` | `--run` (não mexe: cores de estado são iguais em todos os temas) |
| `--radius-panel` `16px` | `--radius-panel` (**novo** em `:root`) |

- Novo bloco `[data-theme="bento"]`; `THEMES` e `DEFAULT_THEME` em `src/store/theme.ts`.
- Cards continuam em `--radius-xl` (12 px), pílula em `--radius-pill`; o `16 px` é só para painéis.
- `--status-context-ring` `#c2415c`: se o anel de contexto quiser essa cor, vira token `--ring-ctx` definido só no tema Bento (hoje ele herda acento/estado).
- `DESIGN.md` atualizado na Fase G.

## 3. Fases

> Ordem: tokens → moldura → composer → conversas → áreas → componentes → validação. Cada fase fecha com `npm run typecheck`; build e fotos no fim.

### Fase A — tokens e tema Bento
`src/styles/tokens.css`, `src/store/theme.ts`.
Bloco `bento` com o mapeamento acima, `--radius-panel: 16px` em `:root`, tema vira padrão. Quem já escolheu tema continua no seu (a escolha fica no localStorage).

### Fase B — moldura (shell bento)
`src/styles/shell.css`, `src/shell/{Shell,TopBar,NavRail,StatusBar,RightPanel,BottomPanel}.tsx`.

- Canvas: fundo do `.shell`/`.sh-body` vira `--page`. **Painéis flutuam**: lista de sessões, centro do workbench, inspetor, terminal inferior e cada área em tela cheia ganham `border-radius: var(--radius-panel)`, `background: var(--surface)`, `border: 1px solid var(--border)` e **gap de 8 px** entre eles (hoje encostados, sem raio).
- Rail de ícones fica sobre o canvas, colado na borda esquerda (como na referência).
- TopBar: a busca central (`.tb-search`) vira pílula (`rounded-full`, fundo `--surface-2`) — o conteúdo já está certo.
- StatusBar: mantém a bandeja da direita; grupos da esquerda (agente, branch, alterações, agentes rodando, servidores) em `--text-2`, sem caixas.
- **Armadilha**: `.sh-main` e o padding das abas (`--title-lead-w`, `--title-ctl-w`) calculam geometria. Aplicar raio/gap primeiro e recalcular essas medidas depois, senão as abas descasam.

### Fase C — composer em pílula (a maior)
`src/views/chat/{Composer,QueueStrip,SuggestPopover,DropZone}.tsx`, `src/views/chat/chat.css`, `src/views/session/{index,pills}.tsx`, `src/views/session/session.css`.

Estrutura nova da caixa:

- **Pílula**: `.chat-box` → `border-radius: var(--radius-pill)` com 1 linha, `var(--radius-panel)` quando multiline. O Composer passa a setar `data-multiline` quando o textarea passa de 1 linha (CSS não conta linhas). Fundo `--surface-2`, borda `--border-strong`, altura mínima 44 px.
- **Esquerda**: botão circular `+` (32 px) → popover com: _Anexar arquivo/imagem_, _Adicionar contexto (@)_, _Usar prompt/skill_, _Ver fila de mensagens_. Os botões soltos da `.chat-bar` (clip, `/`, `@`, polir) **saem de lá**.
- **Centro**: textarea auto-expansível com placeholder `"Digite / para comandos"`. `/` e `@` já têm `SuggestPopover` — mantém como está.
- **Direita**: microfone (vazio, **só visual**), enviar (com texto) ou stop (ocupado).
- **Sub-barra** abaixo da pílula, alinhada à direita, `text-xs`, baixo contraste: `AgentModelPill` (modelo + esforço) · `ComposerOptions` (modo) · `UsageRing` (anel de contexto, tooltip no hover).
- **Redistribuição do resto da fileira atual** (o `pills` montado em `session/index.tsx`):
  - acima da pílula, avisos e fila: `QueueStrip` vira badge/pill compacto **só quando o agente está ocupado**; `StalledPill`, `ContextAdvice` e notices continuam acima;
  - `UsagePill` (custo) sai do fluxo principal e vai para o cabeçalho da aba / hover (spec §1.4);
  - seletor de projeto (conversa nova) fica na sub-barra à esquerda — só existe antes da primeira mensagem.
- **`.chat-hint` some** (a linha "Enter põe na fila…", 10,5 px — P2 da crítica): atalhos só em tooltip do `+` e no menu `/`.
- **"••• trabalhando…"** alinhado à esquerda, logo acima da pílula (hoje é a `WorkingLine` no fim do transcript — mover, não duplicar).
- **Nada disto pode se perder**: Enter / Ctrl+Enter / Shift+Enter / ↑, fila com edição/remoção/pausa, anexos por botão/arrasto/colar, chips, drop de arquivo da árvore, holds (isolado / sessão direta ocupada), Ctrl+I de polir, streaming e stop.

### Fase D — Conversas (a tela da referência)
`src/views/projects/*` (lista de sessões), `src/views/session/{Transcript,MessageView}.tsx`, `src/views/chat/{markdown,MessageView}.tsx`, `src/views/workbench/*`.

- **Lista de sessões**: painel arredondado; busca "Filtrar sessões"; grupos com ponto colorido; item ativo com fundo translúcido `rounded-lg`.
- **Abas do workbench**: faixa de abas com os cantos de cima do painel arredondados; título + ✕ como na referência.
- **Cabeçalho da conversa**: "Conversas anteriores" à esquerda, custo (`cv-total`) à direita — já existe; só alinhar ao novo raio.
- **Blocos de código**: `rounded-xl`, fundo `--surface-2`, botão "Copiar" discreto no topo direito (o `chat-copy` do `markdown.ts` já existe; reposicionar e arredondar).
- **Telemetria por mensagem** ("96 entrada • 26,7 mil saída • …"): por padrão **oculta**, aparece no hover/foco da mensagem (spec §4) — reaproveitar o `cv-pop-meta`, que já faz hover.
- Turnos e mensagens: sem mudança de conteúdo, só raios e tokens.

### Fase E — as nove áreas em tela cheia
Cada área vira painel `rounded-2xl` flutuando no canvas (gap 8 px), `AreaHeader` dentro; cards internos em `--radius-xl`.

| Área | Arquivos | O que muda |
|---|---|---|
| Início | `src/areas/home/*` | `hm-card` → `--radius-xl`; painel arredondado |
| Git | `src/areas/git/*` | abas, tabela e diff em cards arredondados |
| Servidores | `src/areas/servers/*` | busca vira pílula; tabela dentro do painel |
| Dispositivos | `src/areas/devices/*` | moldura do espelho → `rounded-xl` com borda neutra (P2 da crítica: "dois produtos costurados") |
| Métricas | `src/areas/metrics/*` | KPIs em cards; gráfico ocupa o cartão (já feito) |
| Terminal | `src/areas/terminal/*` | painel arredondado; xterm com `overflow: hidden` nos cantos |
| Tarefas | `src/areas/tasks/*` | formulário em cards; campos com `--radius` |
| Configurações | `src/areas/settings/*` | seções em cards; navegação lateral arredondada |
| Agentes | `src/areas/agents/*` | `ag-card` → `--radius-xl` |

### Fase F — componentes de sistema
`src/ui/{components.tsx,ui.css}`, `src/styles/{components,index}.css`, `src/shell/{CommandPalette,ContextMenu,Popover,Select,Tooltips}.tsx`.

- `ui-card`, `ui-btn`, `ui-tab`, `ui-seg`, `ui-chip`, menus (`wb-menu`) e popovers: painéis em `--radius-panel`, cards em `--radius-xl`, controles 8 px.
- Paleta Ctrl+K: painel `rounded-2xl`, fundo `--surface-2`.
- Holds (`cv-hold`) e `AskCard`: mesmo raio.
- `Empty` e `DataTable`: só raio/fundo. **Não reabrir** o que a crítica de 2026-11-23 marcou como corrigido (P0/P1 seguem valendo).

### Fase G — validação (gates)
Na raiz do repo:

1. `npm run typecheck` — ao fim de cada fase.
2. `npm run build` — ao fim de C, D, E e F.
3. Fotos: `$env:ARGUS_PONTE = "1"; npm run app:dev` + `node scripts/telas-print.mjs`; comparar com `docs/referencia-bento/referencia.jpg` e com `docs/telas/` anterior.
4. `npm run teste:telas` — a navegação cai se a área não trocar de verdade (`data-area`).
5. Reler `docs/critica-telas.md`: os itens P0/P1 não podem voltar.

## 4. Fora do escopo desta rodada

- **Abrir qualquer tela como aba a partir do Ctrl+K** — pedido separado, pendência própria em `PENDENCIAS.md`.
- Entrada de voz real (Web Speech API).
- Trocar ou remover os 4 temas existentes.

## 5. Checklist de regressão (nada disto pode quebrar)

fila · Enter/Ctrl+Enter/Shift+Enter/↑ · anexos (botão, arrasto, colar) · `/` e `@` com autocomplete · troca de modelo/esforço/modo com aviso · stop e streaming · rascunho por sessão · links clicáveis · checkpoints/restaurar · subagentes e mapa · anel de contexto e custo equivalente em API · terminais, split e arrasto de abas · Ctrl+K / Ctrl+J / Ctrl+. · contraste do texto secundário (neutro, não acento) · `Empty` de meia tela · barra de estado como bandeja.
