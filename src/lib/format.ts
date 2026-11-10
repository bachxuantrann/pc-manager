const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** 1536 → "1.5 KB". Dùng hệ 1024 giống Task Manager. */
export function formatBytes(n: number | null | undefined, digits = 1): string {
  if (n == null || !Number.isFinite(n)) return "—";
  let v = Math.max(0, n);
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i === 0 ? 0 : digits)} ${UNITS[i]}`;
}

const DAY = 86_400_000;

/** Số ngày từ `ms` tới nay (làm tròn xuống). */
export function daysAgo(ms: number, now = Date.now()): number {
  return Math.max(0, Math.floor((now - ms) / DAY));
}

/** 0 → "hôm nay", 1 → "hôm qua", 45 → "1 tháng trước", 800 → "2 năm trước". */
export function relativeDays(days: number): string {
  if (days <= 0) return "hôm nay";
  if (days === 1) return "hôm qua";
  if (days < 30) return `${days} ngày trước`;
  if (days < 365) return `${Math.floor(days / 30)} tháng trước`;
  return `${Math.floor(days / 365)} năm trước`;
}

export function formatDate(ms: number): string {
  return new Date(ms).toLocaleDateString("vi-VN");
}

export function pct(part: number, total: number): number {
  return total > 0 ? (part * 100) / total : 0;
}
