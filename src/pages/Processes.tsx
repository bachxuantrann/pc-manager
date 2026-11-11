import { useCallback, useMemo, useState } from "react";
import { AdminNotice } from "../components/Elevation";
import Button from "../components/Button";
import ConfirmDialog, { type ConfirmRequest } from "../components/ConfirmDialog";

type Action = "open" | "trim" | "priority" | "suspend" | "resume" | "kill" | "killGroup";
import { Notice } from "../components/Card";
import { useVisibleInterval } from "../hooks/useWindowVisible";
import { api, errorMessage, type PriorityClass, type ProcessInfo, type ProcessSnapshot, type Publisher } from "../lib/api";
import { formatBytes } from "../lib/format";
import { groupByName, matches, procKey, publisherText, sortRows, type Group, type Sort, type SortKey } from "../lib/processTable";

const PRIORITY_LABEL: Record<PriorityClass, string> = {
  idle: "Thấp",
  belowNormal: "Dưới bình thường",
  normal: "Bình thường",
  aboveNormal: "Trên bình thường",
  high: "Cao",
};

function PublisherCell({ p }: { p: Publisher }) {
  const text = publisherText(p);
  const cls =
    p.status === "signed"
      ? ""
      : p.status === "unsigned"
        ? "text-amber-600 dark:text-amber-400"
        : p.status === "invalid"
          ? "text-red-600 dark:text-red-400"
          : "opacity-50";
  return (
    <span className={cls} title={p.status === "invalid" ? `Mã lỗi ${p.code}` : undefined}>
      {p.status === "signed" && <span className="mr-1 text-green-600">✓</span>}
      {text}
    </span>
  );
}

const COLUMNS: { key: SortKey; label: string; cls: string }[] = [
  { key: "name", label: "Tên", cls: "w-[28%] text-left" },
  { key: "pid", label: "PID", cls: "w-20 text-right" },
  { key: "cpu", label: "CPU", cls: "w-20 text-right" },
  { key: "memory", label: "RAM", cls: "w-24 text-right" },
  { key: "publisher", label: "Publisher", cls: "w-48 text-left xl:w-64" },
  { key: "path", label: "Đường dẫn", cls: "text-left" },
];

type Selection = { kind: "proc"; key: string } | { kind: "group"; key: string } | null;

export default function Processes() {
  const [snap, setSnap] = useState<ProcessSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [grouped, setGrouped] = useState(true);
  const [sort, setSort] = useState<Sort>({ key: "memory", desc: true });
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [sel, setSel] = useState<Selection>(null);
  const [confirm, setConfirm] = useState<ConfirmRequest | null>(null);
  const [msg, setMsg] = useState<{ tone: "ok" | "error"; text: string } | null>(null);
  /** Thao tác đang chạy — để hiện spinner đúng trên nút đó. */
  const [busyAction, setBusyAction] = useState<Action | null>(null);
  const busy = busyAction !== null;

  const refresh = useCallback(() => {
    api
      .processList()
      .then((s) => {
        setSnap(s);
        setError(null);
      })
      .catch((e) => setError(errorMessage(e)));
  }, []);
  useVisibleInterval(refresh, 2000);

  const filtered = useMemo(() => (snap?.processes ?? []).filter((p) => matches(p, query.trim())), [snap, query]);
  const groups = useMemo(() => sortRows(groupByName(filtered), sort), [filtered, sort]);
  const flat = useMemo(() => sortRows(filtered, sort), [filtered, sort]);

  const selectedProc: ProcessInfo | null =
    sel?.kind === "proc" ? (snap?.processes.find((p) => procKey(p) === sel.key) ?? null) : null;
  const selectedGroup: Group | null = sel?.kind === "group" ? (groups.find((g) => g.key === sel.key) ?? null) : null;

  const run = async (action: Action, label: string, fn: () => Promise<{ trimmedBytes: number | null }>) => {
    setBusyAction(action);
    setMsg(null);
    try {
      const r = await fn();
      setMsg({ tone: "ok", text: r.trimmedBytes != null ? `${label}: giảm ${formatBytes(r.trimmedBytes)} RAM` : `${label}: xong` });
      refresh();
    } catch (e) {
      setMsg({ tone: "error", text: `${label}: ${errorMessage(e)}` });
    } finally {
      setBusyAction(null);
    }
  };

  const ask = (r: ConfirmRequest) => setConfirm(r);

  const killOne = (p: ProcessInfo) =>
    ask({
      title: `Kết thúc ${p.name}?`,
      body: (
        <>
          PID {p.pid}. Dữ liệu chưa lưu trong ứng dụng này sẽ bị mất.
          {p.path && <div className="mt-1 break-all text-xs opacity-70">{p.path}</div>}
        </>
      ),
      confirmLabel: "Kết thúc",
      danger: true,
      onConfirm: () => run("kill", `Kết thúc ${p.name}`, () => api.processKill(p)),
    });

  const killGroup = (g: Group) => {
    const targets = g.members.filter((m) => !m.protectedReason);
    ask({
      title: `Kết thúc ${targets.length} tiến trình ${g.name}?`,
      body: "Toàn bộ cửa sổ của ứng dụng này sẽ đóng, dữ liệu chưa lưu sẽ bị mất.",
      confirmLabel: "Kết thúc tất cả",
      danger: true,
      onConfirm: () =>
        run("killGroup", `Kết thúc ${g.name}`, async () => {
          const results = await Promise.allSettled(targets.map((m) => api.processKill(m)));
          const failed = results.filter((r) => r.status === "rejected").length;
          if (failed) throw new Error(`${failed}/${targets.length} tiến trình không kết thúc được`);
          return { trimmedBytes: null };
        }),
    });
  };

  const suspend = (p: ProcessInfo) =>
    ask({
      title: `Tạm dừng ${p.name}?`,
      body: "Ứng dụng sẽ đứng hình (không phản hồi) cho tới khi bạn bấm Tiếp tục. Không tạm dừng ứng dụng đang ghi file hoặc tải dữ liệu.",
      confirmLabel: "Tạm dừng",
      onConfirm: () => run("suspend", `Tạm dừng ${p.name}`, () => api.processSuspend(p)),
    });

  const trim = (p: ProcessInfo) =>
    ask({
      title: `Giảm RAM của ${p.name}?`,
      body: "Windows sẽ đẩy bộ nhớ của tiến trình này ra cache/pagefile. Ứng dụng có thể chậm một chút khi dùng lại.",
      confirmLabel: "Giảm RAM",
      onConfirm: () => run("trim", `Giảm RAM ${p.name}`, () => api.processTrim(p)),
    });

  const setPriority = (p: ProcessInfo, pr: PriorityClass) =>
    ask({
      title: `Đổi ưu tiên ${p.name}?`,
      body: (
        <>
          Từ <b>{p.priority ? PRIORITY_LABEL[p.priority] : "?"}</b> sang <b>{PRIORITY_LABEL[pr]}</b>. Thay đổi mất khi ứng dụng khởi động lại.
          {pr === "high" && <div className="mt-1 text-amber-600">Ưu tiên Cao có thể làm các ứng dụng khác chậm lại.</div>}
        </>
      ),
      confirmLabel: "Đổi",
      onConfirm: () => run("priority", `Đổi ưu tiên ${p.name}`, () => api.processSetPriority(p, pr)),
    });

  const onSort = (key: SortKey) =>
    setSort((s) => (s.key === key ? { key, desc: !s.desc } : { key, desc: key === "cpu" || key === "memory" }));

  const toggle = (k: string) =>
    setExpanded((s) => {
      const n = new Set(s);
      if (n.has(k)) n.delete(k);
      else n.add(k);
      return n;
    });

  const procRow = (p: ProcessInfo, indent: boolean) => {
    const k = procKey(p);
    const active = sel?.kind === "proc" && sel.key === k;
    return (
      <tr
        key={k}
        onClick={() => setSel({ kind: "proc", key: k })}
        className={`cursor-pointer border-t border-black/5 dark:border-white/5 ${active ? "bg-accent/15" : "hover:bg-black/5 dark:hover:bg-white/5"}`}
      >
        <td className={`py-1.5 pr-2 ${indent ? "pl-8" : "pl-3"}`} title={p.protectedReason ?? p.name}>
          <div className="flex min-w-0 items-center gap-1">
            {p.protectedReason && <span className="shrink-0 opacity-60">🔒</span>}
            <span className="truncate">{p.name}</span>
            {p.suspended && (
              <span className="ml-1 shrink-0 rounded bg-amber-500/20 px-1.5 text-xs text-amber-700 dark:text-amber-300">Tạm dừng</span>
            )}
          </div>
        </td>
        <td className="pr-3 text-right tabular-nums opacity-70">{p.pid}</td>
        <td className="pr-3 text-right tabular-nums">{p.cpuPct.toFixed(1)}%</td>
        <td className="pr-3 text-right tabular-nums">{formatBytes(p.privateBytes)}</td>
        <td className="truncate pr-3">
          <PublisherCell p={p.publisher} />
        </td>
        <td className="truncate pr-3 text-xs opacity-60" title={p.path ?? undefined}>
          {p.path}
        </td>
      </tr>
    );
  };

  const groupRow = (g: Group) => {
    if (g.members.length === 1) return procRow(g.members[0], false);
    const open = expanded.has(g.key) || query.trim() !== "";
    const active = sel?.kind === "group" && sel.key === g.key;
    return [
      <tr
        key={`g:${g.key}`}
        onClick={() => setSel({ kind: "group", key: g.key })}
        onDoubleClick={() => toggle(g.key)}
        className={`cursor-pointer border-t border-black/5 font-medium dark:border-white/5 ${active ? "bg-accent/15" : "hover:bg-black/5 dark:hover:bg-white/5"}`}
      >
        <td className="truncate py-1.5 pl-3 pr-2">
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              toggle(g.key);
            }}
            className="mr-1 inline-block w-4 opacity-60"
          >
            {open ? "▾" : "▸"}
          </button>
          {g.name} <span className="font-normal opacity-60">({g.members.length})</span>
        </td>
        <td />
        <td className="pr-3 text-right tabular-nums">{g.cpuPct.toFixed(1)}%</td>
        <td className="pr-3 text-right tabular-nums">{formatBytes(g.privateBytes)}</td>
        <td className="truncate pr-3 font-normal">
          <PublisherCell p={g.publisher} />
        </td>
        <td className="truncate pr-3 text-xs font-normal opacity-60">{g.path}</td>
      </tr>,
      ...(open ? sortRows(g.members, sort).map((m) => procRow(m, true)) : []),
    ];
  };

  const p = selectedProc;
  const locked = !!p?.protectedReason;

  return (
    <section className="flex h-full w-full flex-col">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold">Tiến trình</h1>
          <p className="mt-1 text-sm opacity-60">
            {snap ? `${snap.processes.length} tiến trình · CPU ${snap.totalCpuPct.toFixed(0)}% · ${snap.cpuCount} luồng CPU` : "Đang tải…"}
          </p>
        </div>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={grouped} onChange={(e) => setGrouped(e.target.checked)} />
            Gộp theo ứng dụng
          </label>
          <input
            type="search"
            placeholder="Tìm tên, PID, đường dẫn, publisher…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="w-72 rounded-md border xl:w-96 border-black/10 bg-white px-3 py-1.5 text-sm outline-none focus:border-accent dark:border-white/10 dark:bg-white/5"
          />
        </div>
      </div>

      {/* Thanh thao tác cho dòng đang chọn */}
      <div className="mt-4 flex min-h-11 flex-wrap items-center gap-1 rounded-lg bg-white px-3 py-1.5 shadow-sm dark:bg-white/5">
        {p && (
          <>
            <span className="mr-2 max-w-56 truncate text-sm font-medium" title={p.path ?? undefined}>
              {p.name} <span className="font-normal opacity-60">· {p.pid}</span>
            </span>
            <Button
              variant="ghost"
              size="sm"
              loading={busyAction === "open"}
              disabled={busy || !p.path}
              onClick={() => run("open", "Mở vị trí", () => api.processOpenLocation(p))}
            >
              Mở vị trí file
            </Button>
            <Button variant="ghost" size="sm" loading={busyAction === "trim"} disabled={busy || locked} onClick={() => trim(p)}>
              Giảm RAM
            </Button>
            <select
              value=""
              disabled={busy || locked}
              onChange={(e) => e.target.value && setPriority(p, e.target.value as PriorityClass)}
              className="h-8 cursor-pointer rounded-md bg-transparent px-2 text-sm transition hover:bg-black/5 focus-visible:outline-2 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-45 dark:hover:bg-white/10"
            >
              <option value="">Ưu tiên: {p.priority ? PRIORITY_LABEL[p.priority] : "?"}</option>
              {(Object.keys(PRIORITY_LABEL) as PriorityClass[])
                .filter((k) => k !== p.priority)
                .map((k) => (
                  <option key={k} value={k}>
                    → {PRIORITY_LABEL[k]}
                  </option>
                ))}
            </select>
            {p.suspended ? (
              <Button
                variant="ghost"
                size="sm"
                loading={busyAction === "resume"}
                disabled={busy}
                onClick={() => run("resume", `Tiếp tục ${p.name}`, () => api.processResume(p))}
              >
                Tiếp tục
              </Button>
            ) : (
              <Button variant="ghost" size="sm" loading={busyAction === "suspend"} disabled={busy || locked} onClick={() => suspend(p)}>
                Tạm dừng
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              className="text-red-600 dark:text-red-400"
              loading={busyAction === "kill"}
              disabled={busy || locked}
              onClick={() => killOne(p)}
            >
              Kết thúc
            </Button>
            {locked && <span className="ml-2 text-xs opacity-60">🔒 {p.protectedReason}</span>}
          </>
        )}
        {selectedGroup && (
          <>
            <span className="mr-2 text-sm font-medium">
              {selectedGroup.name} <span className="font-normal opacity-60">· {selectedGroup.members.length} tiến trình</span>
            </span>
            <Button variant="ghost" size="sm" onClick={() => toggle(selectedGroup.key)}>
              {expanded.has(selectedGroup.key) ? "Thu gọn" : "Xem từng tiến trình"}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="text-red-600 dark:text-red-400"
              loading={busyAction === "killGroup"}
              disabled={busy || selectedGroup.members.every((m) => m.protectedReason)}
              onClick={() => killGroup(selectedGroup)}
            >
              Kết thúc tất cả
            </Button>
          </>
        )}
        {!p && !selectedGroup && <span className="text-sm opacity-50">Chọn một tiến trình để thao tác</span>}
      </div>

      {msg && (
        <div className="mt-2">
          <Notice tone={msg.tone} onClose={() => setMsg(null)}>{msg.text}</Notice>
        </div>
      )}
      <div className="mt-2 empty:hidden">
        <AdminNotice>Tiến trình của hệ thống / tài khoản khác chỉ kết thúc, tạm dừng hay đổi ưu tiên được với quyền Administrator.</AdminNotice>
      </div>
      {error && (
        <div className="mt-2">
          <Notice tone="error">{error}</Notice>
        </div>
      )}

      <div className="mt-3 min-h-0 flex-1 overflow-auto rounded-lg bg-white shadow-sm dark:bg-white/5">
        <table className="w-full table-fixed text-sm">
          <thead className="sticky top-0 z-10 bg-white text-xs dark:bg-[#1a1d23]">
            <tr>
              {COLUMNS.map((c) => (
                <th
                  key={c.key}
                  onClick={() => onSort(c.key)}
                  className={`cursor-pointer select-none px-3 py-2 font-medium opacity-70 hover:opacity-100 ${c.cls}`}
                >
                  {c.label}
                  {sort.key === c.key && <span className="ml-1">{sort.desc ? "↓" : "↑"}</span>}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>{grouped ? groups.flatMap(groupRow) : flat.map((x) => procRow(x, false))}</tbody>
        </table>
      </div>

      <ConfirmDialog req={confirm} onClose={() => setConfirm(null)} />
    </section>
  );
}
