import { INPUT, SURFACE, THEAD, pill } from "../components/ui";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AdminNotice } from "../components/Elevation";
import Button from "../components/Button";
import { Notice } from "../components/Card";
import { useWindowVisible } from "../hooks/useWindowVisible";
import { api, errorMessage, type StartupEntry, type StartupSource } from "../lib/api";
import { publisherText } from "../lib/processTable";

const SOURCE_LABEL: Record<StartupSource, string> = {
  runUser: "Registry · người dùng",
  runUser32: "Registry 32-bit · người dùng",
  runMachine: "Registry · máy",
  runMachine32: "Registry 32-bit · máy",
  runOnceUser: "RunOnce · người dùng",
  runOnceMachine: "RunOnce · máy",
  runOnceMachine32: "RunOnce 32-bit · máy",
  startupFolderUser: "Thư mục Startup · người dùng",
  startupFolderCommon: "Thư mục Startup · mọi người dùng",
  scheduledTask: "Tác vụ theo lịch",
  service: "Dịch vụ",
};

type Tab = "apps" | "tasks" | "services";
const TAB_OF = (s: StartupSource): Tab => (s === "scheduledTask" ? "tasks" : s === "service" ? "services" : "apps");
const TABS: { id: Tab; label: string; hint: string }[] = [
  { id: "apps", label: "Ứng dụng", hint: "Giống tab Startup của Task Manager — bật/tắt được" },
  { id: "tasks", label: "Tác vụ khi đăng nhập", hint: "Scheduled task có trigger \"At log on\" — chỉ xem" },
  { id: "services", label: "Dịch vụ tự khởi động", hint: "Service Start = Automatic — chỉ xem" },
];

const isMicrosoft = (e: StartupEntry) =>
  e.publisher.status === "signed" && e.publisher.name.toLowerCase().startsWith("microsoft");

function Toggle({ on, disabled, onChange }: { on: boolean; disabled?: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!on);
      }}
      className={`relative h-5 w-9 shrink-0 cursor-pointer rounded-full transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-40 ${on ? "bg-accent hover:bg-blue-700" : "bg-black/20 hover:bg-black/30 dark:bg-white/20 dark:hover:bg-white/30"}`}
    >
      <span className={`absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-[left] ${on ? "left-[18px]" : "left-0.5"}`} />
    </button>
  );
}

export default function Startup() {
  const [entries, setEntries] = useState<StartupEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [msg, setMsg] = useState<{ tone: "ok" | "error"; text: string } | null>(null);
  const [tab, setTab] = useState<Tab>("apps");
  const [hideMs, setHideMs] = useState(true);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const visible = useWindowVisible();
  const polls = useRef(0);

  const load = useCallback(() => {
    api
      .startupList()
      .then((l) => {
        setEntries(l);
        setError(null);
      })
      .catch((e) => setError(errorMessage(e)));
  }, []);

  useEffect(load, [load]);

  const [reloading, setReloading] = useState(false);
  const reload = () => {
    setReloading(true);
    polls.current = 0;
    api
      .startupList()
      .then((l) => {
        setEntries(l);
        setError(null);
      })
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setReloading(false));
  };

  // Publisher được kiểm tra nền: tải lại vài lần cho tới khi hết "Đang kiểm tra…".
  const pending = entries?.some((e) => e.publisher.status === "pending") ?? false;
  useEffect(() => {
    if (!pending || !visible || polls.current > 20) return;
    const id = window.setTimeout(() => {
      polls.current += 1;
      load();
    }, 1500);
    return () => window.clearTimeout(id);
  }, [pending, visible, entries, load]);

  const toggle = async (e: StartupEntry, enabled: boolean) => {
    setBusy(e.id);
    setMsg(null);
    try {
      await api.startupSetEnabled(e, enabled);
      setMsg({ tone: "ok", text: `${enabled ? "Đã bật" : "Đã tắt"} ${e.name}. Có hiệu lực từ lần đăng nhập tới.` });
      load();
    } catch (err) {
      const text = errorMessage(err);
      const hint = /access is denied|truy cập bị từ chối|0x80070005/i.test(text)
        ? " — mục của máy (HKLM / mọi người dùng) cần chạy PC Manager với quyền Administrator."
        : "";
      setMsg({ tone: "error", text: `${e.name}: ${text}${hint}` });
    } finally {
      setBusy(null);
    }
  };

  // Số trên mỗi tab = số dòng thật sự hiện ra (tính cả bộ lọc "Ẩn mục của Microsoft"),
  // kèm số mục Microsoft đang bị ẩn để hiện cạnh ô đánh dấu.
  const { counts, hiddenMs } = useMemo(() => {
    const c = { apps: 0, tasks: 0, services: 0 };
    const h = { apps: 0, tasks: 0, services: 0 };
    for (const e of entries ?? []) {
      const t = TAB_OF(e.source);
      if (t !== "apps" && hideMs && isMicrosoft(e)) h[t]++;
      else c[t]++;
    }
    return { counts: c, hiddenMs: h };
  }, [entries, hideMs]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (entries ?? [])
      .filter((e) => TAB_OF(e.source) === tab)
      .filter((e) => tab === "apps" || !hideMs || !isMicrosoft(e))
      .filter(
        (e) =>
          !q ||
          e.name.toLowerCase().includes(q) ||
          e.command.toLowerCase().includes(q) ||
          publisherText(e.publisher).toLowerCase().includes(q),
      )
      .sort((a, b) => Number(b.enabled) - Number(a.enabled) || a.name.localeCompare(b.name, "vi", { sensitivity: "base" }));
  }, [entries, tab, hideMs, query]);

  const appEntries = (entries ?? []).filter((e) => TAB_OF(e.source) === "apps");
  const enabledApps = appEntries.filter((e) => e.enabled).length;

  return (
    <section className="flex h-full w-full flex-col">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold">Khởi động</h1>
          <p className="mt-1 text-sm opacity-60">
            {entries
              ? `${enabledApps} ứng dụng đang khởi động cùng Windows · ${appEntries.length - enabledApps} đã tắt`
              : "Đang đọc registry, thư mục Startup và Task Scheduler…"}
          </p>
        </div>
        {/* "Tải lại" nằm cạnh ô tìm kiếm (không ở hàng tab): chữ "(đang ẩn N)" dài/ngắn không đẩy nó xuống dòng. */}
        <div className="flex items-center gap-2">
          <input
            type="search"
            placeholder="Tìm tên, lệnh, publisher…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className={`w-72 xl:w-96 ${INPUT}`}
          />
          <Button variant="ghost" size="sm" className="shrink-0" loading={reloading} onClick={reload}>
            {!reloading && "↻ "}Tải lại
          </Button>
        </div>
      </div>

      <div className="mt-4 flex flex-wrap items-center gap-2">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            title={t.hint}
            onClick={() => setTab(t.id)}
            className={pill(tab === t.id)}
          >
            {t.label} <span className="opacity-70">({counts[t.id]})</span>
          </button>
        ))}
        {tab !== "apps" && (
          <label className="ml-2 flex items-center gap-2 whitespace-nowrap text-sm">
            <input type="checkbox" checked={hideMs} onChange={(e) => setHideMs(e.target.checked)} />
            Ẩn mục của Microsoft
            {hideMs && hiddenMs[tab] > 0 && <span className="opacity-60">(đang ẩn {hiddenMs[tab]})</span>}
          </label>
        )}
      </div>

      <p className="mt-2 text-xs opacity-60">{TABS.find((t) => t.id === tab)?.hint}. Bật/tắt dùng cùng cơ chế với Task Manager, không xoá mục gốc.</p>

      {msg && (
        <div className="mt-2">
          <Notice tone={msg.tone} onClose={() => setMsg(null)}>{msg.text}</Notice>
        </div>
      )}
      <div className="mt-2 empty:hidden">
        <AdminNotice>Mục khởi động của máy (HKLM, thư mục Startup chung) chỉ bật/tắt được với quyền Administrator.</AdminNotice>
      </div>
      {error && (
        <div className="mt-2">
          <Notice tone="error">{error}</Notice>
        </div>
      )}

      <div className={`mt-3 min-h-0 flex-1 overflow-auto ${SURFACE}`}>
        <table className="w-full table-fixed text-sm">
          <thead className={THEAD}>
            <tr className="text-left">
              <th className="px-3 py-2 font-medium opacity-70">Tên</th>
              <th className="w-56 px-3 py-2 font-medium opacity-70">Publisher</th>
              <th className="w-56 px-3 py-2 font-medium opacity-70">Nguồn</th>
              <th className="w-28 px-3 py-2 font-medium opacity-70">Trạng thái</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((e) => {
              const expanded = open === e.id;
              return [
                <tr
                  key={e.id}
                  onClick={() => setOpen(expanded ? null : e.id)}
                  className={`cursor-pointer border-t border-black/5 dark:border-white/5 ${expanded ? "bg-accent/10" : "hover:bg-black/5 dark:hover:bg-white/5"} ${e.enabled ? "" : "opacity-60"}`}
                >
                  <td className="truncate py-2 pl-3 pr-2" title={e.name}>
                    <span className="mr-1 inline-block w-3 opacity-50">{expanded ? "▾" : "▸"}</span>
                    {e.name}
                  </td>
                  <td className="truncate px-3">
                    {e.publisher.status === "signed" && <span className="mr-1 text-green-600">✓</span>}
                    <span
                      title={e.publisher.status === "invalid" ? `Mã lỗi WinVerifyTrust ${e.publisher.code}` : undefined}
                      className={
                        e.publisher.status === "unsigned"
                          ? "text-amber-600 dark:text-amber-400"
                          : e.publisher.status === "invalid"
                            ? "text-red-600 dark:text-red-400"
                            : e.publisher.status === "signed"
                              ? ""
                              : "opacity-50"
                      }
                    >
                      {publisherText(e.publisher) || (e.path ? "Không tìm thấy file" : "—")}
                    </span>
                  </td>
                  <td className="truncate px-3 text-xs opacity-70">{SOURCE_LABEL[e.source]}</td>
                  <td className="px-3">
                    {e.canToggle ? (
                      <div className="flex items-center gap-2">
                        <Toggle on={e.enabled} disabled={busy !== null} onChange={(v) => toggle(e, v)} />
                        <span className="text-xs">{busy === e.id ? "…" : e.enabled ? "Bật" : "Tắt"}</span>
                      </div>
                    ) : (
                      <span className="text-xs opacity-60">{e.enabled ? "Bật" : "Tắt"} · chỉ xem</span>
                    )}
                  </td>
                </tr>,
                expanded && (
                  <tr key={`${e.id}:d`} className="bg-accent/5">
                    <td colSpan={4} className="px-8 py-3 text-xs">
                      <dl className="grid grid-cols-[8rem_1fr] gap-x-3 gap-y-1">
                        <dt className="opacity-60">Lệnh</dt>
                        <dd className="break-all font-mono select-text">{e.command || "—"}</dd>
                        <dt className="opacity-60">File thực thi</dt>
                        <dd className="break-all select-text">{e.path ?? "—"}</dd>
                        <dt className="opacity-60">Khai báo tại</dt>
                        <dd className="break-all select-text">{e.location}</dd>
                        {e.disabledAtMs != null && (
                          <>
                            <dt className="opacity-60">Tắt lúc</dt>
                            <dd>{new Date(e.disabledAtMs).toLocaleString("vi-VN")}</dd>
                          </>
                        )}
                        {e.note && (
                          <>
                            <dt className="opacity-60">{e.source === "scheduledTask" ? "Tác giả" : "Ghi chú"}</dt>
                            <dd>{e.note}</dd>
                          </>
                        )}
                      </dl>
                    </td>
                  </tr>
                ),
              ];
            })}
            {entries && rows.length === 0 && (
              <tr>
                <td colSpan={4} className="px-3 py-8 text-center text-sm opacity-50">
                  Không có mục nào
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}
