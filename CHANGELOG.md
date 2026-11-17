# Changelog

O Argus segue versionamento semântico. Enquanto o app estiver na série 0.1.x, cada release
corrige ou reorganiza a interface sem mudar o formato dos dados salvos. A versão fica em três
lugares, que precisam andar juntos: `package.json`, `src-tauri/tauri.conf.json` e
`[workspace.package]` no `Cargo.toml`. Cada release tem uma tag `vX.Y.Z` e o instalador
`Argus_X.Y.Z_x64-setup.exe` anexado no GitHub.

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
