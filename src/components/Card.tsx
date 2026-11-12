import type { ReactNode } from "react";
import { SURFACE } from "./ui";

export function Card({ title, children, className = "" }: { title?: string; children: ReactNode; className?: string }) {
  return (
    <div
      className={`${SURFACE} p-4 2xl:p-6 ${className}`}
    >
      {title && <h2 className="mb-3 text-sm font-medium opacity-70 2xl:text-base">{title}</h2>}
      {children}
    </div>
  );
}

export function Stat({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div title={hint}>
      <div className="text-xs opacity-60 2xl:text-sm">{label}</div>
      <div className="text-lg font-semibold tabular-nums xl:text-xl 2xl:text-2xl">{value}</div>
    </div>
  );
}

/** Thông báo. Có `onClose` => hiện nút × để người dùng tự tắt (kết quả / lỗi của một thao tác). */
export function Notice({ tone, children, onClose }: { tone: "warn" | "ok" | "error"; children: ReactNode; onClose?: () => void }) {
  const cls = {
    warn: "bg-amber-100 text-amber-900 dark:bg-amber-500/15 dark:text-amber-200",
    ok: "bg-green-100 text-green-900 dark:bg-green-500/15 dark:text-green-200",
    error: "bg-red-100 text-red-900 dark:bg-red-500/15 dark:text-red-200",
  }[tone];
  return (
    <div role={tone === "error" ? "alert" : "status"} className={`flex items-start gap-2 rounded-xl px-3 py-2 text-sm ${cls}`}>
      <div className="min-w-0 flex-1">{children}</div>
      {onClose && (
        <button
          type="button"
          onClick={onClose}
          title="Đóng thông báo"
          aria-label="Đóng thông báo"
          className="-mr-1 shrink-0 rounded px-1.5 leading-5 opacity-60 hover:bg-black/10 hover:opacity-100 dark:hover:bg-white/10"
        >
          ×
        </button>
      )}
    </div>
  );
}
