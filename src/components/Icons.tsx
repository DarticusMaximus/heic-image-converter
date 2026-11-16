interface IconProps {
  size?: number;
  className?: string;
}

const base = (size: number, className?: string) => ({
  width: size,
  height: size,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  className,
  "aria-hidden": true,
});

export function IconImage({ size = 20, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <rect x="3" y="4" width="18" height="16" rx="3" />
      <circle cx="8.5" cy="9.5" r="1.6" />
      <path d="M4 17.5 9.5 12l4 4.2L16.5 13l3.5 3.8" />
    </svg>
  );
}

export function IconFolder({ size = 20, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <path d="M3.5 7.5A2.5 2.5 0 0 1 6 5h3.4c.7 0 1.3.3 1.8.8L12.5 7h5A2.5 2.5 0 0 1 20 9.5v7A2.5 2.5 0 0 1 17.5 19H6a2.5 2.5 0 0 1-2.5-2.5v-9Z" />
    </svg>
  );
}

export function IconFiles({ size = 20, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <rect x="8" y="3.5" width="12.5" height="12.5" rx="2.5" />
      <path d="M15.5 20.5H6A2.5 2.5 0 0 1 3.5 18V8.5" />
      <path d="M14.25 6.75v6M11.25 9.75h6" />
    </svg>
  );
}

export function IconFolderPlus({ size = 20, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <path d="M3.5 7.5A2.5 2.5 0 0 1 6 5h3.4c.7 0 1.3.3 1.8.8L12.5 7h5A2.5 2.5 0 0 1 20 9.5v7A2.5 2.5 0 0 1 17.5 19H6a2.5 2.5 0 0 1-2.5-2.5v-9Z" />
      <path d="M11.75 10.75v5M9.25 13.25h5" />
    </svg>
  );
}

export function IconShield({ size = 20, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <path d="M12 3.5 19 6v5.5c0 4.2-2.8 7.4-7 9-4.2-1.6-7-4.8-7-9V6l7-2.5Z" />
      <path d="m9 12 2.2 2.2L15.5 10" />
    </svg>
  );
}

export function IconCheck({ size = 18, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <path d="m4.5 12.5 4.5 4.5 10.5-11" />
    </svg>
  );
}

export function IconAlert({ size = 18, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <path d="M12 4.5 21 20H3L12 4.5Z" />
      <path d="M12 10v4.5M12 17.5h.01" />
    </svg>
  );
}

export function IconX({ size = 18, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <path d="m6 6 12 12M18 6 6 18" />
    </svg>
  );
}

export function IconArrowRight({ size = 16, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <path d="M4.5 12h15M13.5 6l6 6-6 6" />
    </svg>
  );
}

export function IconChevronDown({ size = 16, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <path d="m6 9.5 6 6 6-6" />
    </svg>
  );
}

export function IconReveal({ size = 16, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <path d="M4 10V6a2 2 0 0 1 2-2h4M20 14v4a2 2 0 0 1-2 2h-4" />
      <path d="M14 4h6v6M10 20H4v-6" />
    </svg>
  );
}

export function IconDrop({ size = 28, className }: IconProps) {
  return (
    <svg {...base(size, className)}>
      <path d="M12 3v11M7.5 9.5 12 14l4.5-4.5" />
      <path d="M4 15.5V18a3 3 0 0 0 3 3h10a3 3 0 0 0 3-3v-2.5" />
    </svg>
  );
}

export function Spinner({ size = 16, className }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      className={`spinner ${className ?? ""}`}
      aria-hidden
    >
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="2.5" opacity="0.2" />
      <path
        d="M21 12a9 9 0 0 0-9-9"
        stroke="currentColor"
        strokeWidth="2.5"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function LogoMark({ size = 30 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 32 32"
      aria-hidden
      className="logo-mark"
    >
      <defs>
        <linearGradient id="logo-bg" x1="4" y1="3" x2="29" y2="30" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#8ff2bd" />
          <stop offset="1" stopColor="#22ad7b" />
        </linearGradient>
      </defs>
      <rect width="32" height="32" rx="8.5" fill="url(#logo-bg)" />
      <g transform="rotate(-4 15 16)">
        <rect x="7.4" y="7.6" width="13.6" height="16" rx="2" fill="#fff" />
        <circle cx="17.3" cy="11.9" r="1.35" fill="#22ad7b" opacity="0.7" />
        <path d="M9.2 20.8 13 15.4l2.4 3 1.7-2 3.6 4.4Z" fill="#123d2d" />
      </g>
      <circle cx="23.4" cy="23.6" r="5.1" fill="#0c2a20" />
      <path
        d="M21 23.6h4.8M23.8 21.7l1.9 1.9-1.9 1.9"
        stroke="#8ff2bd"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
    </svg>
  );
}
