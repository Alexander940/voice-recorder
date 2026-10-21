// Iconos de trazo (24×24). Decorativos: siempre acompañan a un texto o a un aria-label.
import type { ReactNode } from "react";

interface Props {
  size?: number;
  stroke?: string;
  width?: number;
}

function icon(paths: ReactNode, defaults: { width?: number } = {}) {
  return function Icon({ size = 18, stroke = "currentColor", width }: Props) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        // Por estilo para admitir variables CSS (`var(--accent)`).
        style={stroke === "currentColor" ? undefined : { stroke }}
        strokeWidth={width ?? defaults.width ?? 2}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        {paths}
      </svg>
    );
  };
}

export const ClockIcon = icon(
  <>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7v5l3 2" />
  </>,
);
export const LayersIcon = icon(
  <>
    <path d="M12 3l9 5-9 5-9-5 9-5z" />
    <path d="M3 13l9 5 9-5" />
  </>,
);
export const BookIcon = icon(<path d="M4 19V5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2z" />);
export const SlidersIcon = icon(
  <>
    <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
    <circle cx="16" cy="7" r="2" />
    <circle cx="10" cy="17" r="2" />
  </>,
);
export const KeyIcon = icon(
  <>
    <circle cx="8" cy="15" r="4" />
    <path d="M11 12l9-9M17 6l3 3" />
  </>,
);
export const RefreshIcon = icon(
  <>
    <path d="M20 11a8 8 0 0 0-14.9-3.5M4 4v4h4" />
    <path d="M4 13a8 8 0 0 0 14.9 3.5M20 20v-4h-4" />
  </>,
);
export const MicIcon = icon(
  <>
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5 11a7 7 0 0 0 14 0" />
    <path d="M12 18v3" />
  </>,
);
export const SearchIcon = icon(
  <>
    <circle cx="11" cy="11" r="7" />
    <path d="M20 20l-4-4" />
  </>,
);
export const TrashIcon = icon(<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" />);
export const WarnIcon = icon(
  <>
    <path d="M12 3l10 18H2z" />
    <path d="M12 10v4M12 17h.01" />
  </>,
);
export const AlertIcon = icon(
  <>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 8v5" />
    <path d="M12 16h.01" />
  </>,
);
export const LockIcon = icon(
  <>
    <rect x="4" y="11" width="16" height="10" rx="2" />
    <path d="M8 11V7a4 4 0 0 1 8 0v4" />
  </>,
);
export const CheckIcon = icon(<path d="M5 12l5 5 9-10" />, { width: 2.5 });
export const PlusIcon = icon(<path d="M12 5v14M5 12h14" />, { width: 2.5 });
export const CloseIcon = icon(<path d="M6 6l12 12M18 6L6 18" />, { width: 2.5 });
export const ArrowIcon = icon(<path d="M5 12h14M13 6l6 6-6 6" />);
export const KeyboardIcon = icon(
  <>
    <rect x="3" y="6" width="18" height="12" rx="2" />
    <path d="M7 10h.01M11 10h.01M15 10h.01M7 14h10" />
  </>,
);
