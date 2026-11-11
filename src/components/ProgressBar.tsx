/**
 * Thanh tiến độ dùng chung cho tác vụ dài. `pct` = null => chưa biết tổng: thanh chạy qua lại
 * (vẫn biết là app đang làm việc), kèm `label` mô tả đang làm gì.
 */
export default function ProgressBar({ pct, label, className = "" }: { pct: number | null; label?: string; className?: string }) {
  const known = pct != null && Number.isFinite(pct);
  const value = known ? Math.min(100, Math.max(0, pct)) : 0;
  return (
    <div className={className}>
      <div
        className="relative h-2 overflow-hidden rounded-full bg-black/10 dark:bg-white/10"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={known ? Math.round(value) : undefined}
      >
        {known ? (
          <div className="h-full rounded-full bg-accent transition-[width] duration-300" style={{ width: `${value}%` }} />
        ) : (
          <div className="progress-indeterminate absolute inset-y-0 w-1/3 rounded-full bg-accent" />
        )}
      </div>
      {label && (
        <div className="mt-1 flex justify-between gap-3 text-xs tabular-nums opacity-70">
          <span className="min-w-0 truncate">{label}</span>
          {known && <span className="shrink-0">{value.toFixed(value < 10 ? 1 : 0)}%</span>}
        </div>
      )}
    </div>
  );
}
