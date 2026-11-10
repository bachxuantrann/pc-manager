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

// ---- Process --------------------------------------------------------------

export type PriorityClass = "idle" | "belowNormal" | "normal" | "aboveNormal" | "high";

export type Publisher =
  | { status: "unknown" }
  | { status: "pending" }
  | { status: "signed"; name: string; catalog: boolean }
  | { status: "unsigned" }
  | { status: "invalid"; code: string };

/** Danh tính tiến trình. createTime là chuỗi (FILETIME vượt 2^53). */
export interface ProcessRef {
  pid: number;
  createTime: string;
}

export interface ProcessInfo extends ProcessRef {
  ppid: number;
  name: string;
  cpuPct: number;
  privateBytes: number;
  workingSet: number;
  threads: number;
  handles: number;
  sessionId: number;
  priority: PriorityClass | null;
  suspended: boolean;
  path: string | null;
  publisher: Publisher;
  protectedReason: string | null;
}

export interface ProcessSnapshot {
  processes: ProcessInfo[];
  cpuCount: number;
  totalCpuPct: number;
}

export interface ProcessActionOutcome {
  trimmedBytes: number | null;
}

const ref = (p: ProcessRef): ProcessRef => ({ pid: p.pid, createTime: p.createTime });

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

  processList: () => invoke<ProcessSnapshot>("process_list"),
  processKill: (p: ProcessRef) => invoke<ProcessActionOutcome>("process_kill", { target: ref(p) }),
  processSuspend: (p: ProcessRef) => invoke<ProcessActionOutcome>("process_suspend", { target: ref(p) }),
  processResume: (p: ProcessRef) => invoke<ProcessActionOutcome>("process_resume", { target: ref(p) }),
  processSetPriority: (p: ProcessRef, priority: PriorityClass) =>
    invoke<ProcessActionOutcome>("process_set_priority", { target: ref(p), priority }),
  processTrim: (p: ProcessRef) => invoke<ProcessActionOutcome>("process_trim", { target: ref(p) }),
  processOpenLocation: (p: ProcessRef) => invoke<ProcessActionOutcome>("process_open_location", { target: ref(p) }),

  /** Boost xong từ bất kỳ nguồn nào (UI, tray, auto). */
  onBoosted: (cb: (r: BoostReport) => void): Promise<UnlistenFn> =>
    listen<BoostReport>("memory://boosted", (e) => cb(e.payload)),
};
