// Mọi lời gọi invoke() tới backend Rust được gom tại đây, có type.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Lỗi backend trả về (khớp với `AppError` bên Rust). */
export interface ApiError {
  code: string;
  message: string;
}

export function isApiError(e: unknown): e is ApiError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

export function errorMessage(e: unknown): string {
  if (isApiError(e)) return e.message;
  return String(e);
}

export interface AppInfo {
  version: string;
  isElevated: boolean;
  pid: number;
}

// ---- Memory ---------------------------------------------------------------

/** Mọi giá trị tính bằng byte. free/standby/modified = null khi không có quyền admin. */
export interface MemoryStatus {
  total: number;
  used: number;
  available: number;
  free: number | null;
  standby: number | null;
  modified: number | null;
  commitTotal: number;
  commitLimit: number;
  systemCache: number;
  loadPct: number;
}

export interface MemoryOverview {
  status: MemoryStatus;
  privileged: boolean;
}

export type BoostTrigger = "manual" | "smart" | "tray" | "auto";
/** light: flush modified + purge standby ưu tiên thấp. deep: purge toàn bộ standby. */
export type BoostMode = "light" | "deep";

export interface BoostGain {
  /** RAM khả dụng tăng thêm — lợi ích thật cho ứng dụng. */
  availableGain: number;
  /** Cache standby đã xoá (đã được tính là khả dụng từ trước). */
  standbyCleared: number | null;
  modifiedFlushed: number | null;
  freeGain: number | null;
}

export interface BoostReport {
  trigger: BoostTrigger;
  mode: BoostMode;
  before: MemoryStatus;
  after: MemoryStatus;
  gain: BoostGain;
  flushedModified: boolean;
  purgedStandby: boolean;
  warnings: string[];
  durationMs: number;
}

export type SmartBoostOutcome =
  | { kind: "skipped"; availablePct: number; thresholdPct: number }
  | { kind: "boosted"; report: BoostReport };

export interface MemorySettings {
  smartThresholdPct: number;
  smartAuto: boolean;
}

export interface ActionLogEntry {
  ts: number;
  action: string;
  target: string;
  ok: boolean;
  detail: string;
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  recentActions: (limit = 50) => invoke<ActionLogEntry[]>("recent_actions", { limit }),

  memoryStatus: () => invoke<MemoryOverview>("memory_status"),
  memoryBoost: (mode: BoostMode) => invoke<BoostReport>("memory_boost", { mode }),
  memorySmartBoost: () => invoke<SmartBoostOutcome>("memory_smart_boost"),
  memorySettings: () => invoke<MemorySettings>("memory_settings"),
  setMemorySettings: (settings: MemorySettings) =>
    invoke<MemorySettings>("set_memory_settings", { settings }),

  /** Boost xong từ bất kỳ nguồn nào (UI, tray, auto). */
  onBoosted: (cb: (r: BoostReport) => void): Promise<UnlistenFn> =>
    listen<BoostReport>("memory://boosted", (e) => cb(e.payload)),
};
