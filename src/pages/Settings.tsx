import { IconCheck, IconRefresh, IconX } from "../components/Icons";
import { INPUT, PANEL, SELECT } from "../components/ui";
import { useCallback, useEffect, useState } from "react";
import Button from "../components/Button";
import { Card, Notice } from "../components/Card";
import PageShell from "../components/PageShell";
import { AdminNotice } from "../components/Elevation";
import { api, errorMessage, type ActionLogEntry, type AppInfo, type GeneralSettings } from "../lib/api";
import { SmartSettings } from "./Memory";
import { UpdateSettings } from "../components/UpdatePanel";
import { useTheme, type ThemePref } from "../lib/theme";

const INTERVALS = [
  { v: 1, label: "Hằng ngày" },
  { v: 7, label: "Hằng tuần" },
  { v: 30, label: "Hằng tháng" },
];

function useFlash(): [string | null, (s: string) => void] {
  const [msg, setMsg] = useState<string | null>(null);
  const flash = useCallback((s: string) => {
    setMsg(s);
    window.setTimeout(() => setMsg(null), 1800);
  }, []);
  return [msg, flash];
}

function AutoCleanSettings({ s, onChange }: { s: GeneralSettings; onChange: (s: GeneralSettings) => void }) {
  const [err, setErr] = useState<string | null>(null);
  const [saved, flash] = useFlash();
  const a = s.autoClean;

  const save = async (enabled: boolean, intervalDays: number, ruleIds: string[]) => {
    setErr(null);
    try {
      const next = await api.settingsSetAutoClean(enabled, intervalDays, ruleIds);
      onChange({ ...s, autoClean: next });
      flash("Đã lưu");
    } catch (e) {
      setErr(errorMessage(e));
    }
  };
  const toggleRule = (id: string) =>
    save(a.enabled, a.intervalDays, a.ruleIds.includes(id) ? a.ruleIds.filter((r) => r !== id) : [...a.ruleIds, id]);

  return (
    <div className="grid gap-3 text-sm">
      <div className="flex flex-wrap items-center gap-3">
        <label className="flex cursor-pointer items-center gap-2">
          <input
            type="checkbox"
            className="accent-[var(--color-accent)]"
            checked={a.enabled}
            disabled={a.ruleIds.length === 0 && !a.enabled}
            onChange={(e) => save(e.target.checked, a.intervalDays, a.ruleIds)}
          />
          Tự động dọn rác
        </label>
        <select
          value={a.intervalDays}
          onChange={(e) => save(a.enabled, Number(e.target.value), a.ruleIds)}
          className={SELECT}
        >
          {INTERVALS.map((i) => (
            <option key={i.v} value={i.v}>
              {i.label}
            </option>
          ))}
        </select>
        <span className="text-xs opacity-60">
          Lần chạy gần nhất: {a.lastRunMs != null ? new Date(a.lastRunMs).toLocaleString("vi-VN") : "chưa chạy"}
        </span>
        {saved && <b className="text-xs text-green-600">{saved}</b>}
      </div>
      <div className="grid gap-1 sm:grid-cols-2">
        {s.autoRules.map((r) => (
          <label key={r.id} className="flex cursor-pointer items-start gap-2 rounded px-2 py-1 hover:bg-black/5 dark:hover:bg-white/5" title={r.description}>
            <input type="checkbox" className="mt-0.5 accent-[var(--color-accent)]" checked={a.ruleIds.includes(r.id)} onChange={() => toggleRule(r.id)} />
            <span>
              {r.name}
              <span className="block text-xs opacity-60">{r.description}</span>
            </span>
          </label>
        ))}
      </div>
      <p className="text-xs opacity-60">
        Chỉ các mục rủi ro thấp được phép dọn tự động (không dừng service, không làm rỗng Thùng rác). Vẫn bỏ qua file mới, file đang
        mở, thư mục bảo vệ. Mỗi lần chạy được ghi vào nhật ký bên dưới. Muốn xem trước, dùng trang Dọn rác.
      </p>
      {err && <Notice tone="error">{err}</Notice>}
    </div>
  );
}

function WhitelistSettings({ s, onChange }: { s: GeneralSettings; onChange: (s: GeneralSettings) => void }) {
  const [input, setInput] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [saved, flash] = useFlash();

  const save = async (paths: string[]) => {
    setErr(null);
    try {
      const list = await api.settingsSetWhitelist(paths);
      onChange({ ...s, whitelist: list });
      setInput("");
      flash("Đã lưu");
    } catch (e) {
      setErr(errorMessage(e));
    }
  };

  return (
    <div className="grid gap-3 text-sm">
      <p className="text-xs opacity-60">
        PC Manager sẽ <b>không bao giờ</b> xoá gì trong các thư mục này — áp dụng cho Dọn rác, Ổ đĩa và Dọn phần còn sót.
      </p>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (input.trim()) save([...s.whitelist, input.trim()]);
        }}
      >
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="vd D:\Projects hoặc C:\Users\…\AppData\Local\MyApp"
          className={`flex-1 ${INPUT}`}
        />
        <Button size="sm" type="submit" disabled={!input.trim()}>
          Thêm
        </Button>
      </form>
      {s.whitelist.length === 0 ? (
        <div className="text-xs opacity-50">Chưa có thư mục nào.</div>
      ) : (
        <ul className={`divide-y divide-black/5 dark:divide-white/5 ${PANEL}`}>
          {s.whitelist.map((p) => (
            <li key={p} className="flex items-center gap-2 px-3 py-1.5">
              <span className="flex-1 truncate font-mono text-xs" title={p}>
                {p}
              </span>
              <Button variant="ghost" size="sm" onClick={() => save(s.whitelist.filter((x) => x !== p))}>
                Xoá
              </Button>
            </li>
          ))}
        </ul>
      )}
      {saved && <b className="text-xs text-green-600">{saved}</b>}
      {err && <Notice tone="error">{err}</Notice>}
    </div>
  );
}

function AutostartSettings({ s, onChange }: { s: GeneralSettings; onChange: (s: GeneralSettings) => void }) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const set = async (enabled: boolean) => {
    setBusy(true);
    setErr(null);
    try {
      onChange({ ...s, autostart: await api.settingsSetAutostart(enabled) });
    } catch (e) {
      setErr(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="grid gap-2 text-sm">
      <label className="flex cursor-pointer items-center gap-2">
        <input
          type="checkbox"
          className="accent-[var(--color-accent)]"
          checked={s.autostart}
          // Luôn cho TẮT; chỉ cho BẬT khi app được cài trong Program Files.
          disabled={busy || !s.elevated || (!s.autostart && !!s.autostartBlocked)}
          onChange={(e) => set(e.target.checked)}
        />
        Khởi động cùng Windows (chạy ẩn ở khay hệ thống)
      </label>
      <p className="text-xs opacity-60">
        Dùng Task Scheduler với quyền cao nhất — vì PC Manager cần quyền Administrator, cách "Run" thông thường sẽ bị UAC chặn. App
        sẽ chạy với quyền Administrator mà không hỏi lại, nên chỉ được bật khi PC Manager được cài trong Program Files (chỉ admin mới
        sửa được file ở đó).
      </p>
      {!s.elevated && <AdminNotice>Bật/tắt tuỳ chọn này cần quyền Administrator.</AdminNotice>}
      {s.elevated && s.autostartBlocked && <Notice tone="warn">{s.autostartBlocked}</Notice>}
      {err && <Notice tone="error">{err}</Notice>}
    </div>
  );
}

function ActivityLog() {
  const [items, setItems] = useState<ActionLogEntry[] | null>(null);
  const load = useCallback(() => {
    api.recentActions(100).then(setItems).catch(() => setItems([]));
  }, []);
  useEffect(load, [load]);
  return (
    <div className="text-sm">
      <div className="mb-2 flex justify-end">
        <Button variant="ghost" size="sm" onClick={load}>
          <IconRefresh /> Tải lại
        </Button>
      </div>
      <div className={`max-h-80 overflow-auto ${PANEL}`}>
        <table className="w-full table-fixed text-xs">
          <tbody>
            {(items ?? []).map((a, i) => (
              <tr key={i} className="border-t border-black/5 first:border-t-0 dark:border-white/5" title={a.detail}>
                <td className="w-36 px-3 py-1 tabular-nums opacity-60">{new Date(a.ts).toLocaleString("vi-VN")}</td>
                <td className="w-44 px-2 py-1 font-mono">{a.action}</td>
                <td className="truncate px-2 py-1">{a.target}</td>
                <td className={`w-10 px-2 py-1 ${a.ok ? "text-green-600" : "text-red-600"}`}>{a.ok ? <IconCheck /> : <IconX />}</td>
              </tr>
            ))}
            {items?.length === 0 && (
              <tr>
                <td className="px-3 py-6 text-center opacity-50">Chưa có thao tác nào</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

const THEMES: { id: ThemePref; label: string }[] = [
  { id: "dark", label: "Tối" },
  { id: "light", label: "Sáng" },
  { id: "system", label: "Theo Windows" },
];

function ThemeSettings() {
  const [pref, setPref] = useTheme();
  return (
    <div className="grid gap-2 text-sm">
      <div className="inline-flex w-fit rounded-lg bg-black/5 p-1 dark:bg-white/5" role="radiogroup" aria-label="Giao diện">
        {THEMES.map((t) => (
          <button
            key={t.id}
            type="button"
            role="radio"
            aria-checked={pref === t.id}
            onClick={() => setPref(t.id)}
            className={`rounded-md px-3 py-1.5 transition ${pref === t.id ? "bg-white shadow-sm dark:bg-white/15" : "opacity-70 hover:opacity-100"}`}
          >
            {t.label}
          </button>
        ))}
      </div>
      <p className="text-xs opacity-60">Chỉ đổi màu giao diện, không ảnh hưởng tốc độ của app.</p>
    </div>
  );
}

export default function Settings() {
  const [s, setS] = useState<GeneralSettings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api.settingsGet().then(setS).catch((e) => setErr(errorMessage(e)));
    api.appInfo().then(setInfo).catch(() => {});
  }, []);

  return (
    <PageShell title="Cài đặt" subtitle="Smart Boost, dọn tự động, khởi động cùng Windows, cập nhật, thư mục bảo vệ.">
      {err && <Notice tone="error">{err}</Notice>}
      {/*
        Hai cột, card cùng hàng cao bằng nhau (grid mặc định stretch): ghép các card có độ cao gần
        nhau thành một hàng; card nhiều nội dung (whitelist, nhật ký) chiếm cả hàng.
      */}
      <div className="grid gap-4 xl:grid-cols-2">
        <Card title="Giao diện">
          <ThemeSettings />
        </Card>
        <Card title="Khởi động">{s && <AutostartSettings s={s} onChange={setS} />}</Card>
        <Card title="Smart Boost">
          <SmartSettings />
        </Card>
        <Card title="Lịch dọn tự động">{s && <AutoCleanSettings s={s} onChange={setS} />}</Card>
        <Card title="Cập nhật" className="xl:col-span-2">
          <UpdateSettings />
        </Card>
        <Card title="Thư mục bảo vệ (whitelist)" className="xl:col-span-2">
          {s && <WhitelistSettings s={s} onChange={setS} />}
        </Card>
        <Card title="Nhật ký thao tác" className="xl:col-span-2">
          <ActivityLog />
        </Card>
      </div>
      {info && (
        <p className="mt-4 text-xs opacity-50">
          PC Manager {info.version} · PID {info.pid} · {info.isElevated ? "đang chạy với quyền Administrator" : "không có quyền Administrator"}
        </p>
      )}
    </PageShell>
  );
}
