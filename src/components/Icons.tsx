import type { ReactNode } from "react";

/**
 * Bộ biểu tượng nét mảnh tự vẽ (SVG 24×24, màu theo `currentColor`) — không thêm thư viện icon
 * để file JS giữ nhỏ.
 */
function Icon({ children, className = "h-[18px] w-[18px]" }: { children: ReactNode; className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      className={`shrink-0 ${className}`}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      {children}
    </svg>
  );
}

export const IconHome = (p: { className?: string }) => (
  <Icon {...p}>
    <path d="M3 10.5 12 3l9 7.5" />
    <path d="M5 9.5V20h5v-6h4v6h5V9.5" />
  </Icon>
);

export const IconChip = (p: { className?: string }) => (
  <Icon {...p}>
    <rect x="6" y="6" width="12" height="12" rx="2" />
    <rect x="9.5" y="9.5" width="5" height="5" rx="1" />
    <path d="M9 2.5v3M15 2.5v3M9 18.5v3M15 18.5v3M2.5 9h3M2.5 15h3M18.5 9h3M18.5 15h3" />
  </Icon>
);

export const IconSparkle = (p: { className?: string }) => (
  <Icon {...p}>
    <path d="M12 3.5 13.8 9 19.5 10.8 13.8 12.6 12 18.5 10.2 12.6 4.5 10.8 10.2 9Z" />
    <path d="M19 3v3M17.5 4.5h3M5 17v3M3.5 18.5h3" />
  </Icon>
);

export const IconActivity = (p: { className?: string }) => (
  <Icon {...p}>
    <path d="M3 12h4l2.5-6 5 12 2.5-6h4" />
  </Icon>
);

export const IconPower = (p: { className?: string }) => (
  <Icon {...p}>
    <path d="M12 3v8" />
    <path d="M7 6.3a8 8 0 1 0 10 0" />
  </Icon>
);

export const IconGrid = (p: { className?: string }) => (
  <Icon {...p}>
    <rect x="3.5" y="3.5" width="7" height="7" rx="1.5" />
    <rect x="13.5" y="3.5" width="7" height="7" rx="1.5" />
    <rect x="3.5" y="13.5" width="7" height="7" rx="1.5" />
    <rect x="13.5" y="13.5" width="7" height="7" rx="1.5" />
  </Icon>
);

export const IconDisk = (p: { className?: string }) => (
  <Icon {...p}>
    <ellipse cx="12" cy="6" rx="8" ry="3" />
    <path d="M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6" />
    <path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3" />
  </Icon>
);

export const IconSettings = (p: { className?: string }) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="3" />
    <path d="M12 2.5v2.2M12 19.3v2.2M4.6 4.6l1.6 1.6M17.8 17.8l1.6 1.6M2.5 12h2.2M19.3 12h2.2M4.6 19.4l1.6-1.6M17.8 6.2l1.6-1.6" />
  </Icon>
);

export const IconShield = (p: { className?: string }) => (
  <Icon {...p}>
    <path d="M12 3 4.5 6v5.5c0 4.5 3.2 8.2 7.5 9.5 4.3-1.3 7.5-5 7.5-9.5V6Z" />
  </Icon>
);

export const IconMonitor = (p: { className?: string }) => (
  <Icon {...p}>
    <rect x="3" y="4" width="18" height="12" rx="2" />
    <path d="M8 20h8M12 16v4" />
  </Icon>
);

export const IconPulse = (p: { className?: string }) => (
  <Icon {...p}>
    <path d="M20.8 11.2A5.5 5.5 0 0 0 12 5.2a5.5 5.5 0 0 0-8.8 6c1.6 4 8.8 8.8 8.8 8.8s1.4-.9 3.1-2.3" />
    <path d="M3.5 12h4l1.5-3 3 6 1.5-3h7" />
  </Icon>
);

// ---- Biểu tượng nhỏ trong dòng chữ (thay emoji/ký tự phụ thuộc font máy) ----
// Mặc định 1em để khớp cỡ chữ xung quanh; căn giữa theo dòng bằng `align-[-0.125em]`.
const INLINE = "inline-block h-[1em] w-[1em] align-[-0.125em]";

export const IconLock = ({ className = INLINE }: { className?: string }) => (
  <Icon className={className}>
    <rect x="5" y="11" width="14" height="10" rx="2" />
    <path d="M8 11V8a4 4 0 0 1 8 0v3" />
  </Icon>
);

export const IconCheck = ({ className = INLINE }: { className?: string }) => (
  <Icon className={className}>
    <path d="m5 12.5 4.5 4.5L19 7.5" />
  </Icon>
);

export const IconX = ({ className = INLINE }: { className?: string }) => (
  <Icon className={className}>
    <path d="M6 6l12 12M18 6 6 18" />
  </Icon>
);

export const IconRefresh = ({ className = INLINE }: { className?: string }) => (
  <Icon className={className}>
    <path d="M20 12a8 8 0 1 1-2.34-5.66" />
    <path d="M20 4v5h-5" />
  </Icon>
);

/** Mũi tên gập: `open` = trỏ xuống (đang mở), không thì trỏ sang phải. */
export const IconChevron = ({ open = false, className = INLINE }: { open?: boolean; className?: string }) => (
  <Icon className={`${className} transition-transform ${open ? "rotate-90" : ""}`}>
    <path d="m9 6 6 6-6 6" />
  </Icon>
);

export const IconWarning = ({ className = INLINE }: { className?: string }) => (
  <Icon className={className}>
    <path d="M12 3.5 2.5 20h19L12 3.5Z" />
    <path d="M12 10v4.5M12 17.2v.3" />
  </Icon>
);

export const IconInfo = ({ className = INLINE }: { className?: string }) => (
  <Icon className={className}>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5.5M12 7.8v.3" />
  </Icon>
);
