import { useLayoutEffect, useRef, useState } from "react";
import { PageActiveContext, PageBusyContext, type ReportBusy } from "./hooks/usePageActive";
import { useRunningJobs } from "./hooks/useJobResume";
import { ThemeContext, useThemePref } from "./lib/theme";
import type { JobKind } from "./lib/api";
import Sidebar, { type PageId } from "./components/Sidebar";
import { MemoryFeedProvider } from "./hooks/useMemoryFeed";
import { ElevationProvider } from "./components/Elevation";
import Dashboard from "./pages/Dashboard";
import Memory from "./pages/Memory";
import Cleaner from "./pages/Cleaner";
import Processes from "./pages/Processes";
import Startup from "./pages/Startup";
import Apps from "./pages/Apps";
import Disk from "./pages/Disk";
import Settings from "./pages/Settings";

const PAGES: Record<PageId, () => JSX.Element> = {
  dashboard: Dashboard,
  memory: Memory,
  cleaner: Cleaner,
  processes: Processes,
  startup: Startup,
  apps: Apps,
  disk: Disk,
  settings: Settings,
};

/** Trang hiển thị từng loại tác vụ dài của backend. */
const JOB_PAGE: Record<JobKind, PageId> = {
  "cleaner.scan": "cleaner",
  "cleaner.clean": "cleaner",
  "winsxs.analyze": "cleaner",
  "winsxs.clean": "cleaner",
  "winold.clean": "cleaner",
  "disk.large": "disk",
  "disk.dups": "disk",
};

export default function App() {
  const theme = useThemePref();
  const [page, setPageState] = useState<PageId>("dashboard");
  // Trang đã mở được giữ sống (chỉ ẩn đi) => tác vụ đang chạy, tiến độ, kết quả quét, lựa chọn
  // vẫn còn khi chuyển tab rồi quay lại. Polling của trang ẩn tự dừng (usePageActive).
  const [opened, setOpened] = useState<PageId[]>(["dashboard"]);
  const scroll = useRef<Partial<Record<PageId, number>>>({});
  const mainRef = useRef<HTMLElement>(null);
  const setPage = (p: PageId) => {
    if (mainRef.current) scroll.current[page] = mainRef.current.scrollTop;
    setOpened((o) => (o.includes(p) ? o : [...o, p]));
    setPageState(p);
  };
  // Mỗi trang nhớ vị trí cuộn của riêng nó.
  useLayoutEffect(() => {
    if (mainRef.current) mainRef.current.scrollTop = scroll.current[page] ?? 0;
  }, [page]);
  const openStart = (p: string) => {
    if (p in PAGES) setPage(p as PageId);
  };
  // Trang nào còn tác vụ đang chạy (mỗi trang có thể có nhiều tác vụ, phân biệt bằng key).
  const [busy, setBusy] = useState<Partial<Record<PageId, string[]>>>({});
  const reporters = useRef<Partial<Record<PageId, ReportBusy>>>({});
  const reporterFor = (id: PageId): ReportBusy =>
    (reporters.current[id] ??= (key, b) =>
      setBusy((prev) => {
        const cur = prev[id] ?? [];
        if (b === cur.includes(key)) return prev;
        return { ...prev, [id]: b ? [...cur, key] : cur.filter((k) => k !== key) };
      }));
  // Cộng thêm tác vụ backend vẫn chạy sau khi cửa sổ bị đóng rồi mở lại (trang có thể chưa mở).
  const jobs = useRunningJobs();
  const busyPages = (Object.keys(PAGES) as PageId[]).filter(
    (id) => (busy[id]?.length ?? 0) > 0 || [...jobs].some((k) => JOB_PAGE[k] === id),
  );
  return (
    <ThemeContext.Provider value={theme}>
    <ElevationProvider page={page} onStartPage={openStart}>
    <MemoryFeedProvider>
      <div className="flex h-full">
        <Sidebar current={page} onSelect={setPage} busy={busyPages} />
        <main ref={mainRef} className="min-w-0 flex-1 overflow-auto p-6 2xl:px-10 2xl:py-8">
          {opened.map((id) => {
            const Page = PAGES[id];
            return (
              <div key={id} hidden={id !== page}>
                <PageActiveContext.Provider value={id === page}>
                  <PageBusyContext.Provider value={reporterFor(id)}>
                    <Page />
                  </PageBusyContext.Provider>
                </PageActiveContext.Provider>
              </div>
            );
          })}
        </main>
      </div>
    </MemoryFeedProvider>
    </ElevationProvider>
    </ThemeContext.Provider>
  );
}
