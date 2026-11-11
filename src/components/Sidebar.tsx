import { Spinner } from "./Button";
import { ElevateButton, useElevation } from "./Elevation";

export type PageId =
  | "dashboard"
  | "memory"
  | "cleaner"
  | "processes"
  | "startup"
  | "apps"
  | "disk"
  | "settings";

const ITEMS: { id: PageId; label: string; icon: string }[] = [
  { id: "dashboard", label: "Tổng quan", icon: "◉" },
  { id: "memory", label: "Bộ nhớ", icon: "▤" },
  { id: "cleaner", label: "Dọn rác", icon: "✦" },
  { id: "processes", label: "Tiến trình", icon: "≡" },
  { id: "startup", label: "Khởi động", icon: "⏻" },
  { id: "apps", label: "Ứng dụng", icon: "▦" },
  { id: "disk", label: "Ổ đĩa", icon: "◫" },
  { id: "settings", label: "Cài đặt", icon: "⚙" },
];

interface Props {
  current: PageId;
  onSelect: (id: PageId) => void;
  /** Trang còn tác vụ đang chạy (quét, dọn, gỡ…) — hiện dấu xoay cạnh tên. */
  busy?: PageId[];
}

export default function Sidebar({ current, onSelect, busy = [] }: Props) {
  const { elevated } = useElevation();
  return (
    <nav className="flex w-52 shrink-0 flex-col gap-1 border-r border-black/10 bg-white/70 p-3 dark:border-white/10 dark:bg-white/5">
      <div className="mb-4 px-3 pt-2 text-lg font-semibold">PC Manager</div>
      {ITEMS.map((it) => {
        const active = it.id === current;
        return (
          <button
            key={it.id}
            type="button"
            onClick={() => onSelect(it.id)}
            className={`flex items-center gap-3 rounded-md px-3 py-2 text-left text-sm transition-colors ${
              active
                ? "bg-accent text-white"
                : "hover:bg-black/5 dark:hover:bg-white/10"
            }`}
          >
            <span className="w-4 text-center">{it.icon}</span>
            {it.label}
            {busy.includes(it.id) && (
              <span className="ml-auto" title="Đang chạy" aria-label="Đang chạy">
                <Spinner />
              </span>
            )}
          </button>
        );
      })}
      {elevated === false && (
        <div className="mt-auto rounded-md bg-amber-100 p-2 text-xs text-amber-900 dark:bg-amber-500/15 dark:text-amber-200">
          <div className="mb-2">Đang chạy với quyền thường — một số chức năng bị hạn chế.</div>
          <ElevateButton label="Quyền Administrator" />
        </div>
      )}
      {elevated === true && <div className="mt-auto px-3 text-xs opacity-50">🛡 Quyền Administrator</div>}
    </nav>
  );
}
