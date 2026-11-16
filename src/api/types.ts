/**
 * Tipos do contrato entre a interface e o backend (src-tauri).
 *
 * Espelham o JSON gerado pelo serde no Rust:
 * - structs viram objetos com os campos em snake_case;
 * - `Option<T>` vira `T | null`; `PathBuf` vira string; tuplas viram arrays;
 * - enums usam a tag externa do serde: variante sem dados vira string
 *   (`"Running"`), variante com dados vira objeto de uma chave
 *   (`{ "Failed": "mensagem" }`). Use `variantOf()` de `./util` para ler
 *   qualquer enum desses sem depender da lista de variantes.
 */

/** Caminho absoluto do sistema (no Windows, com `\`). */
export type Path = string;
/** Id de tarefa/sessão de agente. */
export type TaskId = number;
/** Id de sessão de terminal (PTY). */
export type PtyId = number;

// ------------------------------------------------------------------ workspace

export type ProjectKind = "Rust" | "Node" | "Python" | "Go" | "Java" | "Git";

export interface Project {
  name: string;
  path: Path;
  kinds: ProjectKind[];
  /** A pasta não existe mais no disco, mas o projeto ainda tem sessões. */
  missing?: boolean;
}

export interface RootInfo {
  root: Path;
  projects: Project[];
  /** Preenchido quando o monitoramento de arquivos não pôde ser ligado. */
  watch_error: string | null;
}

// -------------------------------------------------------------------- monitor

export type FileStatus = "New" | "Modified" | "Deleted" | "Renamed" | "TypeChange" | "Conflicted" | "Untracked";

export interface CommitInfo {
  id: string;
  message: string;
  author: string;
  /** Segundos desde a época Unix. */
  time: number;
}

export interface ProjectStatus {
  /** null se HEAD destacado ou repositório vazio. */
  branch: string | null;
  ahead: number;
  behind: number;
  /** Pares [caminho relativo, status]. */
  changed_files: [string, FileStatus][];
  last_commit: CommitInfo | null;
}

/** Payload do evento `project-changed` e item de `monitorStatuses`. */
export interface ProjectChanged {
  path: Path;
  /** null quando a pasta não é um repositório git. */
  status: ProjectStatus | null;
}

// ------------------------------------------------------------------------- fs

export interface DirEntry {
  name: string;
  path: Path;
  is_dir: boolean;
}

export interface FileStat {
  exists: boolean;
  is_dir: boolean;
  size: number;
  /** Segundos desde a época Unix (0 se desconhecido). */
  modified: number;
}

// ------------------------------------------------------------------------ pty

/** Comando para abrir num terminal. Só `program` é obrigatório. */
export interface CmdSpec {
  program: string;
  args?: string[];
  cwd?: Path | null;
  env?: [string, string][];
}

export interface PtySession {
  id: PtyId;
  program: string;
  cols: number;
  rows: number;
}

/** Payload do evento `pty-output`. */
export interface PtyOutput {
  id: PtyId;
  /** Bytes da saída em base64. */
  data: string;
}

/** Payload do evento `pty-exit`. */
export interface PtyExit {
  id: PtyId;
  code: number | null;
}

/** Payload do evento `pty-missing-command` (uma vez por comando). */
export interface PtyMissingCommand {
  id: PtyId;
  command: string;
  plan: InstallPlan;
}

// -------------------------------------------------------------------- agentes

/** Comando de uma CLI de agente (aceito por `ptySpawn` como `CmdSpec`). */
export interface CommandSpec {
  program: string;
  args: string[];
  cwd: Path | null;
  env: [string, string][];
  /** Texto enviado pela entrada padrão (o terminal ignora). */
  stdin?: string | null;
}

export interface ProviderInfo {
  /** "claude" ou "opencode". */
  id: string;
  display_name: string;
  /** Modelo inicial das conversas novas (null = padrão da CLI). */
  default_model: string | null;
}

/** Um modelo que a CLI do provedor aceita. */
export interface ModelInfo {
  /** Valor passado à CLI. */
  value: string;
  label: string;
  /** Descrição curta informada pela CLI (pode vir vazia). */
  hint: string;
}

/**
 * Onde o agente trabalha: "Isolated" = numa cópia do projeto (git worktree),
 * com Aplicar/Descartar depois; "Direct" = na própria pasta do projeto.
 */
export type WorkMode = "Isolated" | "Direct";

/** Modo de trabalho padrão das conversas novas. */
export interface WorkModeInfo {
  global: WorkMode;
  /** Preferência do projeto (null = segue a geral). */
  project: WorkMode | null;
  /** O que vale para uma conversa nova no projeto. */
  effective: WorkMode;
  /** false = a pasta não é repositório git: só aceita "Direct". */
  is_git: boolean;
}

/** O que existe na pasta do projeto e não existiria numa cópia isolada. */
export interface IsolationReport {
  is_git: boolean;
  /** Não versionados (pasta termina com "/"). */
  untracked: string[];
  /** Ignorados pelo git que parecem relevantes (.env, dados locais). */
  ignored: string[];
  /** Versionados com alterações não commitadas. */
  modified: string[];
  /** Caminhos citados no pedido que não existiriam na cópia. */
  mentioned: string[];
  untracked_total: number;
  ignored_total: number;
  modified_total: number;
}

export type LoginStatus = "NotInstalled" | "LoggedOut" | "LoggedIn" | "Unknown";

export interface TokenUsage {
  input_tokens: number;
  output_tokens: number;
  cache_read: number;
  cache_write: number;
  model: string | null;
  /** Custo informado pela própria CLI (pode vir 0 ou null com assinatura). */
  cost_usd: number | null;
}

/**
 * Evento de um agente. O backend repassa o enum do core sem conhecê-lo:
 * variantes novas podem chegar antes de entrarem neste tipo, então trate o
 * caso padrão (use `variantOf`).
 */
export type AgentEvent =
  | { Session: string }
  | { User: string }
  | { Text: string }
  | { ToolUse: { name: string; input: unknown } }
  | { Result: { ok: boolean; summary: string; cost: number | null } }
  | { Usage: TokenUsage }
  | { Raw: string }
  /** Um subagente nasceu; `id` é o id do tool_use que o criou, `parent` o subagente pai (null = agente principal). */
  | { SubagentStart: { id: string; parent: string | null; description: string; subagent_type: string; prompt: string } }
  /** O subagente `id` terminou; `result` é o texto devolvido a quem o chamou. */
  | { SubagentEnd: { id: string; ok: boolean; result: string } }
  /** Evento interno do subagente `subagent`; os do agente principal vêm sem este invólucro. */
  | { Scoped: { subagent: string; event: AgentEvent } }
  /** O usuário trocou o agente da conversa (ids de provedor). */
  | { ProviderSwitch: { from: string; to: string } }
  /** A CLI negou uma ferramenta por falta de permissão (`command` vazio se não for de shell). */
  | { Denied: { tool: string; command: string } };

/** Conjunto padrão de comandos liberados ao agente e o que cada nível de git acrescenta. */
export interface DefaultPermissions {
  git_read: string[];
  inspect: string[];
  inspect_powershell: string[];
  git_local: string[];
  git_remote: string[];
  never: string[];
}

/** Permissões de comando: o padrão e as preferências dos níveis de git (null = segue o padrão). */
export interface PermissionsInfo {
  defaults: DefaultPermissions;
  global_local: boolean | null;
  global_remote: boolean | null;
  project_local: boolean | null;
  project_remote: boolean | null;
}

/** Resumo de um subagente de uma tarefa (visão somente leitura). */
export interface SubagentInfo {
  id: string;
  parent: string | null;
  description: string;
  subagent_type: string;
  prompt: string;
  /** Já terminou. */
  done: boolean;
  /** null enquanto roda. */
  ok: boolean | null;
  result: string | null;
}

export type TaskState =
  | "Queued"
  | "Running"
  | "Completed"
  | "Cancelled"
  | "Applied"
  | "Discarded"
  | { Failed: string };

export interface Task {
  id: TaskId;
  project: Path;
  provider: string;
  prompt: string;
  state: TaskState;
  /** Onde o agente roda (worktree, ou o próprio projeto). */
  workdir: Path | null;
  /** Sessão da CLI, usada para responder. */
  session_id: string | null;
  /** Turnos já iniciados (1 = só o pedido inicial). */
  turns: number;
  /** Modelo dos próximos turnos (null = padrão da CLI). */
  model: string | null;
  mode: WorkMode;
  /** Houve troca de agente: o próximo turno leva o resumo da conversa. */
  handoff: boolean;
}

// ------------------------------------------------------------------ auto-work

export type AutoWorkStatus = "Running" | "Paused" | "Completed" | { Stopped: string };

export interface AutoWorkEvent {
  task: TaskId;
  project: Path;
  status: AutoWorkStatus;
  iteration: number;
  max_iterations: number;
  current_item: string | null;
  done: number;
  blocked: number;
  total: number;
  tokens: number;
  /** Custo equivalente em API (USD). */
  cost_usd: number;
}

export interface AutoWorkSettings {
  enabled: boolean;
  max_iterations: number;
  /** Teto de tokens da execução; null = sem teto. */
  max_tokens: number | null;
  /** Teto de custo equivalente em API (USD); null = sem teto. */
  max_cost_usd: number | null;
  /** Roda build/testes na varredura. */
  run_checks: boolean;
  check_timeout_secs: number;
}

export interface ProjectAutoWork {
  project: Path;
  settings: AutoWorkSettings;
}

// -------------------------------------------------------- evento `orch`

/**
 * Payload do evento `orch`: tudo o que o orquestrador publica.
 * `Other` carrega variantes que o backend ainda não espelha.
 */
export type OrchEvent =
  | { StateChanged: { task: TaskId; state: TaskState } }
  | { Agent: { task: TaskId; event: AgentEvent } }
  | { Stderr: { task: TaskId; line: string } }
  | { AutoWork: AutoWorkEvent }
  | { Removed: { task: TaskId } }
  | { Other: unknown };

// ----------------------------------------------------------------- servidores

/** Dono de um servidor: sessão de agente ou terminal. */
export type Owner = { kind: "Session"; id: TaskId } | { kind: "Terminal"; id: PtyId };

export interface ServerInfo {
  pid: number;
  name: string;
  cmdline: string;
  port: number;
  url: string;
  owner: Owner;
  /** Segundos desde a época Unix. */
  started_at: number;
  cwd: Path | null;
}

// --------------------------------------------------------------- dependências

export type DepKind = "MissingToolchain" | "ProjectDepsMissing" | "ProjectDepsOutdated";

export interface InstallPlan {
  command: CommandSpec;
  needs_admin: boolean;
  /** Linha de comando pronta para exibir. */
  display: string;
}

export interface DepIssue {
  kind: DepKind;
  /** Nome curto da ferramenta/ecossistema (ex.: "node", "pnpm"). */
  tool: string;
  description: string;
  fix: InstallPlan;
}

export interface MissingCommand {
  command: string;
  /** null quando a ferramenta não está na tabela de instalação conhecida. */
  plan: InstallPlan | null;
}

// ------------------------------------------------------------------- métricas

/** Linha agregada de uso. O custo é sempre o equivalente em API (USD). */
export interface CostLine {
  key: string;
  input_tokens: number;
  output_tokens: number;
  cache_read: number;
  cache_write: number;
  turns: number;
  api_cost_usd: number;
}

export interface DayLine {
  /** AAAA-MM-DD. */
  day: string;
  line: CostLine;
}

export interface MetricsReport {
  total: CostLine;
  /** `key` = caminho do projeto. */
  projects: CostLine[];
  providers: CostLine[];
  models: CostLine[];
  /** Todos os dias do período, inclusive os sem uso. */
  days: DayLine[];
  /** false = histórico desligado (sem banco). */
  has_db: boolean;
}

export interface TodayUsage {
  tokens: number;
  /** Custo equivalente em API (USD). */
  cost: number;
}

/** Intervalo em segundos Unix; null = aberto. */
export interface TimeRange {
  from: number | null;
  to: number | null;
}

export interface UsageRow {
  task_id: number;
  project: Path;
  provider: string;
  usage: TokenUsage;
  created_at: number;
  /** Turno que gerou a linha (um turno com dois modelos tem duas linhas); null nas linhas antigas. */
  turn: number | null;
}

export interface ProjectUsage {
  project: Path;
  provider: string;
  model: string | null;
  input_tokens: number;
  output_tokens: number;
  cache_read: number;
  cache_write: number;
  reported_cost_usd: number | null;
  turns: number;
}

/** Preço em USD por milhão de tokens. */
export interface ModelPrice {
  input: number;
  output: number;
  cache_read: number;
  cache_write: number;
}

export interface Pricing {
  models: Record<string, ModelPrice>;
  fallback: ModelPrice;
  /** Usa o custo informado pela CLI quando ele for maior que zero. */
  prefer_reported: boolean;
}

// --------------------------------------------------------------------- remoto

export type AddrKind = "Lan" | "Tailscale" | "Virtual";

export interface LanAddr {
  ip: string;
  iface: string;
  kind: AddrKind;
}

export interface RemoteInfo {
  /** URL de pareamento (contém o token do Argus). */
  url: string;
  /** QR code da URL, em SVG. */
  qr_svg: string;
  port: number;
  /** IP usado na URL. */
  host: string;
  hosts: LanAddr[];
}

// ---------------------------------------------------------------------- mídia

export interface SystemMedia {
  title: string;
  artist: string;
  /** Identificador do app que está tocando. */
  app: string;
  playing: boolean;
}

export type MediaCmd = "PlayPause" | "Next" | "Previous";

export interface MusicFolder {
  dir: Path;
  files: Path[];
}

/** Payload do evento `media-local`. */
export type LocalMediaEvent = { type: "Error"; message: string } | { type: "Finished" };

// -------------------------------------------------------------------- sistema

export interface SystemUsage {
  /** CPU global em % (0 a 100). */
  cpu: number;
  mem_used_mb: number;
  mem_total_mb: number;
}

export interface AppInfo {
  version: string;
  /** Caminho do banco em uso (null = histórico desligado). */
  db_path: Path | null;
  has_db: boolean;
  data_dir: Path | null;
}
