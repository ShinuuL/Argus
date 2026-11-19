/**
 * Ícones da interface: Lucide (lucide-react, licença ISC), importados um a um
 * para o tree-shaking. A API (Icon, IconName, toolIcon) é a mesma de quando os
 * ícones eram SVGs próprios: os arquivos que usam não mudam.
 */
import type { LucideIcon } from "lucide-react";
import {
  ArrowUp,
  AtSign,
  SquareSlash,
  TabletSmartphone,
  Bot,
  Brain,
  Circle,
  ChartColumn,
  Check,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  CircleCheck,
  CircleDashed,
  CircleDot,
  CircleSlash,
  FoldVertical,
  Columns2,
  Eye,
  File,
  FileText,
  Folder,
  FolderOpen,
  FolderSearch,
  GitCompare,
  Globe,
  Hammer,
  LayoutGrid,
  ListChecks,
  ListFilter,
  ListTree,
  LoaderCircle,
  Minus,
  Copy,
  MessageSquare,
  Package,
  PanelLeft,
  PanelRight,
  Palette,
  Paperclip,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Repeat,
  Rows2,
  Search,
  Server,
  Settings,
  Shield,
  ShieldAlert,
  SlidersHorizontal,
  Smartphone,
  Square,
  SquareTerminal,
  TriangleAlert,
  User,
  Trash2,
  Maximize2,
  Wrench,
  X,
  Undo2,
} from "lucide-react";

const ICONS = {
  undo: Undo2,
  sidebarLeft: PanelLeft,
  sidebarRight: PanelRight,
  terminal: SquareTerminal,
  plus: Plus,
  search: Search,
  folder: Folder,
  folderOpen: FolderOpen,
  collapse: FoldVertical,
  filter: ListFilter,
  sliders: SlidersHorizontal,
  chevronRight: ChevronRight,
  chevronDown: ChevronDown,
  chat: MessageSquare,
  grid: LayoutGrid,
  user: User,
  box: Package,
  chart: ChartColumn,
  phone: Smartphone,
  devices: TabletSmartphone,
  gear: Settings,
  clip: Paperclip,
  /** Botões da caixa de composição: comandos de barra e menção. */
  slash: SquareSlash,
  at: AtSign,
  send: ArrowUp,
  stop: Square,
  file: File,
  diff: GitCompare,
  tree: ListTree,
  server: Server,
  wrench: Wrench,
  pencil: Pencil,
  eye: Eye,
  globe: Globe,
  bot: Bot,
  alert: TriangleAlert,
  check: Check,
  close: X,
  splitH: Columns2,
  splitV: Rows2,
  refresh: RefreshCw,
  shield: Shield,
  loop: Repeat,
  play: Play,
  hammer: Hammer,
  palette: Palette,
  // Botões da janela (barra de título própria)
  minus: Minus,
  maximize: Square,
  restore: Copy,
  // Conversa (linhas de ferramenta, raciocínio e tarefas)
  fileText: FileText,
  folderSearch: FolderSearch,
  listChecks: ListChecks,
  brain: Brain,
  circle: Circle,
  // Estados do agente (AgentState)
  spinner: LoaderCircle,
  circleCheck: CircleCheck,
  circleAlert: CircleAlert,
  circleDashed: CircleDashed,
  circleSlash: CircleSlash,
  circleDot: CircleDot,
  shieldAlert: ShieldAlert,
  // Painel de agentes (tarefas em segundo plano)
  trash: Trash2,
  expand: Maximize2,
} as const satisfies Record<string, LucideIcon>;

export type IconName = keyof typeof ICONS;

export interface IconProps {
  name: IconName;
  size?: number;
  /** Preenche em vez de traçar (ex.: botão Parar). */
  filled?: boolean;
  className?: string;
}

export function Icon({ name, size = 16, filled = false, className }: IconProps) {
  const Svg = ICONS[name];
  return (
    <Svg
      className={className ? `ic ${className}` : "ic"}
      size={size}
      strokeWidth={1.75}
      fill={filled ? "currentColor" : "none"}
      aria-hidden="true"
    />
  );
}

/** Ícone de uma ferramenta do agente, pelo nome (Read, Edit, Bash, Grep, WebFetch, Agent...). */
export function toolIcon(tool: string): IconName {
  const t = tool.toLowerCase();
  if (/bash|shell|exec|command|terminal/.test(t)) return "terminal";
  if (/web|fetch|url|http/.test(t)) return "globe";
  if (/todo/.test(t)) return "listChecks";
  if (/edit|write|patch|apply|notebook/.test(t)) return "pencil";
  if (/read|view|cat|open/.test(t)) return "fileText";
  if (/glob|^ls$|list/.test(t)) return "folderSearch";
  if (/grep|search|find/.test(t)) return "search";
  if (/agent|task/.test(t)) return "bot";
  return "wrench";
}
