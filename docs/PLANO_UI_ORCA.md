# Plano: aproximar a UI do Argus da Orca e trocar os ícones

_2026-11-16. Pesquisa e plano. **Fases 1 a 4 e 6 a 9 feitas em 2026-11-16** (ver a tabela da seção 5); as demais seguem pendentes. Base: PROXIMOS_PASSOS.md (Layout, Temas e Decisões fixas), `src/styles/*.css`, `src/shell/icons.tsx` e os prints de validação._

## 1. A Orca (referência)

- Repositório oficial: <https://github.com/stablyai/orca>. Confirmado: é um orquestrador de IA que roda Claude Code, Codex etc. em paralelo, cada um no seu worktree. Site: <https://orca.dev>.
- **Licença: MIT**, "Copyright (c) 2026 Lovecast Inc." ([LICENSE](https://github.com/stablyai/orca/blob/main/LICENSE)).
  - Ela permite copiar e adaptar código, CSS e tokens, desde que **o aviso de copyright e o texto MIT acompanhem as partes copiadas** (por exemplo, um cabeçalho no arquivo e uma entrada em `THIRD_PARTY_NOTICES.md`).
  - Ela não cobre a marca: nada do nome "Orca", do logotipo, da baleia, das ilustrações de marketing nem dos screenshots do site. Pela decisão fixa do Argus (sem marca de empresa), a atribuição também não aparece na interface; fica só no arquivo de avisos.
  - Recomendação: **reproduzir os padrões (medidas, hierarquia, comportamento) em vez de copiar código.** Assim não há obrigação de atribuição. Se algum trecho for copiado literalmente, ele deve ser registrado.
- Pilha (segundo `package.json` e `components.json`): Electron 43, React 19, **Tailwind 4 + shadcn/ui (estilo new-york-v4, base neutral)**, **lucide-react 0.577**, Monaco e xterm 6.
- Tipografia: **Geist** variável (`'Geist', -apple-system, 'Segoe UI', sans-serif`); mono `SF Mono, Cascadia Code, Consolas`. A Geist usa a licença OFL-1.1 e pode ser embutida no app.
- Tokens (`src/renderer/src/assets/main.css`, modo escuro): esquema shadcn com `--background #0a0a0a`, `--card`/`--sidebar #171717`, `--muted #262626`, `--accent #404040`, `--border rgb(255 255 255 / .07)` e `--radius .625rem` (10 px). Há tokens próprios para a barra de worktrees (`--worktree-sidebar*`) e para as abas (`--tab-active`, `--tab-bar`, `--tab-active-indicator`). As bordas são quase invisíveis: a hierarquia vem do tom da superfície, não de linhas.
- Estados do agente (`components/AgentStateDot.tsx`): working (spinner), monitoring, blocked, waiting (pergunta), interrupted, failed, done (check), idle, unverifiable e permission ("Needs attention"). Há anel de progresso opcional e tooltip.
- Padrões visíveis no [hero do README](https://github.com/stablyai/orca/raw/main/docs/assets/readme-hero.jpg) e nos [posters dos recursos](https://github.com/stablyai/orca/tree/main/docs/assets/readme-features):
  - **Barra lateral:** busca no topo, depois Tasks e Automations, e a seção "Projects" com ações à direita. A hierarquia é projeto → worktree (nome, chip "primary" e branch na segunda linha) → sessões (ícone do agente, título, modelo e idade, como "opus-5.5 49m"). O worktree selecionado fica num cartão arredondado. O rodapé tem só ícones (configurações e ajuda).
  - **Abas:** ficam na barra de título, sem uma barra superior separada. Cada aba mostra o ícone de estado, o ícone do agente, o título e o ✕.
  - **Chat:** "Worked for 1m 12s ›" recolhível, ferramentas como linhas leves ("Read CHANGELOG.md", "+6" em verde) e cartões de subagente com hora e chevron.
  - **Caixa de composição:** placeholder "Ask anything, @ to mention files, / for commands", botão "+", pílula de permissão ("Full access" em âmbar), modelo à direita, microfone e botão Enviar circular.
  - **Status bar:** barras de uso e limite por provedor, com o tempo até o reset ("42% 2h 48m").
  - **Outros:** split de qualquer painel (chat, terminal, navegador, diff), diff com anotação por linha e "Jump Anywhere".

## 2. Argus × Orca, tela a tela

| Região | Argus hoje | Orca | O que importa para o usuário | Esforço |
|---|---|---|---|---|
| Tipografia | `13px system-ui`; mono Cascadia | Geist variável; corpo de ~13–14 px; títulos semibold | Leitura mais limpa | P |
| Cor e superfícies | 3 temas com bordas bem visíveis | neutra, bordas com 7% de branco, hierarquia pelo tom | Menos ruído visual | P (tokens) |
| Raio e espaçamento | 5 e 8 px; densidade irregular | 10 px nos cartões, 6–8 px nos controles, escala de 4 px | Coerência | P |
| Ícones | 40 SVGs próprios inline, traço 1.7 | Lucide | Conjunto completo e consistente | P–M |
| Barra superior (32 px) | barra própria, com a linha de abas abaixo | abas na barra de título, sem faixa extra | +32 px de área útil | G (decorações da janela no Tauri) |
| Barra esquerda | "PROJETOS" em caixa alta, projeto → sessões, rodapé com 5 botões rotulados | busca no topo, projeto → worktree → sessões, modelo e idade em cada sessão, rodapé só com ícones | Ver o modelo e "há quanto tempo" sem abrir a sessão | M |
| Estado do agente | ponto colorido (ok/warn/err/run) | spinner, check, pergunta, alerta, anel de progresso e tooltip | Saber de longe quem precisa de mim | M |
| Abas | 34 px, borda superior de foco | ícone de estado e do agente na própria aba | Estado visível sem trocar de aba | P–M |
| Cabeçalho do chat | seletor, "+", estado, provedor · modelo, tokens e custo | quase sem cabeçalho; a informação vai para a aba e a status bar | Menos poluição | M |
| Conversa | pedido em caixa, "N passos", ferramentas recolhidas | "Worked for Xs ›", ferramentas em linhas leves com ícone, cartões de subagente | Ritmo de leitura | M |
| Caixa de composição | anexar, /, @, pílula provedor · modelo, opções, Enviar | "+", pílula de permissão, modelo à direita, microfone, Enviar | Parecido; ajustar ordem e forma | P |
| Painel direito | Arquivos, Alterações, Subagentes e Servidores em abas de texto | painéis em split; diff com comentários por linha | Revisão mais rica | G |
| Status bar | cor sólida `--status-bg`; agentes, servidores, uso, player e pet | neutra; barras de limite por provedor | Menos cara de VS Code azul | P (cor) / M (barras) |
| Configurações | aba única com seções | página com navegação lateral por seção | Achar a opção mais rápido | M |
| Builds, Servidores, Métricas e Mapa | telas próprias | sem equivalente direto (a Orca tem Tasks e Automations) | Manter as telas; só aplicar os tokens | P |

## 3. Ícones da interface

| Biblioteca | Licença | Ícones | Estilo | Peso no bundle (com tree-shaking) |
|---|---|---|---|---|
| **Lucide** (`lucide-react`) | ISC (parte MIT, herdada do Feather) | ~1.600 | traço 2, 24 px | ~0,3–0,6 KB gzip por ícone; ~15–20 KB para ~45 ícones |
| Tabler | MIT | ~5.900 | traço 2, 24 px | parecido |
| Phosphor | MIT | ~1.500 × 6 pesos | vários pesos | maior por ícone |
| Codicons | CC-BY-4.0 | ~500 | estilo VS Code | fonte ou SVG; exige atribuição |

**Recomendação: Lucide.** É a biblioteca da Orca (mesmo visual) e a licença ISC não exige atribuição na interface. Os ícones de que o Argus precisa já existem: `panel-left`, `panel-right`, `square-terminal`, `git-compare`, `bot`, `hammer`, `server`, `shield-alert`, `loader-circle` e `circle-check`. Codicons fica de fora por exigir atribuição (CC-BY).

Como trocar mantendo a API `Icon({ name, size, filled, className })`, `IconName` e `toolIcon()`:

1. `npm i lucide-react`, com a versão fixada.
2. Em `src/shell/icons.tsx`, trocar `PATHS` por um mapa de nome para componente Lucide (`sidebarLeft: PanelLeft`, `gear: Settings`, `diff: GitCompare`, `send: ArrowUp`, `stop: Square`, `alert: TriangleAlert`...). O `Icon` repassa `size`, `strokeWidth={1.75}`, `className="ic ..."` e `aria-hidden`; `filled` vira `fill="currentColor"`. Os 18 arquivos que importam `Icon` não mudam.
3. Os nomes novos (estados do agente: `spinner`, `circleCheck`, `circleHelp`, `circleAlert`) entram no mesmo mapa.
4. Alternativa sem dependência: copiar os SVGs dos ícones Lucide usados para dentro de `PATHS`. A licença ISC permite; nesse caso, incluir o aviso em `THIRD_PARTY_NOTICES.md`.

## 4. Novo ícone do app

Três conceitos originais, sem animal marinho e sem a paleta da Orca. Os esboços estão em `C:\Users\kinga\AppData\Local\Temp\claude\a9397844e61500b9\70cff067-0f75-4d51-91ca-1f9576ad2b5f\scratchpad\icones\`. Essa pasta é temporária: mover para o repositório o conceito escolhido.

| Arquivo | Ideia |
|---|---|
| `conceito-a-cem-olhos.svg` | Argus Panoptes: uma íris central (o orquestrador) cercada por seis pontos num anel (os agentes que ele vigia). Em 16 px, lê bem como um alvo com satélites. |
| `conceito-b-a-orquestra.svg` | Um "A" geométrico cuja barra horizontal é um trilho com um nó no centro: monograma do Argus mais a linha de coordenação. É o mais simples e o que melhor funciona em 16 e 32 px. |
| `conceito-c-vigia-terminal.svg` | Uma janela de terminal com o prompt `>` e um olho estilizado: deixa claro que é ferramenta de desenvolvimento. É detalhado demais para 16 px; nesse tamanho, usar uma versão reduzida só com o olho. |

Processo:

1. Escolher o conceito e refiná-lo em vetor 1024 × 1024, com área segura de ~80% e uma versão simplificada para 16, 24 e 32 px.
2. Gerar os tamanhos: `npx tauri icon caminho/app-icon.png -o src-tauri/icons` (de um PNG 1024; o comando também aceita SVG). Ele gera `32x32`, `128x128`, `128x128@2x`, `icon.ico`, `icon.icns`, `Square*Logo` e `StoreLogo`, os mesmos arquivos que `tauri.conf.json > bundle.icon` já lista.
3. Conferir os outros lugares onde o ícone aparece:
   - Janela e barra de tarefas (`icon.ico`).
   - Instalador NSIS: usa `icon.ico`; se houver `nsis.installerIcon`, apontar para o mesmo arquivo.
   - "Argus Dev": se tiver ícone próprio, gerar uma variante com indicação de desenvolvimento.
   - Web app do celular: `crates/core/assets/remote/icon.svg` e `manifest.webmanifest`. Hoje o ícone acompanha o tema (`?tema=`); manter o desenho e trocar só as cores pelos tokens de cada tema (Oceano, Nebulosa e Brasa), gerado no Rust como hoje.
   - Favicon do `index.html`, se houver.
4. Validar o ícone na barra de tarefas em 100% e 150% de escala, no instalador e no "Adicionar à tela inicial" do celular, nos três temas.

## 5. Plano faseado

Regras: textos em português do Brasil, Windows primeiro, sem marca e toda cor por tokens nos três temas. Validação em todas as fases: `npm run app:dev`, CDP na janela como em PROXIMOS_PASSOS (prints dos três temas: início, conversa com turno terminado, Configurações e aba estreita) e build/typecheck sem erro.

| Fase | Entregável visível | Arquivos | Riscos | Como validar |
|---|---|---|---|---|
| 1. Tipografia e escala ✓ feita | Fonte Geist embutida (woff2 local, OFL), corpo de 13 px, mono Cascadia; escala de espaçamento `--space-1..6` (passo de 4 px); raios `--radius: 6px` e `--radius-lg: 10px` | `src/styles/tokens.css`, `src/styles/index.css`, `src/assets/fonts/` | A largura dos textos muda (truncamentos, aba estreita) | Prints antes e depois; aba estreita (print 07) |
| 2. Tokens de superfície ✓ feita | Bordas mais suaves (`--border` com ~7–10% do texto sobre a superfície), status bar neutra, barra lateral com tom próprio, cartões no lugar de linhas | `tokens.css`, `shell.css`, `components.css`, `crates/core/assets/remote/style.css` | Contraste AA nos temas Brasa e Nebulosa; erro não pode depender só da cor | Medir contraste; prints dos 3 temas |
| 3. Ícones Lucide ✓ feita | Todos os ícones trocados, com a mesma API | `package.json`, `src/shell/icons.tsx` | Ícone faltando no mapa (o TS acusa); diferença de peso do traço | Conferir os `name=` usados contra o mapa; prints |
| 4. Estados do agente ✓ feita | Componente de estado (spinner, check, pergunta, alerta, ponto) na barra lateral, nas abas e no cabeçalho, com tooltip em pt-BR | `src/views/session/AgentState.tsx` (novo; usado na barra lateral, na lista de conversas, nas abas e no cabeçalho), `store/app.ts` (`attention`), `icons.tsx` | Mapear os estados do core para os novos rótulos | Sessão rodando, concluída, com erro e com comando negado |
| 5. Ícone do app | Novo ícone em todos os tamanhos e na web app | `src-tauri/icons/*`, `crates/core/assets/remote/icon.svg`, geração do manifest | Cache de ícones do Windows (reinstalar ou limpar o cache) | Instalador NSIS, barra de tarefas e celular |
| 6. Barra esquerda no estilo da Orca ✓ feita | Busca no topo; sessão com modelo à direita (a idade fica para quando o core registrar a hora da sessão); sessão selecionada em cartão neutro (resolve o ícone de erro sumindo no Brasa); rodapé só com ícones, tooltip e aria-label; lista de conversas usa o rótulo do AgentState | `src/views/projects/*`, `shell.css` | Perder os rótulos do rodapé (compensar com tooltip e aria-label) | Prints 01, 18 e 36 |
| 7. Conversa e caixa de composição ✓ feita (referência: painel de chat do Claude Code no VS Code) | Resposta sem balão numa linha do tempo (bolinha por bloco, traço vertical); ferramenta em linha compacta e recolhida (bolinha verde/vermelha/cinza piscando, ícone Lucide, nome em negrito, resumo em mono) que abre IN/OUT, ou diff +/− no Edit/Write; "Read caminho (linhas x–y)"; raciocínio "Pensou" recolhido em itálico; TodoWrite como checklist; subagente, negado e falha no mesmo estilo compacto; caixa de composição mantida (já era retângulo arredondado com pílulas e envio circular) | Core: eventos `ToolResult { ok, output }` (saída cortada em 4000 caracteres, na ordem dos `ToolUse` do dono) e `Thinking` no parser do claude; `src/views/session/ToolRow.tsx` (novo), `Transcript.tsx`, `events.ts`, `session.css`, `icons.tsx` | Histórico antigo não tem saída nem status (bolinha vazada "sem saída registrada"); codex/gemini/opencode ainda não emitem `ToolResult` | Prints dos 3 temas: linhas recolhidas e abertas, diff, raciocínio, caixa e aba estreita |
| 8. Abas na barra de título ✓ feita | Some a faixa de 32 px: `decorations: false` (também no perfil dev); as faixas de abas do topo viram a linha de título, com o alternar da barra esquerda flutuando no canto esquerdo e terminal, painel direito e minimizar/maximizar/fechar no direito; arrasto e duplo clique (maximizar) pela sobra da faixa de abas, pelo título "Argus" da barra esquerda e pelo topo do painel direito (`data-tauri-drag-region`); a trilha "projeto › aba" saiu (a informação já está na aba) | `tauri.conf.json`, `tauri.dev.conf.json`, `capabilities/default.json`, `TopBar.tsx`, `Shell.tsx`, `RightPanel.tsx`, `projects/index.tsx`, `workbench/index.tsx`, `shell.css`, `tokens.css` | Snap Layouts do Windows 11, arrasto, duplo clique e acessibilidade | Testar maximizar, snap e arrasto |
| 9. Painéis em split e diff anotável ✓ feita | Divisor arrastável entre os grupos (lado a lado e empilhados; duplo clique iguala) e botão na faixa de abas para trocar a direção; diff da sessão como aba "Alterações #n" (botão no painel Alterações), que vai para qualquer grupo do split, com as linhas +/− do chat numeradas; clicar no número da linha abre um comentário que entra no rascunho da conversa como `arquivo:linha — texto` (o usuário revisa e envia). Fora: terminal e arquivos já eram abas divisíveis; o painel direito continua fixo (não vira grupo); comentário não é enviado sozinho nem fica guardado no core | `store/workbench.ts` (aba `diff`, `sizes`, `openDiff`), `workbench/index.tsx`, `session/DiffTab.tsx` (novo), `session/ToolRow.tsx` (`Diff` exportado, com número e comentário), `session/panels.tsx`, `session.css`, `shell.css` | Diff de arquivo binário ou renomeado aparece só pelo cabeçalho | Dividir lado a lado e empilhado, redimensionar, abrir o diff e comentar uma linha |

## Fontes

- Orca: <https://github.com/stablyai/orca>, [LICENSE](https://github.com/stablyai/orca/blob/main/LICENSE), [main.css](https://github.com/stablyai/orca/blob/main/src/renderer/src/assets/main.css), [AgentStateDot.tsx](https://github.com/stablyai/orca/blob/main/src/renderer/src/components/AgentStateDot.tsx), [components.json](https://github.com/stablyai/orca/blob/main/components.json), <https://orca.dev>
- Lucide: <https://lucide.dev> (ISC). Tabler: <https://tabler.io/icons> (MIT). Phosphor: <https://phosphoricons.com> (MIT). Codicons: <https://github.com/microsoft/vscode-codicons> (CC-BY-4.0)
- Geist: <https://github.com/vercel/geist-font> (OFL-1.1)
- `tauri icon`: <https://v2.tauri.app/reference/cli/#icon>
