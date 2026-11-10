import { useState } from "react";
import Sidebar, { type PageId } from "./components/Sidebar";
import { MemoryFeedProvider } from "./hooks/useMemoryFeed";
import Dashboard from "./pages/Dashboard";
import Memory from "./pages/Memory";
import Cleaner from "./pages/Cleaner";
import Processes from "./pages/Processes";
import Startup from "./pages/Startup";
import Apps from "./pages/Apps";
import Settings from "./pages/Settings";

const PAGES: Record<PageId, () => JSX.Element> = {
  dashboard: Dashboard,
  memory: Memory,
  cleaner: Cleaner,
  processes: Processes,
  startup: Startup,
  apps: Apps,
  settings: Settings,
};

export default function App() {
  const [page, setPage] = useState<PageId>("dashboard");
  const Page = PAGES[page];
  return (
    <MemoryFeedProvider>
      <div className="flex h-full">
        <Sidebar current={page} onSelect={setPage} />
        <main className="flex-1 overflow-auto p-6">
          <Page />
        </main>
      </div>
    </MemoryFeedProvider>
  );
}
