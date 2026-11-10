import { useCallback, useEffect, useMemo, useState } from "react";
import Button, { Spinner } from "../components/Button";
import { Card, Notice } from "../components/Card";
import ConfirmDialog, { type ConfirmRequest } from "../components/ConfirmDialog";
import LeftoverDialog, { type LeftoverTarget } from "../components/LeftoverDialog";
import PageShell from "../components/PageShell";
import { api, errorMessage, type AppsOverview, type InstalledApp, type UsageSource } from "../lib/api";
import { daysAgo, formatBytes, formatDate, relativeDays } from "../lib/format";

type SortKey = "size" | "name" | "lastUsed" | "installed";

const UNUSED_FILTERS: { days: number; label: string }[] = [
  { days: 0, label: "Tất cả" },
  { days: 30, label: "Không dùng > 30 ngày" },
  { days: 90, label: "> 90 ngày" },
  { days: 180, label: "> 6 tháng" },
  { days: 365, label: "> 1 năm" },
];

const SOURCE_LABEL: Record<UsageSource, string> = {
  userAssist: "Windows ghi nhận khi mở từ Start/Explorer (UserAssist)",
  prefetch: "Windows Prefetch",
  pcManager: "PC Manager thấy ứng dụng đang chạy",
};

function LastUsed({ a }: { a: InstalledApp }) {
  if (a.lastUsedMs == null) {
    return (
      <span className="opacity-50" title="Không nguồn nào ghi nhận lần chạy. Có thể app chạy ngầm, qua dòng lệnh, hoặc thật sự chưa được dùng.">
        Chưa rõ
      </span>
    );
  }
  const d = daysAgo(a.lastUsedMs);
  const tone = d > 365 ? "text-red-600 dark:text-red-400" : d > 90 ? "text-amber-700 dark:text-amber-300" : "";
  return (
    <span className={tone} title={`${formatDate(a.lastUsedMs)} — theo ${a.lastUsedSource ? SOURCE_LABEL[a.lastUsedSource] : "?"}`}>
      {relativeDays(d)}
    </span>
  );
}

export default function Apps() {
  const [data, setData] = useState<AppsOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [msg, setMsg] = useState<{ tone: "ok" | "error"; text: string } | null>(null);
  const [query, setQuery] = useState("");
  const [unusedDays, setUnusedDays] = useState(0);
  const [includeUnknown, setIncludeUnknown] = useState(false);
  const [sort, setSort] = useState<SortKey>("size");
  const [confirm, setConfirm] = useState<ConfirmRequest | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [leftoverApp, setLeftoverApp] = useState<LeftoverTarget | null>(null);
  /** App đang chạy trình gỡ — chờ backend báo xong. */
  const [uninstalling, setUninstalling] = useState<Set<string>>(new Set());

  const load = useCallback(() => {
    setLoading(true);
    api
      .appsList()
      .then((d) => {
        setData(d);
        setError(null);
      })
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, []);
  useEffect(load, [load]);

  // Trình gỡ chạy xong: tải lại danh sách; gỡ thành công thì mở luôn "Dọn phần còn sót".
  useEffect(() => {
    const un = api.onUninstallFinished((e) => {
      setUninstalling((prev) => {
        const next = new Set(prev);
        next.delete(e.id);
        return next;
      });
      load();
      if (e.timedOut) {
        setMsg({ tone: "error", text: `${e.name}: trình gỡ chạy quá lâu, PC Manager thôi theo dõi. Gỡ xong hãy bấm "Dọn sót".` });
      } else if (e.stillInstalled) {
        setMsg({
          tone: "error",
          text: `Trình gỡ của ${e.name} đã đóng nhưng ứng dụng vẫn còn (đã huỷ hoặc gỡ lỗi). Nếu trình gỡ báo lỗi, dùng "Dọn sót".`,
        });
      } else {
        setMsg({ tone: "ok", text: `Đã gỡ ${e.name}. Đang tìm phần còn sót…` });
        setLeftoverApp({ id: e.id, name: e.name, afterUninstall: true });
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, [load]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    const now = Date.now();
    const list = (data?.apps ?? []).filter((a) => {
      if (q && !`${a.name} ${a.publisher ?? ""}`.toLowerCase().includes(q)) return false;
      if (unusedDays === 0) return true;
      if (a.lastUsedMs == null) {
        // Không rõ lần dùng: chỉ tính khi người dùng muốn VÀ đã cài đủ lâu.
        return includeUnknown && (a.installDateMs == null || daysAgo(a.installDateMs, now) > unusedDays);
      }
      return daysAgo(a.lastUsedMs, now) > unusedDays;
    });
    const by: Record<SortKey, (a: InstalledApp, b: InstalledApp) => number> = {
      size: (a, b) => (b.sizeBytes ?? -1) - (a.sizeBytes ?? -1),
      name: (a, b) => a.name.localeCompare(b.name, "vi", { sensitivity: "base" }),
      // Lâu chưa dùng lên đầu; "chưa rõ" xuống cuối.
      lastUsed: (a, b) => (a.lastUsedMs ?? Infinity) - (b.lastUsedMs ?? Infinity),
      installed: (a, b) => (a.installDateMs ?? Infinity) - (b.installDateMs ?? Infinity),
    };
    return [...list].sort((a, b) => by[sort](a, b) || a.name.localeCompare(b.name));
  }, [data, query, unusedDays, includeUnknown, sort]);

  const totalSize = rows.reduce((s, a) => s + (a.sizeBytes ?? 0), 0);

  const uninstall = (a: InstalledApp) =>
    setConfirm({
      title: `Gỡ cài đặt ${a.name}?`,
      body: (
        <div className="space-y-2">
          <p>PC Manager sẽ mở trình gỡ cài đặt của chính ứng dụng này. Làm theo hướng dẫn trên cửa sổ đó để hoàn tất.</p>
          <p>Gỡ xong, PC Manager tự tìm phần còn sót (cache, AppData, shortcut…) để bạn xem và dọn.</p>
          {a.lastUsedMs != null && (
            <p className="text-xs opacity-70">
              Lần dùng cuối: {formatDate(a.lastUsedMs)} ({relativeDays(daysAgo(a.lastUsedMs))}).
            </p>
          )}
        </div>
      ),
      confirmLabel: "Mở trình gỡ cài đặt",
      danger: true,
      onConfirm: async () => {
        setBusyId(a.id);
        setMsg(null);
        try {
          const name = await api.appsUninstall(a.id);
          setUninstalling((prev) => new Set(prev).add(a.id));
          setMsg({
            tone: "ok",
            text: `Đã mở trình gỡ cài đặt của ${name}. Làm theo cửa sổ đó — gỡ xong, PC Manager sẽ tự tìm phần còn sót.`,
          });
        } catch (e) {
          setMsg({ tone: "error", text: `${a.name}: ${errorMessage(e)}` });
        } finally {
          setBusyId(null);
        }
      },
    });

  return (
    <PageShell
      title="Ứng dụng"
      subtitle="Ứng dụng đã cài, dung lượng và lần dùng cuối — tìm app lâu không dùng để gỡ bớt."
      actions={
        <div className="flex items-center gap-2">
          <input
            type="search"
            placeholder="Tìm tên, nhà phát hành…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="w-64 rounded-md border border-black/10 bg-white px-3 py-1.5 text-sm outline-none focus:border-accent xl:w-80 dark:border-white/10 dark:bg-white/5"
          />
          <Button variant="ghost" size="sm" loading={loading} onClick={load}>
            {!loading && "↻ "}Tải lại
          </Button>
        </div>
      }
    >
      <div className="flex flex-wrap items-center gap-2">
        {UNUSED_FILTERS.map((f) => (
          <button
            key={f.days}
            type="button"
            onClick={() => setUnusedDays(f.days)}
            className={`rounded-full px-3 py-1 text-sm transition ${
              unusedDays === f.days ? "bg-accent text-white" : "bg-black/5 hover:bg-black/10 dark:bg-white/10 dark:hover:bg-white/15"
            }`}
          >
            {f.label}
          </button>
        ))}
        {unusedDays > 0 && (
          <label className="ml-1 flex cursor-pointer items-center gap-2 text-sm" title="App không nguồn nào ghi nhận lần chạy và đã cài lâu hơn số ngày đã chọn">
            <input type="checkbox" checked={includeUnknown} onChange={(e) => setIncludeUnknown(e.target.checked)} className="accent-[var(--color-accent)]" />
            Gồm cả app chưa rõ lần dùng
          </label>
        )}
        <label className="ml-auto flex items-center gap-2 text-sm">
          Sắp xếp
          <select
            value={sort}
            onChange={(e) => setSort(e.target.value as SortKey)}
            className="h-8 cursor-pointer rounded-md border border-black/10 bg-white px-2 text-sm dark:border-white/10 dark:bg-[#1a1d23]"
          >
            <option value="size">Dung lượng</option>
            <option value="lastUsed">Lâu chưa dùng nhất</option>
            <option value="installed">Cài lâu nhất</option>
            <option value="name">Tên</option>
          </select>
        </label>
      </div>

      {data && (
        <p className="mt-2 text-xs opacity-60">
          Lần dùng cuối lấy từ: UserAssist ({data.userassistEntries} mục) ·{" "}
          {data.prefetchEntries != null ? `Prefetch (${data.prefetchEntries} mục)` : "Prefetch (cần quyền Administrator)"} · PC Manager
          {data.trackingSinceMs != null ? ` theo dõi từ ${formatDate(data.trackingSinceMs)}` : " (vừa bắt đầu theo dõi)"}.
        </p>
      )}

      {(msg || error) && (
        <div className="mt-3">
          <Notice tone={msg?.tone ?? "error"}>{msg?.text ?? error}</Notice>
        </div>
      )}

      {loading && !data ? (
        <Card className="mt-3 flex items-center justify-center gap-3 py-12 text-sm opacity-70">
          <Spinner /> Đang đọc danh sách ứng dụng và đo dung lượng…
        </Card>
      ) : (
        <>
          <div className="mt-3 text-sm opacity-70">
            {rows.length} ứng dụng · tổng {formatBytes(totalSize)}
            {unusedDays > 0 && rows.length > 0 && " có thể gỡ để lấy lại dung lượng"}
          </div>
          <div className="mt-2 min-h-0 flex-1 overflow-auto rounded-lg bg-white shadow-sm dark:bg-white/5">
            <table className="w-full table-fixed text-sm">
              <thead className="sticky top-0 z-10 bg-white text-left text-xs dark:bg-[#1a1d23]">
                <tr>
                  <th className="px-4 py-2 font-medium opacity-70">Ứng dụng</th>
                  <th className="w-28 px-3 py-2 text-right font-medium opacity-70">Dung lượng</th>
                  <th className="w-28 px-3 py-2 font-medium opacity-70">Ngày cài</th>
                  <th className="w-36 px-3 py-2 font-medium opacity-70">Lần dùng cuối</th>
                  <th className="w-52 px-3 py-2" />
                </tr>
              </thead>
              <tbody>
                {rows.map((a) => (
                  <tr key={a.id} className="border-t border-black/5 hover:bg-black/[0.03] dark:border-white/5 dark:hover:bg-white/[0.03]">
                    <td className="px-4 py-2" title={a.installLocation ?? undefined}>
                      <div className="truncate font-medium">{a.name}</div>
                      <div className="truncate text-xs opacity-60">{[a.publisher, a.version].filter(Boolean).join(" · ") || "—"}</div>
                    </td>
                    <td className="px-3 text-right tabular-nums" title={a.sizeMeasured ? "Đo thực tế thư mục cài đặt" : "Theo số ứng dụng khai báo"}>
                      {a.sizeBytes != null ? `${a.sizeMeasured ? "" : "≈ "}${formatBytes(a.sizeBytes)}` : <span className="opacity-40">—</span>}
                    </td>
                    <td className="px-3 tabular-nums opacity-80">{a.installDateMs != null ? formatDate(a.installDateMs) : <span className="opacity-40">—</span>}</td>
                    <td className="px-3">
                      <LastUsed a={a} />
                    </td>
                    <td className="whitespace-nowrap px-3 text-right">
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={busyId !== null}
                        title="Tìm thư mục, shortcut, registry còn sót của app — chuyển vào Thùng rác"
                        onClick={() => setLeftoverApp(a)}
                      >
                        Dọn sót
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-red-600 dark:text-red-400"
                        disabled={!a.canUninstall || busyId !== null || uninstalling.has(a.id)}
                        loading={busyId === a.id || uninstalling.has(a.id)}
                        title={a.canUninstall ? undefined : "Ứng dụng này không cho phép gỡ từ đây"}
                        onClick={() => uninstall(a)}
                      >
                        {uninstalling.has(a.id) ? "Đang gỡ…" : "Gỡ cài đặt"}
                      </Button>
                    </td>
                  </tr>
                ))}
                {rows.length === 0 && (
                  <tr>
                    <td colSpan={5} className="px-4 py-10 text-center text-sm opacity-50">
                      {unusedDays > 0 ? "Không có ứng dụng nào lâu không dùng theo bộ lọc này" : "Không có ứng dụng"}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </>
      )}

      <ConfirmDialog req={confirm} onClose={() => setConfirm(null)} />
      <LeftoverDialog
        app={leftoverApp}
        onClose={(changed) => {
          setLeftoverApp(null);
          if (changed) load();
        }}
      />
    </PageShell>
  );
}
