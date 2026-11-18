# Changelog

O Argus segue versionamento semântico. Enquanto o app estiver na série 0.1.x, cada release
corrige ou reorganiza a interface sem mudar o formato dos dados salvos. A versão fica em três
lugares, que precisam andar juntos: `package.json`, `src-tauri/tauri.conf.json` e
`[workspace.package]` no `Cargo.toml`. Cada release tem uma tag `vX.Y.Z` e o instalador
`Argus_X.Y.Z_x64-setup.exe` anexado no GitHub.

## Não lançado

- Servidores: a lista agora enxerga servidor que roda na pasta do projeto da sessão mesmo fora
  da árvore de processos dela — é o caso do opencode, que executa os comandos por um daemon
  próprio, anterior ao Argus (por isso o dev server do projeto não aparecia nem no computador
  nem no celular). Esses entram marcados como **"pela pasta"**, para não se confundirem com os
  que descendem do processo lançado pelo Argus.
- Túneis: `cloudflared`, `ngrok` e `tailscale funnel` são reconhecidos pela linha de comando e
  viram o selo **"exposto"** no servidor da porta publicada, com o endereço público lido da API
  local da própria ferramenta (nada sai da máquina). Dá para copiar o endereço, abrir e
  **fechar só o túnel**, sem derrubar o servidor. Vale no computador e no celular.

- Permissões: nível novo **"Compilar e testar"**, ligado por padrão — `npm run`, `cargo test`,
  `npx tsc`, `pytest`, `go test`, `dotnet build`, `make` e companhia deixam de ser negados ao
  agente, inclusive no auto-work. Instalar dependência e publicar continuam de fora. Dá para
  desligar no geral ou por projeto (Configurações › Permissões).
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
