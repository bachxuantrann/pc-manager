// Lọc / gộp / sắp xếp danh sách tiến trình (thuần, không gọi backend).
import type { ProcessInfo, Publisher } from "./api";

export type SortKey = "name" | "pid" | "cpu" | "memory" | "publisher" | "path";
export interface Sort {
  key: SortKey;
  desc: boolean;
}

export interface Group {
  key: string; // tên exe viết thường
  name: string;
  members: ProcessInfo[];
  cpuPct: number;
  privateBytes: number;
  publisher: Publisher;
  path: string | null;
}

export const procKey = (p: { pid: number; createTime: string }) => `${p.pid}:${p.createTime}`;

export function publisherText(p: Publisher): string {
  switch (p.status) {
    case "signed":
      return p.name || "Đã ký";
    case "unsigned":
      return "Không có chữ ký";
    case "invalid":
      return "Chữ ký không hợp lệ";
    case "pending":
      return "Đang kiểm tra…";
    default:
      return "";
  }
}

export function matches(p: ProcessInfo, q: string): boolean {
  if (!q) return true;
  const s = q.toLowerCase();
  return (
    p.name.toLowerCase().includes(s) ||
    String(p.pid) === s ||
    (p.path?.toLowerCase().includes(s) ?? false) ||
    publisherText(p.publisher).toLowerCase().includes(s)
  );
}

export function groupByName(list: ProcessInfo[]): Group[] {
  const map = new Map<string, Group>();
  for (const p of list) {
    const key = p.name.toLowerCase();
    let g = map.get(key);
    if (!g) {
      g = { key, name: p.name, members: [], cpuPct: 0, privateBytes: 0, publisher: p.publisher, path: p.path };
      map.set(key, g);
    }
    g.members.push(p);
    g.cpuPct += p.cpuPct;
    g.privateBytes += p.privateBytes;
    if (g.publisher.status !== "signed" && p.publisher.status === "signed") g.publisher = p.publisher;
    if (!g.path && p.path) g.path = p.path;
  }
  return [...map.values()];
}

type Row = { name: string; pid?: number; cpuPct: number; privateBytes: number; publisher: Publisher; path: string | null };

function compare(a: Row, b: Row, key: SortKey): number {
  switch (key) {
    case "name":
      return a.name.localeCompare(b.name, "vi", { sensitivity: "base" });
    case "pid":
      return (a.pid ?? 0) - (b.pid ?? 0);
    case "cpu":
      return a.cpuPct - b.cpuPct;
    case "memory":
      return a.privateBytes - b.privateBytes;
    case "publisher":
      return publisherText(a.publisher).localeCompare(publisherText(b.publisher), "vi");
    case "path":
      return (a.path ?? "").localeCompare(b.path ?? "", "vi");
  }
}

export function sortRows<T extends Row>(rows: T[], sort: Sort): T[] {
  const dir = sort.desc ? -1 : 1;
  // Tie-break theo tên để thứ tự ổn định giữa các lần refresh (không nhảy dòng).
  return [...rows].sort((a, b) => compare(a, b, sort.key) * dir || a.name.localeCompare(b.name) || (a.pid ?? 0) - (b.pid ?? 0));
}
