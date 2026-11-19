/** Ícones e botão do player (viewBox 0 0 24 24, preenchidos com a cor do texto). */

export const ICON = {
  prev: "M6 5h2v14H6zM20 5v14L9 12z",
  next: "M16 5h2v14h-2zM4 5v14l11-7z",
  play: "M7 4v16l13-8z",
  pause: "M6 5h4v14H6zM14 5h4v14h-4z",
  folder: "M3 5h7l2 2h9v12H3z",
  list: "M4 6h16v2H4zM4 11h16v2H4zM4 16h16v2H4z",
  system: "M3 4h18v12H3zM9 18h6v2H9z",
  note: "M9 4h10v3h-7v10.5a3.5 3.5 0 11-3-3.46z",
  alert: "M12 3l10 18H2zM11 10v5h2v-5zM11 17v2h2v-2z",
} as const;

export function Icon({ d, size = 13 }: { d: string; size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" fill="currentColor" fillRule="evenodd">
      <path d={d} />
    </svg>
  );
}

export function Btn(props: {
  label: string;
  icon: string;
  onClick: () => void;
  disabled?: boolean;
  pressed?: boolean;
  className?: string;
  size?: number;
}) {
  return (
    <button
      className={props.className ? `mp-btn ${props.className}` : "mp-btn"}
      title={props.label}
      aria-label={props.label}
      aria-pressed={props.pressed}
      disabled={props.disabled}
      onClick={props.onClick}
    >
      <Icon d={props.icon} size={props.size} />
    </button>
  );
}
