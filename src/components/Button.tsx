import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary" | "danger" | "warning" | "ghost";
type Size = "sm" | "md";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  /** Hiện spinner, khoá nút, giữ nguyên độ rộng để layout không nhảy. */
  loading?: boolean;
  icon?: ReactNode;
}

const VARIANT: Record<Variant, string> = {
  primary: "bg-accent text-white hover:bg-blue-700 active:bg-blue-800 shadow-sm",
  secondary:
    "bg-black/5 text-current hover:bg-black/10 active:bg-black/15 dark:bg-white/10 dark:hover:bg-white/15 dark:active:bg-white/20",
  danger: "bg-red-600 text-white hover:bg-red-700 active:bg-red-800 shadow-sm",
  warning: "bg-amber-600 text-white hover:bg-amber-700 active:bg-amber-800 shadow-sm",
  ghost: "text-current hover:bg-black/5 active:bg-black/10 dark:hover:bg-white/10 dark:active:bg-white/15",
};

const SIZE: Record<Size, string> = {
  sm: "h-8 px-3 text-sm gap-1.5",
  md: "h-9 px-4 text-sm gap-2",
};

export function Spinner({ className = "" }: { className?: string }) {
  return (
    <svg className={`h-4 w-4 animate-spin ${className}`} viewBox="0 0 24 24" fill="none" aria-hidden>
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="3" className="opacity-25" />
      <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}

/**
 * Nút dùng chung: phản hồi rõ khi hover / nhấn (lún nhẹ) / focus bàn phím /
 * đang chạy / bị khoá. Không bao giờ trông như "tab đang chọn".
 */
export default function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  icon,
  disabled,
  className = "",
  children,
  type = "button",
  ...rest
}: Props) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={`inline-flex shrink-0 select-none items-center justify-center rounded-md font-medium transition
        focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent
        enabled:active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-45
        ${VARIANT[variant]} ${SIZE[size]} ${className}`}
      {...rest}
    >
      {loading ? <Spinner /> : icon}
      {children}
    </button>
  );
}
