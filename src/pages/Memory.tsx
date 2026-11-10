import { useEffect, useState } from "react";
import PageShell from "../components/PageShell";
import BoostPanel from "../components/BoostPanel";
import UsageChart from "../components/UsageChart";
import { Card, Notice, Stat } from "../components/Card";
import { useMemoryFeed } from "../hooks/useMemoryFeed";
import { api, errorMessage, type MemorySettings, type MemoryStatus } from "../lib/api";
import { formatBytes, pct } from "../lib/format";

/** Thanh ngang chia RAM: đang dùng | modified | standby | free. */
function Composition({ s }: { s: MemoryStatus }) {
  const modified = s.modified ?? 0;
  const standby = s.standby ?? 0;
  const free = s.free ?? 0;
  const inUse = Math.max(0, s.used - modified);
  const parts = [
    { label: "Đang dùng", v: inUse, cls: "bg-accent" },
    { label: "Modified", v: modified, cls: "bg-amber-500" },
    { label: "Standby", v: standby, cls: "bg-sky-400" },
    { label: "Free", v: free, cls: "bg-black/10 dark:bg-white/15" },
  ];
  return (
    <div>
      <div className="flex h-5 overflow-hidden rounded">
        {parts.map((p) => (
          <div key={p.label} className={p.cls} style={{ width: `${pct(p.v, s.total)}%` }} title={`${p.label}: ${formatBytes(p.v)}`} />
        ))}
      </div>
      <div className="mt-2 flex flex-wrap gap-4 text-xs">
        {parts.map((p) => (
          <span key={p.label} className="flex items-center gap-1.5">
            <span className={`inline-block h-2.5 w-2.5 rounded-sm ${p.cls}`} />
            {p.label} {formatBytes(p.v)}
          </span>
        ))}
      </div>
    </div>
  );
}

function SmartSettings() {
  const [s, setS] = useState<MemorySettings | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api.memorySettings().then(setS).catch((e) => setErr(errorMessage(e)));
  }, []);

  const save = async (next: MemorySettings) => {
    setS(next);
    setErr(null);
    try {
      setS(await api.setMemorySettings(next));
      setSaved("Đã lưu");
      window.setTimeout(() => setSaved(null), 1500);
    } catch (e) {
      setErr(errorMessage(e));
    }
  };

  if (!s) return err ? <Notice tone="error">{err}</Notice> : null;
  return (
    <div className="grid gap-3 text-sm">
      <label className="flex items-center gap-3">
        <span className="w-48">Ngưỡng RAM trống</span>
        <input
          type="range"
          min={5}
          max={50}
          value={s.smartThresholdPct}
          onChange={(e) => setS({ ...s, smartThresholdPct: Number(e.target.value) })}
          onPointerUp={() => save(s)}
          onKeyUp={() => save(s)}
          className="flex-1 accent-[var(--color-accent)]"
        />
        <span className="w-12 text-right tabular-nums">{s.smartThresholdPct}%</span>
      </label>
      <label className="flex items-center gap-3">
        <input type="checkbox" checked={s.smartAuto} onChange={(e) => save({ ...s, smartAuto: e.target.checked })} />
        Tự động Smart Boost khi chạy nền (kiểm tra mỗi 60 giây, nghỉ 5 phút giữa 2 lần)
      </label>
      <p className="text-xs opacity-60">
        Smart Boost chỉ chạy khi RAM còn trống thấp hơn ngưỡng. {saved && <b className="text-green-600">{saved}</b>}
      </p>
      {err && <Notice tone="error">{err}</Notice>}
    </div>
  );
}

export default function Memory() {
  const { overview, history, error } = useMemoryFeed();
  const s = overview?.status;

  return (
    <PageShell title="Bộ nhớ" subtitle="Chi tiết RAM và bộ nhớ ảo (commit)">
      {error && <Notice tone="error">{error}</Notice>}
      {s && (
        <div className="grid gap-4">
          <Card title="Phân bổ RAM">
            <Composition s={s} />
          </Card>
          <Card>
            <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
              <Stat label="Tổng RAM" value={formatBytes(s.total)} />
              <Stat label="Đang dùng" value={formatBytes(s.used)} />
              <Stat label="Còn trống (available)" value={formatBytes(s.available)} hint="Free + Standby — Windows có thể cấp ngay" />
              <Stat label="Free" value={formatBytes(s.free)} />
              <Stat label="Standby (cache)" value={formatBytes(s.standby)} hint="Cache file/app vừa dùng, đã tính vào khả dụng. Boost nhẹ chỉ xoá phần ưu tiên thấp, Boost sâu xoá hết." />
              <Stat label="Modified" value={formatBytes(s.modified)} hint="Trang chờ ghi ra đĩa — Boost sẽ ghi để chúng thành khả dụng" />
              <Stat label="Commit" value={`${formatBytes(s.commitTotal)} / ${formatBytes(s.commitLimit)}`} />
              <Stat label="Cache hệ thống" value={formatBytes(s.systemCache)} />
            </div>
          </Card>
          <Card title="% RAM đang dùng — 60 giây gần nhất">
            <UsageChart samples={history} height={180} />
          </Card>
          <Card title="Boost">
            <BoostPanel />
          </Card>
          <Card title="Smart Boost">
            <SmartSettings />
          </Card>
        </div>
      )}
    </PageShell>
  );
}
