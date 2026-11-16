/**
 * Ícones da interface: Lucide (lucide-react, licença ISC), importados um a um
 * para o tree-shaking. A API (Icon, IconName, toolIcon) é a mesma de quando os
 * ícones eram SVGs próprios: os arquivos que usam não mudam.
 */
import type { LucideIcon } from "lucide-react";
import {
  ArrowUp,
  TabletSmartphone,
  Bot,
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
  Folder,
  FolderOpen,
  GitCompare,
  Globe,
  Hammer,
  LayoutGrid,
  ListFilter,
  ListTree,
  LoaderCircle,
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
  Wrench,
  X,
} from "lucide-react";

const ICONS = {
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
  // Estados do agente (AgentState)
  spinner: LoaderCircle,
  circleCheck: CircleCheck,
  circleAlert: CircleAlert,
  circleDashed: CircleDashed,
  circleSlash: CircleSlash,
  circleDot: CircleDot,
  shieldAlert: ShieldAlert,
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
  if (/edit|write|patch|apply|notebook/.test(t)) return "pencil";
  if (/read|view|cat|open/.test(t)) return "eye";
  if (/grep|glob|search|find|ls|list/.test(t)) return "search";
  if (/web|fetch|url|http/.test(t)) return "globe";
  if (/agent|task/.test(t)) return "bot";
  return "wrench";
}
