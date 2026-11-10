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

export function pct(part: number, total: number): number {
  return total > 0 ? (part * 100) / total : 0;
}
