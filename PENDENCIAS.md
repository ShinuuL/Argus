# Argus: integração de outros projetos

_Criado em 2026-11-16. Complementa o [PROXIMOS_PASSOS.md](PROXIMOS_PASSOS.md)._

A meta é que o Argus seja o hub único das ferramentas de desenvolvimento. Três projetos separados entram nele como módulos:

| Projeto | O que é hoje | Stack | Como entra no Argus |
|---|---|---|---|
| **Screen-Mirror** | espelha e controla celulares Android por ADB e scrcpy; tem MCP para agentes e API HTTP | Node 22 + TS, web sem framework | **sidecar** primeiro; porte gradual para Rust depois |
| **Gerenciador de Portas** | portas TCP/UDP, processos, conflitos, firewall, faixas reservadas | C# .NET 8 WPF + WebView2 | **porte para Rust** (`argus-core::ports`); a UI web é reaproveitada |
| **AllSeenEye** | captura de tráfego no próprio celular (VPN local), proxy MITM, scan Wi-Fi, conteúdo didático | Kotlin + Compose (Android) | **continua como app Android**; o Argus o instala, controla e lê os dados pelo módulo do Screen-Mirror |

---

## 0. Base comum (fazer antes dos módulos)

- [ ] **Registro de módulos.** Cada ferramenta vira uma aba ou tela de apoio aberta pelo rodapé da barra esquerda (como `servers/`, `metrics/`), com comandos Tauri em `src-tauri/src/commands/<modulo>.rs` e a lógica em `crates/core/src/<modulo>/`.
- [ ] **Gerenciador de sidecars** no core: inicia, faz health-check, reinicia e encerra processos auxiliares junto com a janela. Reaproveita `procs` e `servers`. Serve para o Screen-Mirror (Node) e para futuros auxiliares.
- [ ] **Helper elevado.** Matar processos de outros usuários, parar serviços e mexer no firewall exigem administrador. O Argus **não** deve rodar inteiro elevado. Proposta: um executável pequeno `argus-elevated.exe`, iniciado sob demanda via UAC, que fala com o app por named pipe e só aceita uma lista fechada de comandos. Toda ação vai para o log de auditoria.
- [ ] **Política e auditoria unificadas.** Juntar `permissions` (comandos liberados ao agente) com o `policy.ts` do Screen-Mirror (modos leitura/interativo/total, apps bloqueados, limite de taxa, `audit.log`) numa política única em `%APPDATA%\argus\`, com tela de edição em Configurações.
- [ ] **Ferramentas para os agentes.** Expor os módulos novos aos agentes do orquestrador (portas, celular, captura) como ferramentas MCP servidas pelo próprio Argus, sempre passando pela política.
- [ ] **Componentes de UI.** Tabela filtrável, selo de status e painel de detalhes compartilhados, seguindo `tokens.css` e os ícones Lucide.

---

## 1. Gerenciador de Portas → módulo `ports`

O backend é pequeno e cada serviço tem interface própria, então portar é mais barato do que manter .NET junto do app. O Argus já lista servidores e processos (`procs`, aba Servidores), e o módulo de portas fica ao lado deles.

### Reaproveitamento

| Origem (C#) | Destino (Rust) | Observação |
|---|---|---|
| `NativeMethods.cs` + `PortService.cs` | `core/ports/scan.rs` | crate `windows` (`GetExtendedTcpTable`/`UdpTable`, IPv4 e IPv6) ou `netstat2` |
| detecção de conflito (`PortService`, `MainViewModel`) | `core/ports/conflict.rs` | mesma regra: mesma porta e protocolo com mais de um listener |
| `ProcessService.cs` (WMI) | `core/ports/process.rs` | `sysinfo`; `wmi` só para linha de comando quando faltar |
| `ServiceService.cs` | `core/ports/service.rs` | crate `windows-service`; parar serviço vai pelo helper elevado |
| `FirewallService.cs` | `core/ports/firewall.rs` | `netsh advfirewall` com os mesmos argumentos; regra renomeada para `Argus Block :{porta}`; vai pelo helper elevado |
| `ReservedPortService.cs` | `core/ports/reserved.rs` | portar as regex dos dois formatos de saída; nunca falha |
| `ExportService.cs` | `core/ports/export.rs` | CSV/JSON |
| `PortEntry` / `Settings` | structs serde + tipos em `src/api/` | manter os contratos JSON para reaproveitar o `app.js` |
| `Assets/Web/app.js` + `styles.css` | `src/views/ports/` | reescrever em React usando a lógica de filtros e busca; trocar `hostObjects.backend.*Json()` por `invoke` |
| `GerenciadorDePortas.Tests` | testes em `core/ports` | usar os casos como checklist do porte |

**Descartar:** Views XAML, ViewModels, ponte COM do WebView2 e ícone de bandeja do WPF.

### Etapas

- [ ] P1. `scan` + `conflict` + `reserved` em Rust, com testes copiados dos casos do xUnit.
- [ ] P2. Aba **Portas** no painel direito: tabela, busca (inclusive na linha de comando), filtros TCP/UDP e estado, selo de conflito, atualização a cada 2 s só com a aba visível.
- [ ] P3. Ações: encerrar processo ou árvore (com confirmação nativa), parar serviço, bloquear porta. As que precisam de admin vão pelo helper elevado.
- [ ] P4. Integração com o resto do Argus:
  - antes de subir um servidor de projeto, avisar se a porta já está ocupada e por quem;
  - selo de porta reservada (Hyper-V/WSL) ao lado dos servidores;
  - ação "liberar porta" no menu de contexto de um servidor.
- [ ] P5. Exportar CSV/JSON. Itens da v2 da spec (histórico, bloqueio por IP, cards) entram depois.
- [ ] P6. Arquivar o repositório antigo, apontando para o Argus no README.

---

## 2. Screen-Mirror → módulo `devices` (celulares Android)

O Screen-Mirror funciona e tem MCP pronto. Reescrever o protocolo do scrcpy de cara é caro, então entra primeiro como sidecar e é portado aos poucos.

### Fase A: sidecar (sem mudar o projeto)

> **Feito em 2026-11-16 (branch `integracao-screen-mirror`):** `argus-core::devices` inicia, confere e encerra o Screen-Mirror; a aba **Dispositivos** embute o painel completo num iframe (sem mudar o Screen-Mirror) e lista os aparelhos. Falta conferir na janela e empacotar.

- [ ] A1. Empacotar o Screen-Mirror como recurso do Argus e iniciá-lo com `node src/cli.ts serve` pelo gerenciador de sidecars, em `127.0.0.1` com porta livre escolhida pelo Argus (`SM_PORT`). Requisito: Node 22.18 ou mais novo; avisar na aba Provedores/Dependências se faltar.
- [ ] A2. Aba **Dispositivos**: lista de aparelhos (`/api/devices`), info, apps, logcat e arquivos usando a API HTTP.
- [ ] A3. Espelhamento dentro do Argus: portar `web/mirror.js` (WebCodecs + canvas + mapeamento de mouse e teclado) para um componente React que fala o protocolo WebSocket do `MirrorHub`. O WebView2 suporta WebCodecs.
- [x] A4. Registrar o MCP do Screen-Mirror automaticamente nas conversas de agente, com a política aplicada. Feito para **claude** (`--mcp-config` + `--allowedTools mcp__screen-mirror`) e **codex** (`-c mcp_servers.screen-mirror.*`), com liga/desliga na aba Dispositivos. Nada é injetado se a CLI já tiver o Screen-Mirror configurado. **opencode** e **gemini** ficam de fora: o gemini não tem opção de linha de comando para MCP, e o opencode 2.3.0 não somou servidores por `OPENCODE_CONFIG_CONTENT` nem `OPENCODE_CONFIG` nos testes. Pendente: conferir um turno do codex de verdade (não instalado na máquina dos testes).
- [x] Som do celular no PC: feito no Screen-Mirror (branch `audio-no-pc`). O scrcpy manda PCM cru (48 kHz, estéreo) por um terceiro socket e o painel toca pela Web Audio API. Tem botão de liga/desliga, e o celular fica mudo enquanto o som vai para o PC. Precisa de Android 11 ou mais novo. Testado com um tom de 440 Hz no SM-M556B.
- [x] A5. Instalar APK do projeto ativo (faixa na aba Dispositivos; `adb install -r` com confirmação; só APKs das pastas de build do Gradle e do Flutter). Testado até o adb; ainda não instalei nada de verdade no celular.
- [ ] ~~A5 original:~~ Instalar APK do projeto ativo: detectar o APK gerado (Gradle/Flutter) e oferecer "instalar no aparelho", mantendo a confirmação de usuário que já existe.

**Cuidado:** a API HTTP do Screen-Mirror não tem autenticação. Manter em `127.0.0.1` e, se possível, passar um token aleatório gerado pelo Argus (exige um ajuste pequeno no `http.ts`).

### Fase B: porte para Rust (quando o sidecar estiver estável)

| Origem (TS) | Destino (Rust) |
|---|---|
| `core/adb.ts`, `DeviceTracker` | `core/devices/adb.rs` (processo `adb` ou crate `adb_client`) |
| `core/device.ts` (input, screenshot, uiautomator, apps, arquivos) | `core/devices/device.rs` |
| `core/logcat.ts` | `core/devices/logcat.rs` com eventos Tauri |
| `core/scrcpy.ts` (protocolo v5, demux de vídeo, controle) | `core/devices/scrcpy.rs`: a parte mais valiosa e a mais trabalhosa |
| `core/policy.ts` | já absorvido pela política unificada (seção 0) |
| `mcp/server.ts` | ferramentas MCP servidas pelo Argus |
| `config.ts` (localizar adb, SHA-256 do scrcpy-server) | mesma lógica no Rust |

- [ ] B1. adb + device + logcat em Rust; desligar as rotas HTTP equivalentes.
- [ ] B2. scrcpy em Rust; vídeo enviado à interface por canal binário em vez de WebSocket.
- [ ] B3. Remover a dependência de Node; aposentar o sidecar.
- [ ] Pendências que já existiam: pareamento Wi-Fi (`adb pair`/`adb connect`) pela interface, áudio e gravação.

---

## 3. AllSeenEye → app companheiro Android

É um app Android (VPN local, proxy MITM, scan Wi-Fi), e o código só existe para o celular. Não se porta para o desktop. A integração acontece por meio do módulo `devices`:

- [ ] E1. O Argus reconhece o projeto como Android (Gradle) e compila, instala e abre o app pelo módulo de dispositivos.
- [ ] E2. **Ponte de dados:** o AllSeenEye expõe o que capturou (pacotes do `CaptureBus`, trocas HTTP do `ProxyBus`, scan Wi-Fi) num socket local ou num arquivo `.pcap`/JSON. O Argus lê por `adb forward` ou `adb pull` e mostra numa aba **Rede do aparelho**. Junto vem o item do roadmap do próprio app: salvar sessões (Room) e exportar `.pcap`.
- [ ] E3. Tipos compartilhados: `CapturedPacket`, `HttpExchange` e `WifiModels` viram structs Rust e tipos TS, com o mesmo JSON do Kotlin.
- [ ] E4. Depuração de apps em teste: durante uma conversa de agente, o tráfego HTTP do app em teste aparece junto ao logcat, o que ajuda a depurar integrações (APIs de ERP/PDV).
- [ ] E5 (opcional, desktop): capturar o tráfego do próprio PC no Argus, com Npcap e a crate `pcap` e a lógica do `PacketParser` (ou `etherparse`). Só depois de E2 e só para o tráfego da própria máquina.
- [ ] E6. `EducationContent.kt` pode virar JSON para cards de ajuda nas abas Rede e Portas.

**Conformidade e segurança:**
- Captura só do próprio aparelho e da própria máquina, como diz o README do app (Lei 12.737/2012). Nada de spoofing nem captura de terceiros.
- A chave privada da CA do MITM nunca sai do aparelho e nunca vai para o repositório ou para logs. **Pendente:** conferir onde o app guarda essa chave hoje.
- Capturas podem conter dados de clientes (LGPD): mascarar `Authorization`, cookies e tokens antes de exibir ou mandar a um agente. Apagar sessões antigas, como já é feito com os anexos.

---

## Conversa (interface)

- [x] C1. Links clicáveis no chat: URLs na resposta do agente (ex.: link de PR) não abrem. Tornar links do markdown e URLs soltas clicáveis, abrindo no navegador padrão pelo opener do Tauri (nunca dentro do WebView); caminhos de arquivo (`arquivo:linha`) abrindo no editor do Argus.
- [x] C2. Rascunho por sessão: o texto digitado e não enviado (e os anexos) fica salvo por sessão/aba e volta ao trocar de aba, de sessão ou de janela e ao reabrir o app. Some só ao enviar ou apagar.

## 4. Ordem sugerida

1. Base comum: registro de módulos, gerenciador de sidecars e política unificada.
2. **Portas** P1 a P4: valor imediato, porte pequeno e se conecta aos servidores que o Argus já gerencia.
3. **Dispositivos** fase A (sidecar + espelhamento + MCP).
4. Helper elevado (destrava as ações de admin das Portas).
5. **AllSeenEye** E1 a E3.
6. Dispositivos fase B (porte para Rust) e E5, conforme a necessidade.

## 5. Dúvidas em aberto

- Os repositórios antigos continuam vivos (versão standalone) ou são arquivados depois da integração?
- O Screen-Mirror continua utilizável sozinho (CLI/MCP fora do Argus)? Se sim, a fase B precisa manter uma CLI Rust equivalente.
- Distribuição: incluir `adb`/platform-tools e o scrcpy-server no instalador do Argus ou baixar sob demanda, verificando o SHA-256 como o Screen-Mirror já faz?
