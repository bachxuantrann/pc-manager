import PageShell from "../components/PageShell";
import BoostPanel from "../components/BoostPanel";
import UsageChart from "../components/UsageChart";
import { Card, Notice, Stat } from "../components/Card";
import { useMemoryFeed } from "../hooks/useMemoryFeed";
import { formatBytes, pct } from "../lib/format";

function Gauge({ value }: { value: number }) {
  const r = 52;
  const c = 2 * Math.PI * r;
  const v = Math.min(100, Math.max(0, value));
  const tone = v >= 85 ? "stroke-red-500" : v >= 70 ? "stroke-amber-500" : "stroke-accent";
  return (
    <svg viewBox="0 0 128 128" className="h-32 w-32">
      <circle cx={64} cy={64} r={r} className="fill-none stroke-black/10 dark:stroke-white/10" strokeWidth={12} />
      <circle
        cx={64}
        cy={64}
        r={r}
        className={`fill-none ${tone} transition-[stroke-dashoffset] duration-500`}
        strokeWidth={12}
        strokeLinecap="round"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - v / 100)}
        transform="rotate(-90 64 64)"
      />
      <text x={64} y={70} textAnchor="middle" className="fill-current text-2xl font-semibold">
        {v.toFixed(0)}%
      </text>
    </svg>
  );
}

export default function Dashboard() {
  const { overview, history, error } = useMemoryFeed();
  const s = overview?.status;

  return (
    <PageShell title="Tổng quan" subtitle="Tình trạng bộ nhớ, cập nhật mỗi 2 giây khi cửa sổ đang mở">
      {error && <Notice tone="error">{error}</Notice>}
      <div className="grid gap-4 md:grid-cols-[auto_1fr]">
        <Card title="RAM">
          <div className="flex items-center gap-6">
            <Gauge value={s ? pct(s.used, s.total) : 0} />
            <div className="grid gap-2">
              <Stat label="Đang dùng" value={formatBytes(s?.used)} />
              <Stat label="Còn trống" value={formatBytes(s?.available)} />
              <Stat label="Tổng" value={formatBytes(s?.total)} />
            </div>
          </div>
        </Card>
        <Card title="60 giây gần nhất">
          <UsageChart samples={history} />
        </Card>
      </div>
      <Card title="Tăng tốc" className="mt-4">
        <BoostPanel />
      </Card>
    </PageShell>
  );
}
