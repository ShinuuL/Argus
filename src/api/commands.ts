/**
 * Uma função por comando do backend (src-tauri/src/commands/*).
 *
 * Todas devolvem Promise e rejeitam com uma string (mensagem em português
 * pronta para exibir). Use `errorMessage(e)` de `./util` no `catch`.
 */
import { invoke } from "@tauri-apps/api/core";
import { ensurePtyListeners, forgetPty } from "./events";
import type {
  AccessMode,
  AgentEvent,
  AppInfo,
  AutostartInfo,
  Schedule,
  AutoWorkEvent,
  AutoWorkSettings,
  CmdSpec,
  CommandSpec,
  DepIssue,
  DeviceList,
  Apk,
  MirrorStatus,
  DirEntry,
  RemoteExposure,
  FileStat,
  InstallPlan,
  LanAddr,
  LoginStatus,
  MediaCmd,
  MetricsReport,
  IsolationReport,
  MissingCommand,
  ModelInfo,
  MusicFolder,
  OwnedProc,
  Owner,
  Path,
  PermissionMode,
  PermissionsInfo,
  Pricing,
  Project,
  ProjectAutoWork,
  ProjectChanged,
  ProjectStatus,
  ProjectUsage,
  ProviderInfo,
  PtyId,
  PtySession,
  RemoteInfo,
  TailscaleStatus,
  RootInfo,
  ServerInfo,
  BuildRecord,
  DetectedCommand,
  PreparedRun,
  ProjectCommands,
  RememberedServer,
  RunInfo,
  SubagentInfo,
  SystemMedia,
  SystemUsage,
  Task,
  TaskId,
  TimeRange,
  TodayUsage,
  UsageRow,
  WorkMode,
  WorkModeInfo,
  Checklist,
} from "./types";

// ------------------------------------------------------------------ workspace

/** Abre o seletor de pasta do sistema; null se o usuário cancelar. */
export const workspacePickRoot = (): Promise<Path | null> => invoke("workspace_pick_root");

/** Abre `path` como pasta raiz: varre projetos, liga o watcher e dispara `project-changed` por projeto. */
export const workspaceOpenRoot = (path: Path): Promise<RootInfo> => invoke("workspace_open_root", { path });

/** Pasta raiz já aberta no backend (ex.: via ARGUS_ROOT), ou null. */
export const workspaceCurrent = (): Promise<RootInfo | null> => invoke("workspace_current");

/** Só varre uma pasta em busca de projetos, sem mudar a raiz atual. */
export const workspaceScan = (path: Path): Promise<Project[]> => invoke("workspace_scan", { path });

/** Varre de novo a raiz atual e recalcula o status git de todos os projetos; null sem raiz aberta. */
export const workspaceRefresh = (): Promise<RootInfo | null> => invoke("workspace_refresh");

// -------------------------------------------------------------------- monitor

/** Status git de um projeto (null se não for repositório). */
export const monitorStatus = (path: Path): Promise<ProjectStatus | null> => invoke("monitor_status", { path });

/** Status git de vários projetos de uma vez. */
export const monitorStatuses = (paths: Path[]): Promise<ProjectChanged[]> => invoke("monitor_statuses", { paths });

/** Troca o watcher para observar `paths` (mudanças chegam em `onProjectChanged`). */
export const monitorWatch = (paths: Path[]): Promise<void> => invoke("monitor_watch", { paths });

/** Desliga o watcher de arquivos. */
export const monitorUnwatch = (): Promise<void> => invoke("monitor_unwatch");

// ------------------------------------------------------------------------- fs

/** Lê um arquivo como texto UTF-8 (limite de 20 MB). */
export const fsReadFile = (path: Path): Promise<string> => invoke("fs_read_file", { path });

/** Check-list de pendências do projeto (PROXIMOS_PASSOS.md, PENDENCIAS.md ou TODO.md); null se não houver. */
export const projectChecklist = (project: Path): Promise<Checklist | null> => invoke("project_checklist", { project });

/** Grava o texto no arquivo, substituindo o conteúdo. */
export const fsWriteFile = (path: Path, content: string): Promise<void> => invoke("fs_write_file", { path, content });

/** Lista um diretório, pastas primeiro; sem `showHidden` oculta `.algo`, node_modules e target. */
export const fsListDir = (path: Path, showHidden = false): Promise<DirEntry[]> =>
  invoke("fs_list_dir", { path, showHidden });

/** Existência, tipo, tamanho e data de modificação de um caminho. */
export const fsStat = (path: Path): Promise<FileStat> => invoke("fs_stat", { path });

// ------------------------------------------------------------------------ pty

export interface PtySpawnOptions {
  /** Pasta inicial; vale sobre o `cwd` do comando. */
  cwd?: Path | null;
  /** Sem comando, abre o shell padrão. Aceita o `CommandSpec` de `agentsLoginCommand` e de `InstallPlan`. */
  command?: CmdSpec | CommandSpec | null;
  cols?: number;
  rows?: number;
}

/** Abre um terminal e devolve o id; a saída chega em `onPtyOutput(id)` (nada se perde antes de assinar). */
export async function ptySpawn(opts: PtySpawnOptions = {}): Promise<PtyId> {
  await ensurePtyListeners();
  return invoke("pty_spawn", {
    cwd: opts.cwd ?? null,
    command: opts.command ?? null,
    cols: opts.cols ?? null,
    rows: opts.rows ?? null,
  });
}

/** Envia texto digitado para o terminal. */
export const ptyWrite = (id: PtyId, data: string): Promise<void> => invoke("pty_write", { id, data });

/** Redimensiona o terminal (colunas x linhas). */
export const ptyResize = (id: PtyId, cols: number, rows: number): Promise<void> =>
  invoke("pty_resize", { id, cols, rows });

/** Encerra o processo do terminal e descarta os assinantes dele. */
export async function ptyKill(id: PtyId): Promise<void> {
  try {
    await invoke("pty_kill", { id });
  } finally {
    forgetPty(id);
  }
}

/** Terminais abertos no backend. */
export const ptyList = (): Promise<PtySession[]> => invoke("pty_list");

/** Shell padrão do sistema (pwsh, powershell ou $SHELL). */
export const ptyDefaultShell = (): Promise<CmdSpec> => invoke("pty_default_shell");

// -------------------------------------------------------------------- agentes

/** Provedores de agente disponíveis (claude, opencode). */
export const agentsProviders = (): Promise<ProviderInfo[]> => invoke("agents_providers");

/** Status de login de um provedor, perguntando à própria CLI (pode levar alguns segundos). */
export const agentsLoginStatus = (provider: string): Promise<LoginStatus> =>
  invoke("agents_login_status", { provider });

/** Comando de login da CLI, para rodar num terminal (`ptySpawn({ command })`). */
export const agentsLoginCommand = (provider: string): Promise<CommandSpec> =>
  invoke("agents_login_command", { provider });

/** Todas as tarefas, inclusive as antigas do histórico. */
export const tasksList = (): Promise<Task[]> => invoke("tasks_list");

/** Uma tarefa pelo id (null se não existir). */
export const taskGet = (id: TaskId): Promise<Task | null> => invoke("task_get", { id });

/** Opções de uma conversa nova. */
export interface TaskCreateOptions {
  /** Modelo (null ou vazio = padrão da CLI). */
  model?: string | null;
  /** Modo de trabalho (ausente = o padrão do projeto). */
  mode?: WorkMode | null;
  /** Acesso desde o primeiro turno (ausente = normal). */
  access?: AccessMode | null;
}

/** Cria uma tarefa (isolada num worktree, ou direto na pasta) e devolve o id; o andamento chega em `onOrch`. */
export const taskCreate = (project: Path, provider: string, prompt: string, opts: TaskCreateOptions = {}): Promise<TaskId> =>
  invoke("task_create", { project, provider, prompt, model: opts.model || null, mode: opts.mode ?? null, access: opts.access ?? null });

/** Troca o modelo da tarefa (null = padrão da CLI); vale a partir do próximo turno. */
export const taskSetModel = (id: TaskId, model: string | null): Promise<void> =>
  invoke("task_set_model", { id, model: model || null });

/** Troca o modo de acesso da conversa (Planejar / normal / Bypass); vale a partir do próximo turno. */
export const taskSetAccess = (id: TaskId, access: AccessMode): Promise<void> =>
  invoke("task_set_access", { id, access });

/** Troca o agente (provedor) da tarefa, com o turno parado; o próximo turno leva o resumo da conversa. */
export const taskSetProvider = (id: TaskId, provider: string): Promise<void> =>
  invoke("task_set_provider", { id, provider });

/** Modelos que a CLI do provedor aceita (sem gastar tokens; a lista fica em memória no backend). */
export const agentsModels = (provider: string, refresh = false): Promise<ModelInfo[]> =>
  invoke("agents_models", { provider, refresh });

/** Preferências de modo de trabalho; com `project`, inclui a do projeto e o que vale para ele. */
export const workmodeGet = (project?: Path | null): Promise<WorkModeInfo> =>
  invoke("workmode_get", { project: project ?? null });

/** Grava a preferência geral (`project` null) ou a do projeto; `mode` null apaga a do projeto. */
export const workmodeSet = (project: Path | null, mode: WorkMode | null): Promise<void> =>
  invoke("workmode_set", { project, mode });

/** Conjunto padrão de comandos liberados e as preferências dos níveis de git (geral e do projeto). */
export const permissionsGet = (project?: Path | null): Promise<PermissionsInfo> =>
  invoke("permissions_get", { project: project ?? null });

/** Liga/desliga um nível no geral (`project` null) ou no projeto; `on` null apaga a preferência. */
export const permissionsSet = (project: Path | null, level: "local" | "remote" | "build", on: boolean | null): Promise<void> =>
  invoke("permissions_set", { project, level, on });

/** Grava o modo de permissão no geral (`project` null) ou no projeto; `mode` null apaga a do projeto. */
/** Limite de contexto (tokens) para compactar antes do próximo turno; 0 = desligado. */
export const autocompactGet = (): Promise<number> => invoke("autocompact_get");
export const autocompactSet = (tokens: number | null): Promise<void> => invoke("autocompact_set", { tokens });
export const permissionModeSet = (project: Path | null, mode: PermissionMode | null): Promise<void> =>
  invoke("permission_mode_set", { project, mode });

/** Acrescenta um comando negado à lista de comandos permitidos do projeto; devolve a lista. */
export const permissionsAllowDenied = (project: Path, tool: string, command: string): Promise<string[]> =>
  invoke("permissions_allow_denied", { project, tool, command });

/** O que a cópia isolada do projeto não teria (não versionados, ignorados, não commitados, caminhos citados). */
export const isolationCheck = (project: Path, prompt: string): Promise<IsolationReport> =>
  invoke("isolation_check", { project, prompt });

/** Cancela o turno em andamento da tarefa. */
export const taskCancel = (id: TaskId): Promise<void> => invoke("task_cancel", { id });

/** Responde ao agente: novo turno na mesma sessão da CLI. */
export const taskReply = (id: TaskId, message: string): Promise<void> => invoke("task_reply", { id, message });

/** Eventos gravados da tarefa (vazio se o histórico estiver desligado). */
export const taskHistory = (id: TaskId): Promise<AgentEvent[]> => invoke("task_history", { id });

/** Subagentes criados pela tarefa (visão somente leitura), na ordem em que nasceram. */
export const taskSubagents = (id: TaskId): Promise<SubagentInfo[]> => invoke("task_subagents", { id });

/** Eventos internos de um subagente da tarefa (`subagent` = `SubagentInfo.id`). */
export const taskSubagentHistory = (id: TaskId, subagent: string): Promise<AgentEvent[]> =>
  invoke("task_subagent_history", { id, subagent });

/** Diff da tarefa: o do worktree (isolada) ou o da pasta em relação ao último commit (direta). "" = sem alterações. */
export const taskDiff = (id: TaskId): Promise<string> => invoke("task_diff", { id });

/** Rejeita as alterações da tarefa num arquivo (caminho do diff): volta ao último commit ou apaga o arquivo novo. */
export const taskRevertFile = (id: TaskId, path: string): Promise<void> => invoke("task_revert_file", { id, path });

/** Rejeita um trecho do diff de um arquivo (`hunk` = a linha "@@ -a,b +c,d @@"). */
export const taskRevertHunk = (id: TaskId, path: string, hunk: string): Promise<void> =>
  invoke("task_revert_hunk", { id, path, hunk });

/** Aplica as alterações da tarefa no projeto. */
export const taskApply = (id: TaskId): Promise<void> => invoke("task_apply", { id });

/** Descarta as alterações da tarefa e remove o worktree. */
export const taskDiscard = (id: TaskId): Promise<void> => invoke("task_discard", { id });

/**
 * Exclui a sessão: cancela o turno, remove worktree e branch (modo isolado), apaga
 * a tarefa e o histórico. O consumo continua nas métricas. Chega `Removed` em `onOrch`.
 */
export const taskDelete = (id: TaskId): Promise<void> => invoke("task_delete", { id });

/** Exclui as sessões encerradas (aplicadas ou descartadas) do projeto; devolve quantas. */
export const taskDeleteClosed = (project: Path): Promise<number> => invoke("task_delete_closed", { project });

// ------------------------------------------------------------------ auto-work

/** Configuração do auto-work de um projeto (padrões se nunca foi salva). */
export const autoworkGetSettings = (project: Path): Promise<AutoWorkSettings> =>
  invoke("autowork_get_settings", { project });

/** Salva a configuração do auto-work do projeto e devolve o que ficou gravado. */
export const autoworkSetSettings = (project: Path, settings: AutoWorkSettings): Promise<AutoWorkSettings> =>
  invoke("autowork_set_settings", { project, settings });

/** Configurações de auto-work salvas de todos os projetos. */
export const autoworkAllSettings = (): Promise<ProjectAutoWork[]> => invoke("autowork_all_settings");

/** Progresso do auto-work de uma tarefa (null se nunca rodou). */
export const autoworkStatus = (task: TaskId): Promise<AutoWorkEvent | null> => invoke("autowork_status", { task });

/** Progresso de todas as execuções de auto-work. */
export const autoworkAllStatus = (): Promise<AutoWorkEvent[]> => invoke("autowork_all_status");

/** Inicia ou reinicia o auto-work numa tarefa; false se não foi possível. */
export const autoworkStart = (task: TaskId): Promise<boolean> => invoke("autowork_start", { task });

/** Pausa o auto-work (o turno atual termina). */
export const autoworkPause = (task: TaskId): Promise<boolean> => invoke("autowork_pause", { task });

/** Retoma o auto-work pausado. */
export const autoworkResume = (task: TaskId): Promise<boolean> => invoke("autowork_resume", { task });

/** Para o auto-work da tarefa. */
export const autoworkStop = (task: TaskId): Promise<boolean> => invoke("autowork_stop", { task });

// ----------------------------------------------------------------- servidores

/** Servidores em execução abertos por sessões e terminais. */
export const serversList = (): Promise<ServerInfo[]> => invoke("servers_list");

/** Servidores de um dono (sessão ou terminal). */
export const serversOf = (owner: Owner): Promise<ServerInfo[]> => invoke("servers_of", { owner });

/** Processos vivos de um dono (tarefas em segundo plano da sessão). */
export const ownerProcesses = (owner: Owner): Promise<OwnedProc[]> => invoke("owner_processes", { owner });

/** Derruba um servidor (processo e filhos) e devolve a lista atualizada. */
export const serverKill = (pid: number): Promise<ServerInfo[]> => invoke("server_kill", { pid });

/** Derruba todos os servidores de um dono; devolve quantos caíram. */
export const serversStopAll = (owner: Owner): Promise<number> => invoke("servers_stop_all", { owner });

/** Reinicia um servidor com a mesma linha de comando; devolve o novo PID. */
export const serverRestart = (server: ServerInfo): Promise<number> => invoke("server_restart", { server });

/** Padrões de comando liberados ao agente no projeto (um por item). */
export const allowedCommandsGet = (project: Path): Promise<string[]> => invoke("allowed_commands_get", { project });

/** Salva os comandos permitidos do projeto; vale a partir do próximo turno. */
export const allowedCommandsSet = (project: Path, commands: string[]): Promise<string[]> =>
  invoke("allowed_commands_set", { project, commands });

// --------------------------------------------------------------- dependências

/** Pendências de dependências do projeto, cada uma com o comando de correção (nada é executado). */
export const depsAnalyze = (project: Path): Promise<DepIssue[]> => invoke("deps_analyze", { project });

/** Reconhece "comando não encontrado" num texto de terminal (null se não houver). */
export const depsDetectMissingCommand = (text: string): Promise<MissingCommand | null> =>
  invoke("deps_detect_missing_command", { text });

/** Plano de instalação de uma ferramenta conhecida (null se desconhecida). */
export const depsInstallPlan = (command: string): Promise<InstallPlan | null> =>
  invoke("deps_install_plan", { command });

// ------------------------------------------------------------------- métricas

/** Relatório de tokens e custo equivalente em API dos últimos `days` dias (0 = tudo). */
export const metricsReport = (days: number): Promise<MetricsReport> => invoke("metrics_report", { days });

/** Tokens e custo equivalente em API de hoje. */
export const metricsToday = (): Promise<TodayUsage> => invoke("metrics_today");

/** Linhas brutas de uso (uma por turno) no intervalo; sem intervalo, tudo. */
export const metricsUsageRows = (range?: TimeRange): Promise<UsageRow[]> =>
  invoke("metrics_usage_rows", { range: range ?? null });

/** Uso agregado por projeto, provedor e modelo no intervalo. */
export const metricsUsageByProject = (range?: TimeRange): Promise<ProjectUsage[]> =>
  invoke("metrics_usage_by_project", { range: range ?? null });

/** Tabela de preços em uso (padrões mesclados com o pricing.json). */
export const pricingGet = (): Promise<Pricing> => invoke("pricing_get");

/** Salva a tabela de preços no pricing.json e devolve a tabela resultante. */
export const pricingSet = (pricing: Pricing): Promise<Pricing> => invoke("pricing_set", { pricing });

/** Caminho do pricing.json (null se não houver pasta de dados). */
export const pricingPath = (): Promise<Path | null> => invoke("pricing_path");

// --------------------------------------------------------------------- remoto

/** Estado do servidor remoto (null se desligado). */
// ------------------------------------------------------------ celulares (Screen-Mirror)

/** Estado do Screen-Mirror (pasta, Node, servidor no ar). */
export const devicesStatus = (): Promise<MirrorStatus> => invoke("devices_status");

/** Inicia o Screen-Mirror como processo auxiliar; devolve o estado com a URL do painel. */
export const devicesStart = (): Promise<MirrorStatus> => invoke("devices_start");

/** Encerra o Screen-Mirror. */
export const devicesStop = (): Promise<void> => invoke("devices_stop");

/** Salva a pasta do projeto Screen-Mirror. */
export const devicesSetDir = (dir: Path): Promise<MirrorStatus> => invoke("devices_set_dir", { dir });

/** Liga/desliga o MCP do Screen-Mirror nas conversas de agente. */
export const devicesSetAgents = (on: boolean): Promise<MirrorStatus> => invoke("devices_set_agents", { on });

/** Aparelhos conectados por ADB. */
export const devicesList = (): Promise<DeviceList> => invoke("devices_list");

/** APKs gerados pelo projeto, o mais novo primeiro. */
export const devicesApks = (project: Path): Promise<Apk[]> => invoke("devices_apks", { project });

/** Instala um APK do projeto no aparelho; devolve a saída do adb. */
export const devicesInstall = (project: Path, apk: Path, serial: string): Promise<string> =>
  invoke("devices_install", { project, apk, serial });

export const remoteInfo = (): Promise<RemoteInfo | null> => invoke("remote_info");

/**
 * Liga o servidor remoto na porta; devolve a URL de pareamento e o QR code em SVG.
 * `exposure` omitido = Funnel (HTTPS pelo Tailscale), o padrão.
 */
export const remoteStart = (port: number, exposure?: RemoteExposure): Promise<RemoteInfo> =>
  invoke("remote_start", { port, exposure });

/** Estado do Tailscale neste computador (instalado, conectado, nome no tailnet). */
export const remoteTailscaleStatus = (): Promise<TailscaleStatus> => invoke("remote_tailscale_status");

/** Desliga o servidor remoto. */
export const remoteStop = (): Promise<void> => invoke("remote_stop");

/** Gera um novo token de pareamento (revoga todos os aparelhos). */
export const remoteRegenerateToken = (): Promise<RemoteInfo> => invoke("remote_regenerate_token");

/** Escolhe o IP da URL de pareamento (null = automático). */
export const remoteSetHost = (host: string | null): Promise<RemoteInfo> => invoke("remote_set_host", { host });

/** IPs locais disponíveis (LAN primeiro, depois VPN/virtuais). */
export const remoteHosts = (): Promise<LanAddr[]> => invoke("remote_hosts");

// ---------------------------------------------------------------------- mídia

/** Mídia tocando no sistema (null se nada estiver tocando). */
export const mediaSystemCurrent = (): Promise<SystemMedia | null> => invoke("media_system_current");

/** Capa do álbum da mídia do sistema, como data URL (null quando o app não publica a imagem). */
export const mediaSystemArt = (): Promise<string | null> => invoke("media_system_art");

/** Envia tocar/pausar, próxima ou anterior à mídia do sistema; devolve o estado novo. */
export const mediaSystemCommand = (cmd: MediaCmd): Promise<SystemMedia | null> =>
  invoke("media_system_command", { cmd });

/** Abre o seletor de pasta de músicas e lista os áudios dela (null se cancelar). */
export const mediaPickFolder = (): Promise<MusicFolder | null> => invoke("media_pick_folder");

/** Lista os arquivos de áudio de uma pasta. */
export const mediaScanFolder = (dir: Path): Promise<Path[]> => invoke("media_scan_folder", { dir });

/** Toca um arquivo local; fim de faixa e erros chegam em `onMediaLocal`. */
export const mediaLocalPlay = (path: Path): Promise<void> => invoke("media_local_play", { path });

/** Alterna tocar/pausar da faixa local. */
export const mediaLocalToggle = (): Promise<void> => invoke("media_local_toggle");

/** Para a faixa local. */
export const mediaLocalStop = (): Promise<void> => invoke("media_local_stop");

/** Volume da faixa local, de 0 a 1. */
export const mediaLocalVolume = (volume: number): Promise<void> => invoke("media_local_volume", { volume });

// -------------------------------------------------------------------- sistema

/** CPU (média desde a chamada anterior) e memória do sistema; chame a cada ~2 s. */
export const systemUsage = (): Promise<SystemUsage> => invoke("system_usage");

/** Abre uma URL http(s) no navegador padrão. */
export const systemOpenUrl = (url: string): Promise<void> => invoke("system_open_url", { url });

/** Caixa nativa de confirmação Sim/Não; true = Sim. */
/** `warn` = ícone de aviso (ações perigosas). */
export const systemConfirm = (title: string, message: string, warn = false): Promise<boolean> =>
  invoke("system_confirm", { title, message, warn });

/** Versão do app, caminho do banco e pasta de dados. */
export const appInfo = (): Promise<AppInfo> => invoke("app_info");

// --------------------------------------------- builds e servidores de dev

/** Comandos de build e de servidor detectados no projeto. */
export const runsDetect = (project: Path): Promise<DetectedCommand[]> => invoke("runs_detect", { project });

/** Comandos detectados em todos os projetos da raiz (só os que têm algum). */
export const runsDetectAll = (): Promise<ProjectCommands[]> => invoke("runs_detect_all");

/**
 * Valida e prepara um comando para abrir num terminal da IDE (troca a porta se
 * preciso; recusa servidor com build do projeto rodando). Lembra o servidor do projeto.
 */
export const runsPrepare = (project: Path, commandId: string): Promise<PreparedRun> =>
  invoke("runs_prepare", { project, commandId });

/** Execuções em andamento e as encerradas nesta sessão do app. */
export const runsList = (): Promise<RunInfo[]> => invoke("runs_list");

/** Histórico de builds (do projeto ou de todos), mais recentes primeiro. */
export const runsHistory = (project: Path | null, limit?: number): Promise<BuildRecord[]> =>
  invoke("runs_history", { project, limit });

/** Para uma execução (árvore de processos e terminal). */
export const runsStop = (id: number): Promise<void> => invoke("runs_stop", { id });

/** Último servidor iniciado em cada projeto da raiz. */
export const runsRemembered = (): Promise<RememberedServer[]> => invoke("runs_remembered");

/** Abre a pasta (ou seleciona o arquivo) no gerenciador de arquivos. */
export const runsOpenFolder = (path: Path): Promise<void> => invoke("runs_open_folder", { path });

/** Uso de CPU/memória/GPU por dono rastreado. `gpu` consulta os contadores do Windows (mais lento). */
export const ownersUsage = (gpu: boolean): Promise<import("./types").OwnerUsage[]> =>
  invoke("owners_usage", { gpu });

// ------------------------------------------- agentes e skills em arquivo

/** Agentes dos dois escopos; o global ofuscado por um do projeto vem com `shadowed`. */
export const agentDefsList = (project: Path | null): Promise<import("./types").AgentDef[]> =>
  invoke("agentdefs_list", { project });

/** Grava o `.md` do agente e devolve o caminho. */
export const agentDefSave = (def: import("./types").AgentDef, project: Path | null): Promise<Path> =>
  invoke("agentdefs_save", { def, project });

export const agentDefDelete = (name: string, scope: import("./types").AgentScope, project: Path | null): Promise<void> =>
  invoke("agentdefs_delete", { name, scope, project });

export const agentSkillsList = (project: Path | null): Promise<import("./types").SkillDef[]> =>
  invoke("agentdefs_skills", { project });

export const agentSkillSave = (def: import("./types").SkillDef, project: Path | null): Promise<Path> =>
  invoke("agentdefs_skill_save", { def, project });

export const agentSkillDelete = (name: string, scope: import("./types").AgentScope, project: Path | null): Promise<void> =>
  invoke("agentdefs_skill_delete", { name, scope, project });

/** Grava o trecho do Argus no `AGENTS.md` do projeto (o resto fica intacto). */
export const agentDefsSyncMd = (project: Path, body: string): Promise<Path> =>
  invoke("agentdefs_sync_md", { project, body });

/** Slug sugerido a partir de um título digitado. */
export const agentDefSlug = (text: string): Promise<string> => invoke("agentdefs_slug", { text });

// ---------- preferências e memória (<raiz>/_Argus) ----------

/** Uma nota de `<raiz>/_Argus/Memoria/` (frontmatter `tipo`, `projetos`, `atualizado`). */
export interface MemoryNote {
  name: string;
  path: Path;
  tipo: string | null;
  projetos: string[];
  atualizado: string | null;
  summary: string;
}

export interface MemoryInfo {
  root: Path;
  dir: Path;
  /** A pasta Argus existe: os agentes recebem o aviso de preferências e memória. */
  active: boolean;
  prefs_path: Path;
  index_path: Path;
  prefs: string | null;
  notes: MemoryNote[];
}

export const memoryGet = (): Promise<MemoryInfo> => invoke("memory_get");
/** Grava `Preferencias.md` (cria a pasta Argus, o índice e a pasta de notas se faltarem). */
export const memorySetPrefs = (text: string): Promise<void> => invoke("memory_set_prefs", { text });
/** Abre um arquivo da pasta Argus no Obsidian. */
export const memoryOpenObsidian = (path: Path): Promise<void> => invoke("memory_open_obsidian", { path });
// ------------------------------------------------------- tarefas agendadas

/** Tarefas agendadas, na ordem de criação. */
export const schedulesList = (): Promise<Schedule[]> => invoke("schedules_list");

/** Cria (id 0) ou edita; devolve com a próxima execução calculada. */
export const schedulesSave = (schedule: Schedule): Promise<Schedule> => invoke("schedules_save", { schedule });

export const schedulesSetEnabled = (id: number, enabled: boolean): Promise<Schedule> =>
  invoke("schedules_set_enabled", { id, enabled });

export const schedulesDelete = (id: number): Promise<void> => invoke("schedules_delete", { id });

/** Executa agora, sem mudar a próxima execução. */
export const schedulesRunNow = (id: number): Promise<Schedule> => invoke("schedules_run_now", { id });

/** Estado da opção "Iniciar com o Windows". */
export const autostartGet = (): Promise<AutostartInfo> => invoke("autostart_get");

/** Liga ou desliga "Iniciar com o Windows" (indisponível na cópia de desenvolvimento). */
export const autostartSet = (enabled: boolean): Promise<AutostartInfo> => invoke("autostart_set", { enabled });
