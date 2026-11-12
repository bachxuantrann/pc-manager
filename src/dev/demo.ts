// CHẾ ĐỘ DEMO — chỉ dùng để chụp ảnh màn hình cho README, chỉ có trong bản dev.
// Mở http://localhost:1420/?demo&page=<trang> : giao diện chạy bằng dữ liệu GIẢ (không đọc máy thật).
// main.tsx chỉ nạp file này khi import.meta.env.DEV, nên bản build phát hành không chứa nó.
// Chụp tất cả: scripts/screenshots.ps1

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const DAY = 86_400_000;
const now = Date.now();
const params = new URLSearchParams(location.search);
const page = params.get("page") ?? "dashboard";

const memory = (loadPct: number) => {
  const total = 16 * GB;
  const used = (total * loadPct) / 100;
  return {
    status: {
      total,
      used,
      available: total - used,
      free: 1.6 * GB,
      standby: 4.1 * GB,
      modified: 180 * MB,
      commitTotal: 13.2 * GB,
      commitLimit: 24 * GB,
      systemCache: 3.4 * GB,
      loadPct,
    },
    privileged: true,
  };
};

const signed = (name: string) => ({ status: "signed", name, catalog: false });
let pid = 1000;
const proc = (name: string, cpuPct: number, mb: number, publisher: string | null, extra: Record<string, unknown> = {}) => ({
  pid: (pid += 132),
  createTime: String(133_000_000_000_000_000 + pid),
  ppid: 4,
  name,
  cpuPct,
  privateBytes: mb * MB,
  workingSet: mb * 1.3 * MB,
  threads: 12 + (pid % 40),
  handles: 300 + (pid % 900),
  sessionId: 1,
  priority: "normal",
  suspended: false,
  path: `C:\\Program Files\\${name.replace(".exe", "")}\\${name}`,
  publisher: publisher ? signed(publisher) : { status: "unsigned" },
  protectedReason: null,
  ...extra,
});

const processes = {
  cpuCount: 12,
  totalCpuPct: 14.2,
  processes: [
    proc("chrome.exe", 4.8, 612, "Google LLC"),
    proc("Code.exe", 3.1, 486, "Microsoft Corporation"),
    proc("Discord.exe", 0.9, 318, "Discord Inc."),
    proc("Spotify.exe", 1.2, 241, "Spotify AB"),
    proc("explorer.exe", 0.4, 96, "Microsoft Windows", { path: "C:\\Windows\\explorer.exe" }),
    proc("msedgewebview2.exe", 0.2, 74, "Microsoft Corporation"),
    proc("pc-manager.exe", 0.1, 5, null, { protectedReason: "chính PC Manager" }),
    proc("svchost.exe", 0.3, 42, "Microsoft Windows", { path: "C:\\Windows\\System32\\svchost.exe" }),
    proc("csrss.exe", 0.1, 3, "Microsoft Windows", { path: null, protectedReason: "tiến trình hệ thống quan trọng" }),
  ],
};

const startup = (name: string, source: string, enabled: boolean, publisher: string | null, command: string) => ({
  id: `${source}:${name}`,
  name,
  keyName: name,
  source,
  location: source.startsWith("run") ? "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" : "Task Scheduler",
  command,
  path: command.replace(/"/g, "").split(" --")[0],
  publisher: publisher ? signed(publisher) : { status: "unsigned" },
  enabled,
  canToggle: source !== "scheduledTask" && source !== "packagedTask",
  disabledAtMs: enabled ? null : now - 12 * DAY,
  note: null,
});

const startupList = [
  startup("Discord", "runUser", false, "Discord Inc.", '"C:\\Users\\Demo\\AppData\\Local\\Discord\\Update.exe" --processStart Discord.exe'),
  startup("Spotify", "runUser", false, "Spotify AB", '"C:\\Users\\Demo\\AppData\\Roaming\\Spotify\\Spotify.exe" --autostart'),
  startup("OneDrive", "runUser", true, "Microsoft Corporation", '"C:\\Program Files\\Microsoft OneDrive\\OneDrive.exe" /background'),
  startup("SecurityHealth", "runMachine", true, "Microsoft Windows", "C:\\Windows\\System32\\SecurityHealthSystray.exe"),
  startup("Steam", "runUser", false, "Valve Corp.", '"C:\\Program Files (x86)\\Steam\\steam.exe" -silent'),
  startup("Microsoft Teams", "packagedTask", true, "Microsoft", "C:\\Program Files\\WindowsApps\\MSTeams_26288.2004.5269.3420_x64__8wekyb3d8bbwe\\msteams_autostarter.exe"),
  startup("PC Manager Autostart", "scheduledTask", true, null, '"C:\\Program Files\\PC Manager\\pc-manager.exe" --tray'),
];

const rule = (id: string, name: string, group: string, risk: string, bytes: number, files: number, extra: Record<string, unknown> = {}) => ({
  id,
  name,
  group,
  description: "",
  risk,
  status: { kind: "ready" },
  bytes,
  files,
  skippedYoung: 0,
  skippedLinks: 0,
  defaultSelected: risk !== "high",
  usesService: false,
  ...extra,
});

const cleanerScan = {
  scanId: 1,
  durationMs: 2140,
  cancelled: false,
  totalBytes: 0,
  rules: [
    rule("user-temp", "File tạm của bạn", "File tạm", "low", 1.42 * GB, 3812, { skippedYoung: 41 }),
    rule("windows-temp", "File tạm của Windows", "File tạm", "low", 384 * MB, 911),
    rule("thumbnail-cache", "Cache ảnh thu nhỏ", "Cache hệ thống", "low", 212 * MB, 14),
    rule("directx-shader-cache", "Cache shader DirectX", "Cache hệ thống", "low", 96 * MB, 402),
    rule("chrome-cache", "Cache Google Chrome", "Trình duyệt", "low", 0, 0, { status: { kind: "blocked", reason: "Chrome đang chạy" } }),
    rule("edge-cache", "Cache Microsoft Edge", "Trình duyệt", "low", 318 * MB, 2210),
    rule("crash-dumps", "Bản ghi lỗi ứng dụng (crash dump)", "Bản ghi lỗi", "low", 640 * MB, 6),
    rule("wer-user", "Báo cáo lỗi Windows (của bạn)", "Bản ghi lỗi", "low", 58 * MB, 37),
    rule("windows-update-cache", "Bản cập nhật Windows đã cài", "Windows Update", "medium", 2.31 * GB, 128, { usesService: true }),
    rule("recycle-bin", "Thùng rác", "Thùng rác", "high", 1.05 * GB, 64, { defaultSelected: false }),
  ],
};
cleanerScan.totalBytes = cleanerScan.rules.reduce((s, r) => s + r.bytes, 0);

const app = (name: string, publisher: string, version: string, sizeGB: number, installedDaysAgo: number, usedDaysAgo: number | null) => ({
  id: `HKLM\\${name}`,
  name,
  publisher,
  version,
  installDateMs: now - installedDaysAgo * DAY,
  sizeBytes: sizeGB * GB,
  sizeMeasured: false,
  installLocation: `C:\\Program Files\\${name}`,
  lastUsedMs: usedDaysAgo == null ? null : now - usedDaysAgo * DAY,
  lastUsedSource: usedDaysAgo == null ? null : "prefetch",
  canUninstall: true,
  storeApp: false,
});

const apps = {
  userassistEntries: 214,
  prefetchEntries: 486,
  trackingSinceMs: now - 40 * DAY,
  apps: [
    app("Steam", "Valve Corporation", "2.10", 4.2, 700, 2),
    app("Adobe Photoshop 2024", "Adobe Inc.", "25.4", 3.6, 420, 410),
    app("Visual Studio Code", "Microsoft Corporation", "1.95", 0.42, 300, 0),
    app("Google Chrome", "Google LLC", "131.0", 0.61, 900, 0),
    app("Zoom Workplace", "Zoom Video Communications", "6.2", 0.38, 380, 260),
    app("VLC media player", "VideoLAN", "3.0.21", 0.16, 800, 95),
    app("7-Zip 24.08 (x64)", "Igor Pavlov", "24.08", 0.006, 600, 14),
    app("Blender 3.6", "Blender Foundation", "3.6", 1.1, 520, 500),
    app("Discord", "Discord Inc.", "1.0", 0.35, 650, 1),
    app("Audacity 3.4", "Audacity Team", "3.4", 0.09, 330, null),
  ],
};

const file = (id: number, path: string, size: number, warning: string | null = null) => ({
  id,
  path,
  size,
  modifiedMs: now - (id + 3) * 9 * DAY,
  blocked: null,
  warning,
});

const dups = {
  scanId: 7,
  root: "C:\\Users\\Demo",
  scannedFiles: 184_220,
  durationMs: 41_800,
  cancelled: false,
  wastedBytes: 0,
  groups: [
    { size: 3.1 * GB, files: [file(0, "C:\\Users\\Demo\\Downloads\\ubuntu-24.04-desktop-amd64.iso", 3.1 * GB), file(1, "C:\\Users\\Demo\\Documents\\ISO\\ubuntu-24.04-desktop-amd64.iso", 3.1 * GB)] },
    { size: 812 * MB, files: [file(2, "C:\\Users\\Demo\\Videos\\Da Lat 2024.mp4", 812 * MB), file(3, "C:\\Users\\Demo\\Downloads\\Da Lat 2024 (1).mp4", 812 * MB), file(4, "C:\\Users\\Demo\\Desktop\\Da Lat 2024 - copy.mp4", 812 * MB)] },
    { size: 146 * MB, files: [file(5, "C:\\Users\\Demo\\Downloads\\setup-tools.zip", 146 * MB), file(6, "C:\\Users\\Demo\\Documents\\backup\\setup-tools.zip", 146 * MB)] },
    { size: 24 * MB, files: [file(7, "C:\\Users\\Demo\\Pictures\\Camera\\IMG_2041.HEIC", 24 * MB), file(8, "C:\\Users\\Demo\\Pictures\\Phone backup\\IMG_2041.HEIC", 24 * MB)] },
  ],
};
dups.wastedBytes = dups.groups.reduce((s, g) => s + g.size * (g.files.length - 1), 0);

const settings = {
  autoClean: { enabled: true, intervalDays: 7, ruleIds: ["user-temp", "thumbnail-cache", "crash-dumps"], lastRunMs: now - 2 * DAY },
  autoRules: [
    { id: "user-temp", name: "File tạm của bạn", description: "%TEMP% — file cũ hơn 24 giờ" },
    { id: "thumbnail-cache", name: "Cache ảnh thu nhỏ", description: "thumbcache_*.db" },
    { id: "directx-shader-cache", name: "Cache shader DirectX", description: "D3DSCache" },
    { id: "crash-dumps", name: "Bản ghi lỗi ứng dụng (crash dump)", description: "*.dmp" },
  ],
  whitelist: ["D:\\Projects", "C:\\Users\\Demo\\AppData\\Local\\Steam"],
  autostart: true,
  autostartBlocked: null,
  elevated: true,
};

const actions = [
  { ts: now - 2 * 3600_000, action: "memory.boost", target: "light", ok: true, detail: "" },
  { ts: now - 5 * 3600_000, action: "cleaner.clean", target: "user-temp,windows-temp", ok: true, detail: "4 723 file" },
  { ts: now - 2 * DAY, action: "cleaner.auto", target: "user-temp,thumbnail-cache,crash-dumps", ok: true, detail: "3 812 file" },
  { ts: now - 3 * DAY, action: "disk.recycle", target: "C:\\Users\\Demo\\Downloads\\old-backup.zip", ok: true, detail: "" },
  { ts: now - 4 * DAY, action: "startup.disable", target: "Spotify", ok: true, detail: "" },
];

const drives = [
  { root: "C:\\", kind: "fixed", label: "Windows", fs: "NTFS", total: 476 * GB, free: 118 * GB },
  { root: "D:\\", kind: "fixed", label: "Data", fs: "NTFS", total: 931 * GB, free: 512 * GB },
];

// Lịch sử RAM dao động nhẹ để biểu đồ trông thật.
let tick = 0;
let diagRuns = 0;
const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
  app_info: () => ({ version: "1.0.0", isElevated: true, pid: 4321, startPage: page }),
  memory_status: () => memory(Math.round(58 + 6 * Math.sin(tick++ / 3))),
  memory_settings: () => ({ smartThresholdPct: 15, smartAuto: true }),
  process_list: () => processes,
  startup_list: () => startupList,
  cleaner_scan: () => cleanerScan,
  apps_list: () => apps,
  disk_drives: () => drives,
  disk_scan_duplicates: () => dups,
  settings_get: () => settings,
  recent_actions: () => actions,
  jobs_snapshot: () => [],
  update_check: () => ({
    current: "1.1.3",
    available: { version: "1.2.0", notes: "### Bảo mật\n- Gỡ app cài riêng cho người dùng luôn chạy với quyền thường.\n\n### Thay đổi\n- Tác vụ dài có tiến độ và nối lại được.", dateMs: now - DAY },
    auto: true,
    lastCheckMs: now - 60_000,
    configured: true,
  }),
  update_set_auto: (args) => ({ current: "1.1.3", available: null, auto: !!args.enabled, lastCheckMs: now - 60_000, configured: true }),
  system_windows_old_info: () => ({ path: "C:\\Windows.old", exists: false, bytes: null }),
  diagnosis_run: () => {
    diagRuns++;
    const fixed = diagRuns > 1;
    const findings = [
      fixed
        ? { id: "junk", category: "storage", severity: "ok", title: "Ít rác", detail: "Không có nhiều file tạm/cache cần dọn.", metric: "120 MB", action: null }
        : { id: "junk", category: "storage", severity: "warn", title: "Có 4.2 GB rác dọn được", detail: "File tạm, cache, bản ghi lỗi… Xem trước rồi chọn mục muốn dọn.", metric: "4.2 GB", action: { kind: "open", page: "cleaner", label: "Xem & dọn" } },
      { id: "disk-system", category: "storage", severity: fixed ? "warn" : "critical", title: "Ổ hệ thống C: sắp đầy", detail: "Windows cần chỗ trống để cập nhật và tạo file tạm; ổ đầy làm máy chậm.", metric: fixed ? "ổ C: còn 11% (27.5 GB)" : "ổ C: còn 9% (23.1 GB)", action: { kind: "open", page: "cleaner", label: "Dọn rác" } },
      { id: "startup", category: "startup", severity: "warn", title: "16 app khởi động cùng Windows", detail: "Nhiều app chạy lúc bật máy làm khởi động lâu và tốn RAM. Tắt bớt app không cần (không xoá app).", metric: "16 app", action: { kind: "open", page: "startup", label: "Xem app khởi động" } },
      { id: "ram", category: "memory", severity: "ok", title: "RAM ổn", detail: "Còn đủ bộ nhớ trống.", metric: "còn trống 41% (13.1 GB)", action: null },
      { id: "apps-unused", category: "apps", severity: "info", title: "3 app không dùng hơn 6 tháng (9.4 GB)", detail: "Lớn nhất: Adobe Photoshop 2024, Blender 3.6. Gỡ app không cần để lấy lại dung lượng.", metric: "3 app · 9.4 GB", action: { kind: "open", page: "apps", label: "Xem ứng dụng" } },
      { id: "restart", category: "windows", severity: "info", title: "18 ngày chưa khởi động lại hẳn", detail: "Restart giúp giải phóng bộ nhớ bị giữ lâu và áp dụng cập nhật. Fast Startup đang bật nên Shut down rồi mở máy không làm mới Windows — chỉ Restart mới làm.", metric: "18 ngày", action: null },
      { id: "ram-xmp", category: "hardware", severity: "info", title: "RAM đang chạy chậm hơn mức hỗ trợ", detail: "Bật XMP (Intel) / EXPO (AMD) trong BIOS để RAM chạy đúng tốc độ. Chỉ làm nếu bạn quen chỉnh BIOS.", metric: "4800 / 6000 MT/s", action: null },
    ];
    const rank = { ok: 0, info: 1, warn: 2, critical: 3 } as Record<string, number>;
    findings.sort((a, b) => rank[b.severity] - rank[a.severity]);
    const cats = ["memory", "storage", "startup", "apps", "windows", "hardware"].map((c) => ({
      category: c,
      severity: findings.filter((f) => f.category === c).reduce((m, f) => (rank[f.severity] > rank[m] ? f.severity : m), "ok"),
    }));
    return { findings, categories: cats, notes: [], deep: false, durationMs: 214 };
  },
  memory_boost: () => ({ gain: { availableGain: 1.2 * GB, standbyCleared: 900 * MB } }),
  system_info: () => ({
    windows: { product: "Windows 11 Pro", displayVersion: "24H2", build: "26100.4061", installDateMs: now - 400 * DAY, lastPowerOnMs: now - (7 * 3600 + 49 * 60) * 1000, uptimeSecs: 12 * 86400 + 8 * 3600, sinceWakeSecs: 3 * 3600 + 20 * 60, fastStartup: true, architecture: "x64" },
    cpu: { name: "AMD Ryzen 7 7800X3D 8-Core Processor", socket: "AM5", cores: 8, threads: 16, baseMhz: 4200, maxMhz: 5050 },
    gpus: [{ name: "NVIDIA GeForce RTX 4070", vendor: "NVIDIA", vramBytes: 12 * GB, driverVersion: "32.0.15.7270", driverDate: "5-20-2025" }],
    board: { systemManufacturer: null, systemProduct: null, manufacturer: "ASUSTeK COMPUTER INC.", product: "TUF GAMING B650-PLUS WIFI", version: "Rev 1.xx" },
    bios: { vendor: "American Megatrends Inc.", version: "3222", releaseDate: "03/14/2025" },
    memory: {
      totalBytes: 32 * GB,
      slotsTotal: 4,
      slots: [
        { locator: "DIMM_A1", bank: "BANK 0", sizeMb: null, kind: null, speedMts: null, configuredMts: null, manufacturer: null, partNumber: null, formFactor: null },
        { locator: "DIMM_A2", bank: "BANK 1", sizeMb: 16384, kind: "DDR5", speedMts: 6000, configuredMts: 6000, manufacturer: "Kingston", partNumber: "KF560C30-16", formFactor: "DIMM" },
        { locator: "DIMM_B1", bank: "BANK 2", sizeMb: null, kind: null, speedMts: null, configuredMts: null, manufacturer: null, partNumber: null, formFactor: null },
        { locator: "DIMM_B2", bank: "BANK 3", sizeMb: 16384, kind: "DDR5", speedMts: 6000, configuredMts: 4800, manufacturer: "Kingston", partNumber: "KF560C30-16", formFactor: "DIMM" },
      ],
    },
    disks: [
      { index: 0, model: "Samsung SSD 990 PRO 2TB", sizeBytes: 2_000_398_934_016, bus: "NVMe", media: "SSD", firmware: "4B2QJXD7" },
      { index: 1, model: "WDC WD20EZBX-00AYRA0", sizeBytes: 2_000_398_934_016, bus: "SATA", media: "HDD", firmware: "01.01A01" },
    ],
  }),
  "plugin:event|listen": () => 1,
  "plugin:event|unlisten": () => null,
};

let cb = 1;
(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
  transformCallback: (fn: unknown) => {
    const id = cb++;
    (window as unknown as Record<string, unknown>)[`_${id}`] = fn;
    return id;
  },
  invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
    const h = handlers[cmd];
    if (!h) return null;
    await new Promise((r) => setTimeout(r, 50));
    return h(args);
  },
};

// Thao tác tự động theo trang để ảnh có nội dung (vd. kết quả quét).
const clickText = (text: string) =>
  [...document.querySelectorAll("button")].find((b) => b.textContent?.trim() === text)?.click();
const script: Record<string, Array<[number, () => void]>> = {
  cleaner: [[600, () => clickText("Quét ngay")]],
  disk: [
    [600, () => clickText("File trùng")],
    [900, () => clickText("Quét")],
    [1500, () => clickText("Chọn bản thừa")],
  ],
};
for (const [delay, fn] of script[page] ?? []) setTimeout(fn, delay);
