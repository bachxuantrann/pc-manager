import type { ReactNode } from "react";

export function Card({ title, children, className = "" }: { title?: string; children: ReactNode; className?: string }) {
  return (
    <div className={`rounded-lg bg-white p-4 shadow-sm dark:bg-white/5 2xl:p-6 ${className}`}>
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

export function Notice({ tone, children }: { tone: "warn" | "ok" | "error"; children: ReactNode }) {
  const cls = {
    warn: "bg-amber-100 text-amber-900 dark:bg-amber-500/15 dark:text-amber-200",
    ok: "bg-green-100 text-green-900 dark:bg-green-500/15 dark:text-green-200",
    error: "bg-red-100 text-red-900 dark:bg-red-500/15 dark:text-red-200",
  }[tone];
  return <div className={`rounded-md px-3 py-2 text-sm ${cls}`}>{children}</div>;
}
