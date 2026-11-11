import type { ComponentType } from "react";
import { Spinner } from "./Button";
import { IconActivity, IconChip, IconDisk, IconGrid, IconHome, IconMonitor, IconPower, IconPulse, IconSettings, IconShield, IconSparkle } from "./Icons";
import { ElevateButton, useElevation } from "./Elevation";
import appIcon from "../../app-icon.svg";

export type PageId =
  | "dashboard"
  | "diagnosis"
  | "system"
  | "memory"
  | "cleaner"
  | "processes"
  | "startup"
  | "apps"
  | "disk"
  | "settings";

const ITEMS: { id: PageId; label: string; icon: ComponentType<{ className?: string }> }[] = [
  { id: "dashboard", label: "Tổng quan", icon: IconHome },
  { id: "diagnosis", label: "Chẩn đoán", icon: IconPulse },
  { id: "system", label: "Cấu hình máy", icon: IconMonitor },
  { id: "memory", label: "Bộ nhớ", icon: IconChip },
  { id: "cleaner", label: "Dọn rác", icon: IconSparkle },
  { id: "processes", label: "Tiến trình", icon: IconActivity },
  { id: "startup", label: "Khởi động", icon: IconPower },
  { id: "apps", label: "Ứng dụng", icon: IconGrid },
  { id: "disk", label: "Ổ đĩa", icon: IconDisk },
  { id: "settings", label: "Cài đặt", icon: IconSettings },
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
    <nav
      className="m-2 mr-0 flex w-56 shrink-0 flex-col gap-1 rounded-2xl border border-black/5 bg-white/80 p-3
        dark:border-white/[0.06] dark:bg-[#0a0d13]/90"
    >
      <div className="mb-5 flex items-center gap-2 px-3 pt-3">
        {/* Icon thật của app (nguồn của mọi icon: app-icon.svg) — nhỏ nên Vite nhúng thẳng vào JS. */}
        <img src={appIcon} alt="" className="h-7 w-7 shrink-0" draggable={false} />
        <span className="text-[17px] font-semibold tracking-tight">PC Manager</span>
      </div>
      {ITEMS.map((it) => {
        const active = it.id === current;
        const Icon = it.icon;
        return (
          <button
            key={it.id}
            type="button"
            onClick={() => onSelect(it.id)}
            aria-current={active ? "page" : undefined}
            className={`flex items-center gap-3 rounded-xl px-3 py-2.5 text-left text-sm transition-colors ${
              active
                ? "bg-accent/10 font-medium text-accent dark:bg-white/10 dark:text-white"
                : "opacity-75 hover:bg-black/5 hover:opacity-100 dark:hover:bg-white/[0.06]"
            }`}
          >
            <Icon />
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
      {elevated === true && (
        <div className="mt-auto flex items-center gap-2 px-3 pb-1 text-xs opacity-50">
          <IconShield className="h-3.5 w-3.5" /> Quyền Administrator
        </div>
      )}
    </nav>
  );
}
