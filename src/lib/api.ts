// Mọi lời gọi invoke() tới backend Rust được gom tại đây, có type.
import { Channel, invoke } from "@tauri-apps/api/core";
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

// ---- Startup --------------------------------------------------------------

export type StartupSource =
  | "runUser"
  | "runUser32"
  | "runMachine"
  | "runMachine32"
  | "runOnceUser"
  | "runOnceMachine"
  | "runOnceMachine32"
  | "startupFolderUser"
  | "startupFolderCommon"
  | "scheduledTask"
  | "service";

export interface StartupEntry {
  id: string;
  name: string;
  /** Tên value trong Run hoặc tên file trong thư mục Startup — gửi lại khi bật/tắt. */
  keyName: string;
  source: StartupSource;
  location: string;
  command: string;
  path: string | null;
  publisher: Publisher;
  enabled: boolean;
  canToggle: boolean;
  disabledAtMs: number | null;
  note: string | null;
}

// ---- Cleaner --------------------------------------------------------------

export type Risk = "low" | "medium" | "high";

export type RuleStatus =
  | { kind: "ready" }
  | { kind: "empty" }
  | { kind: "needsAdmin" }
  | { kind: "blocked"; reason: string }
  | { kind: "error"; message: string };

export interface RuleScan {
  id: string;
  name: string;
  group: string;
  description: string;
  risk: Risk;
  status: RuleStatus;
  bytes: number;
  files: number;
  skippedYoung: number;
  skippedLinks: number;
  defaultSelected: boolean;
  usesService: boolean;
}

export interface ScanResult {
  scanId: number;
  rules: RuleScan[];
  totalBytes: number;
  durationMs: number;
  cancelled: boolean;
}

export interface CleanerProgress {
  phase: "scan" | "clean";
  doneRules: number;
  totalRules: number;
  current: string;
  files: number;
  bytes: number;
}

export interface RuleClean {
  id: string;
  name: string;
  deletedFiles: number;
  freedBytes: number;
  skippedLocked: number;
  skippedChanged: number;
  errors: number;
  firstError: string | null;
  message: string | null;
}

export interface CleanReport {
  dryRun: boolean;
  rules: RuleClean[];
  freedBytes: number;
  deletedFiles: number;
  skippedLocked: number;
  skippedChanged: number;
  errors: number;
  durationMs: number;
  cancelled: boolean;
}

// ---- Apps -----------------------------------------------------------------

export type UsageSource = "userAssist" | "prefetch" | "pcManager";

export interface InstalledApp {
  /** Key registry (HKLM\..., HKCU\...) — chỉ dùng làm định danh. */
  id: string;
  name: string;
  publisher: string | null;
  version: string | null;
  installDateMs: number | null;
  sizeBytes: number | null;
  sizeMeasured: boolean;
  installLocation: string | null;
  lastUsedMs: number | null;
  lastUsedSource: UsageSource | null;
  canUninstall: boolean;
}

export interface AppsOverview {
  apps: InstalledApp[];
  userassistEntries: number;
  prefetchEntries: number | null;
  trackingSinceMs: number | null;
}

export type LeftoverKind = "installDir" | "dataDir" | "shortcut" | "registry";

export interface Leftover {
  id: number;
  kind: LeftoverKind;
  path: string;
  bytes: number | null;
  /** strong: chắc chắn thuộc app (mặc định chọn). possible: khớp tên gần đúng (mặc định không chọn). */
  confidence: "strong" | "possible";
  reason: string;
  /** Lý do không thể dọn — hiện nhưng không cho chọn. */
  blocked: string | null;
}

/** Service/driver/scheduled task trỏ vào thư mục sắp dọn. */
export interface Dependency {
  kind: "service" | "driver" | "task";
  name: string;
  command: string;
}

export interface LeftoverScan {
  scanId: number;
  appName: string;
  uninstallerExists: boolean;
  items: Leftover[];
  dependencies: Dependency[];
}

/** Trình gỡ cài đặt (và mọi tiến trình con) đã kết thúc. */
export interface UninstallFinished {
  id: string;
  name: string;
  /** App vẫn còn trong danh sách — người dùng huỷ hoặc trình gỡ lỗi. */
  stillInstalled: boolean;
  timedOut: boolean;
}

export interface LeftoverReport {
  recycled: number;
  recycledBytes: number;
  registryRemoved: boolean;
  backup: string | null;
  failed: number;
  items: { id: number; ok: boolean; message: string | null }[];
}

// ---- Disk -----------------------------------------------------------------

export interface DriveInfo {
  root: string;
  kind: "fixed" | "removable" | "network" | "cdRom" | "ramDisk" | "unknown";
  label: string;
  fs: string;
  total: number;
  free: number;
}

export interface FileItem {
  id: number;
  path: string;
  size: number;
  modifiedMs: number | null;
  /** Không được chuyển vào Thùng rác (file hệ thống, thư mục cài app…). */
  blocked: string | null;
  /** Được phép nhưng nên cân nhắc. */
  warning: string | null;
}

export interface LargeScan {
  scanId: number;
  root: string;
  files: FileItem[];
  scannedFiles: number;
  scannedBytes: number;
  durationMs: number;
  cancelled: boolean;
}

export interface DupGroup {
  size: number;
  files: FileItem[];
}

export interface DupScan {
  scanId: number;
  root: string;
  groups: DupGroup[];
  wastedBytes: number;
  scannedFiles: number;
  durationMs: number;
  cancelled: boolean;
}

export interface DiskProgress {
  phase: "walk" | "head" | "full";
  files: number;
  bytes: number;
  done: number;
  total: number;
}

export interface RecycleReport {
  recycled: number;
  recycledBytes: number;
  failed: number;
  items: { id: number; ok: boolean; message: string | null }[];
}

// ---- Settings ---------------------------------------------------------------

export interface AutoClean {
  enabled: boolean;
  intervalDays: number;
  ruleIds: string[];
  lastRunMs: number | null;
}

export interface GeneralSettings {
  autoClean: AutoClean;
  autoRules: { id: string; name: string; description: string }[];
  whitelist: string[];
  autostart: boolean;
}

function channel<T>(cb: (v: T) => void): Channel<T> {
  const ch = new Channel<T>();
  ch.onmessage = cb;
  return ch;
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

  processList: () => invoke<ProcessSnapshot>("process_list"),
  processKill: (p: ProcessRef) => invoke<ProcessActionOutcome>("process_kill", { target: ref(p) }),
  processSuspend: (p: ProcessRef) => invoke<ProcessActionOutcome>("process_suspend", { target: ref(p) }),
  processResume: (p: ProcessRef) => invoke<ProcessActionOutcome>("process_resume", { target: ref(p) }),
  processSetPriority: (p: ProcessRef, priority: PriorityClass) =>
    invoke<ProcessActionOutcome>("process_set_priority", { target: ref(p), priority }),
  processTrim: (p: ProcessRef) => invoke<ProcessActionOutcome>("process_trim", { target: ref(p) }),
  processOpenLocation: (p: ProcessRef) => invoke<ProcessActionOutcome>("process_open_location", { target: ref(p) }),

  startupList: () => invoke<StartupEntry[]>("startup_list"),
  startupSetEnabled: (e: StartupEntry, enabled: boolean) =>
    invoke<void>("startup_set_enabled", { source: e.source, name: e.keyName, enabled }),

  cleanerScan: (onProgress: (p: CleanerProgress) => void) =>
    invoke<ScanResult>("cleaner_scan", { onProgress: channel(onProgress) }),
  /** Chỉ gửi id rule — backend tự xoá đúng các file nó đã quét. */
  cleanerClean: (scanId: number, ruleIds: string[], dryRun: boolean, onProgress: (p: CleanerProgress) => void) =>
    invoke<CleanReport>("cleaner_clean", { scanId, ruleIds, dryRun, onProgress: channel(onProgress) }),
  cleanerCancel: () => invoke<void>("cleaner_cancel"),

  appsList: () => invoke<AppsOverview>("apps_list"),
  /** Mở trình gỡ cài đặt của app; trả về tên app. */
  appsUninstall: (id: string) => invoke<string>("apps_uninstall", { id }),
  /** Backend tự tìm phần còn sót và giữ kết quả. */
  appsLeftoversScan: (id: string) => invoke<LeftoverScan>("apps_leftovers_scan", { id }),
  /** Chỉ gửi scanId + id mục — file vào Thùng rác, key registry được sao lưu .reg trước. */
  appsLeftoversClean: (scanId: number, itemIds: number[]) =>
    invoke<LeftoverReport>("apps_leftovers_clean", { scanId, itemIds }),

  onUninstallFinished: (cb: (e: UninstallFinished) => void): Promise<UnlistenFn> =>
    listen<UninstallFinished>("apps://uninstall-finished", (e) => cb(e.payload)),

  diskDrives: () => invoke<DriveInfo[]>("disk_drives"),
  diskScanLarge: (root: string, minBytes: number, onProgress: (p: DiskProgress) => void) =>
    invoke<LargeScan>("disk_scan_large", { root, minBytes, onProgress: channel(onProgress) }),
  diskScanDuplicates: (root: string, minBytes: number, onProgress: (p: DiskProgress) => void) =>
    invoke<DupScan>("disk_scan_duplicates", { root, minBytes, onProgress: channel(onProgress) }),
  diskCancel: () => invoke<void>("disk_cancel"),
  /** Chỉ gửi scanId + id file — backend chuyển vào Thùng rác (khôi phục được). */
  diskRecycle: (scanId: number, fileIds: number[]) => invoke<RecycleReport>("disk_recycle", { scanId, fileIds }),

  settingsGet: () => invoke<GeneralSettings>("settings_get"),
  settingsSetAutoClean: (enabled: boolean, intervalDays: number, ruleIds: string[]) =>
    invoke<AutoClean>("settings_set_auto_clean", { enabled, intervalDays, ruleIds }),
  settingsSetWhitelist: (paths: string[]) => invoke<string[]>("settings_set_whitelist", { paths }),
  settingsSetAutostart: (enabled: boolean) => invoke<boolean>("settings_set_autostart", { enabled }),

  /** Boost xong từ bất kỳ nguồn nào (UI, tray, auto). */
  onBoosted: (cb: (r: BoostReport) => void): Promise<UnlistenFn> =>
    listen<BoostReport>("memory://boosted", (e) => cb(e.payload)),
};
